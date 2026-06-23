//! Alternative AC OPF backend using `pounce`, a pure-Rust port of Ipopt (filter
//! line search + restoration phase, FERAL pure-Rust LDLᵀ linear solver). Gated
//! behind the `acopf-pounce` feature, alongside the default `interiors` backend.
//!
//! It produces the same [`AcOpfSolution`](super::acopf::AcOpfSolution) the
//! `interiors` path does — primal `x` plus the six multiplier vectors in the MIPS
//! sign convention — so the differentiable [`AcOpfKkt`](crate::AcOpfKkt) sensitivity
//! and every test consume it unchanged.
//!
//! The work is an adapter: tellegen's [`AcOpfModel`] holds the verified objective,
//! flow atoms (`branch_flows`), and Jacobian/Hessian math; this module wraps it in
//! pounce's `TNLP` interface. Ipopt models every constraint as `g_l ≤ g(x) ≤ g_u`,
//! so the four interiors constraint classes (nonlinear equalities, thermal
//! inequalities, the linear angle-difference constraints, and variable bounds)
//! become one combined `g` plus the variable-bound duals `z_l`/`z_u`. The entry
//! point slices pounce's `lambda`/`z_l`/`z_u` back into the six vectors.
//!
//! The adapter OWNS a clone of the network so it is `'static` (pounce takes the
//! TNLP as `Rc<RefCell<dyn TNLP>>`); each eval rebuilds a cheap temporary
//! `AcOpfModel` so all the solve math is the existing verified code, never a copy.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use interiors::{NonlinearConstraint, ObjectiveFunction};
use pounce_algorithm::application::IpoptApplication;
use pounce_common::types::{Index, Number};
use pounce_nlp::return_codes::ApplicationReturnStatus;
use pounce_nlp::tnlp::{
    BoundsInfo, IndexStyle, IpoptCq, IpoptData, IterStats, NlpInfo, Solution, SparsityRequest,
    StartingPoint, TNLP,
};

use super::acopf::{jitter, AcOpfCache, AcOpfModel, AcOpfSolution, Layout};
use crate::model::AcNetwork;
use crate::solve::SolveIteration;

/// A large finite stand-in for `-inf` on the thermal one-sided bound (`h ≤ 0`),
/// the value upstream Ipopt uses for `nlp_lower_bound_inf`.
const NEG_INF: Number = -1.0e19;

/// Combined-constraint row map for the single pounce `g` vector:
/// `[ equalities (Layout order, ng) | thermal fr (m) | thermal to (m) | angle diff (m) ]`,
/// total `m_p = 2n + 7m + 1`.
#[derive(Clone, Copy)]
struct CombinedRows {
    ng: usize,
    m: usize,
}

impl CombinedRows {
    fn th_fr(&self, e: usize) -> usize {
        self.ng + e
    }
    fn th_to(&self, e: usize) -> usize {
        self.ng + self.m + e
    }
    fn ang(&self, e: usize) -> usize {
        self.ng + 2 * self.m + e
    }
    fn total(&self) -> usize {
        self.ng + 3 * self.m
    }
}

/// How to evaluate one Jacobian entry at the current `x` / flow terms.
enum JacVal {
    /// A constant (`±1` flow-var coefficient, `±sw` angle row, `1` reference row).
    Const(f64),
    /// Active power balance shunt curvature `2·gs[bus]·x[col]` (`col == vm(bus)`).
    ShuntP { bus: usize },
    /// Reactive power balance shunt curvature `-2·bs[bus]·x[col]`.
    ShuntQ { bus: usize },
    /// Flow-definition voltage gradient `-sw[e]·∂flow/∂local[l]`, the local index
    /// `l` selecting `[vm_f, vm_t, va_f, va_t]`.
    FlowGrad { e: usize, flow: usize, l: usize },
    /// Thermal-limit gradient `2·x[col]` on a flow variable.
    ThermLin,
}

/// One Jacobian triplet: fixed `(row, col)` declared once, value recomputed per iter.
struct JacEntry {
    row: usize,
    col: usize,
    val: JacVal,
}

/// How to evaluate one Lagrangian-Hessian contribution. Several contributions can
/// land in the same lower-triangle slot (branches share buses; the shunt diagonal
/// overlaps the flow-def voltage block), so the value pass accumulates by slot.
enum HessKind {
    /// Objective `obj_factor·2·cq[g]` on the `pg` diagonal.
    ObjPg { g: usize },
    /// Shunt `λ[r_pbal]·2·gs - λ[r_qbal]·2·bs` on the `vm` diagonal.
    ShuntVm { bus: usize },
    /// Flow-def curvature `-sw[e]·λ[flow_row]·∂²flow/∂local[a]∂local[b]`.
    FlowHess {
        e: usize,
        flow: usize,
        a: usize,
        b: usize,
    },
    /// Thermal curvature `2·λ[lam_row]` on a flow-variable diagonal.
    Therm { lam_row: usize },
}

struct HessContrib {
    slot: usize,
    kind: HessKind,
}

/// Captured from `finalize_solution` (the only delivery point; slices are borrowed
/// from the solver and must be copied out here).
struct PounceSolution {
    x: Vec<Number>,
    z_l: Vec<Number>,
    z_u: Vec<Number>,
    lambda: Vec<Number>,
    obj_value: Number,
}

