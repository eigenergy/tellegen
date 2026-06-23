//! Full nonlinear AC optimal power flow in polar coordinates, solved with the
//! `interiors` primal-dual interior-point NLP backend (pure Rust, behind the `acopf`
//! feature). Unlike the SOCWR relaxation, this is the exact nonconvex AC OPF, so its
//! objective matches the published AC reference rather than lower-bounding it.
//!
//! The formulation is the expanded polar model (explicit branch-flow variables), so the
//! only nonlinearity sits in the per-branch flow-definition equalities; the thermal
//! limits are quadratic in the flow variables and the angle-difference limits are linear:
//!
//! ```text
//! variables   x = [va(n), vm(n), pg(k), qg(k), pf(m), qf(m), pt(m), qt(m)]
//! min         sum_g cq_g pg_g^2 + cl_g pg_g + cc_g
//! s.t. (g=0)  sum(flows out of i) - sum(pg at i) + pd_i + gs_i vm_i^2 = 0     (active balance)
//!             sum(flows out of i) - sum(qg at i) + qd_i - bs_i vm_i^2 = 0     (reactive balance)
//!             pf_l - F_pf(vm, va) = 0, and qf / pt / qt likewise              (Ohm flow defs)
//!             va_slack = 0                                                    (reference)
//!      (h<=0) pf_l^2 + qf_l^2 - rate_l^2 <= 0, and the to-side               (thermal)
//!      (lin)  angmin_l <= va_f - va_t <= angmax_l                            (angle diff)
//!      (bnd)  vm/pg/qg in their bounds, flows in [-rate, rate]
//! ```
//!
//! The flow definitions are linear combinations of three voltage atoms — `vm_f^2` (or
//! `vm_t^2`), `vm_f vm_t cos(va_f - va_t)`, `vm_f vm_t sin(va_f - va_t)` — with the same
//! pi-model coefficients the conic relaxation uses in W-space. The gradients and Hessians
//! of those atoms give the constraint Jacobian and the Lagrangian Hessian the solver needs.

use std::borrow::Cow;
use std::cell::RefCell;
// `Duration` is a clock-free value type, so it always comes from `std`. `Instant` reads the
// wall clock, which panics on `wasm32-unknown-unknown` ("time not implemented on this
// platform"); there it comes from `web-time` (backed by `performance.now()`), keeping the AC
// OPF restart budget working in the browser. Native targets use `std::time::Instant` as before.
use std::time::Duration;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

use interiors::{nlp, Lambda, NonlinearConstraint, ObjectiveFunction, Options, ProgressMonitor};
use sparsetools::coo::Coo;
use sparsetools::csr::CSR;
use spsolve::rlu::RLU;

use crate::model::AcNetwork;
use crate::solve::SolveIteration;

/// One flow term evaluated at a state: its value, the gradient over the local 4-vector
/// `[vm_f, vm_t, va_f, va_t]`, and the full symmetric 4×4 Hessian over the same.
type FlowTerm = (f64, [f64; 4], [[f64; 4]; 4]);

/// A flow term's value and gradient only — the form the constraint evaluation and restart
/// seeding read, which never touch the curvature, so the dense 4×4 Hessian is not built.
type FlowTermVg = (f64, [f64; 4]);

/// Variable layout `x = [va(n), vm(n), pg(k), qg(k), pf(m), qf(m), pt(m), qt(m)]`.
#[derive(Clone, Copy)]
pub(crate) struct Layout {
    n: usize,
    m: usize,
    k: usize,
}

impl Layout {
    pub(crate) fn nvar(&self) -> usize {
        2 * self.n + 2 * self.k + 4 * self.m
    }
    pub(crate) fn va(&self, i: usize) -> usize {
        i
    }
    pub(crate) fn vm(&self, i: usize) -> usize {
        self.n + i
    }
    pub(crate) fn pg(&self, g: usize) -> usize {
        2 * self.n + g
    }
    pub(crate) fn qg(&self, g: usize) -> usize {
        2 * self.n + self.k + g
    }
    pub(crate) fn pf(&self, e: usize) -> usize {
        2 * self.n + 2 * self.k + e
    }
    pub(crate) fn qf(&self, e: usize) -> usize {
        2 * self.n + 2 * self.k + self.m + e
    }
    pub(crate) fn pt(&self, e: usize) -> usize {
        2 * self.n + 2 * self.k + 2 * self.m + e
    }
    pub(crate) fn qt(&self, e: usize) -> usize {
        2 * self.n + 2 * self.k + 3 * self.m + e
    }
    // Equality-constraint rows: p_bal(n), q_bal(n), then the four flow defs (m each), ref(1).
    pub(crate) fn ng(&self) -> usize {
        2 * self.n + 4 * self.m + 1
    }
    pub(crate) fn r_pbal(&self, i: usize) -> usize {
        i
    }
    pub(crate) fn r_qbal(&self, i: usize) -> usize {
        self.n + i
    }
    pub(crate) fn r_pf(&self, e: usize) -> usize {
        2 * self.n + e
    }
    pub(crate) fn r_qf(&self, e: usize) -> usize {
        2 * self.n + self.m + e
    }
    pub(crate) fn r_pt(&self, e: usize) -> usize {
        2 * self.n + 2 * self.m + e
    }
    pub(crate) fn r_qt(&self, e: usize) -> usize {
        2 * self.n + 3 * self.m + e
    }
    pub(crate) fn r_ref(&self) -> usize {
        2 * self.n + 4 * self.m
    }
    // Inequality-constraint rows: thermal from (m), thermal to (m).
    pub(crate) fn nh(&self) -> usize {
        2 * self.m
    }
    pub(crate) fn r_th_fr(&self, e: usize) -> usize {
        e
    }
    pub(crate) fn r_th_to(&self, e: usize) -> usize {
        self.m + e
    }
}

/// Per-branch flow-definition coefficients on the three atoms `(self², cos, sin)` for
/// each of the four flows, plus the endpoints and switching state. `self²` is `vm_f²` for
/// the from-side flows and `vm_t²` for the to-side.
#[derive(Clone, Copy)]
struct BranchCoef {
    f: usize,
    t: usize,
    sw: f64,
    // (c_self, c_cos, c_sin) per flow.
    pf: (f64, f64, f64),
    qf: (f64, f64, f64),
    pt: (f64, f64, f64),
    qt: (f64, f64, f64),
}

fn branch_coeffs(net: &AcNetwork, e: usize) -> BranchCoef {
    let (g, b) = (net.g[e], net.b[e]);
    let (gfr, bfr, gto, bto) = (net.g_fr[e], net.b_fr[e], net.g_to[e], net.b_to[e]);
    let tr = net.tap[e] * net.shift[e].cos();
    let ti = net.tap[e] * net.shift[e].sin();
    let tm = net.tap[e] * net.tap[e];
    BranchCoef {
        f: net.br_from[e],
        t: net.br_to[e],
        sw: net.sw[e],
        // From-side (self² = vm_f²).
        pf: (
            (g + gfr) / tm,
            (-g * tr + b * ti) / tm,
            (-b * tr - g * ti) / tm,
        ),
        qf: (
            -(b + bfr) / tm,
            (b * tr + g * ti) / tm,
            (-g * tr + b * ti) / tm,
        ),
        // To-side (self² = vm_t²).
        pt: (g + gto, (-g * tr - b * ti) / tm, (b * tr - g * ti) / tm),
        qt: (-(b + bto), (b * tr - g * ti) / tm, (g * tr + b * ti) / tm),
    }
}

