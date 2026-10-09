//! DC OPF differentiable system: the [`DcKkt`] wrapper over a solved DC OPF, mapping
//! the physical [`Operand`]/[`Parameter`] vocabulary onto the KKT rows it owns.
//!
//! Three axes generalize the original single `dLMP/dd` column, all hand derived:
//!
//! - **operand** — a linear selector `S` on the KKT solution `z`: the price is the
//!   `nu_bal` rows, dispatch the `pg` rows, flow the `f` rows, the angle the `va`
//!   rows.
//! - **parameter** — a hand derived `dK/dp` per parameter type: demand, the costs
//!   `cq`/`cl`, the line limit `fmax`, the series susceptance `b`, and the switching
//!   `sw`. That is the supported DC parameter set.
//! - **mode** — forward or adjoint, run by the shared [`crate::sens::sensitivity`]
//!   driver.
//!
//! The KKT Jacobian and the demand column are hand derived, including the
//! complementarity snapping that puts the solution at strict complementarity before the
//! derivative is taken; the other parameter columns extend the same hand derivation.

use faer::Mat;

use crate::model::DcNetwork;
use crate::problem::DcOpfSolution;

use super::{
    Axis, CostTerm, Differentiable, ElementId, End, Operand, Parameter, Power, Selector, SensError,
    SolveSpec, VoltageKind, GB,
};

/// Strict-complementarity / structural-zero-shed threshold.
const SNAP_TOL: f64 = 1e-6;

/// Tikhonov perturbation for the derivative factorization only. It does not
/// alter the primal program or its declared objective.
const TIKHONOV_EPS: f64 = 1e-10;

/// `DcNetwork::shed_cap(i) < SNAP_TOL`: the bus has no curtailable load (or shedding
/// is disabled), so its shedding variable is structurally fixed at zero. Keying off the
/// model's `shed_cap` — the same bound the solve imposes — is what keeps the linearized
/// program in step with the solved one.
#[inline]
fn is_fixed_zero_shed(dc: &DcNetwork, i: usize) -> bool {
    dc.shed_cap(i) < SNAP_TOL
}

/// The KKT rows of one caller-supplied linear constraint: a multiplier for each
/// stated limit of an inequality, or one for an equality.
#[derive(Clone, Copy, Default)]
struct LinearKktRows {
    upper: Option<usize>,
    lower: Option<usize>,
    equality: Option<usize>,
}

/// The snapped multipliers of one caller-supplied linear constraint, matching
/// [`LinearKktRows`]: the nonnegative upper and lower limit duals of an
/// inequality, or the free dual of an equality.
#[derive(Clone, Copy, Default)]
struct LinearDuals {
    upper: f64,
    lower: f64,
    equality: f64,
}

impl LinearDuals {
    /// The signed multiplier the row adds to stationarity: `a` times this.
    fn signed(&self) -> f64 {
        self.upper - self.lower + self.equality
    }
}

/// Offsets of each block in the flattened KKT variable vector, in the order:
/// `[va, pg, f, psh, cost_epigraph, lam_lb, lam_ub, gamma_lb, gamma_ub,
/// rho_lb, rho_ub, mu_lb, mu_ub, cost_segment_dual, xi, nu_bal, nu_flow, eta]`,
/// where `xi` holds the multipliers of the caller-supplied linear constraints in
/// request order.
struct KktIdx {
    dim: usize,
    va: usize,
    pg: usize,
    f: usize,
    psh: usize,
    cost: Vec<Option<usize>>,
    lam_lb: usize,
    lam_ub: usize,
    gamma_lb: usize,
    gamma_ub: usize,
    rho_lb: usize,
    rho_ub: usize,
    mu_lb: usize,
    mu_ub: usize,
    cost_segment_dual: Vec<Option<usize>>,
    linear: Vec<LinearKktRows>,
    nu_bal: usize,
    nu_flow: usize,
    eta: usize,
}

impl KktIdx {
    fn new(dc: &DcNetwork) -> Self {
        let (n, m, k) = (dc.n, dc.m, dc.k);
        let mut o = 0usize;
        let mut take = |len: usize| {
            let start = o;
            o += len;
            start
        };
        let va = take(n);
        let pg = take(k);
        let f = take(m);
        let psh = take(n);
        let cost = dc
            .piecewise_costs
            .iter()
            .map(|cost| cost.as_ref().map(|_| take(1)))
            .collect();
        let lam_lb = take(m);
        let lam_ub = take(m);
        let gamma_lb = take(m);
        let gamma_ub = take(m);
        let rho_lb = take(k);
        let rho_ub = take(k);
        let mu_lb = take(n);
        let mu_ub = take(n);
        let cost_segment_dual = dc
            .piecewise_costs
            .iter()
            .map(|cost| cost.as_ref().map(|cost| take(cost.segment_count())))
            .collect();
        let linear = dc
            .linear_rows
            .iter()
            .map(|row| {
                if row.is_equality() {
                    LinearKktRows {
                        equality: Some(take(1)),
                        ..LinearKktRows::default()
                    }
                } else {
                    LinearKktRows {
                        upper: row.upper_mw.map(|_| take(1)),
                        lower: row.lower_mw.map(|_| take(1)),
                        equality: None,
                    }
                }
            })
            .collect();
        let nu_bal = take(n);
        let nu_flow = take(m);
        let eta = take(1);
        KktIdx {
            dim: o,
            va,
            pg,
            f,
            psh,
            cost,
            lam_lb,
            lam_ub,
            gamma_lb,
            gamma_ub,
            rho_lb,
            rho_ub,
            mu_lb,
            mu_ub,
            cost_segment_dual,
            linear,
            nu_bal,
            nu_flow,
            eta,
        }
    }
}

/// Split each caller-supplied linear constraint's signed dual into its
/// per-limit multipliers and snap them to strict complementarity: an inequality
/// limit with slack keeps no multiplier, a binding one keeps a nonnegative one.
fn snap_linear(dc: &DcNetwork, sol: &DcOpfSolution) -> Vec<LinearDuals> {
    dc.linear_rows
        .iter()
        .zip(&sol.linear_duals)
        .map(|(row, &dual)| {
            if row.is_equality() {
                return LinearDuals {
                    equality: dual,
                    ..LinearDuals::default()
                };
            }
            let value = row.evaluate(&sol.f, &sol.pg);
            let base = dc.base_mva;
            let binds = |slack: Option<f64>| slack.is_some_and(|slack| slack <= SNAP_TOL);
            LinearDuals {
                upper: if binds(row.upper_mw.map(|upper| upper / base - value)) {
                    dual.max(0.0)
                } else {
                    0.0
                },
                lower: if binds(row.lower_mw.map(|lower| value - lower / base)) {
                    (-dual).max(0.0)
                } else {
                    0.0
                },
                equality: 0.0,
            }
        })
        .collect()
}