/// The pounce `TNLP` adapter. Owns the network (so it is `'static`) and the fixed
/// sparsity patterns + value plans; rebuilds a temporary `AcOpfModel` per eval.
pub(crate) struct AcOpfTnlp {
    net: AcNetwork,
    /// Net-derived model data (branch coefficients + bus incidence), built once and
    /// lent to the per-callback `AcOpfModel` so no eval rebuilds it.
    cache: AcOpfCache,
    lay: Layout,
    rows: CombinedRows,
    nvar: usize,
    xmin: Vec<Number>,
    xmax: Vec<Number>,
    x0: Vec<Number>,
    jac: Vec<JacEntry>,
    hess_rows: Vec<usize>,
    hess_cols: Vec<usize>,
    hess_plan: Vec<HessContrib>,
    trace: Vec<SolveIteration>,
    out: Option<PounceSolution>,
}

/// `[r_pf, r_qf, r_pt, r_qt]` for branch `e`, the equality rows that carry the four
/// flow-definition duals (same order as `branch_flows` returns the terms).
fn flow_rows(lay: &Layout, e: usize) -> [usize; 4] {
    [lay.r_pf(e), lay.r_qf(e), lay.r_pt(e), lay.r_qt(e)]
}

/// The four flow variable columns for branch `e`, in `[pf, qf, pt, qt]` order.
fn flow_cols(lay: &Layout, e: usize) -> [usize; 4] {
    [lay.pf(e), lay.qf(e), lay.pt(e), lay.qt(e)]
}

/// Local voltage columns `[vm_f, vm_t, va_f, va_t]` for branch `e`.
fn local_cols(lay: &Layout, f: usize, t: usize) -> [usize; 4] {
    [lay.vm(f), lay.vm(t), lay.va(f), lay.va(t)]
}

impl AcOpfTnlp {
    pub(crate) fn new(net: &AcNetwork) -> Self {
        let owned = net.clone();
        let (n, m, k) = (owned.n, owned.m, owned.k);
        let cache = AcOpfCache::new(&owned);
        let model = AcOpfModel::from_cache(&owned, &cache);
        let lay = model.lay;
        let nvar = lay.nvar();
        let rows = CombinedRows { ng: lay.ng(), m };

        // Variable bounds, identical to the `interiors` path: vm/pg/qg in their
        // bounds, flows boxed by the rate, angles free.
        let mut xmin = vec![Number::NEG_INFINITY; nvar];
        let mut xmax = vec![Number::INFINITY; nvar];
        for i in 0..n {
            xmin[lay.vm(i)] = owned.vm_min[i];
            xmax[lay.vm(i)] = owned.vm_max[i];
        }
        for g in 0..k {
            xmin[lay.pg(g)] = owned.pmin[g];
            xmax[lay.pg(g)] = owned.pmax[g];
            xmin[lay.qg(g)] = owned.qmin[g];
            xmax[lay.qg(g)] = owned.qmax[g];
        }
        for e in 0..m {
            let r = owned.rate_a[e];
            for col in flow_cols(&lay, e) {
                xmin[col] = -r;
                xmax[col] = r;
            }
        }

        let x0 = build_start(&owned, &lay, &cache, 0);
        let jac = build_jac(&owned, &lay, &rows, &cache);
        let (hess_rows, hess_cols, hess_plan) = build_hess(&owned, &lay);

        drop(model);
        AcOpfTnlp {
            net: owned,
            cache,
            lay,
            rows,
            nvar,
            xmin,
            xmax,
            x0,
            jac,
            hess_rows,
            hess_cols,
            hess_plan,
            trace: Vec::new(),
            out: None,
        }
    }

    fn model(&self) -> AcOpfModel<'_> {
        AcOpfModel::from_cache(&self.net, &self.cache)
    }
}

/// Build a starting point for restart `attempt`. Attempt 0 is the flat start (setpoint
/// magnitudes, zero angles, bound-midpoint dispatch); restarts jitter the voltage and
/// spread the dispatch from its demand-proportional point. Either way the branch-flow
/// variables are seeded on the flow-definition manifold. Mirrors the `interiors`
/// restart schedule so pounce gets the same start diversity (its restoration phase
/// gives up on some flat starts that a perturbed start clears).
fn build_start(net: &AcNetwork, lay: &Layout, cache: &AcOpfCache, attempt: usize) -> Vec<f64> {
    let (n, k) = (net.n, net.k);
    let mut x0 = vec![0.0; lay.nvar()];
    for i in 0..n {
        let base = net.vm_set[i].clamp(net.vm_min[i], net.vm_max[i]);
        x0[lay.vm(i)] = if attempt == 0 {
            base
        } else {
            (base * (1.0 + 0.04 * jitter(i as u64, attempt as u64)))
                .clamp(net.vm_min[i], net.vm_max[i])
        };
        if attempt > 0 && i != net.slack {
            x0[lay.va(i)] = 0.05 * jitter((i as u64).wrapping_add(7), attempt as u64);
        }
    }
    let total_pmax: f64 = net.pmax.iter().sum();
    let load_frac = if total_pmax > 0.0 {
        (net.pd.iter().sum::<f64>() / total_pmax).clamp(0.0, 1.0)
    } else {
        0.5
    };
    for g in 0..k {
        let pmid = 0.5 * (net.pmin[g] + net.pmax[g]);
        let qmid = 0.5 * (net.qmin[g] + net.qmax[g]);
        x0[lay.pg(g)] = if attempt == 0 {
            pmid
        } else {
            let span = net.pmax[g] - net.pmin[g];
            (net.pmin[g]
                + load_frac * span
                + 0.2 * span * jitter((g as u64).wrapping_add(101), attempt as u64))
            .clamp(net.pmin[g], net.pmax[g])
        };
        x0[lay.qg(g)] = if attempt == 0 {
            qmid
        } else {
            let span = net.qmax[g] - net.qmin[g];
            (qmid + 0.2 * span * jitter((g as u64).wrapping_add(211), attempt as u64))
                .clamp(net.qmin[g], net.qmax[g])
        };
    }
    // Seed the branch-flow variables on the flow-definition manifold using the prebuilt
    // cache (branch coefficients + bus incidence) the TNLP already holds, rather than
    // rebuilding the model from scratch on every restart attempt.
    AcOpfModel::from_cache(net, cache).seed_branch_flows(&mut x0);
    x0
}