/// Value, gradient, and Hessian of one flow term `c_self·SELF + c_cos·COS + c_sin·SIN`
/// at a branch, where `SELF = vm_s²` (`s = f` if `self_from`, else `t`),
/// `COS = vm_f vm_t cos(Δ)`, `SIN = vm_f vm_t sin(Δ)`, `Δ = va_f − va_t`. Gradient and
/// Hessian are over the local 4-vector `[vm_f, vm_t, va_f, va_t]`; the Hessian is the
/// dense symmetric 4×4.
fn flow_term_vg(
    coef: (f64, f64, f64),
    self_from: bool,
    vmf: f64,
    vmt: f64,
    dtheta: f64,
) -> FlowTermVg {
    let (cs, cc, ci) = coef;
    let (c, s) = (dtheta.cos(), dtheta.sin());
    // SELF = vm_s^2 (s = from or to).
    let (self_v, self_g): (f64, [f64; 4]) = if self_from {
        (vmf * vmf, [2.0 * vmf, 0.0, 0.0, 0.0])
    } else {
        (vmt * vmt, [0.0, 2.0 * vmt, 0.0, 0.0])
    };
    // COS = vm_f vm_t cos(Δ).
    let cos_v = vmf * vmt * c;
    let cos_g = [vmt * c, vmf * c, -vmf * vmt * s, vmf * vmt * s];
    // SIN = vm_f vm_t sin(Δ).
    let sin_v = vmf * vmt * s;
    let sin_g = [vmt * s, vmf * s, vmf * vmt * c, -vmf * vmt * c];

    let val = cs * self_v + cc * cos_v + ci * sin_v;
    let mut grad = [0.0; 4];
    #[allow(clippy::needless_range_loop)]
    for i in 0..4 {
        grad[i] = cs * self_g[i] + cc * cos_g[i] + ci * sin_g[i];
    }
    (val, grad)
}

/// As [`flow_term_vg`] plus the full symmetric 4×4 Hessian over `[vm_f, vm_t, va_f, va_t]`.
/// Only the Lagrangian Hessian assembly needs the curvature, so the value+gradient algebra
/// is shared with [`flow_term_vg`] and this adds the Hessian on top.
fn flow_term(coef: (f64, f64, f64), self_from: bool, vmf: f64, vmt: f64, dtheta: f64) -> FlowTerm {
    let (val, grad) = flow_term_vg(coef, self_from, vmf, vmt, dtheta);
    let (cs, cc, ci) = coef;
    let (c, s) = (dtheta.cos(), dtheta.sin());
    // SELF Hessian: 2 on the self-magnitude diagonal.
    let mut self_h = [[0.0; 4]; 4];
    if self_from {
        self_h[0][0] = 2.0;
    } else {
        self_h[1][1] = 2.0;
    }
    // COS = vm_f vm_t cos(Δ).
    let mut cos_h = [[0.0; 4]; 4];
    cos_h[0][1] = c;
    cos_h[0][2] = -vmt * s;
    cos_h[0][3] = vmt * s;
    cos_h[1][2] = -vmf * s;
    cos_h[1][3] = vmf * s;
    cos_h[2][2] = -vmf * vmt * c;
    cos_h[3][3] = -vmf * vmt * c;
    cos_h[2][3] = vmf * vmt * c;
    // SIN = vm_f vm_t sin(Δ).
    let mut sin_h = [[0.0; 4]; 4];
    sin_h[0][1] = s;
    sin_h[0][2] = vmt * c;
    sin_h[0][3] = -vmt * c;
    sin_h[1][2] = vmf * c;
    sin_h[1][3] = -vmf * c;
    sin_h[2][2] = -vmf * vmt * s;
    sin_h[3][3] = -vmf * vmt * s;
    sin_h[2][3] = vmf * vmt * s;

    let mut hess = [[0.0; 4]; 4];
    #[allow(clippy::needless_range_loop)]
    for i in 0..4 {
        // The atom Hessians carry the upper triangle; mirror into a full symmetric block.
        for j in i..4 {
            let v = cs * self_h[i][j] + cc * cos_h[i][j] + ci * sin_h[i][j];
            hess[i][j] = v;
            hess[j][i] = v;
        }
    }
    (val, grad, hess)
}

/// The AC OPF NLP for one network: holds the model and the bus incidence so the solver
/// callbacks can evaluate the objective, constraints, and Lagrangian Hessian at any `x`.
pub(crate) struct AcOpfModel<'a> {
    pub(crate) net: &'a AcNetwork,
    pub(crate) lay: Layout,
    coeffs: Cow<'a, [BranchCoef]>,
    /// Generators at each bus.
    gens_at: Cow<'a, [Vec<usize>]>,
    /// Branches whose from-end / to-end is each bus.
    from_at: Cow<'a, [Vec<usize>]>,
    to_at: Cow<'a, [Vec<usize>]>,
}

/// Bus incidence borrowed from an [`AcOpfCache`]: generators at each bus, then the
/// branches whose from-end / to-end is each bus.
type Incidence<'a> = (&'a [Vec<usize>], &'a [Vec<usize>], &'a [Vec<usize>]);

/// The net-derived data an [`AcOpfModel`] reads — the per-branch coefficient table and
/// the bus incidence. Build it once with [`AcOpfCache::new`] and lend it to many
/// short-lived models through [`AcOpfModel::from_cache`], so a caller that evaluates the
/// model repeatedly (the pounce TNLP callbacks, several per interior-point iteration)
/// never rebuilds it.
pub(crate) struct AcOpfCache {
    coeffs: Vec<BranchCoef>,
    gens_at: Vec<Vec<usize>>,
    from_at: Vec<Vec<usize>>,
    to_at: Vec<Vec<usize>>,
}

impl AcOpfCache {
    pub(crate) fn new(net: &AcNetwork) -> Self {
        let coeffs = (0..net.m).map(|e| branch_coeffs(net, e)).collect();
        let mut gens_at = vec![Vec::new(); net.n];
        for (g, &bus) in net.gen_bus.iter().enumerate() {
            gens_at[bus].push(g);
        }
        let mut from_at = vec![Vec::new(); net.n];
        let mut to_at = vec![Vec::new(); net.n];
        for e in 0..net.m {
            from_at[net.br_from[e]].push(e);
            to_at[net.br_to[e]].push(e);
        }
        AcOpfCache {
            coeffs,
            gens_at,
            from_at,
            to_at,
        }
    }