/// Snap the solution to strict complementarity before differentiating: zero the
/// non-binding phase-angle duals (and clamp the binding side), and canonicalize the
/// shedding duals / value by regime
/// (degenerate, lower-bound active, upper-bound active, interior).
fn snap(dc: &DcNetwork, sol: &DcOpfSolution) -> DcOpfSolution {
    let mut s = sol.clone();
    for e in 0..dc.m {
        let atheta = s.va[dc.br_from[e]] - s.va[dc.br_to[e]];
        if dc.angmax[e] - atheta > SNAP_TOL {
            s.gamma_ub[e] = 0.0;
        } else {
            s.gamma_ub[e] = s.gamma_ub[e].max(0.0);
        }
        if atheta - dc.angmin[e] > SNAP_TOL {
            s.gamma_lb[e] = 0.0;
        } else {
            s.gamma_lb[e] = s.gamma_lb[e].max(0.0);
        }
    }
    for i in 0..dc.n {
        let cap = dc.shed_cap(i);
        if cap < SNAP_TOL {
            // Degenerate: 0 <= psh <= 0. Fold the duplicate dual pair.
            s.psh[i] = 0.0;
            s.mu_lb[i] -= s.mu_ub[i];
            s.mu_ub[i] = 0.0;
        } else if s.psh[i] < SNAP_TOL {
            s.psh[i] = 0.0;
            s.mu_ub[i] = 0.0;
        } else if cap - s.psh[i] < SNAP_TOL {
            s.psh[i] = cap;
            s.mu_lb[i] = 0.0;
        } else {
            s.mu_lb[i] = 0.0;
            s.mu_ub[i] = 0.0;
        }
    }
    for (generator, cost) in dc.piecewise_costs.iter().enumerate() {
        let Some(cost) = cost else {
            continue;
        };
        let epigraph = s.cost_epigraph[generator].expect("piecewise epigraph primal");
        let duals = s.cost_segment_duals[generator]
            .as_mut()
            .expect("piecewise segment duals");
        for (segment, dual) in duals.iter_mut().enumerate() {
            let slack =
                epigraph - cost.slopes[segment] * s.pg[generator] - cost.intercepts[segment];
            if slack > SNAP_TOL {
                *dual = 0.0;
            } else {
                *dual = dual.max(0.0);
            }
        }
    }
    s
}

/// Per-bus incidence list `(branch, sign)` with `+1` at the from-bus and `-1` at
/// the to-bus — the columns of the incidence matrix `A`.
fn incidence_by_bus(dc: &DcNetwork) -> Vec<Vec<(usize, f64)>> {
    let mut inc = vec![Vec::new(); dc.n];
    for e in 0..dc.m {
        inc[dc.br_from[e]].push((e, 1.0));
        inc[dc.br_to[e]].push((e, -1.0));
    }
    inc
}

/// Per-bus generator list.
fn gens_by_bus(dc: &DcNetwork) -> Vec<Vec<usize>> {
    let mut g = vec![Vec::new(); dc.n];
    for (j, &bus) in dc.gen_bus.iter().enumerate() {
        g[bus].push(j);
    }
    g
}

/// Columns of the susceptance Laplacian `B`: `b_cols[j] = [(row i, B[i,j])]`.
/// `B` is symmetric, so a column doubles as the matching row.
fn susceptance_cols(dc: &DcNetwork) -> Vec<Vec<(usize, f64)>> {
    let mut cols = vec![Vec::new(); dc.n];
    for (r, c, v) in dc.susceptance_coo() {
        cols[c].push((r, v));
    }
    cols
}

/// Assemble the KKT Jacobian `dK/dz` as `(row, col, value)` triplets, hand derived
/// column by column. `s` must already be snapped to strict complementarity.
fn kkt_triplets(
    dc: &DcNetwork,
    s: &DcOpfSolution,
    linear: &[LinearDuals],
    idx: &KktIdx,
) -> Vec<(usize, usize, f64)> {
    let (n, m, k) = (dc.n, dc.m, dc.k);
    let inc = incidence_by_bus(dc);
    let gens = gens_by_bus(dc);
    let bcols = susceptance_cols(dc);

    let mut t: Vec<(usize, usize, f64)> = Vec::new();
    macro_rules! e {
        ($r:expr, $c:expr, $v:expr) => {{
            let v = $v;
            if v != 0.0 {
                t.push(($r, $c, v));
            }
        }};
    }

    // va columns: gated angle stationarity, -B (power balance), -W*A (flow def),
    // and the reference indicator.
    for j in 0..n {
        let col = idx.va + j;
        for &(e, aej) in &inc[j] {
            e!(idx.gamma_lb + e, col, s.gamma_lb[e] * dc.sw[e] * aej);
            e!(idx.gamma_ub + e, col, -s.gamma_ub[e] * dc.sw[e] * aej);
            e!(idx.nu_flow + e, col, dc.b[e] * dc.sw[e] * aej);
        }
        for &(i, val) in &bcols[j] {
            e!(idx.nu_bal + i, col, -val);
        }
        if j == dc.ref_bus {
            e!(idx.eta, col, 1.0);
        }
    }

    // pg columns: objective Hessian, gen-bound stationarity, power balance.
    for j in 0..k {
        let col = idx.pg + j;
        e!(idx.pg + j, col, 2.0 * dc.cq[j]);
        e!(idx.rho_lb + j, col, s.rho_lb[j]);
        e!(idx.rho_ub + j, col, -s.rho_ub[j]);
        e!(idx.nu_bal + dc.gen_bus[j], col, 1.0);
        if let Some(cost) = &dc.piecewise_costs[j] {
            let duals = s.cost_segment_duals[j]
                .as_ref()
                .expect("piecewise segment duals");
            let row_start = idx.cost_segment_dual[j].expect("piecewise dual rows");
            for (segment, &dual) in duals.iter().enumerate() {
                e!(row_start + segment, col, -dual * cost.slopes[segment]);
            }
        }
    }

    // f columns: line-bound stationarity and flow definition.
    for e in 0..m {
        let col = idx.f + e;
        e!(idx.lam_lb + e, col, s.lam_lb[e]);
        e!(idx.lam_ub + e, col, -s.lam_ub[e]);
        e!(idx.nu_flow + e, col, 1.0);
    }

    // psh columns: shedding-bound stationarity (degenerate buses use I) and
    // power balance.
    for i in 0..n {
        let col = idx.psh + i;
        if is_fixed_zero_shed(dc, i) {
            e!(idx.mu_lb + i, col, 1.0);
        } else {
            e!(idx.mu_lb + i, col, s.mu_lb[i]);
            e!(idx.mu_ub + i, col, -s.mu_ub[i]);
        }
        e!(idx.nu_bal + i, col, 1.0);
    }

    // Piecewise cost epigraph primal columns.
    for (generator, cost) in dc.piecewise_costs.iter().enumerate() {
        let Some(_) = cost else {
            continue;
        };
        let col = idx.cost[generator].expect("piecewise cost primal");
        let row_start = idx.cost_segment_dual[generator].expect("piecewise dual rows");
        let duals = s.cost_segment_duals[generator]
            .as_ref()
            .expect("piecewise segment duals");
        for (segment, &dual) in duals.iter().enumerate() {
            e!(row_start + segment, col, dual);
        }
    }

    // lambda (line-bound) columns.
    for e in 0..m {
        let col = idx.lam_lb + e;
        e!(idx.f + e, col, -1.0);
        e!(idx.lam_lb + e, col, s.f[e] + dc.fmax[e]);
    }
    for e in 0..m {
        let col = idx.lam_ub + e;
        e!(idx.f + e, col, 1.0);
        e!(idx.lam_ub + e, col, dc.fmax[e] - s.f[e]);
    }

    // gamma (phase-angle-difference) columns.
    for e in 0..m {
        let col = idx.gamma_lb + e;
        let (fb, tb) = (dc.br_from[e], dc.br_to[e]);
        let atheta = s.va[fb] - s.va[tb];
        e!(idx.va + fb, col, -dc.sw[e]);
        e!(idx.va + tb, col, dc.sw[e]);
        e!(idx.gamma_lb + e, col, dc.sw[e] * (atheta - dc.angmin[e]));
    }
    for e in 0..m {
        let col = idx.gamma_ub + e;
        let (fb, tb) = (dc.br_from[e], dc.br_to[e]);
        let atheta = s.va[fb] - s.va[tb];
        e!(idx.va + fb, col, dc.sw[e]);
        e!(idx.va + tb, col, -dc.sw[e]);
        e!(idx.gamma_ub + e, col, dc.sw[e] * (dc.angmax[e] - atheta));
    }

    // rho (gen-bound) columns.
    for j in 0..k {
        let col = idx.rho_lb + j;
        e!(idx.pg + j, col, -1.0);
        e!(idx.rho_lb + j, col, s.pg[j] - dc.gmin[j]);
    }
    for j in 0..k {
        let col = idx.rho_ub + j;
        e!(idx.pg + j, col, 1.0);
        e!(idx.rho_ub + j, col, dc.gmax[j] - s.pg[j]);
    }

    // mu (shedding-bound) columns.
    for i in 0..n {
        let col = idx.mu_lb + i;
        e!(idx.psh + i, col, -1.0);
        if !is_fixed_zero_shed(dc, i) {
            e!(idx.mu_lb + i, col, s.psh[i]);
        }
    }

    // Piecewise segment dual columns: segment-line contributions to pg and
    // epigraph stationarity, plus segment complementarity.
    for (generator, cost) in dc.piecewise_costs.iter().enumerate() {
        let Some(cost) = cost else {
            continue;
        };
        let epigraph = s.cost_epigraph[generator].expect("piecewise epigraph primal");
        let cost_row = idx.cost[generator].expect("piecewise cost primal");
        let dual_start = idx.cost_segment_dual[generator].expect("piecewise dual rows");
        for segment in 0..cost.segment_count() {
            let col = dual_start + segment;
            e!(idx.pg + generator, col, cost.slopes[segment]);
            e!(cost_row, col, -1.0);
            let slack =
                epigraph - cost.slopes[segment] * s.pg[generator] - cost.intercepts[segment];
            e!(dual_start + segment, col, slack);
        }
    }
    for i in 0..n {
        let col = idx.mu_ub + i;
        e!(idx.psh + i, col, 1.0);
        if is_fixed_zero_shed(dc, i) {
            e!(idx.mu_ub + i, col, 1.0);
        } else {
            e!(idx.mu_ub + i, col, dc.shed_cap(i) - s.psh[i]);
        }
    }

    // nu_bal columns: B' (theta), -G_inc' (pg), -I (psh).
    for i in 0..n {
        let col = idx.nu_bal + i;
        for &(r, val) in &bcols[i] {
            e!(idx.va + r, col, val);
        }
        for &g in &gens[i] {
            e!(idx.pg + g, col, -1.0);
        }
        e!(idx.psh + i, col, -1.0);
    }

    // nu_flow columns: (W*A)' (theta), -I (f).
    for e in 0..m {
        let col = idx.nu_flow + e;
        let (fb, tb) = (dc.br_from[e], dc.br_to[e]);
        e!(idx.va + fb, col, -dc.b[e] * dc.sw[e]);
        e!(idx.va + tb, col, dc.b[e] * dc.sw[e]);
        e!(idx.f + e, col, -1.0);
    }

    // eta column: reference indicator.
    e!(idx.va + dc.ref_bus, idx.eta, 1.0);

    // Caller-supplied linear constraints `a'x` over the flow and dispatch
    // columns. An upper limit enters like a line's upper limit: its multiplier
    // adds `+a` to stationarity, and its complementarity row `xi (u - a'x)`
    // differentiates to `-xi a` and the slack. A lower limit mirrors it, and an
    // equality adds `+a` to stationarity with the primal row `a'x = e`.
    let base = dc.base_mva;
    for ((row, rows), duals) in dc.linear_rows.iter().zip(&idx.linear).zip(linear) {
        let value = row.evaluate(&s.f, &s.pg);
        let columns = || {
            row.flow
                .iter()
                .map(|&(e, c)| (idx.f + e, c))
                .chain(row.generation.iter().map(|&(j, c)| (idx.pg + j, c)))
        };
        if let (Some(r), Some(upper)) = (rows.upper, row.upper_mw) {
            for (col, c) in columns() {
                e!(r, col, -duals.upper * c);
                e!(col, r, c);
            }
            e!(r, r, upper / base - value);
        }
        if let (Some(r), Some(lower)) = (rows.lower, row.lower_mw) {
            for (col, c) in columns() {
                e!(r, col, duals.lower * c);
                e!(col, r, -c);
            }
            e!(r, r, value - lower / base);
        }
        if let Some(r) = rows.equality {
            for (col, c) in columns() {
                e!(r, col, c);
                e!(col, r, c);
            }
        }
    }

    t
}