/// Enumerate the fixed Jacobian pattern (rows = constraints, cols = variables). The
/// full structural pattern is emitted unconditionally — unlike `AcOpfModel::gh`,
/// which drops momentarily-zero flow gradients — so the structure never shifts
/// between iterations (Ipopt declares it once).
fn build_jac(
    net: &AcNetwork,
    lay: &Layout,
    rows: &CombinedRows,
    cache: &AcOpfCache,
) -> Vec<JacEntry> {
    let (n, m) = (net.n, net.m);
    // Bus incidence reused from the prebuilt cache — one source, shared with the model.
    let (gens_at, from_at, to_at) = cache.incidence();

    let mut j = Vec::new();
    let mut push = |row: usize, col: usize, val: JacVal| j.push(JacEntry { row, col, val });

    // Power balance: d/d vm (shunt), the generators, and the incident flow vars.
    for i in 0..n {
        push(lay.r_pbal(i), lay.vm(i), JacVal::ShuntP { bus: i });
        push(lay.r_qbal(i), lay.vm(i), JacVal::ShuntQ { bus: i });
        for &g in &gens_at[i] {
            push(lay.r_pbal(i), lay.pg(g), JacVal::Const(-1.0));
            push(lay.r_qbal(i), lay.qg(g), JacVal::Const(-1.0));
        }
        for &e in &from_at[i] {
            push(lay.r_pbal(i), lay.pf(e), JacVal::Const(1.0));
            push(lay.r_qbal(i), lay.qf(e), JacVal::Const(1.0));
        }
        for &e in &to_at[i] {
            push(lay.r_pbal(i), lay.pt(e), JacVal::Const(1.0));
            push(lay.r_qbal(i), lay.qt(e), JacVal::Const(1.0));
        }
    }

    // Flow definitions: the flow variable (+1) and all four voltage columns
    // (unconditionally, for a stable pattern).
    for e in 0..m {
        let (f, t) = (net.br_from[e], net.br_to[e]);
        let local = local_cols(lay, f, t);
        let frows = flow_rows(lay, e);
        let fcols = flow_cols(lay, e);
        for flow in 0..4 {
            push(frows[flow], fcols[flow], JacVal::Const(1.0));
            for (l, &col) in local.iter().enumerate() {
                push(frows[flow], col, JacVal::FlowGrad { e, flow, l });
            }
        }
    }

    // Reference bus: va_slack = 0.
    push(lay.r_ref(), lay.va(net.slack), JacVal::Const(1.0));

    // Thermal limits: 2·pf, 2·qf (from side) and 2·pt, 2·qt (to side).
    for e in 0..m {
        push(rows.th_fr(e), lay.pf(e), JacVal::ThermLin);
        push(rows.th_fr(e), lay.qf(e), JacVal::ThermLin);
        push(rows.th_to(e), lay.pt(e), JacVal::ThermLin);
        push(rows.th_to(e), lay.qt(e), JacVal::ThermLin);
    }

    // Angle difference: +sw on va_from, -sw on va_to (gated by switching state).
    for e in 0..m {
        let sw = net.sw[e];
        push(rows.ang(e), lay.va(net.br_from[e]), JacVal::Const(sw));
        push(rows.ang(e), lay.va(net.br_to[e]), JacVal::Const(-sw));
    }

    j
}