    /// Bus incidence: generators at each bus, then the branches whose from-end / to-end
    /// is each bus. Shared with the pounce Jacobian pattern so it is built once.
    pub(crate) fn incidence(&self) -> Incidence<'_> {
        (
            self.gens_at.as_slice(),
            self.from_at.as_slice(),
            self.to_at.as_slice(),
        )
    }
}

impl<'a> AcOpfModel<'a> {
    pub(crate) fn new(net: &'a AcNetwork) -> Self {
        let cache = AcOpfCache::new(net);
        AcOpfModel {
            net,
            lay: Layout {
                n: net.n,
                m: net.m,
                k: net.k,
            },
            coeffs: Cow::Owned(cache.coeffs),
            gens_at: Cow::Owned(cache.gens_at),
            from_at: Cow::Owned(cache.from_at),
            to_at: Cow::Owned(cache.to_at),
        }
    }

    /// Build a model that borrows a prebuilt [`AcOpfCache`] instead of allocating its
    /// own copy — the cheap path for repeated evaluation at a fixed network.
    pub(crate) fn from_cache(net: &'a AcNetwork, cache: &'a AcOpfCache) -> Self {
        AcOpfModel {
            net,
            lay: Layout {
                n: net.n,
                m: net.m,
                k: net.k,
            },
            coeffs: Cow::Borrowed(cache.coeffs.as_slice()),
            gens_at: Cow::Borrowed(cache.gens_at.as_slice()),
            from_at: Cow::Borrowed(cache.from_at.as_slice()),
            to_at: Cow::Borrowed(cache.to_at.as_slice()),
        }
    }

    /// The four flow terms of branch `e` at state `x`: `(value, grad, hess)` each, in the
    /// order `[pf, qf, pt, qt]`. The local variable order of the grad/hess is
    /// `[vm_f, vm_t, va_f, va_t]`.
    pub(crate) fn branch_flows(&self, x: &[f64], e: usize) -> [FlowTerm; 4] {
        let bc = &self.coeffs[e];
        let vmf = x[self.lay.vm(bc.f)];
        let vmt = x[self.lay.vm(bc.t)];
        let dtheta = x[self.lay.va(bc.f)] - x[self.lay.va(bc.t)];
        [
            flow_term(bc.pf, true, vmf, vmt, dtheta),
            flow_term(bc.qf, true, vmf, vmt, dtheta),
            flow_term(bc.pt, false, vmf, vmt, dtheta),
            flow_term(bc.qt, false, vmf, vmt, dtheta),
        ]
    }

    /// The four flow terms of branch `e` as value+gradient only, the form the constraint
    /// evaluation ([`gh`](Self::gh)) and the restart seeding read. Skips the dense 4×4 Hessian
    /// that only the Lagrangian Hessian ([`branch_flows`](Self::branch_flows)) needs, so the
    /// per-iteration constraint pass does not build curvature it throws away.
    pub(crate) fn branch_flows_vg(&self, x: &[f64], e: usize) -> [FlowTermVg; 4] {
        let bc = &self.coeffs[e];
        let vmf = x[self.lay.vm(bc.f)];
        let vmt = x[self.lay.vm(bc.t)];
        let dtheta = x[self.lay.va(bc.f)] - x[self.lay.va(bc.t)];
        [
            flow_term_vg(bc.pf, true, vmf, vmt, dtheta),
            flow_term_vg(bc.qf, true, vmf, vmt, dtheta),
            flow_term_vg(bc.pt, false, vmf, vmt, dtheta),
            flow_term_vg(bc.qt, false, vmf, vmt, dtheta),
        ]
    }

    /// Seed the four branch-flow variables of `x` onto the flow-definition manifold
    /// (`pf = sw·F(vm, va)`, …), clamped into the per-branch rate box. With the voltages
    /// already set in `x`, this starts every flow-definition equality satisfied rather than
    /// violated by the full flow magnitude. Shared by both OPF backends' restart seeding.
    pub(crate) fn seed_branch_flows(&self, x: &mut [f64]) {
        for e in 0..self.net.m {
            let ft = self.branch_flows_vg(x, e);
            let r = self.net.rate_a[e];
            let sw = self.net.sw[e];
            let cols = [
                self.lay.pf(e),
                self.lay.qf(e),
                self.lay.pt(e),
                self.lay.qt(e),
            ];
            for (f, col) in cols.into_iter().enumerate() {
                x[col] = (sw * ft[f].0).clamp(-r, r);
            }
        }
    }
}

impl ObjectiveFunction for AcOpfModel<'_> {
    fn f(&self, x: &[f64], hessian: bool) -> (f64, Vec<f64>, Option<CSR<usize, f64>>) {
        let nvar = self.lay.nvar();
        let mut val = 0.0;
        let mut grad = vec![0.0; nvar];
        for g in 0..self.net.k {
            let pg = x[self.lay.pg(g)];
            val += self.net.cq[g] * pg * pg + self.net.cl[g] * pg + self.net.cc[g];
            grad[self.lay.pg(g)] = 2.0 * self.net.cq[g] * pg + self.net.cl[g];
        }
        if !hessian {
            return (val, grad, None);
        }
        // Objective Hessian: 2 cq on the diagonal of the pg block.
        let mut rows = Vec::new();
        let mut cols = Vec::new();
        let mut data = Vec::new();
        for g in 0..self.net.k {
            rows.push(self.lay.pg(g));
            cols.push(self.lay.pg(g));
            data.push(2.0 * self.net.cq[g]);
        }
        let h = Coo::new(nvar, nvar, rows, cols, data)
            .expect("obj hessian coo")
            .to_csr();
        (val, grad, Some(h))
    }
}