/// The flow-definition equality dual `nu_flow`, recovered from the `f`
/// stationarity row `lam_ub - lam_lb + sum_l a_le xi_l - nu_flow = 0`, where
/// `xi_l` is a caller-supplied linear constraint's signed multiplier. The solve
/// carries the inequality duals and the primals but not this equality dual,
/// which the susceptance and switching Jacobians need.
fn nu_flow_values(dc: &DcNetwork, s: &DcOpfSolution, linear: &[LinearDuals]) -> Vec<f64> {
    let mut nu_flow: Vec<f64> = (0..dc.m).map(|e| s.lam_ub[e] - s.lam_lb[e]).collect();
    for (row, duals) in dc.linear_rows.iter().zip(linear) {
        for &(e, c) in &row.flow {
            nu_flow[e] += c * duals.signed();
        }
    }
    nu_flow
}

/// A solved DC OPF as a differentiable KKT system. Snaps to strict complementarity
/// and recovers the flow-definition dual once at construction, so the Jacobian and
/// every parameter column see the same canonicalized state. Borrows the network; the
/// snapped solution is owned.
#[non_exhaustive]
pub struct DcKkt<'a> {
    dc: &'a DcNetwork,
    idx: KktIdx,
    snapped: DcOpfSolution,
    linear: Vec<LinearDuals>,
    nu_flow: Vec<f64>,
}

impl<'a> DcKkt<'a> {
    /// Wrap a solved DC OPF, doing the one-time snap and `nu_flow` recovery.
    pub fn new(dc: &'a DcNetwork, sol: &DcOpfSolution) -> Self {
        let snapped = snap(dc, sol);
        let idx = KktIdx::new(dc);
        let linear = snap_linear(dc, &snapped);
        let nu_flow = nu_flow_values(dc, &snapped, &linear);
        DcKkt {
            dc,
            idx,
            snapped,
            linear,
            nu_flow,
        }
    }
}