/// Enumerate the fixed lower-triangle Lagrangian-Hessian pattern and the value
/// plan. Mirrors `AcOpfModel::hess` term for term (same signs), but with a stable
/// pattern and accumulation by `(row, col)` slot.
fn build_hess(net: &AcNetwork, lay: &Layout) -> (Vec<usize>, Vec<usize>, Vec<HessContrib>) {
    let (n, m, k) = (net.n, net.m, net.k);
    let mut slot_of: BTreeMap<(usize, usize), usize> = BTreeMap::new();
    let mut hrows: Vec<usize> = Vec::new();
    let mut hcols: Vec<usize> = Vec::new();
    let mut plan: Vec<HessContrib> = Vec::new();
    let mut push = |r: usize, c: usize, kind: HessKind| {
        let (rr, cc) = if r >= c { (r, c) } else { (c, r) };
        let slot = *slot_of.entry((rr, cc)).or_insert_with(|| {
            hrows.push(rr);
            hcols.push(cc);
            hrows.len() - 1
        });
        plan.push(HessContrib { slot, kind });
    };

    // Objective: 2·cq on the pg diagonal.
    for g in 0..k {
        push(lay.pg(g), lay.pg(g), HessKind::ObjPg { g });
    }
    // Shunt curvature on the vm diagonal (declare whenever the bus has a shunt).
    for i in 0..n {
        if net.gs[i] != 0.0 || net.bs[i] != 0.0 {
            push(lay.vm(i), lay.vm(i), HessKind::ShuntVm { bus: i });
        }
    }
    // Flow-definition curvature: the per-branch symmetric 4×4 block over
    // [vm_f, vm_t, va_f, va_t], lower triangle, summed over the four flow terms.
    for e in 0..m {
        let (f, t) = (net.br_from[e], net.br_to[e]);
        let local = local_cols(lay, f, t);
        for flow in 0..4 {
            for a in 0..4 {
                for b in 0..4 {
                    if local[a] >= local[b] {
                        push(local[a], local[b], HessKind::FlowHess { e, flow, a, b });
                    }
                }
            }
        }
    }
    // Thermal curvature: 2·μ on the flow-variable diagonals.
    let rows = CombinedRows { ng: lay.ng(), m };
    for e in 0..m {
        push(
            lay.pf(e),
            lay.pf(e),
            HessKind::Therm {
                lam_row: rows.th_fr(e),
            },
        );
        push(
            lay.qf(e),
            lay.qf(e),
            HessKind::Therm {
                lam_row: rows.th_fr(e),
            },
        );
        push(
            lay.pt(e),
            lay.pt(e),
            HessKind::Therm {
                lam_row: rows.th_to(e),
            },
        );
        push(
            lay.qt(e),
            lay.qt(e),
            HessKind::Therm {
                lam_row: rows.th_to(e),
            },
        );
    }

    (hrows, hcols, plan)
}

impl TNLP for AcOpfTnlp {
    fn get_nlp_info(&mut self) -> Option<NlpInfo> {
        Some(NlpInfo {
            n: self.nvar as Index,
            m: self.rows.total() as Index,
            nnz_jac_g: self.jac.len() as Index,
            nnz_h_lag: self.hess_rows.len() as Index,
            index_style: IndexStyle::C,
        })
    }

    fn get_bounds_info(&mut self, b: BoundsInfo<'_>) -> bool {
        b.x_l.copy_from_slice(&self.xmin);
        b.x_u.copy_from_slice(&self.xmax);
        let ng = self.rows.ng;
        for r in 0..ng {
            b.g_l[r] = 0.0;
            b.g_u[r] = 0.0;
        }
        for r in ng..ng + 2 * self.rows.m {
            b.g_l[r] = NEG_INF;
            b.g_u[r] = 0.0;
        }
        for e in 0..self.rows.m {
            let r = self.rows.ang(e);
            b.g_l[r] = self.net.sw[e] * self.net.angmin[e];
            b.g_u[r] = self.net.sw[e] * self.net.angmax[e];
        }
        true
    }

    fn get_starting_point(&mut self, sp: StartingPoint<'_>) -> bool {
        if sp.init_x {
            sp.x.copy_from_slice(&self.x0);
        }
        true
    }

    fn eval_f(&mut self, x: &[Number], _new_x: bool) -> Option<Number> {
        Some(self.model().f(x, false).0)
    }

    fn eval_grad_f(&mut self, x: &[Number], _new_x: bool, grad_f: &mut [Number]) -> bool {
        let (_, grad, _) = self.model().f(x, false);
        grad_f.copy_from_slice(&grad);
        true
    }

    fn eval_g(&mut self, x: &[Number], _new_x: bool, g: &mut [Number]) -> bool {
        let model = self.model();
        let (h, g_eq, _, _) = model.gh(x, false);
        let ng = self.rows.ng;
        g[0..ng].copy_from_slice(&g_eq);
        g[ng..ng + h.len()].copy_from_slice(&h);
        for e in 0..self.rows.m {
            let f = self.net.br_from[e];
            let t = self.net.br_to[e];
            g[self.rows.ang(e)] = self.net.sw[e] * (x[self.lay.va(f)] - x[self.lay.va(t)]);
        }
        true
    }

    fn eval_jac_g(
        &mut self,
        x: Option<&[Number]>,
        _new_x: bool,
        mode: SparsityRequest<'_>,
    ) -> bool {
        match mode {
            SparsityRequest::Structure { irow, jcol } => {
                for (k, e) in self.jac.iter().enumerate() {
                    irow[k] = e.row as Index;
                    jcol[k] = e.col as Index;
                }
            }
            SparsityRequest::Values { values } => {
                let x = match x {
                    Some(x) => x,
                    None => return false,
                };
                let model = self.model();
                // The Jacobian reads only flow-term gradients, so take the value+gradient
                // path and skip the dense 4×4 curvature the full `branch_flows` would build.
                let flows: Vec<_> = (0..self.rows.m)
                    .map(|e| model.branch_flows_vg(x, e))
                    .collect();
                for (k, ent) in self.jac.iter().enumerate() {
                    values[k] = match &ent.val {
                        JacVal::Const(c) => *c,
                        JacVal::ShuntP { bus } => 2.0 * self.net.gs[*bus] * x[ent.col],
                        JacVal::ShuntQ { bus } => -2.0 * self.net.bs[*bus] * x[ent.col],
                        JacVal::FlowGrad { e, flow, l } => {
                            -self.net.sw[*e] * flows[*e][*flow].1[*l]
                        }
                        JacVal::ThermLin => 2.0 * x[ent.col],
                    };
                }
            }
        }
        true
    }