impl NonlinearConstraint for AcOpfModel<'_> {
    fn gh(
        &self,
        x: &[f64],
        gradients: bool,
    ) -> (
        Vec<f64>,
        Vec<f64>,
        Option<CSR<usize, f64>>,
        Option<CSR<usize, f64>>,
    ) {
        let lay = &self.lay;
        let (n, m) = (self.net.n, self.net.m);
        let mut g = vec![0.0; lay.ng()];
        let mut h = vec![0.0; lay.nh()];
        // dg / dh are transposes of the Jacobians (columns = constraint gradients), so
        // entries are (variable row, constraint col, value).
        let mut gr: Vec<usize> = Vec::new();
        let mut gc: Vec<usize> = Vec::new();
        let mut gv: Vec<f64> = Vec::new();
        let mut hr: Vec<usize> = Vec::new();
        let mut hc: Vec<usize> = Vec::new();
        let mut hv: Vec<f64> = Vec::new();

        // Precompute the branch flow terms once (value+gradient only; the constraint pass
        // never reads the curvature, so the dense 4×4 Hessian is not built here).
        let flows: Vec<[FlowTermVg; 4]> = (0..m).map(|e| self.branch_flows_vg(x, e)).collect();

        // Power balance.
        for i in 0..n {
            let vmi = x[lay.vm(i)];
            // Active: sum(out flows) - sum(pg) + pd + gs vm^2.
            let mut pbal = self.net.pd[i] + self.net.gs[i] * vmi * vmi;
            let mut qbal = self.net.qd[i] - self.net.bs[i] * vmi * vmi;
            for &e in &self.from_at[i] {
                pbal += flows[e][0].0; // pf
                qbal += flows[e][1].0; // qf
            }
            for &e in &self.to_at[i] {
                pbal += flows[e][2].0; // pt
                qbal += flows[e][3].0; // qt
            }
            for &gg in &self.gens_at[i] {
                pbal -= x[lay.pg(gg)];
                qbal -= x[lay.qg(gg)];
            }
            g[lay.r_pbal(i)] = pbal;
            g[lay.r_qbal(i)] = qbal;
            if gradients {
                // d/d vm: 2 gs vm (active), -2 bs vm (reactive).
                gr.push(lay.vm(i));
                gc.push(lay.r_pbal(i));
                gv.push(2.0 * self.net.gs[i] * vmi);
                gr.push(lay.vm(i));
                gc.push(lay.r_qbal(i));
                gv.push(-2.0 * self.net.bs[i] * vmi);
                for &gg in &self.gens_at[i] {
                    gr.push(lay.pg(gg));
                    gc.push(lay.r_pbal(i));
                    gv.push(-1.0);
                    gr.push(lay.qg(gg));
                    gc.push(lay.r_qbal(i));
                    gv.push(-1.0);
                }
                for &e in &self.from_at[i] {
                    gr.push(lay.pf(e));
                    gc.push(lay.r_pbal(i));
                    gv.push(1.0);
                    gr.push(lay.qf(e));
                    gc.push(lay.r_qbal(i));
                    gv.push(1.0);
                }
                for &e in &self.to_at[i] {
                    gr.push(lay.pt(e));
                    gc.push(lay.r_pbal(i));
                    gv.push(1.0);
                    gr.push(lay.qt(e));
                    gc.push(lay.r_qbal(i));
                    gv.push(1.0);
                }
            }
        }

        // Flow definitions and thermal limits.
        for e in 0..m {
            let bc = &self.coeffs[e];
            let (f, t) = (bc.f, bc.t);
            let local = [lay.vm(f), lay.vm(t), lay.va(f), lay.va(t)];
            let defs = [
                (lay.r_pf(e), lay.pf(e), &flows[e][0]),
                (lay.r_qf(e), lay.qf(e), &flows[e][1]),
                (lay.r_pt(e), lay.pt(e), &flows[e][2]),
                (lay.r_qt(e), lay.qt(e), &flows[e][3]),
            ];
            for (row, flowvar, term) in defs {
                // g = flowvar - sw * value.
                g[row] = x[flowvar] - bc.sw * term.0;
                if gradients {
                    gr.push(flowvar);
                    gc.push(row);
                    gv.push(1.0);
                    // `l` jointly indexes the local gradient `term.1` and the global
                    // column map `local`.
                    #[allow(clippy::needless_range_loop)]
                    for l in 0..4 {
                        let coeff = -bc.sw * term.1[l];
                        if coeff != 0.0 {
                            gr.push(local[l]);
                            gc.push(row);
                            gv.push(coeff);
                        }
                    }
                }
            }
            // Thermal: pf^2 + qf^2 - rate^2 <= 0, and the to-side.
            let rate2 = self.net.rate_a[e] * self.net.rate_a[e];
            let (pf, qf, pt, qt) = (x[lay.pf(e)], x[lay.qf(e)], x[lay.pt(e)], x[lay.qt(e)]);
            h[lay.r_th_fr(e)] = pf * pf + qf * qf - rate2;
            h[lay.r_th_to(e)] = pt * pt + qt * qt - rate2;
            if gradients {
                hr.push(lay.pf(e));
                hc.push(lay.r_th_fr(e));
                hv.push(2.0 * pf);
                hr.push(lay.qf(e));
                hc.push(lay.r_th_fr(e));
                hv.push(2.0 * qf);
                hr.push(lay.pt(e));
                hc.push(lay.r_th_to(e));
                hv.push(2.0 * pt);
                hr.push(lay.qt(e));
                hc.push(lay.r_th_to(e));
                hv.push(2.0 * qt);
            }
        }

        // Reference bus: va_slack = 0.
        g[lay.r_ref()] = x[lay.va(self.net.slack)];
        if gradients {
            gr.push(lay.va(self.net.slack));
            gc.push(lay.r_ref());
            gv.push(1.0);
        }

        if !gradients {
            return (h, g, None, None);
        }
        let nvar = lay.nvar();
        let dg = Coo::new(nvar, lay.ng(), gr, gc, gv)
            .expect("dg coo")
            .to_csr();
        let dh = Coo::new(nvar, lay.nh(), hr, hc, hv)
            .expect("dh coo")
            .to_csr();
        (h, g, Some(dh), Some(dg))
    }

    fn hess(&self, x: &[f64], lam: &Lambda, cost_mult: f64) -> CSR<usize, f64> {
        let lay = &self.lay;
        let nvar = lay.nvar();
        let (n, m) = (self.net.n, self.net.m);
        // Accumulate the Lagrangian Hessian into a dense-by-key map, then emit triplets.
        let mut rows: Vec<usize> = Vec::new();
        let mut cols: Vec<usize> = Vec::new();
        let mut data: Vec<f64> = Vec::new();
        let mut push = |r: usize, c: usize, v: f64| {
            if v != 0.0 {
                rows.push(r);
                cols.push(c);
                data.push(v);
            }
        };

        // Objective: 2 cq on pg diagonal. `cost_mult` (the MIPS sigma) scales only the
        // objective Hessian; the constraint-curvature blocks below already carry their own
        // duals and must not be scaled by it, so fold it in here rather than at the end.
        for gg in 0..self.net.k {
            push(lay.pg(gg), lay.pg(gg), 2.0 * self.net.cq[gg] * cost_mult);
        }
        // Power-balance shunt curvature, contracted with the balance duals.
        for i in 0..n {
            push(
                lay.vm(i),
                lay.vm(i),
                2.0 * self.net.gs[i] * lam.eq_non_lin[lay.r_pbal(i)],
            );
            push(
                lay.vm(i),
                lay.vm(i),
                -2.0 * self.net.bs[i] * lam.eq_non_lin[lay.r_qbal(i)],
            );
        }
        // Flow-definition curvature (the only voltage nonlinearity), contracted with the
        // flow-def duals. The flow def is `flowvar - sw·value`, so its Hessian is
        // `-sw·(atom Hessian)`; emit the symmetric 4×4 block over `[vm_f, vm_t, va_f, va_t]`.
        for e in 0..m {
            let bc = &self.coeffs[e];
            let local = [lay.vm(bc.f), lay.vm(bc.t), lay.va(bc.f), lay.va(bc.t)];
            let flows = self.branch_flows(x, e);
            let duals = [
                lam.eq_non_lin[lay.r_pf(e)],
                lam.eq_non_lin[lay.r_qf(e)],
                lam.eq_non_lin[lay.r_pt(e)],
                lam.eq_non_lin[lay.r_qt(e)],
            ];
            for (term, &nu) in flows.iter().zip(duals.iter()) {
                let scale = -bc.sw * nu;
                if scale == 0.0 {
                    continue;
                }
                let hb = &term.2;
                // `hb` is the full symmetric 4×4 block; emit both triangles globally.
                // `a`/`b` jointly index `hb` and the global map `local`.
                #[allow(clippy::needless_range_loop)]
                for a in 0..4 {
                    for b in 0..4 {
                        push(local[a], local[b], scale * hb[a][b]);
                    }
                }
            }
        }
        // Thermal curvature: 2 on the flow-variable diagonal, contracted with the duals.
        for e in 0..m {
            let mfr = lam.ineq_non_lin[lay.r_th_fr(e)];
            let mto = lam.ineq_non_lin[lay.r_th_to(e)];
            push(lay.pf(e), lay.pf(e), 2.0 * mfr);
            push(lay.qf(e), lay.qf(e), 2.0 * mfr);
            push(lay.pt(e), lay.pt(e), 2.0 * mto);
            push(lay.qt(e), lay.qt(e), 2.0 * mto);
        }

        Coo::new(nvar, nvar, rows, cols, data)
            .expect("hess coo")
            .to_csr()
    }
}