impl Differentiable for DcKkt<'_> {
    fn formulation(&self) -> &'static str {
        "dc"
    }

    fn dim(&self) -> usize {
        self.idx.dim
    }

    fn jacobian(&self) -> Vec<(usize, usize, f64)> {
        kkt_triplets(self.dc, &self.snapped, &self.linear, &self.idx)
    }

    fn parameter_len(&self, p: Parameter) -> Option<usize> {
        match p {
            Parameter::Demand(Power::Active) => Some(self.dc.n),
            Parameter::Cost(_) => Some(self.dc.k),
            Parameter::LineLimit
            | Parameter::SeriesAdmittance(GB::Susceptance)
            | Parameter::Switching => Some(self.dc.m),
            Parameter::ConstraintLimit => Some(self.dc.linear_rows.len()),
            _ => None,
        }
    }

    /// Parameter Jacobian `dK/dp` for the requested indices, the dense forward
    /// right-hand side (one column per index). Each column is the hand-derived
    /// partial of the KKT residual at the snapped solution; the demand column is the
    /// hand-derived base, the rest extend it.
    ///
    /// Demand and cost touch a single residual row each. The line limit touches the
    /// two line-complementarity rows. Susceptance enters through the edge weight
    /// `w = -b sw`, and switching additionally through the explicit `sw` multiplier on
    /// the phase-limit rows, so they touch power balance, the flow definition, and
    /// angle stationarity (and, for switching, the phase-limit rows).
    fn parameter_jacobian(&self, p: Parameter, idx_cols: &[usize]) -> Result<Mat<f64>, SensError> {
        let dc = self.dc;
        if matches!(p, Parameter::Cost(_)) {
            if let Some(&generator) = idx_cols
                .iter()
                .find(|&&generator| dc.piecewise_costs[generator].is_some())
            {
                return Err(SensError::InvalidInput(format!(
                    "generator {} has a piecewise linear cost, not a quadratic or linear coefficient",
                    dc.gen_ids[generator]
                )));
            }
        }
        let s = &self.snapped;
        let idx = &self.idx;
        let nu_flow = &self.nu_flow;
        let mut j = Mat::<f64>::zeros(idx.dim, idx_cols.len());
        for (c, &col) in idx_cols.iter().enumerate() {
            match p {
                Parameter::Demand(Power::Active) => {
                    j[(idx.nu_bal + col, c)] = -1.0;
                    let dcap = if is_fixed_zero_shed(dc, col) {
                        0.0
                    } else {
                        1.0
                    };
                    j[(idx.mu_ub + col, c)] = s.mu_ub[col] * dcap;
                }
                Parameter::Cost(CostTerm::Quadratic) => {
                    // pg stationarity carries 2 cq pg.
                    j[(idx.pg + col, c)] = 2.0 * s.pg[col];
                }
                Parameter::Cost(CostTerm::Linear) => {
                    // pg stationarity carries cl.
                    j[(idx.pg + col, c)] = 1.0;
                }
                Parameter::LineLimit => {
                    // Line complementarity rows: lam_lb (f + fmax), lam_ub (fmax - f).
                    j[(idx.lam_lb + col, c)] = s.lam_lb[col];
                    j[(idx.lam_ub + col, c)] = s.lam_ub[col];
                }
                Parameter::ConstraintLimit => {
                    // Shifting both limits by delta: xi_ub (u + delta - a'x),
                    // xi_lb (a'x - l - delta), and a'x - e - delta = 0.
                    let rows = idx.linear[col];
                    let duals = self.linear[col];
                    if let Some(r) = rows.upper {
                        j[(r, c)] = duals.upper;
                    }
                    if let Some(r) = rows.lower {
                        j[(r, c)] = -duals.lower;
                    }
                    if let Some(r) = rows.equality {
                        j[(r, c)] = -1.0;
                    }
                }
                Parameter::SeriesAdmittance(GB::Susceptance) => {
                    let (fb, tb) = (dc.br_from[col], dc.br_to[col]);
                    let dth = s.va[fb] - s.va[tb];
                    let dnu = s.nu_bal[fb] - s.nu_bal[tb];
                    let sw = dc.sw[col];
                    let shift = dc.shift[col];
                    let nf = nu_flow[col];
                    // `dc.b` is the negative inductive susceptance and
                    // `flow_offset = dc.b * shift`. Vary both the Laplacian and
                    // its affine phase term, as the source branch does.
                    j[(idx.nu_bal + fb, c)] += sw * (dth - shift);
                    j[(idx.nu_bal + tb, c)] -= sw * (dth - shift);
                    j[(idx.nu_flow + col, c)] += sw * (dth - shift);
                    // Angle stationarity (B nu_bal + w A' nu_flow): d/db.
                    j[(idx.va + fb, c)] += -sw * dnu - sw * nf;
                    j[(idx.va + tb, c)] += sw * dnu + sw * nf;
                }
                Parameter::Switching => {
                    let (fb, tb) = (dc.br_from[col], dc.br_to[col]);
                    let dth = s.va[fb] - s.va[tb];
                    let dnu = s.nu_bal[fb] - s.nu_bal[tb];
                    let b = dc.b[col];
                    let offset = dc.flow_offset[col];
                    let nf = nu_flow[col];
                    let g = s.gamma_ub[col] - s.gamma_lb[col];
                    // Opening a phase shifter removes both its angle dependent
                    // weight and its affine flow/nodal withdrawal.
                    j[(idx.nu_bal + fb, c)] += b * dth - offset;
                    j[(idx.nu_bal + tb, c)] -= b * dth - offset;
                    j[(idx.nu_flow + col, c)] += b * dth - offset;
                    // Angle stationarity: the w terms (scaled by b) plus the explicit
                    // sw multiplier in the phase-limit gradient.
                    j[(idx.va + fb, c)] += -b * dnu - b * nf + g;
                    j[(idx.va + tb, c)] += b * dnu + b * nf - g;
                    // Phase-limit complementarity rows carry sw explicitly.
                    j[(idx.gamma_lb + col, c)] += s.gamma_lb[col] * (dth - dc.angmin[col]);
                    j[(idx.gamma_ub + col, c)] += s.gamma_ub[col] * (dc.angmax[col] - dth);
                }
                _ => {
                    return Err(SensError::InvalidInput(format!(
                        "dc does not support parameter {p:?}"
                    )))
                }
            }
        }
        Ok(j)
    }

    fn operand_len(&self, o: Operand) -> Option<usize> {
        match o {
            Operand::Price(Power::Active) | Operand::Voltage(VoltageKind::Angle) => Some(self.dc.n),
            Operand::Dispatch(Power::Active) => Some(self.dc.k),
            Operand::Flow {
                power: Power::Active,
                end: End::From,
            } => Some(self.dc.m),
            _ => None,
        }
    }

    /// The KKT-solution rows the operand selects, with reporting sign `+1` (`nu_bal`
    /// is already the positive price, so the DC engine has no per-operand flip). The
    /// leading minus of `dz/dp = -K⁻¹ dK/dp` is supplied by the driver.
    fn operand_selector(&self, o: Operand) -> Result<Selector, SensError> {
        let idx = &self.idx;
        let (rows, n_elem): (Vec<usize>, usize) = match o {
            Operand::Price(Power::Active) => {
                ((0..self.dc.n).map(|i| idx.nu_bal + i).collect(), self.dc.n)
            }
            Operand::Dispatch(Power::Active) => {
                ((0..self.dc.k).map(|j| idx.pg + j).collect(), self.dc.k)
            }
            Operand::Flow {
                power: Power::Active,
                end: End::From,
            } => ((0..self.dc.m).map(|e| idx.f + e).collect(), self.dc.m),
            Operand::Voltage(VoltageKind::Angle) => {
                ((0..self.dc.n).map(|i| idx.va + i).collect(), self.dc.n)
            }
            _ => {
                return Err(SensError::InvalidInput(format!(
                    "dc does not support operand {o:?}"
                )))
            }
        };
        Ok(Selector::new(rows, (0..n_elem).collect(), 1.0))
    }

    fn solve_spec(&self) -> SolveSpec {
        SolveSpec::new(TIKHONOV_EPS, 8, 1e-12)
    }

    fn element_id(&self, axis: Axis, index: usize) -> ElementId {
        match axis {
            Axis::Bus => ElementId::Bus(self.dc.bus_ids[index]),
            Axis::Branch => ElementId::Branch(self.dc.branch_ids[index]),
            Axis::Generator => ElementId::Generator(self.dc.gen_ids[index]),
            Axis::Constraint => ElementId::Constraint(index),
        }
    }

    fn unit_scale(&self, o: Operand, p: Parameter) -> f64 {
        super::served_unit_scale(o, p, self.dc.base_mva)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{parse_case3, DcNetwork};
    use crate::problem::dc_opf;
    use crate::sens::{sensitivity, weighted_sensitivity, Mode};
    use faer::linalg::solvers::Solve;
    use faer::sparse::{SparseColMat, Triplet};
    use faer::Mat;

    const PRICE: Operand = Operand::Price(Power::Active);
    const DEMAND: Parameter = Parameter::Demand(Power::Active);

    /// Central finite difference of `nu_bal` w.r.t. demand at bus `j`.
    fn central_fd(dc: &DcNetwork, j: usize, eps: f64) -> Vec<f64> {
        let mut dp = dc.clone();
        dp.demand[j] += eps;
        let mut dm = dc.clone();
        dm.demand[j] -= eps;
        let sp = dc_opf(&dp).expect("solve +eps");
        let sm = dc_opf(&dm).expect("solve -eps");
        (0..dc.n)
            .map(|i| (sp.nu_bal[i] - sm.nu_bal[i]) / (2.0 * eps))
            .collect()
    }

    #[test]
    fn faer_sparse_lu_solves_a_small_nonsymmetric_system() {
        // Standalone faer sparse-LU validation (runs without case data): a 4x4
        // nonsymmetric system with a known solution. Guards faer-rs#222.
        let entries = vec![
            Triplet::new(0usize, 0usize, 2.0f64),
            Triplet::new(0, 1, -1.0),
            Triplet::new(1, 0, -1.0),
            Triplet::new(1, 1, 2.0),
            Triplet::new(1, 2, -1.0),
            Triplet::new(2, 1, 3.0),
            Triplet::new(2, 2, 2.0),
            Triplet::new(2, 3, -1.0),
            Triplet::new(3, 2, -1.0),
            Triplet::new(3, 3, 2.0),
        ];
        let a = SparseColMat::<usize, f64>::try_new_from_triplets(4, 4, &entries).unwrap();
        let lu = a.sp_lu().expect("sp_lu");
        let mut rhs = Mat::<f64>::zeros(4, 1);
        let b = [1.0, 2.0, 3.0, 4.0];
        for (i, &v) in b.iter().enumerate() {
            rhs[(i, 0)] = v;
        }
        lu.solve_in_place(rhs.as_mut());
        // Residual A x - b.
        let x: Vec<f64> = (0..4).map(|i| rhs[(i, 0)]).collect();
        let mut r = b;
        for tr in &entries {
            r[tr.row] -= tr.val * x[tr.col];
        }
        let resid = r.iter().fold(0.0f64, |acc, &v| acc.max(v.abs()));
        assert!(resid < 1e-10, "faer LU residual {resid}");
    }

    #[test]
    fn dlmp_dd_matches_central_differences_three_bus() {
        let dc = parse_case3();
        let sol = dc_opf(&dc).expect("solve");
        let buses: Vec<usize> = (0..dc.n).collect();
        let sys = DcKkt::new(&dc, &sol);
        // m.values[i][j] = d(price_i)/d(demand_j).
        let m = sensitivity(&sys, PRICE, DEMAND, Some(&buses), Mode::Forward).expect("dlmp/dd");
        for j in 0..dc.n {
            let fd = central_fd(&dc, j, 1e-4);
            for (i, &f) in fd.iter().enumerate().take(dc.n) {
                let a = m.values[i][j];
                let rel = (a - f).abs() / f.abs().max(1.0);
                assert!(
                    rel < 1e-3,
                    "d(nu_bal[{i}])/d(d[{j}]): analytic {a}, fd {f}, rel {rel}"
                );
            }
        }
        // Uncongested: the price rises with demand everywhere (sensitivity > 0).
        for row in &m.values {
            for &v in row {
                assert!(v > 0.0, "expected positive price sensitivity, got {v}");
            }
        }
    }

    /// Parity check: compute the full sensitivity and compare to central finite
    /// differences. Compute the full
    /// dLMP/dd matrix, take the three columns with the largest norm (the most
    /// significant sensitivities, away from near-kink buses), and compare each to
    /// central differences with the same 1 MW step. Returns the worst relative
    /// column error `norm(fd - exact)/norm(exact)`, or `None` if the case file is
    /// absent.
    fn parity_vs_finite_differences(casefile: &str) -> Option<f64> {
        let text = std::fs::read_to_string(casefile).ok()?;
        let net = crate::model::parse_matpower(&text).expect("parse");
        let dc = DcNetwork::from_network(&net).expect("model");
        let sol = dc_opf(&dc).expect("solve");

        let all: Vec<usize> = (0..dc.n).collect();
        let sys = DcKkt::new(&dc, &sol);
        // m.values[i][j] = d(price_i)/d(demand_j); column j is the dLMP/dd column for
        // demand bus j.
        let m = sensitivity(&sys, PRICE, DEMAND, Some(&all), Mode::Forward).expect("dlmp/dd");
        let exact = |j: usize| -> Vec<f64> { (0..dc.n).map(|i| m.values[i][j]).collect() };
        let norm = |c: &[f64]| c.iter().map(|v| v * v).sum::<f64>().sqrt();
        let mut order: Vec<usize> = (0..dc.n).collect();
        order.sort_by(|&a, &b| norm(&exact(b)).total_cmp(&norm(&exact(a))));

        let h = 1e-2; // 1 MW at 100 MVA base
        let mut worst = 0.0f64;
        for &j in order.iter().take(3) {
            let ex = exact(j);
            let fd = central_fd(&dc, j, h);
            let diff: Vec<f64> = (0..dc.n).map(|i| fd[i] - ex[i]).collect();
            let rel = norm(&diff) / norm(&ex).max(f64::EPSILON);
            worst = worst.max(rel);
        }
        Some(worst)
    }

    #[test]
    fn parity_with_finite_differences_activsg200() {
        // The reference exact sensitivity criterion on the Rust
        // path: the top sensitivity columns match central differences within
        // 1e-3. Skips when the data is absent.
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../data/ACTIVSg200/case_ACTIVSg200.m"
        );
        match parity_vs_finite_differences(path) {
            Some(rel) => assert!(rel < 1e-3, "ACTIVSg200 dLMP/dd vs FD rel {rel}"),
            None => eprintln!("skipping ACTIVSg200 parity: {path} not found"),
        }
    }

    // ACTIVSg500 builds a larger full sensitivity matrix, so run it explicitly
    // with `cargo test --release -- --ignored`.
    #[test]
    #[ignore = "heavy: run with --release --ignored"]
    fn parity_with_finite_differences_large_cases() {
        let case = "ACTIVSg500";
        let path = format!(
            "{}/../data/{case}/case_{case}.m",
            env!("CARGO_MANIFEST_DIR")
        );
        match parity_vs_finite_differences(&path) {
            Some(rel) => assert!(rel < 1e-3, "{case} dLMP/dd vs FD rel {rel}"),
            None => eprintln!("skipping {case} parity: {path} not found"),
        }
    }

    // --- Generalized engine: every (operand, parameter) pair, both modes --------

    const OPERANDS: [Operand; 4] = [
        Operand::Price(Power::Active),
        Operand::Dispatch(Power::Active),
        Operand::Flow {
            power: Power::Active,
            end: End::From,
        },
        Operand::Voltage(VoltageKind::Angle),
    ];
    const PARAMETERS: [Parameter; 7] = [
        Parameter::Demand(Power::Active),
        Parameter::Cost(CostTerm::Quadratic),
        Parameter::Cost(CostTerm::Linear),
        Parameter::LineLimit,
        Parameter::SeriesAdmittance(GB::Susceptance),
        Parameter::Switching,
        Parameter::ConstraintLimit,
    ];

    /// The operand vector read straight from a solved DC OPF, in the per-unit
    /// units the analytic selector produces.
    fn operand_vec(sol: &DcOpfSolution, operand: Operand) -> Vec<f64> {
        match operand {
            Operand::Price(Power::Active) => sol.nu_bal.clone(),
            Operand::Dispatch(Power::Active) => sol.pg.clone(),
            Operand::Flow {
                power: Power::Active,
                end: End::From,
            } => sol.f.clone(),
            Operand::Voltage(VoltageKind::Angle) => sol.va.clone(),
            other => unreachable!("unsupported DC test operand {other:?}"),
        }
    }

    /// How many indices a DC parameter ranges over: buses for demand, generators for
    /// cost, branches for the line parameters.
    fn param_count(dc: &DcNetwork, parameter: Parameter) -> usize {
        match parameter {
            Parameter::Demand(Power::Active) => dc.n,
            Parameter::Cost(_) => dc.k,
            Parameter::LineLimit | Parameter::SeriesAdmittance(_) | Parameter::Switching => dc.m,
            Parameter::ConstraintLimit => dc.linear_rows.len(),
            other => unreachable!("unsupported DC test parameter {other:?}"),
        }
    }

    /// A copy of `dc` with parameter index `p` shifted by `delta`.
    fn perturbed(dc: &DcNetwork, parameter: Parameter, p: usize, delta: f64) -> DcNetwork {
        let mut d = dc.clone();
        match parameter {
            Parameter::Demand(Power::Active) => d.demand[p] += delta,
            Parameter::Cost(CostTerm::Quadratic) => d.cq[p] += delta,
            Parameter::Cost(CostTerm::Linear) => d.cl[p] += delta,
            Parameter::LineLimit => d.fmax[p] += delta,
            Parameter::SeriesAdmittance(GB::Susceptance) => {
                d.b[p] += delta;
                d.flow_offset[p] += delta * d.shift[p];
            }
            Parameter::Switching => d.sw[p] += delta,
            Parameter::ConstraintLimit => {
                let row = &mut d.linear_rows[p];
                let shift = delta * d.base_mva;
                row.lower_mw = row.lower_mw.map(|limit| limit + shift);
                row.upper_mw = row.upper_mw.map(|limit| limit + shift);
            }
            other => unreachable!("unsupported DC test parameter {other:?}"),
        }
        d
    }

    /// Central finite difference of `operand` w.r.t. parameter index `p`.
    fn fd_column(
        dc: &DcNetwork,
        operand: Operand,
        parameter: Parameter,
        p: usize,
        eps: f64,
    ) -> Vec<f64> {
        let sp = dc_opf(&perturbed(dc, parameter, p, eps)).expect("solve +eps");
        let sm = dc_opf(&perturbed(dc, parameter, p, -eps)).expect("solve -eps");
        let op = operand_vec(&sp, operand);
        let om = operand_vec(&sm, operand);
        (0..op.len())
            .map(|i| (op[i] - om[i]) / (2.0 * eps))
            .collect()
    }

    /// Per-parameter finite-difference step: small enough to hold the active set,
    /// large enough to clear the 1e-9 solver tolerance. Demand and the line limit
    /// enter the KKT linearly, so the central difference is truncation-free and a
    /// 1 MW (1e-2 pu) step is cleanest; the cost and branch parameters enter
    /// rationally, so they take a smaller step against the mild curvature.
    fn eps_for(parameter: Parameter) -> f64 {
        match parameter {
            Parameter::Demand(Power::Active) => 1e-2,
            Parameter::Cost(_) => 1e-1,
            Parameter::LineLimit | Parameter::ConstraintLimit => 1e-3,
            Parameter::SeriesAdmittance(GB::Susceptance) | Parameter::Switching => 1e-4,
            other => unreachable!("unsupported DC test parameter {other:?}"),
        }
    }

    fn l2(v: &[f64]) -> f64 {
        v.iter().map(|x| x * x).sum::<f64>().sqrt()
    }

    /// Assert analytic (forward) matches central FD for every (operand, parameter)
    /// pair on `dc`, as a per-column 2-norm relative error — the standard
    /// sensitivity parity metric.
    ///
    /// A column whose analytic norm sits below the solve's regularization floor
    /// (the `1e-10` Tikhonov term, scaled to the operand magnitude) carries no
    /// derivative a finite difference can resolve — the floor would dominate any
    /// relative comparison — so for those we only confirm the FD likewise finds
    /// nothing large, guarding against a missed term.
    fn check_parity(dc: &DcNetwork, label: &str) {
        let sol = dc_opf(dc).expect("solve");
        let sys = DcKkt::new(dc, &sol);
        for &op in &OPERANDS {
            let floor = 1e-4 * l2(&operand_vec(&sol, op)).max(1.0);
            for &par in &PARAMETERS {
                if param_count(dc, par) == 0 {
                    continue;
                }
                let m = sensitivity(&sys, op, par, None, Mode::Forward).expect("analytic");
                let eps = eps_for(par);
                // `p` is the parameter (column) index into the row-major matrix, also
                // fed to the finite difference; not a slice walk to enumerate.
                #[allow(clippy::needless_range_loop)]
                for p in 0..param_count(dc, par) {
                    let fd = fd_column(dc, op, par, p, eps);
                    let a: Vec<f64> = (0..fd.len()).map(|i| m.values[i][p]).collect();
                    let diff: Vec<f64> = (0..fd.len()).map(|i| a[i] - fd[i]).collect();
                    let (an, dn, fnorm) = (l2(&a), l2(&diff), l2(&fd));
                    if an <= floor {
                        assert!(
                            fnorm <= floor,
                            "{label}: d({op:?})/d({par:?}[{p}]) analytic ~0 ({an}) but FD finds {fnorm}"
                        );
                        continue;
                    }
                    let rel = dn / an;
                    assert!(
                        rel < 1e-3,
                        "{label}: d({op:?})/d({par:?}[{p}]) rel {rel} (||a||={an} ||diff||={dn})"
                    );
                }
            }
        }
    }

    /// case3 with the bus2-bus3 line tightened below its ~0.5 pu natural flow, so
    /// it binds (its dual is nonzero) without forcing load shedding — needed to
    /// exercise the line-limit parameter and the binding-constraint duals.
    fn congested_case3() -> DcNetwork {
        let mut dc = parse_case3();
        dc.fmax[2] = 0.4;
        dc
    }

    /// One caller-supplied linear row in per unit terms of the dense columns.
    fn linear_row(
        flow: Vec<(usize, f64)>,
        generation: Vec<(usize, f64)>,
        lower_mw: Option<f64>,
        upper_mw: Option<f64>,
    ) -> crate::model::LinearRow {
        crate::model::LinearRow {
            id: "row".to_owned(),
            flow,
            generation,
            lower_mw,
            upper_mw,
        }
    }

    /// case3 with an upper interface limit that binds. Unconstrained, g1 is about
    /// 29.5 MW, g3 about 60.5 MW, and line 1-2 carries 30 + g1/3, about 39.8 MW.
    /// The interface holds line 1-2 to 38 MW; a two-sided row on total
    /// generation sits between its limits.
    fn constrained_case3() -> DcNetwork {
        let mut dc = parse_case3();
        dc.linear_rows = vec![
            linear_row(vec![(0, 1.0)], vec![], Some(-200.0), Some(38.0)),
            linear_row(vec![], vec![(0, 1.0), (1, 1.0)], Some(0.0), Some(200.0)),
        ];
        dc
    }

    /// case3 with a lower limit that binds: bus 3's net injection (the flow
    /// leaving it on lines 1-3 and 2-3, both of which end there) must reach
    /// 70 MW, above its unconstrained 60.5 MW.
    fn injection_constrained_case3() -> DcNetwork {
        let mut dc = parse_case3();
        dc.linear_rows = vec![linear_row(
            vec![(1, -1.0), (2, -1.0)],
            vec![],
            Some(70.0),
            None,
        )];
        dc
    }

    /// case3 with an equality row fixing g1 at 35 MW.
    fn equality_constrained_case3() -> DcNetwork {
        let mut dc = parse_case3();
        dc.linear_rows = vec![linear_row(vec![], vec![(0, 1.0)], Some(35.0), Some(35.0))];
        dc
    }

    #[test]
    fn caller_supplied_rows_bind_where_the_fixtures_intend() {
        let dc = constrained_case3();
        let sol = dc_opf(&dc).expect("solve");
        let values: Vec<f64> = sol.linear_values.iter().map(|v| v * dc.base_mva).collect();
        assert!((values[0] - 38.0).abs() < 1e-5, "{values:?}");
        assert!(sol.linear_duals[0] > 1e-3, "{:?}", sol.linear_duals);
        assert!(sol.linear_duals[1].abs() < 1e-6, "{:?}", sol.linear_duals);
        let dc = injection_constrained_case3();
        let sol = dc_opf(&dc).expect("solve");
        assert!((sol.linear_values[0] * dc.base_mva - 70.0).abs() < 1e-5);
        assert!(sol.linear_duals[0] < -1e-3, "{:?}", sol.linear_duals);
        let eq = equality_constrained_case3();
        let sol = dc_opf(&eq).expect("solve");
        assert!((sol.pg[0] * eq.base_mva - 35.0).abs() < 1e-5);
        assert!(sol.linear_duals[0].abs() > 1e-3, "{:?}", sol.linear_duals);
    }

    #[test]
    fn parity_with_binding_caller_supplied_rows() {
        check_parity(&constrained_case3(), "interface case3");
        check_parity(&injection_constrained_case3(), "injection case3");
    }

    #[test]
    fn parity_with_an_equality_row() {
        check_parity(&equality_constrained_case3(), "equality case3");
    }

    #[test]
    fn constraint_limit_columns_agree_forward_and_adjoint() {
        let dc = constrained_case3();
        let sol = dc_opf(&dc).expect("solve");
        let sys = DcKkt::new(&dc, &sol);
        let parameter = Parameter::ConstraintLimit;
        for operand in OPERANDS {
            let forward = sensitivity(&sys, operand, parameter, None, Mode::Forward).unwrap();
            let adjoint = sensitivity(&sys, operand, parameter, None, Mode::Adjoint).unwrap();
            for (f, a) in forward.values.iter().zip(&adjoint.values) {
                for (x, y) in f.iter().zip(a) {
                    assert!(
                        (x - y).abs() < 1e-8 * x.abs().max(1.0),
                        "{operand:?}: {x} vs {y}"
                    );
                }
            }
            assert!(forward
                .cols
                .iter()
                .enumerate()
                .all(|(i, col)| col.element == ElementId::Constraint(i)));
        }
    }

    #[test]
    fn a_binding_interface_moves_the_flow_one_for_one_with_its_limit() {
        let dc = constrained_case3();
        let sol = dc_opf(&dc).expect("solve");
        let sys = DcKkt::new(&dc, &sol);
        let flow = Operand::Flow {
            power: Power::Active,
            end: End::From,
        };
        let m = sensitivity(
            &sys,
            flow,
            Parameter::ConstraintLimit,
            Some(&[0]),
            Mode::Auto,
        )
        .unwrap();
        // The interface is line 1-2 alone: raising its limit by a MW raises the flow
        // by a MW.
        assert!((m.values[0][0] - 1.0).abs() < 1e-6, "{:?}", m.values);
    }

    fn piecewise_congested_case3() -> DcNetwork {
        let text = crate::model::CASE3
            .replace(" 2 0 0 3 0.11  5   0;", " 1 0 0 3 0 0 50 5 250 405;")
            .replace(" 2 0 0 3 0.085 1.2 0;", " 1 0 0 2 0 0 270 135;");
        let network = crate::model::parse_matpower(&text).expect("parse piecewise case3");
        let mut dc = DcNetwork::from_network(&network).expect("prepare piecewise case3");
        dc.fmax[2] = 0.4;
        dc
    }

    /// A nonzero affine branch term makes switching and susceptance derivatives
    /// exercise the phase contribution, rather than only the angle dependent
    /// Laplacian term.
    fn phase_shifter_case3() -> DcNetwork {
        let text = crate::model::CASE3
            .replace(
                " 2 1 90 30 0 0 1 1 0 230 1 1.1 0.9;",
                " 2 1 90 30 5 0 1 1 0 230 1 1.1 0.9;",
            )
            .replace(
                " 1 2 0.01 0.1 0 250 250 250 0 0 1 -360 360;",
                " 1 2 0.01 0.1 0 250 250 250 1 1 1 -60 60;",
            );
        let net = crate::model::parse_matpower(&text).expect("parse phase shifter case");
        DcNetwork::from_network(&net).expect("build phase shifter case")
    }

    /// case3 with generation capacity cut below the 0.9 pu load, so the optimum
    /// must shed: both generators pin at `gmax` and the bus-2 shedding variable
    /// goes interior. This is the only fixture that drives `psh > 0`, exercising
    /// the shedding KKT branch — the `mu` duals, the active-set snap regimes, and
    /// the demand-column `dcap` term — that the served cases never reach.
    fn shedding_case3() -> DcNetwork {
        let mut dc = parse_case3();
        dc.gmax = vec![0.4, 0.4]; // 0.8 pu capacity < 0.9 pu load
        dc.allow_shed = true;
        dc
    }

    #[test]
    fn shedding_regime_kkt_is_consistent() {
        let dc = shedding_case3();
        let sol = dc_opf(&dc).expect("solve");
        // The case must actually shed for this to test the shedding regime.
        let total_shed: f64 = sol.psh.iter().sum();
        assert!(
            total_shed > 1e-3,
            "shedding_case3 did not shed (psh sum {total_shed})"
        );
        // Both generators pin at gmax, so the price is set by the shed cost and the
        // operating point no longer responds to demand — finite differences are
        // trivial here, so parity is the wrong test. What this regime uniquely
        // exercises is the shedding KKT (the `mu` duals, the interior/fixed-zero
        // snap regimes, the demand `dcap` term); that the differentiated system
        // factorizes and every sensitivity is finite is the real check, and
        // `adjoint_equals_forward` (which includes this case) pins it further.
        let sys = DcKkt::new(&dc, &sol);
        for &op in &OPERANDS {
            for &par in &PARAMETERS {
                let m = sensitivity(&sys, op, par, None, Mode::Forward).expect("shed sensitivity");
                for row in &m.values {
                    for &v in row {
                        assert!(
                            v.is_finite(),
                            "non-finite {op:?}/{par:?} under shedding: {v}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn sensitivity_parity_uncongested() {
        check_parity(&parse_case3(), "uncongested");
    }

    #[test]
    fn sensitivity_parity_with_a_phase_shifter_and_shunt() {
        check_parity(&phase_shifter_case3(), "phase shifter");
    }

    #[test]
    fn sensitivity_parity_congested() {
        let dc = congested_case3();
        let sol = dc_opf(&dc).expect("solve");
        // The case must actually congest (a line dual is nonzero) and not shed, so
        // the line-limit parity is a real test rather than a trivial 0 == 0.
        let binding_line = (0..dc.m)
            .max_by(|&a, &b| {
                (sol.lam_lb[a] + sol.lam_ub[a]).total_cmp(&(sol.lam_lb[b] + sol.lam_ub[b]))
            })
            .expect("a branch");
        assert!(
            sol.lam_lb[binding_line] + sol.lam_ub[binding_line] > 1e-3,
            "congested_case3 did not bind a line"
        );
        for &p in &sol.psh {
            assert!(p.abs() < 1e-6, "congested_case3 unexpectedly sheds {p}");
        }
        // The binding line's limit must move prices, so the line-limit parity is a
        // real validation of dK/dfmax rather than a skipped near-zero column.
        let sys = DcKkt::new(&dc, &sol);
        let lim = sensitivity(&sys, PRICE, Parameter::LineLimit, None, Mode::Forward)
            .expect("line-limit sensitivity");
        let lim_col = l2(&(0..dc.n)
            .map(|i| lim.values[i][binding_line])
            .collect::<Vec<_>>());
        assert!(
            lim_col > 1.0,
            "line-limit price sensitivity at binding line {binding_line} is trivial: {lim_col}"
        );
        check_parity(&dc, "congested");
    }

    #[test]
    fn adjoint_equals_forward() {
        for dc in [parse_case3(), congested_case3(), shedding_case3()] {
            let sol = dc_opf(&dc).expect("solve");
            let sys = DcKkt::new(&dc, &sol);
            for &op in &OPERANDS {
                for &par in &PARAMETERS {
                    let fwd = sensitivity(&sys, op, par, None, Mode::Forward).expect("fwd");
                    let adj = sensitivity(&sys, op, par, None, Mode::Adjoint).expect("adj");
                    assert_eq!(fwd.values.len(), adj.values.len());
                    for (rf, ra) in fwd.values.iter().zip(adj.values.iter()) {
                        for (a, b) in rf.iter().zip(ra.iter()) {
                            assert!(
                                (a - b).abs() < 1e-10,
                                "{op:?}/{par:?}: forward {a} adjoint {b} diff {}",
                                (a - b).abs()
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn piecewise_price_adjoint_matches_a_rating_finite_difference() {
        let dc = piecewise_congested_case3();
        let sol = dc_opf(&dc).expect("solve piecewise congested case3");
        let branch = (0..dc.m)
            .max_by(|&a, &b| {
                (sol.lam_lb[a] + sol.lam_ub[a]).total_cmp(&(sol.lam_lb[b] + sol.lam_ub[b]))
            })
            .expect("a branch");
        assert!(
            sol.lam_lb[branch] + sol.lam_ub[branch] > 1e-3,
            "piecewise fixture did not bind a branch"
        );

        let sys = DcKkt::new(&dc, &sol);
        let weights = [(0, 0.75), (1, -0.25), (2, 0.5)];
        let analytic =
            weighted_sensitivity(&sys, PRICE, &weights, Parameter::LineLimit, Some(&[branch]))
                .expect("piecewise weighted price adjoint")[0];

        let h = 1e-4;
        let objective = |model: &DcNetwork| {
            let solved = dc_opf(model).expect("rating perturbation solve");
            weights
                .iter()
                .map(|&(bus, weight)| weight * solved.nu_bal[bus])
                .sum::<f64>()
        };
        let mut plus = dc.clone();
        plus.fmax[branch] += h;
        let mut minus = dc.clone();
        minus.fmax[branch] -= h;
        let finite_difference = (objective(&plus) - objective(&minus)) / (2.0 * h);
        let tolerance = 1e-5_f64.max(1e-3 * finite_difference.abs());
        assert!(
            (analytic - finite_difference).abs() < tolerance,
            "analytic {analytic}, finite difference {finite_difference}, tolerance {tolerance}"
        );

        let forward = sensitivity(
            &sys,
            PRICE,
            Parameter::LineLimit,
            Some(&[branch]),
            Mode::Forward,
        )
        .expect("piecewise forward price sensitivity");
        let from_forward: f64 = weights
            .iter()
            .map(|&(bus, weight)| weight * forward.values[bus][0])
            .sum();
        assert!((analytic - from_forward).abs() < 1e-10);
    }

    #[test]
    fn piecewise_generators_reject_coefficient_sensitivity() {
        let dc = piecewise_congested_case3();
        let sol = dc_opf(&dc).expect("solve piecewise congested case3");
        let sys = DcKkt::new(&dc, &sol);
        let error = sensitivity(
            &sys,
            PRICE,
            Parameter::Cost(CostTerm::Linear),
            Some(&[0]),
            Mode::Forward,
        )
        .expect_err("a piecewise cost has no linear coefficient parameter");
        assert!(
            matches!(error, SensError::InvalidInput(message) if message.contains("piecewise linear cost"))
        );
    }

    #[test]
    fn auto_mode_matches_explicit_directions() {
        // Auto resolves to forward when params <= operands, adjoint otherwise; both
        // give the same matrix, so Auto must equal an explicit direction everywhere.
        let dc = congested_case3();
        let sol = dc_opf(&dc).expect("solve");
        let sys = DcKkt::new(&dc, &sol);
        for &op in &OPERANDS {
            for &par in &PARAMETERS {
                let auto = sensitivity(&sys, op, par, None, Mode::Auto).expect("auto");
                let fwd = sensitivity(&sys, op, par, None, Mode::Forward).expect("fwd");
                for (ra, rf) in auto.values.iter().zip(fwd.values.iter()) {
                    for (a, f) in ra.iter().zip(rf.iter()) {
                        assert!((a - f).abs() < 1e-10, "{op:?}/{par:?}: auto {a} fwd {f}");
                    }
                }
            }
        }
    }

    #[test]
    fn unsupported_requests_surface_errors() {
        let dc = parse_case3();
        let sol = dc_opf(&dc).expect("solve");
        let sys = DcKkt::new(&dc, &sol);
        // Reactive price has no DC analogue.
        assert!(matches!(
            sensitivity(
                &sys,
                Operand::Price(Power::Reactive),
                DEMAND,
                None,
                Mode::Auto
            ),
            Err(SensError::Unsupported { .. })
        ));
        // Reactive demand likewise.
        assert!(matches!(
            sensitivity(
                &sys,
                PRICE,
                Parameter::Demand(Power::Reactive),
                None,
                Mode::Auto
            ),
            Err(SensError::Unsupported { .. })
        ));
    }

    #[test]
    fn matrix_metadata_names_source_elements() {
        let dc = parse_case3();
        let sol = dc_opf(&dc).expect("solve");
        let sys = DcKkt::new(&dc, &sol);
        // Dispatch (per generator) against demand (per bus): a non-square matrix.
        let m = sensitivity(
            &sys,
            Operand::Dispatch(Power::Active),
            DEMAND,
            None,
            Mode::Forward,
        )
        .expect("dispatch/demand");
        assert_eq!(m.values.len(), dc.k);
        assert_eq!(m.rows.len(), dc.k);
        assert_eq!(m.cols.len(), dc.n);
        assert!(matches!(m.rows[0].element, ElementId::Generator(id) if id == dc.gen_ids[0]));
        assert!(matches!(m.cols[1].element, ElementId::Bus(id) if id == dc.bus_ids[1]));
        assert_eq!(m.units, "per unit");
    }
}