    fn eval_h(
        &mut self,
        x: Option<&[Number]>,
        _new_x: bool,
        obj_factor: Number,
        lambda: Option<&[Number]>,
        _new_lambda: bool,
        mode: SparsityRequest<'_>,
    ) -> bool {
        match mode {
            SparsityRequest::Structure { irow, jcol } => {
                for k in 0..self.hess_rows.len() {
                    irow[k] = self.hess_rows[k] as Index;
                    jcol[k] = self.hess_cols[k] as Index;
                }
            }
            SparsityRequest::Values { values } => {
                let (x, lambda) = match (x, lambda) {
                    (Some(x), Some(l)) => (x, l),
                    _ => return false,
                };
                for v in values.iter_mut() {
                    *v = 0.0;
                }
                let model = self.model();
                let flows: Vec<_> = (0..self.rows.m).map(|e| model.branch_flows(x, e)).collect();
                for c in &self.hess_plan {
                    let v = match &c.kind {
                        HessKind::ObjPg { g } => obj_factor * 2.0 * self.net.cq[*g],
                        HessKind::ShuntVm { bus } => {
                            lambda[self.lay.r_pbal(*bus)] * 2.0 * self.net.gs[*bus]
                                - lambda[self.lay.r_qbal(*bus)] * 2.0 * self.net.bs[*bus]
                        }
                        HessKind::FlowHess { e, flow, a, b } => {
                            -self.net.sw[*e]
                                * lambda[flow_rows(&self.lay, *e)[*flow]]
                                * flows[*e][*flow].2[*a][*b]
                        }
                        HessKind::Therm { lam_row } => 2.0 * lambda[*lam_row],
                    };
                    values[c.slot] += v;
                }
            }
        }
        true
    }

    fn intermediate_callback(
        &mut self,
        stats: IterStats,
        _ip_data: &IpoptData,
        _ip_cq: &IpoptCq,
    ) -> bool {
        // Env-gated per-iteration trace for diagnosing non-convergence: shows the barrier
        // parameter, the inertia-correction regularization, and whether the algorithm has
        // fallen into the restoration phase. `POUNCE_DIAG=1` to enable.
        if std::env::var_os("POUNCE_DIAG").is_some() {
            eprintln!(
                "  it={:>3} {:?} inf_pr={:.2e} inf_du={:.2e} mu={:.2e} ||d||={:.2e} rg={:.2e} a_pr={:.2e} ls={}",
                stats.iter, stats.mode, stats.inf_pr, stats.inf_du, stats.mu, stats.d_norm,
                stats.regularization_size, stats.alpha_pr, stats.ls_trials
            );
        }
        self.trace.push(SolveIteration {
            iter: stats.iter as u32,
            objective: stats.obj_value,
            inf_pr: stats.inf_pr,
            inf_du: stats.inf_du,
        });
        true
    }

    fn finalize_solution(&mut self, sol: Solution<'_>, _d: &IpoptData, _q: &IpoptCq) {
        self.out = Some(PounceSolution {
            x: sol.x.to_vec(),
            z_l: sol.z_l.to_vec(),
            z_u: sol.z_u.to_vec(),
            lambda: sol.lambda.to_vec(),
            obj_value: sol.obj_value,
        });
    }
}

/// Solve the full nonlinear AC OPF for `net` with the `pounce` Ipopt-port backend.
/// Returns the same [`AcOpfSolution`] the `interiors` path does (primal plus the
/// six multiplier vectors in the MIPS convention), so the KKT sensitivity consumes
/// it unchanged. Errors if the solver does not converge.
pub fn acopf_pounce(net: &AcNetwork) -> Result<AcOpfSolution, String> {
    acopf_pounce_core(net, None)
}

/// Solve the AC OPF with the `pounce` backend, warm-started from a SOCWR relaxation
/// solution. Reconstructs the AC primal point from `warm` (see
/// [`socwr_warm_start`](super::acopf::socwr_warm_start)) and tries it first, ahead of
/// the flat-start restart schedule — the path that recovers the near-infeasible giants
/// IPOPT's restoration phase abandons from the flat start. The relaxation is solved once
/// by the caller and handed in, so no second conic solve is paid.
#[cfg(feature = "conic")]
pub fn acopf_pounce_warm(
    net: &AcNetwork,
    warm: &crate::problem::SocWrSolution,
) -> Result<AcOpfSolution, String> {
    let model = AcOpfModel::new(net);
    let x0 = super::acopf::socwr_warm_start(net, &model, warm);
    drop(model);
    acopf_pounce_core(net, Some(x0))
}