/// A solved AC OPF: the optimal voltages, dispatch, and branch flows, the nodal price
/// (active power balance dual), the objective (cost incl. the constant term), and the
/// solver diagnostics. The raw primal `x` and the full multiplier set are kept for the
/// KKT sensitivity.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct AcOpfSolution {
    pub va: Vec<f64>,
    pub vm: Vec<f64>,
    pub pg: Vec<f64>,
    pub qg: Vec<f64>,
    pub pf: Vec<f64>,
    pub qf: Vec<f64>,
    pub pt: Vec<f64>,
    pub qt: Vec<f64>,
    /// Active nodal price (LMP), per bus.
    pub lmp: Vec<f64>,
    /// Reactive nodal price, per bus.
    pub lmp_q: Vec<f64>,
    pub objective: f64,
    pub iterations: Vec<SolveIteration>,
    /// Raw primal `x` and the full multiplier set from the solve (equality, inequality,
    /// linear-constraint, and variable-bound duals), kept for the KKT sensitivity.
    pub(crate) x: Vec<f64>,
    pub(crate) eq_dual: Vec<f64>,
    pub(crate) ineq_dual: Vec<f64>,
    pub(crate) lin_l_dual: Vec<f64>,
    pub(crate) lin_u_dual: Vec<f64>,
    pub(crate) bnd_l_dual: Vec<f64>,
    pub(crate) bnd_u_dual: Vec<f64>,
}

impl AcOpfSolution {
    /// Assemble a solution from a solved primal `x`, the full multiplier set, the objective,
    /// and the iteration trace. The two backends build their duals differently (interiors
    /// reads MIPS multipliers, pounce gates Ipopt's by primal proximity) but feed the same
    /// six vectors here, so the field mapping and the nodal-price readout (the balance-dual
    /// blocks) live in one place and cannot drift between the paths.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_solve(
        lay: &Layout,
        x: Vec<f64>,
        objective: f64,
        iterations: Vec<SolveIteration>,
        eq_dual: Vec<f64>,
        ineq_dual: Vec<f64>,
        lin_l_dual: Vec<f64>,
        lin_u_dual: Vec<f64>,
        mut bnd_l_dual: Vec<f64>,
        mut bnd_u_dual: Vec<f64>,
    ) -> AcOpfSolution {
        let (n, m, k) = (lay.n, lay.m, lay.k);
        // Branch-flow box bounds [-rate, rate] are redundant with the thermal cone, which owns
        // flow limiting; zero their duals so a flow saturated at its rate does not enter the
        // KKT through both its box bound and the cone, double-counting the same limit. Both
        // backends route through here, so the cone stays the single owner. (`pf(0) == nvar`
        // when m == 0, so the range is empty and the loop is a no-op.)
        for v in lay.pf(0)..lay.nvar() {
            bnd_l_dual[v] = 0.0;
            bnd_u_dual[v] = 0.0;
        }
        let read = |sel: &dyn Fn(usize) -> usize, len: usize| (0..len).map(|i| x[sel(i)]).collect();
        // Nodal prices are the power balance equality multipliers. The MIPS sign convention
        // makes the active price the positive marginal cost of demand.
        let lmp: Vec<f64> = (0..n).map(|i| eq_dual[lay.r_pbal(i)]).collect();
        let lmp_q: Vec<f64> = (0..n).map(|i| eq_dual[lay.r_qbal(i)]).collect();
        AcOpfSolution {
            va: read(&|i| lay.va(i), n),
            vm: read(&|i| lay.vm(i), n),
            pg: read(&|g| lay.pg(g), k),
            qg: read(&|g| lay.qg(g), k),
            pf: read(&|e| lay.pf(e), m),
            qf: read(&|e| lay.qf(e), m),
            pt: read(&|e| lay.pt(e), m),
            qt: read(&|e| lay.qt(e), m),
            lmp,
            lmp_q,
            objective,
            iterations,
            x,
            eq_dual,
            ineq_dual,
            lin_l_dual,
            lin_u_dual,
            bnd_l_dual,
            bnd_u_dual,
        }
    }
}

/// Collects the interior-point iteration trace as the solve runs. `interiors` calls the
/// monitor once per iteration; the feasibility and gradient conditions are recorded as the
/// primal and dual residuals so the AC OPF carries the same [`SolveIteration`] trace the DC
/// and conic paths do.
struct TraceMonitor {
    trace: RefCell<Vec<SolveIteration>>,
}

impl ProgressMonitor for TraceMonitor {
    fn update(
        &self,
        i: usize,
        feas_cond: f64,
        grad_cond: f64,
        _comp_cond: f64,
        _cost_cond: f64,
        _gamma: f64,
        _step_size: f64,
        obj: f64,
        _alpha_p: f64,
        _alpha_d: f64,
    ) {
        self.trace.borrow_mut().push(SolveIteration {
            iter: i as u32,
            objective: obj,
            inf_pr: feas_cond,
            inf_du: grad_cond,
        });
    }
}

/// Deterministic perturbation in `[-1, 1]` from two indices (SplitMix64), used to spread
/// the AC OPF restart initial points without a random source (so the solve stays
/// reproducible and wasm-free).
pub(crate) fn jitter(a: u64, b: u64) -> f64 {
    let mut z = a
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(b.wrapping_mul(0xD1B5_4A32_D192_ED03))
        .wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z as f64 / u64::MAX as f64) * 2.0 - 1.0
}