/// The shared pounce solve. `warm_x0`, when present, is a reconstructed primal start tried
/// as the first attempt, ahead of the flat start and the jittered restarts.
fn acopf_pounce_core(net: &AcNetwork, warm_x0: Option<Vec<f64>>) -> Result<AcOpfSolution, String> {
    // The flat start then a few jittered restarts. IPOPT converges most cases on the
    // flat start, but its restoration phase gives up on some that a perturbed start
    // clears (the same reason the interiors path restarts); pounce is fast enough that
    // a handful of restarts costs little, and only the failure path pays for them.
    const RESTARTS: usize = 6;
    let m = net.m;
    let tnlp = AcOpfTnlp::new(net);
    let lay = tnlp.lay;
    let rows = tnlp.rows;
    let rc = Rc::new(RefCell::new(tnlp));

    // When a warm start is supplied it is attempt 0; the flat start (`build_start` index 0)
    // and the jittered restarts (1..RESTARTS) follow as fallbacks. Without one, the schedule
    // is the plain flat-then-jittered restarts.
    let has_warm = warm_x0.is_some();
    let n_attempts = RESTARTS + usize::from(has_warm);
    let mut solved: Option<(PounceSolution, Vec<SolveIteration>)> = None;
    let mut last_err = String::from("AC OPF (pounce) did not converge");
    for attempt in 0..n_attempts {
        {
            let mut g = rc.borrow_mut();
            g.out = None;
            g.trace.clear();
            g.x0 = if has_warm && attempt == 0 {
                warm_x0.clone().expect("has_warm")
            } else {
                build_start(net, &lay, &g.cache, attempt - usize::from(has_warm))
            };
        }
        let dyn_rc: Rc<RefCell<dyn TNLP>> = rc.clone();
        let mut app = IpoptApplication::new();
        // tol/max_iter are upstream Ipopt option tags; silence the per-iteration log.
        let _ = app.options_mut().set_numeric_value("tol", 1e-8, true, true);
        let _ = app
            .options_mut()
            .set_integer_value("max_iter", 3000, true, true);
        let _ = app
            .options_mut()
            .set_integer_value("print_level", 0, true, true);
        if let Err(e) = app.initialize() {
            last_err = format!("AC OPF (pounce) init failed: {e}");
            continue;
        }
        let status = app.optimize_tnlp(dyn_rc);
        if matches!(
            status,
            ApplicationReturnStatus::SolveSucceeded
                | ApplicationReturnStatus::SolvedToAcceptableLevel
        ) {
            let mut g = rc.borrow_mut();
            if let Some(out) = g.out.take() {
                let trace = std::mem::take(&mut g.trace);
                solved = Some((out, trace));
                break;
            }
            last_err = "AC OPF (pounce): finalize_solution not called".into();
        } else {
            last_err = format!("AC OPF (pounce) did not converge: {status:?}");
        }
    }
    let (out, trace) = solved.ok_or(last_err)?;
    // Variable bounds (constant across attempts) for the bound active-set gating.
    let (xmin, xmax) = {
        let g = rc.borrow();
        (g.xmin.clone(), g.xmax.clone())
    };

    let x = out.x;
    let nvar = lay.nvar();
    if x.len() != nvar {
        return Err(format!(
            "AC OPF (pounce): solution length {} != nvar {}",
            x.len(),
            nvar
        ));
    }

    // Equalities (and the active prices) share the convention with MIPS directly.
    let ng = rows.ng;
    let eq_dual = out.lambda[0..ng].to_vec();

    // Gate the inequality / bound active sets by PRIMAL proximity to the constraint
    // rather than raw dual magnitude. An interior-point solver leaves a barrier-residual
    // dual (`z ≈ mu / slack`) on non-binding bounds; on a tight bound (a small slack)
    // that residual can exceed the active-set threshold `AcOpfKkt` keys on, which would
    // pull inactive constraints into the KKT and corrupt the sensitivity. The interiors
    // backend pre-zeros these via its `mu_threshold`; here we zero any multiplier whose
    // constraint is not primal-binding, so the recovered active set matches.
    const PRIMAL_TOL: f64 = 1e-6;

    // Thermal: `h = pf² + qf² - rate² ≤ 0`, binding when `h` is near zero.
    let mut ineq_dual = vec![0.0; 2 * m];
    for e in 0..m {
        let rate2 = net.rate_a[e] * net.rate_a[e];
        let htol = PRIMAL_TOL * (rate2 + 1.0);
        if x[lay.pf(e)].powi(2) + x[lay.qf(e)].powi(2) - rate2 > -htol {
            ineq_dual[e] = out.lambda[rows.th_fr(e)];
        }
        if x[lay.pt(e)].powi(2) + x[lay.qt(e)].powi(2) - rate2 > -htol {
            ineq_dual[m + e] = out.lambda[rows.th_to(e)];
        }
    }

    // Angle difference: binding at the lower or upper limit (magnitude on that side).
    // Skip open branches (sw == 0): their row collapses to the trivial 0 ≤ 0 ≤ 0, where
    // the proximity test would otherwise always fire and credit a phantom active row.
    let mut lin_l_dual = vec![0.0; m];
    let mut lin_u_dual = vec![0.0; m];
    for e in 0..m {
        let sw = net.sw[e];
        if sw == 0.0 {
            continue;
        }
        let g = sw * (x[lay.va(net.br_from[e])] - x[lay.va(net.br_to[e])]);
        let a = out.lambda[rows.ang(e)].abs();
        if g <= sw * net.angmin[e] + PRIMAL_TOL {
            lin_l_dual[e] = a;
        } else if g >= sw * net.angmax[e] - PRIMAL_TOL {
            lin_u_dual[e] = a;
        }
    }

    // Variable bounds: binding iff `x` sits at the bound. Skip the branch-flow variables:
    // their `[-rate, rate]` box is redundant with the thermal cone, so a flow saturated at
    // the rate would otherwise enter the KKT through both its box bound and the thermal
    // inequality, double-counting the same physical limit. The cone owns flow limiting.
    let flow_start = if m > 0 { lay.pf(0) } else { nvar };
    let mut bnd_l_dual = vec![0.0; nvar];
    let mut bnd_u_dual = vec![0.0; nvar];
    for v in 0..flow_start {
        if xmin[v].is_finite() && x[v] <= xmin[v] + PRIMAL_TOL {
            bnd_l_dual[v] = out.z_l[v];
        }
        if xmax[v].is_finite() && x[v] >= xmax[v] - PRIMAL_TOL {
            bnd_u_dual[v] = out.z_u[v];
        }
    }

    Ok(AcOpfSolution::from_solve(
        &lay,
        x,
        out.obj_value,
        trace,
        eq_dual,
        ineq_dual,
        lin_l_dual,
        lin_u_dual,
        bnd_l_dual,
        bnd_u_dual,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::parse_case9_ac;
    use crate::problem::acopf::acopf;
    use crate::sens::{sensitivity, AcOpfKkt, Mode, Operand, Parameter, Power, VoltageKind};

    const PD: Parameter = Parameter::Demand(Power::Active);
    const QD: Parameter = Parameter::Demand(Power::Reactive);
    const VM: Operand = Operand::Voltage(VoltageKind::Magnitude);
    const PG: Operand = Operand::Dispatch(Power::Active);
    const PRICE: Operand = Operand::Price(Power::Active);

    fn l2(v: &[f64]) -> f64 {
        v.iter().map(|x| x * x).sum::<f64>().sqrt()
    }

    /// pounce reproduces the case9 AC OPF reference (the same value the interiors path
    /// asserts), stays in bounds, and prices are finite and positive (the MIPS sign —
    /// the decisive check that the eq/ineq multiplier convention came through right).
    #[test]
    fn case9_pounce_matches_reference() {
        let net = parse_case9_ac();
        let sol = acopf_pounce(&net).expect("acopf_pounce solve");
        assert!(
            (sol.objective - 5296.6862).abs() < 0.5,
            "pounce AC OPF objective {} != reference 5296.6862",
            sol.objective
        );
        for i in 0..net.n {
            assert!(
                sol.vm[i] >= net.vm_min[i] - 1e-6 && sol.vm[i] <= net.vm_max[i] + 1e-6,
                "vm[{i}] = {} out of bounds",
                sol.vm[i]
            );
        }
        for g in 0..net.k {
            assert!(sol.pg[g] >= net.pmin[g] - 1e-6 && sol.pg[g] <= net.pmax[g] + 1e-6);
        }
        assert!(sol.lmp.iter().all(|v| v.is_finite()));
        // Marginal prices are positive (positive marginal cost of demand): the eq dual
        // convention must match MIPS, since the lmp is the un-negated balance multiplier.
        assert!(
            sol.lmp.iter().all(|&v| v > 0.0),
            "pounce lmp has non-positive entries (wrong dual sign): {:?}",
            sol.lmp
        );
    }

    /// The pounce backend warm-started from the SOCWR relaxation reaches the case9
    /// reference, matching the flat-start pounce solve. The warm start is the lever for the
    /// near-infeasible giants (the benchmark sweep measures that); here it must not perturb
    /// the converged optimum on an easy case.
    #[test]
    fn case9_pounce_warm_matches_reference() {
        let net = parse_case9_ac();
        let soc = crate::problem::socwr_opf(&net).expect("socwr");
        let warm = acopf_pounce_warm(&net, &soc).expect("acopf_pounce_warm");
        let flat = acopf_pounce(&net).expect("acopf_pounce");
        assert!(
            (warm.objective - 5296.6862).abs() < 0.5,
            "warm pounce objective {} != reference 5296.6862",
            warm.objective
        );
        assert!(
            (warm.objective - flat.objective).abs() / flat.objective.abs() < 1e-4,
            "warm {} vs flat {} reach different optima",
            warm.objective,
            flat.objective
        );
    }

    /// The tightest sign/convention check: pounce and interiors solve the same problem,
    /// so their primal and prices agree elementwise. If the eq/ineq dual sign were
    /// flipped, the lmp comparison would fail.
    #[test]
    fn case9_pounce_matches_interiors() {
        let net = parse_case9_ac();
        let p = acopf_pounce(&net).expect("pounce");
        let i = acopf(&net).expect("interiors");
        assert!((p.objective - i.objective).abs() / i.objective.abs() < 1e-4);
        for b in 0..net.n {
            assert!(
                (p.lmp[b] - i.lmp[b]).abs() < 1e-3 * (1.0 + i.lmp[b].abs()),
                "lmp[{b}]: pounce {} vs interiors {}",
                p.lmp[b],
                i.lmp[b]
            );
        }
        // Primal voltages/dispatch agree (same optimum).
        assert!(l2(&p.vm) > 0.0 && (l2(&p.vm) - l2(&i.vm)).abs() < 1e-3);
        for g in 0..net.k {
            assert!((p.pg[g] - i.pg[g]).abs() < 1e-2 * (1.0 + i.pg[g].abs()));
        }
    }

    /// The differentiable path works on a pounce-produced solution: adjoint == forward
    /// (solve-consistency, sign-agnostic) and the analytic columns match a central
    /// difference whose perturbed re-solves go through interiors. Agreement proves the
    /// pounce multipliers carry the correct MIPS-convention values into AcOpfKkt.
    #[test]
    fn pounce_sensitivity_matches_central_differences() {
        let net = parse_case9_ac();
        let sol = acopf_pounce(&net).expect("pounce");
        let sys = AcOpfKkt::new(&net, &sol).expect("kkt");
        let buses: Vec<usize> = (0..net.n).collect();

        // adjoint == forward.
        for op in [PRICE, VM, PG, Operand::Voltage(VoltageKind::Angle)] {
            for par in [PD, QD] {
                let fwd = sensitivity(&sys, op, par, Some(&buses), Mode::Forward).expect("fwd");
                let adj = sensitivity(&sys, op, par, Some(&buses), Mode::Adjoint).expect("adj");
                for (rf, ra) in fwd.values.iter().zip(adj.values.iter()) {
                    for (a, b) in rf.iter().zip(ra.iter()) {
                        assert!((a - b).abs() < 1e-6, "{op:?}/{par:?}: fwd {a} adj {b}");
                    }
                }
            }
        }

        // analytic vs central difference (FD via interiors re-solves).
        let eps = 1e-5;
        for op in [VM, PG, PRICE] {
            let m = sensitivity(&sys, op, PD, Some(&buses), Mode::Forward).expect("analytic");
            for (c, &b) in buses.iter().enumerate() {
                let (mut np, mut nm) = (net.clone(), net.clone());
                np.pd[b] += eps;
                nm.pd[b] -= eps;
                let opv = |s: &AcOpfSolution| match op {
                    VM => s.vm.clone(),
                    PG => s.pg.clone(),
                    PRICE => s.lmp.clone(),
                    _ => unreachable!(),
                };
                let sp = opv(&acopf(&np).expect("+eps"));
                let sm = opv(&acopf(&nm).expect("-eps"));
                let fd: Vec<f64> = (0..sp.len())
                    .map(|i| (sp[i] - sm[i]) / (2.0 * eps))
                    .collect();
                let an: Vec<f64> = (0..m.values.len()).map(|o| m.values[o][c]).collect();
                let diff: Vec<f64> = (0..an.len()).map(|i| an[i] - fd[i]).collect();
                let anorm = l2(&an);
                if anorm < 1e-4 {
                    assert!(
                        l2(&fd) < 1e-2,
                        "{op:?} d/d pd[{b}]: analytic ~0 but FD {}",
                        l2(&fd)
                    );
                    continue;
                }
                assert!(
                    l2(&diff) / anorm < 5e-3,
                    "{op:?} d/d pd[{b}]: rel {}",
                    l2(&diff) / anorm
                );
            }
        }
    }

    /// Exercise the pounce angle-difference dual recovery: clamp case9's angle limits
    /// tight enough to bind, then check the pounce-built KKT sensitivity. Without this,
    /// `lin_l_dual`/`lin_u_dual` from pounce are untested (case9's default ±60° never bind).
    #[test]
    fn pounce_active_angle_limit_matches_central_differences() {
        let mut net = parse_case9_ac();
        for e in 0..net.m {
            net.angmin[e] = net.angmin[e].max(-0.08);
            net.angmax[e] = net.angmax[e].min(0.08);
        }
        let sol = acopf_pounce(&net).expect("pounce");
        let sys = AcOpfKkt::new(&net, &sol).expect("kkt");
        let bound = (0..net.m)
            .filter(|&e| sol.lin_l_dual[e].abs() > 1e-7 || sol.lin_u_dual[e].abs() > 1e-7)
            .count();
        assert!(
            bound > 0,
            "expected an angle-difference limit to bind via pounce"
        );

        let buses: Vec<usize> = (0..net.n).collect();
        let va = Operand::Voltage(VoltageKind::Angle);
        for op in [VM, va, PG] {
            let fwd = sensitivity(&sys, op, PD, Some(&buses), Mode::Forward).expect("fwd");
            let adj = sensitivity(&sys, op, PD, Some(&buses), Mode::Adjoint).expect("adj");
            for (rf, ra) in fwd.values.iter().zip(adj.values.iter()) {
                for (a, b) in rf.iter().zip(ra.iter()) {
                    assert!((a - b).abs() < 1e-6, "{op:?}: fwd {a} adj {b}");
                }
            }
        }
        let eps = 1e-5;
        for op in [VM, va, PG] {
            let m = sensitivity(&sys, op, PD, Some(&buses), Mode::Forward).expect("analytic");
            for (c, &b) in buses.iter().enumerate() {
                let (mut np, mut nm) = (net.clone(), net.clone());
                np.pd[b] += eps;
                nm.pd[b] -= eps;
                let opv = |s: &AcOpfSolution| match op {
                    VM => s.vm.clone(),
                    PG => s.pg.clone(),
                    _ => s.va.clone(),
                };
                let sp = opv(&acopf(&np).expect("+eps"));
                let sm = opv(&acopf(&nm).expect("-eps"));
                let fd: Vec<f64> = (0..sp.len())
                    .map(|i| (sp[i] - sm[i]) / (2.0 * eps))
                    .collect();
                let an: Vec<f64> = (0..m.values.len()).map(|o| m.values[o][c]).collect();
                let diff: Vec<f64> = (0..an.len()).map(|i| an[i] - fd[i]).collect();
                let anorm = l2(&an);
                if anorm < 1e-4 {
                    continue;
                }
                assert!(
                    l2(&diff) / anorm < 3e-2,
                    "{op:?} d/d pd[{b}] angle-binding: rel {}",
                    l2(&diff) / anorm
                );
            }
        }
    }
}