/// Reconstruct a full AC OPF primal start `x = [va, vm, pg, qg, pf, qf, pt, qt]` from a
/// SOCWR relaxation solution. Bus magnitudes come from `vm = sqrt(w)`; bus angles by a
/// susceptance-weighted least-squares fit to the per-branch differences
/// `δ_e = atan2(wi, wr) ≈ θ_from − θ_to`, grounded at the reference bus (`va_slack = 0`); the
/// dispatch from the relaxed `pg`/`qg`; and the branch-flow variables seeded on the
/// flow-definition manifold. The relaxation is solved over the same network and is a
/// near-tight lower bound, so this point sits close to the AC optimum — the warm start the
/// near-infeasible / flat-voltage cases need, where the flat start lands the interior point
/// in an infeasible basin.
#[cfg(feature = "conic")]
pub(crate) fn socwr_warm_start(
    net: &AcNetwork,
    model: &AcOpfModel,
    soc: &crate::problem::SocWrSolution,
) -> Vec<f64> {
    let lay = &model.lay;
    let mut x = vec![0.0; lay.nvar()];

    // Magnitudes from `w`, clamped into the voltage box (the relaxation respects the same
    // bounds, so the clamp only guards against solver tolerance at a binding bound).
    for i in 0..net.n {
        let vm = soc.w[i].max(0.0).sqrt();
        x[lay.vm(i)] = vm.clamp(net.vm_min[i], net.vm_max[i]);
    }

    // Angles: a global least-squares recovery, not a spanning-tree walk. With
    // `wr = vm_f vm_t cos(θ_f − θ_t)` and `wi = vm_f vm_t sin(θ_f − θ_t)`, each branch gives
    // a target difference `δ_e = atan2(wi, wr) ≈ θ_from − θ_to`; these are inconsistent
    // around loops (the relaxation works in W-space, not on a true angle field), so a tree
    // walk that fixes them exactly on tree branches leaves arbitrary, large differences on
    // the off-tree branches — and across a stiff transformer a small angle error is a huge
    // flow error, which is what wrecks the power balance the OPF reads off the voltages.
    // Minimize instead the susceptance-weighted flow mismatch `Σ_e w_e (θ_f − θ_t − δ_e)²`
    // with `w_e = (|y_e| / tap_e)²` (the branch's angle→flow gain squared): the normal
    // equations are a reduced weighted graph Laplacian, grounded at the slack (`va = 0`).
    // The stiff branches are matched tightly and the inconsistency is pushed onto the weak
    // branches, where it costs little flow.
    let mut red = vec![usize::MAX; net.n];
    let mut dim = 0;
    for (b, slot) in red.iter_mut().enumerate() {
        if b != net.slack {
            *slot = dim;
            dim += 1;
        }
    }
    let mut lap: Vec<(usize, usize, f64)> = Vec::new();
    let mut rhs = vec![0.0; dim];
    for e in 0..net.m {
        if net.sw[e] == 0.0 {
            continue;
        }
        let (f, t) = (net.br_from[e], net.br_to[e]);
        let tap = if net.tap[e] != 0.0 { net.tap[e] } else { 1.0 };
        let w = (net.g[e] * net.g[e] + net.b[e] * net.b[e]) / (tap * tap);
        if w <= 0.0 {
            continue;
        }
        let delta = soc.wi[e].atan2(soc.wr[e]);
        let (rf, rt) = (red[f], red[t]);
        if rf != usize::MAX {
            lap.push((rf, rf, w));
            rhs[rf] += w * delta;
        }
        if rt != usize::MAX {
            lap.push((rt, rt, w));
            rhs[rt] -= w * delta;
        }
        if rf != usize::MAX && rt != usize::MAX {
            lap.push((rf, rt, -w));
            lap.push((rt, rf, -w));
        }
    }
    // Solve the grounded Laplacian; on failure (disconnected island) fall back to flat
    // angles, which at least carries the magnitudes and dispatch.
    if let Ok(va_red) = crate::solve::solve_sparse(dim, &lap, &rhs) {
        for b in 0..net.n {
            if red[b] != usize::MAX {
                x[lay.va(b)] = va_red[red[b]];
            }
        }
    }

    // Dispatch from the relaxation, clamped into the generator box.
    for g in 0..net.k {
        x[lay.pg(g)] = soc.pg[g].clamp(net.pmin[g], net.pmax[g]);
        x[lay.qg(g)] = soc.qg[g].clamp(net.qmin[g], net.qmax[g]);
    }

    // Branch flows on the flow-definition manifold at the reconstructed voltages.
    model.seed_branch_flows(&mut x);
    x
}

/// Solve the full nonlinear AC OPF for `net` with the `interiors` NLP backend. Returns
/// the optimal operating point, the nodal prices, and the objective (matching the
/// published AC OPF value). Errors if the solver does not converge.
pub fn acopf(net: &AcNetwork) -> Result<AcOpfSolution, String> {
    let model = AcOpfModel::new(net);
    acopf_core(net, &model, None)
}

/// Solve the AC OPF warm-started from a SOCWR relaxation solution. Reconstructs the AC
/// primal point from `warm` (see [`socwr_warm_start`]) and tries it first — globalized,
/// with the merit line search on — before the flat-start restart schedule. This is the
/// path that recovers the near-infeasible giants the flat start cannot crack; the
/// relaxation is solved once by the caller and handed in, so no second conic solve is paid.
#[cfg(feature = "conic")]
pub fn acopf_warm(
    net: &AcNetwork,
    warm: &crate::problem::SocWrSolution,
) -> Result<AcOpfSolution, String> {
    let model = AcOpfModel::new(net);
    let x0 = socwr_warm_start(net, &model, warm);
    acopf_core(net, &model, Some(x0))
}

/// The shared AC OPF solve. `warm_x0`, when present, is a reconstructed primal start tried
/// first (globalized) ahead of the flat-start restart schedule.
fn acopf_core(
    net: &AcNetwork,
    model: &AcOpfModel,
    warm_x0: Option<Vec<f64>>,
) -> Result<AcOpfSolution, String> {
    let lay = &model.lay;
    let (n, m, k) = (net.n, net.m, net.k);
    let nvar = lay.nvar();

    // Variable bounds.
    let mut xmin = vec![f64::NEG_INFINITY; nvar];
    let mut xmax = vec![f64::INFINITY; nvar];
    for i in 0..n {
        xmin[lay.vm(i)] = net.vm_min[i];
        xmax[lay.vm(i)] = net.vm_max[i];
    }
    for g in 0..k {
        xmin[lay.pg(g)] = net.pmin[g];
        xmax[lay.pg(g)] = net.pmax[g];
        xmin[lay.qg(g)] = net.qmin[g];
        xmax[lay.qg(g)] = net.qmax[g];
    }
    for e in 0..m {
        let r = net.rate_a[e];
        for col in [lay.pf(e), lay.qf(e), lay.pt(e), lay.qt(e)] {
            xmin[col] = -r;
            xmax[col] = r;
        }
    }

    // Linear angle-difference constraints: angmin <= va_f - va_t <= angmax.
    let (mut ar, mut ac, mut av) = (Vec::new(), Vec::new(), Vec::new());
    let mut l = vec![0.0; m];
    let mut u = vec![0.0; m];
    for e in 0..m {
        // Gate by the switching state so an open branch carries no angle constraint,
        // matching the flow-definition gating and the conic/DC paths.
        let sw = net.sw[e];
        if sw == 0.0 {
            // Leave the row free (-inf <= 0 <= +inf), which `interiors` treats as an
            // inactive constraint, rather than the degenerate l = u = 0 equality: that is
            // a structurally all-zero row in the stacked KKT and makes the factorization
            // singular. (Upstream filtering keeps sw == 1 in practice; the pounce readout
            // skips sw == 0 the same way, so this completes the open-branch handling.)
            l[e] = f64::NEG_INFINITY;
            u[e] = f64::INFINITY;
            continue;
        }
        ar.push(e);
        ac.push(lay.va(net.br_from[e]));
        av.push(sw);
        ar.push(e);
        ac.push(lay.va(net.br_to[e]));
        av.push(-sw);
        l[e] = sw * net.angmin[e];
        u[e] = sw * net.angmax[e];
    }
    let a_mat = Coo::new(m, nvar, ar, ac, av).expect("A coo").to_csr();

    let solver = RLU::default();

    // Flat start (setpoint magnitudes, zero angles, midpoint dispatch), then a few
    // deterministic perturbations if it does not converge: the MIPS interior point can
    // stall from the flat start on congested / small-angle cases, and a perturbed restart
    // recovers most of them. Only the failure path pays for the extra solves.
    let base_vm: Vec<f64> = (0..n)
        .map(|i| net.vm_set[i].clamp(net.vm_min[i], net.vm_max[i]))
        .collect();
    // The restart schedule has two families. Attempts `0..PLAIN_RESTARTS` are the plain
    // full-step solve: a flat start then voltage-only perturbations, no merit line search.
    // Attempts at and above `PLAIN_RESTARTS` turn on the MIPS line search (step-control) and
    // diversify the start (dispatch spread + branch-flow seeding). The two families recover
    // disjoint sets of hard cases — the full step reaches basins the line search backs away
    // from, and vice versa — so running the plain family first keeps every case that already
    // converges unchanged, and the step-control family is purely additive on top.
    const PLAIN_RESTARTS: usize = 6;
    const RESTARTS: usize = 12;
    // The step-control restarts recover congested cases up to ~1500 buses; above that the
    // genuinely hard giants do not converge from any start, and a single step-control solve
    // on them runs for a minute or more, so the enhanced family is wasted effort there. Cap
    // the enhanced restarts to medium cases and let the giants fail fast on the plain family.
    const ENHANCED_MAX_BUS: usize = 1500;
    let n_restarts = if n <= ENHANCED_MAX_BUS {
        RESTARTS
    } else {
        PLAIN_RESTARTS
    };
    // The enhanced family seeds dispatch proportional to total demand instead of the bound
    // midpoint: on congested / API cases the optimum sits near `pmax` (total load can be
    // most of total capacity), which the midpoint ±span jitter never reaches. `load_frac` is
    // the share of total active capacity the load needs; each generator starts at that share
    // of its own range, so the seeded dispatch roughly meets load.
    let total_pmax: f64 = net.pmax.iter().sum();
    let load_frac = if total_pmax > 0.0 {
        (net.pd.iter().sum::<f64>() / total_pmax).clamp(0.0, 1.0)
    } else {
        0.5
    };
    // Bound the restart effort by wall time so a large case that will not converge fails in
    // bounded time rather than starving the caller's per-case timeout. Convergent cases hit
    // their basin in the first few seconds, so a modest budget keeps every recovery while
    // stopping the fruitless step-control restarts on the genuinely hard giants. Checked
    // between attempts, so the actual stop is this plus at most one in-flight solve.
    const RESTART_BUDGET: Duration = Duration::from_secs(60);
    let start = Instant::now();
    let mut solved: Option<(Vec<f64>, f64, Lambda, Vec<SolveIteration>)> = None;
    let mut last_err = String::from("AC OPF did not converge");

    // The SOCWR warm start, tried first when provided. It runs globalized (the merit line
    // search on) with the enhanced iteration cap, regardless of network size — the giants it
    // targets are well above `ENHANCED_MAX_BUS`, so the restart schedule below would never
    // give them a step-controlled solve. The flat-start schedule still runs as a fallback if
    // the warm start does not converge, so a case that converges from flat is never lost.
    if let Some(x0) = warm_x0.as_ref() {
        let opt = Options {
            step_control: true,
            max_it: 300,
            ..Options::default()
        };
        let monitor = TraceMonitor {
            trace: RefCell::new(Vec::new()),
        };
        match nlp(
            model,
            x0,
            &a_mat,
            &l,
            &u,
            &xmin,
            &xmax,
            Some(model),
            &solver,
            &opt,
            Some(&monitor),
        ) {
            Ok((x, f, true, _iters, lambda)) => {
                solved = Some((x, f, lambda, monitor.trace.into_inner()));
            }
            Ok((_, _, false, ..)) => last_err = "AC OPF did not converge (SOCWR warm start)".into(),
            Err(e) => last_err = format!("AC OPF solve failed (SOCWR warm start): {e}"),
        }
    }

    for attempt in 0..n_restarts {
        if solved.is_some() {
            break;
        }
        let enhanced = attempt >= PLAIN_RESTARTS;
        // Step-control needs more iterations than the plain full step to converge (e.g.
        // case118/api takes ~300); the larger cap only costs the convergent solves nothing,
        // since they exit as soon as they converge. The enhanced family is gated to small /
        // medium cases below (`n_restarts`), so this cap is never paid on a giant.
        let opt = Options {
            step_control: enhanced,
            max_it: if enhanced { 300 } else { 150 },
            ..Options::default()
        };
        // Each family jitters from its own attempt-0 (the flat start for the plain family,
        // the first enhanced attempt for the other) so both explore a fresh spread of starts.
        let jit = attempt % PLAIN_RESTARTS;
        let mut x0 = vec![0.0; nvar];
        for i in 0..n {
            x0[lay.vm(i)] = if jit == 0 {
                base_vm[i]
            } else {
                (base_vm[i] * (1.0 + 0.04 * jitter(i as u64, attempt as u64)))
                    .clamp(net.vm_min[i], net.vm_max[i])
            };
            if jit != 0 && i != net.slack {
                x0[lay.va(i)] = 0.05 * jitter((i as u64).wrapping_add(7), attempt as u64);
            }
        }
        for g in 0..k {
            let pmid = 0.5 * (net.pmin[g] + net.pmax[g]);
            let qmid = 0.5 * (net.qmin[g] + net.qmax[g]);
            // The plain family always starts dispatch at the bound midpoint. The enhanced
            // family starts at the demand-proportional point and spreads from there (±20% of
            // the span on the jittered restarts), so congested cases whose optimum sits near
            // `pmax` get a start in the right neighborhood instead of the midpoint.
            x0[lay.pg(g)] = if enhanced {
                let span = net.pmax[g] - net.pmin[g];
                let base = net.pmin[g] + load_frac * span;
                let j = if jit == 0 {
                    0.0
                } else {
                    0.2 * span * jitter((g as u64).wrapping_add(101), attempt as u64)
                };
                (base + j).clamp(net.pmin[g], net.pmax[g])
            } else {
                pmid
            };
            x0[lay.qg(g)] = if enhanced {
                let span = net.qmax[g] - net.qmin[g];
                let j = if jit == 0 {
                    0.0
                } else {
                    0.2 * span * jitter((g as u64).wrapping_add(211), attempt as u64)
                };
                (qmid + j).clamp(net.qmin[g], net.qmax[g])
            } else {
                qmid
            };
        }
        // The enhanced family seeds the branch-flow variables on the flow-definition
        // manifold. The flow defs are `pf = sw·F(vm, va)` etc.; left at zero, all 4m of them
        // start violated by the full flow magnitude, which the interior point must then drive
        // to zero from scratch. Evaluating the flow atoms at the seeded voltages (clamped
        // into the rate box the flows are bounded by) starts the equalities satisfied.
        if enhanced {
            model.seed_branch_flows(&mut x0);
        }
        let monitor = TraceMonitor {
            trace: RefCell::new(Vec::new()),
        };
        match nlp(
            model,
            &x0,
            &a_mat,
            &l,
            &u,
            &xmin,
            &xmax,
            Some(model),
            &solver,
            &opt,
            Some(&monitor),
        ) {
            Ok((x, f, true, _iters, lambda)) => {
                solved = Some((x, f, lambda, monitor.trace.into_inner()));
                break;
            }
            Ok((_, _, false, ..)) => {
                last_err = format!("AC OPF did not converge (after {} restarts)", attempt + 1)
            }
            Err(e) => last_err = format!("AC OPF solve failed: {e}"),
        }
        if start.elapsed() >= RESTART_BUDGET {
            break;
        }
    }
    let (x, f, lambda, trace) = solved.ok_or(last_err)?;

    // `from_solve` maps the primal/duals into the solution and zeros the redundant
    // flow-box bound duals (the thermal cone owns flow limiting).
    Ok(AcOpfSolution::from_solve(
        lay,
        x,
        f,
        trace,
        lambda.eq_non_lin,
        lambda.ineq_non_lin,
        lambda.mu_l,
        lambda.mu_u,
        lambda.lower,
        lambda.upper,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::parse_case9_ac;
    use crate::problem::socwr_opf;

    /// The SOCWR warm start reaches the same optimum as the flat start on a case that
    /// converges either way, and the reconstruction is sound: magnitudes are `sqrt(w)` in
    /// bounds and the slack angle is the reference zero. The warm start is the lever for the
    /// near-infeasible giants (covered by the benchmark sweep); here it must not perturb the
    /// converged optimum on an easy case.
    #[test]
    fn case9_acopf_warm_matches_reference() {
        let net = parse_case9_ac();
        let soc = socwr_opf(&net).expect("socwr");
        let warm = acopf_warm(&net, &soc).expect("acopf_warm");
        let flat = acopf(&net).expect("acopf");
        assert!(
            (warm.objective - 5296.6862).abs() < 0.5,
            "warm AC OPF objective {} != reference 5296.6862",
            warm.objective
        );
        assert!(
            (warm.objective - flat.objective).abs() / flat.objective < 1e-5,
            "warm {} vs flat {} reach different optima",
            warm.objective,
            flat.objective
        );

        // Reconstruction soundness.
        let model = AcOpfModel::new(&net);
        let x0 = socwr_warm_start(&net, &model, &soc);
        assert!(
            x0[model.lay.va(net.slack)].abs() < 1e-12,
            "slack angle not pinned"
        );
        for i in 0..net.n {
            let vm = x0[model.lay.vm(i)];
            assert!(
                vm >= net.vm_min[i] - 1e-9 && vm <= net.vm_max[i] + 1e-9,
                "warm vm[{i}] = {vm} out of bounds"
            );
            let root = soc.w[i].max(0.0).sqrt();
            assert!(
                (vm - root).abs() < 1e-9
                    || (vm - net.vm_min[i]).abs() < 1e-9
                    || (vm - net.vm_max[i]).abs() < 1e-9,
                "warm vm[{i}] = {vm} is neither sqrt(w) = {root} nor a bound"
            );
        }
    }

    #[test]
    fn case9_acopf_matches_reference() {
        let net = parse_case9_ac();
        // `acopf` returns `Ok` only on convergence, so a successful solve is convergence.
        let sol = acopf(&net).expect("acopf solve");
        // Reference AC OPF objective from the PowerModels (Julia) parity harness.
        assert!(
            (sol.objective - 5296.6862).abs() < 0.5,
            "AC OPF objective {} != reference 5296.6862",
            sol.objective
        );
        // Voltages and dispatch stay within their bounds.
        for i in 0..net.n {
            assert!(
                sol.vm[i] >= net.vm_min[i] - 1e-6 && sol.vm[i] <= net.vm_max[i] + 1e-6,
                "vm[{i}] = {} out of [{}, {}]",
                sol.vm[i],
                net.vm_min[i],
                net.vm_max[i]
            );
        }
        for g in 0..net.k {
            assert!(sol.pg[g] >= net.pmin[g] - 1e-6 && sol.pg[g] <= net.pmax[g] + 1e-6);
        }
        // Every nodal price is finite.
        assert!(sol.lmp.iter().all(|v| v.is_finite()));
    }

    /// The AC OPF objective matches the published PGLib BASELINE `AC ($/h)` on a few
    /// small typical cases. Skips when the corpus is absent.
    #[test]
    fn acopf_matches_pglib_baseline() {
        let root = std::env::var("PGLIB_OPF_PATH")
            .unwrap_or_else(|_| format!("{}/Datasets/pglib-opf", std::env::var("HOME").unwrap()));
        // (file, BASELINE AC $/h) for the typical variant.
        let cases = [
            ("pglib_opf_case5_pjm.m", 17552.0),
            ("pglib_opf_case14_ieee.m", 2178.1),
            ("pglib_opf_case30_ieee.m", 8208.5),
            ("pglib_opf_case57_ieee.m", 37589.0),
        ];
        let mut ran = 0;
        for (file, ac_ref) in cases {
            let path = format!("{root}/{file}");
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let net =
                AcNetwork::from_network(&powerio::parse_str(&text, "matpower").unwrap().network)
                    .expect("build AcNetwork");
            let sol = acopf(&net).expect("acopf solve");
            let rel = (sol.objective - ac_ref).abs() / ac_ref;
            assert!(
                rel < 1e-3,
                "{file}: AC OPF {} vs BASELINE AC {ac_ref} (rel {rel})",
                sol.objective
            );
            ran += 1;
        }
        if ran == 0 {
            eprintln!("skipping acopf_matches_pglib_baseline: corpus absent at {root}");
        }
    }
}
