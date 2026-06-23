//! Conic AC OPF (SOCWR) differentiable system: the [`ConicKkt`] wrapper over a solved
//! relaxation, mapping the physical vocabulary onto its `[x, z]` KKT rows.
//!
//! The relaxation solves `min ½xᵀPx + qᵀx s.t. Ax + s = b, s ∈ K`, with K a product
//! of the zero cone (power balance, Ohm), the nonnegative cone (voltage and generator
//! bounds), and second-order cones (the Jabr and apparent-power limits). Eliminating
//! `s = b − Ax`, the smooth KKT residual in `(x, z)` is
//!   stationarity:        P x + q + Aᵀ z = 0
//!   zero-cone rows:       A_eq x − b_eq = 0
//!   nonnegative rows:     s_i z_i = 0
//!   second-order blocks:  s ∘ z = 0           (the SOC Jordan product)
//! at strict complementarity. Its Jacobian uses the arrow matrices `arrow(s)`,
//! `arrow(z)` on each cone block, assembled once at construction.
//!
//! Real demand enters only the real-power-balance rows of `b`, so `∂(KKT)/∂pd` is a
//! selector. The sign convention is unified with the DC engine: the parameter Jacobian
//! is the natural `+dK/dp` column (a `−1` on the balance residual row), and the driver
//! applies the leading minus of `dz/dp = −K⁻¹ dK/dp`, composing the price flip carried
//! in the operand selector's sign.

use clarabel::solver::SupportedConeT::{NonnegativeConeT, SecondOrderConeT, ZeroConeT};
use faer::Mat;

use crate::formulation::SocWr;
use crate::model::AcNetwork;
use crate::problem::{build_conic_opf, SocWrLayout, SocWrSolution};

use super::{
    Axis, Bound, CostTerm, Differentiable, ElementId, End, Operand, Parameter, Power, Selector,
    SensError, SolveSpec, VoltageKind, GB,
};

/// Tikhonov term for the conic KKT factorization, the conic analogue of the DC
/// engine's Tikhonov perturbation.
const CONIC_KKT_EPS: f64 = 1e-9;

/// The arrow matrix entry `arrow(v)[j, l]` for a second-order cone block, where
/// `v = (v0, rest…)`: `[[v0, restᵀ], [rest, v0 I]]`.
#[inline]
fn arrow(v0: f64, rest: &[f64], j: usize, l: usize) -> f64 {
    if j == 0 {
        if l == 0 {
            v0
        } else {
            rest[l - 1]
        }
    } else if l == 0 {
        rest[j - 1]
    } else if l == j {
        v0
    } else {
        0.0
    }
}

/// A solved SOCWR relaxation as a differentiable conic KKT system. Assembles the KKT
/// Jacobian `G` over `[x, z]` once at construction (validating the cone product), so
/// the operands and parameter columns read it without rebuilding. Borrows the network.
#[non_exhaustive]
pub struct ConicKkt<'a> {
    net: &'a AcNetwork,
    lay: SocWrLayout,
    g: Vec<(usize, usize, f64)>,
    /// The conic dual `z` at the solution, read off the bound and cone rows by the
    /// Tier-1 b-only parameter columns (`s∘z` complementarity carries `z` into them).
    z: Vec<f64>,
    /// The primal `x` at the solution: the cost column carries `2·pg`, and the
    /// admittance columns the `dA·x` zero-cone residual term.
    x: Vec<f64>,
    nvar: usize,
    dim: usize,
}

impl<'a> ConicKkt<'a> {
    /// Wrap a solved SOCWR relaxation: reassemble the program, recover the slack
    /// `s = b − Ax`, and build the KKT Jacobian `G`. Errors if the program carries a
    /// cone the differentiation does not handle.
    pub fn new(net: &'a AcNetwork, sol: &SocWrSolution) -> Result<Self, SensError> {
        let lay = SocWrLayout::new(net);
        let prog = build_conic_opf(&SocWr::new(), net);
        let nvar = lay.nvar;
        let ncon = lay.ncon;
        let dim = nvar + ncon;

        // The KKT indexes `sol.x` by program column and `sol.z` by constraint row, both
        // sized from the layout rebuilt from `net`. A solution paired with a differently
        // sized network would index out of range, or — if the sizes happen to coincide —
        // silently assemble the slack `s = b − Ax` against the wrong primal. Reject the
        // mismatch, matching `AcOpfKkt::new`.
        if sol.x.len() != nvar || sol.z.len() != ncon {
            return Err(SensError::Assembly(
                "SOCWR solution does not match the network layout".into(),
            ));
        }

        // A as row-major (col, value) lists, for Aᵀ (stationarity) and the per-row /
        // per-cone-block access the complementarity rows need.
        let mut a_rows: Vec<Vec<(usize, f64)>> = vec![Vec::new(); ncon];
        for c in 0..nvar {
            for idx in prog.a.colptr[c]..prog.a.colptr[c + 1] {
                a_rows[prog.a.rowval[idx]].push((c, prog.a.nzval[idx]));
            }
        }
        let x = &sol.x;
        let z = &sol.z;
        let s: Vec<f64> = (0..ncon)
            .map(|k| prog.b[k] - a_rows[k].iter().map(|&(c, v)| v * x[c]).sum::<f64>())
            .collect();

        // Assemble the KKT Jacobian G over the unknown vector [x(nvar), z(ncon)].
        let mut g: Vec<(usize, usize, f64)> = Vec::new();
        // Stationarity rows 0..nvar: ∂/∂x = P, ∂/∂z = Aᵀ.
        for c in 0..nvar {
            for idx in prog.p.colptr[c]..prog.p.colptr[c + 1] {
                let r = prog.p.rowval[idx];
                let v = prog.p.nzval[idx];
                g.push((r, c, v));
                if r != c {
                    g.push((c, r, v)); // symmetrize (Clarabel stores upper triangle)
                }
            }
        }
        for (k, entries) in a_rows.iter().enumerate() {
            for &(c, v) in entries {
                g.push((c, nvar + k, v));
            }
        }
        // Complementarity rows nvar+k, by cone. The loop variable k is a constraint-row
        // index used to address several aligned vectors (a_rows, z, s), not a plain
        // slice walk.
        let cones = lay.cones();
        let mut row = 0usize;
        for cone in &cones {
            match cone {
                ZeroConeT(d) => {
                    #[allow(clippy::needless_range_loop)]
                    for k in row..row + d {
                        for &(c, v) in &a_rows[k] {
                            g.push((nvar + k, c, v));
                        }
                    }
                    row += d;
                }
                NonnegativeConeT(d) => {
                    #[allow(clippy::needless_range_loop)]
                    for k in row..row + d {
                        for &(c, v) in &a_rows[k] {
                            g.push((nvar + k, c, -z[k] * v));
                        }
                        g.push((nvar + k, nvar + k, s[k]));
                    }
                    row += d;
                }
                SecondOrderConeT(d) => {
                    let dd = *d;
                    let z0 = z[row];
                    let s0 = s[row];
                    let z_rest: Vec<f64> = (1..dd).map(|l| z[row + l]).collect();
                    let s_rest: Vec<f64> = (1..dd).map(|l| s[row + l]).collect();
                    for j in 0..dd {
                        let grow = nvar + row + j;
                        // ∂(s∘z)/∂x = −arrow(z) A_block.
                        for l in 0..dd {
                            let az = arrow(z0, &z_rest, j, l);
                            if az != 0.0 {
                                for &(c, v) in &a_rows[row + l] {
                                    g.push((grow, c, -az * v));
                                }
                            }
                        }
                        // ∂(s∘z)/∂z = arrow(s).
                        for l in 0..dd {
                            let as_ = arrow(s0, &s_rest, j, l);
                            if as_ != 0.0 {
                                g.push((grow, nvar + row + l, as_));
                            }
                        }
                    }
                    row += dd;
                }
                other => {
                    return Err(SensError::Assembly(format!(
                        "unsupported cone in conic sensitivity: {other:?}"
                    )))
                }
            }
        }

        Ok(ConicKkt {
            net,
            lay,
            g,
            z: sol.z.clone(),
            x: sol.x.clone(),
            nvar,
            dim,
        })
    }
}

impl Differentiable for ConicKkt<'_> {
    fn formulation(&self) -> &'static str {
        "socwr"
    }

    fn dim(&self) -> usize {
        self.dim
    }

    fn jacobian(&self) -> Vec<(usize, usize, f64)> {
        self.g.clone()
    }

    fn parameter_len(&self, p: Parameter) -> Option<usize> {
        match p {
            // Per bus: demand (both powers), the voltage bounds, the bus shunt.
            Parameter::Demand(_) | Parameter::VoltageBound(_) | Parameter::ShuntAdmittance(_) => {
                Some(self.lay.n)
            }
            // Per branch: the apparent-power line limit and the series admittance.
            Parameter::LineLimit | Parameter::SeriesAdmittance(_) => Some(self.lay.m),
            // Per generator: the output bounds and the cost coefficients.
            Parameter::GenBound { .. } | Parameter::Cost(_) => Some(self.lay.k),
            // The transformer tap / phase shift is deferred to C6.
            Parameter::Transformer(_) | Parameter::Switching => None,
        }
    }

    /// `+dK/dp` for the requested indices. Every Tier-1 parameter enters only the
    /// right-hand side `b` of `Ax + s = b`, so it reaches the residual through
    /// `s = b − Ax`:
    ///
    /// - **Demand** sits on a zero-cone balance row (`K = Ax − b`), so `+dK/dp = −1`
    ///   on that row — `r_pbal` for active, `r_qbal` for reactive.
    /// - **LineLimit** (`rate_a`) sits at the head of both apparent-power SOC blocks;
    ///   the complementarity residual is `s∘z`, so `+dK/d(rate_a) = arrow(z)·e0`, the
    ///   cone's `z` block, at the cone rows.
    /// - **VoltageBound** and **GenBound** sit on nonnegative-cone rows
    ///   (`F_k = s_k z_k`), so `+dK/dp = z_k · db_k/dp`; `db/dp = +1` for an upper
    ///   bound (`b = ub`), `−1` for a lower bound (`b = −lb`).
    ///
    /// The Tier-2/3 parameters reach the smooth KKT rows instead of `b`:
    ///
    /// - **Cost** sits in the objective: `Cost(Quadratic) = 2·pg`, `Cost(Linear) = 1`
    ///   on the pg stationarity row (`P x + q + Aᵀz`).
    /// - **ShuntAdmittance** and **SeriesAdmittance** sit in `A` (the balance and Ohm
    ///   zero-cone rows), so each touches the residual through `dA·x` (at the equality
    ///   row) and the stationarity rows through `dAᵀ·z` (at the affected columns). No
    ///   SOC term — their `A`-entries are on equality rows.
    ///
    /// The driver supplies the leading minus of `dz/dp = −K⁻¹ dK/dp`.
    fn parameter_jacobian(&self, p: Parameter, idx: &[usize]) -> Result<Mat<f64>, SensError> {
        let mut rhs = Mat::<f64>::zeros(self.dim, idx.len());
        let (nvar, lay, z, x) = (self.nvar, &self.lay, &self.z, &self.x);
        match p {
            Parameter::Demand(power) => {
                for (c, &bus) in idx.iter().enumerate() {
                    let row = match power {
                        Power::Active => lay.r_pbal(bus),
                        Power::Reactive => lay.r_qbal(bus),
                    };
                    rhs[(nvar + row, c)] = -1.0;
                }
            }
            Parameter::LineLimit => {
                for (c, &e) in idx.iter().enumerate() {
                    // Both apparent-power cones (from, to) are SOC(3) with rate_a at
                    // the head; the column is the cone's z block on its rows.
                    for base in [lay.r_sf(e), lay.r_st(e)] {
                        for j in 0..3 {
                            rhs[(nvar + base + j, c)] = z[base + j];
                        }
                    }
                }
            }
            Parameter::VoltageBound(bound) => {
                for (c, &bus) in idx.iter().enumerate() {
                    let (k, dbdp) = match bound {
                        Bound::Max => (lay.r_wub(bus), 1.0),
                        Bound::Min => (lay.r_wlb(bus), -1.0),
                    };
                    rhs[(nvar + k, c)] = z[k] * dbdp;
                }
            }
            Parameter::GenBound { power, bound } => {
                for (c, &g) in idx.iter().enumerate() {
                    let (k, dbdp) = match (power, bound) {
                        (Power::Active, Bound::Max) => (lay.r_pgub(g), 1.0),
                        (Power::Active, Bound::Min) => (lay.r_pglb(g), -1.0),
                        (Power::Reactive, Bound::Max) => (lay.r_qgub(g), 1.0),
                        (Power::Reactive, Bound::Min) => (lay.r_qglb(g), -1.0),
                    };
                    rhs[(nvar + k, c)] = z[k] * dbdp;
                }
            }
            Parameter::Cost(term) => {
                // Objective: P[pg,pg] = 2 cq, q[pg] = cl; the pg stationarity row
                // carries d/dcq = 2 pg and d/dcl = 1.
                for (c, &g) in idx.iter().enumerate() {
                    let col = lay.col_pg(g);
                    rhs[(col, c)] = match term {
                        CostTerm::Quadratic => 2.0 * x[col],
                        CostTerm::Linear => 1.0,
                    };
                }
            }
            Parameter::ShuntAdmittance(gb) => {
                // gs sits at A[r_pbal, w] = −gs, bs at A[r_qbal, w] = +bs (a single A
                // entry each). dA·x lands on the balance residual, dAᵀ·z on the w
                // stationarity row.
                for (c, &i) in idx.iter().enumerate() {
                    let (row, da) = match gb {
                        GB::Conductance => (lay.r_pbal(i), -1.0),
                        GB::Susceptance => (lay.r_qbal(i), 1.0),
                    };
                    let cw = lay.col_w(i);
                    rhs[(nvar + row, c)] += da * x[cw];
                    rhs[(cw, c)] += da * z[row];
                }
            }
            Parameter::SeriesAdmittance(gb) => {
                // The series g / b enter every Ohm coefficient; dA/dp has the ten
                // entries below over the four Ohm rows and the {w_f, w_t, wr, wi}
                // columns (hand-derived from the Ohm assembly). Each accumulates the
                // dA·x residual term and the dAᵀ·z stationarity term.
                for (c, &e) in idx.iter().enumerate() {
                    let (f, t) = (self.net.br_from[e], self.net.br_to[e]);
                    let tap = self.net.tap[e];
                    let tr = tap * self.net.shift[e].cos();
                    let ti = tap * self.net.shift[e].sin();
                    let tm2 = tap * tap;
                    let (cf, ct) = (lay.col_w(f), lay.col_w(t));
                    let (cwr, cwi) = (lay.col_wr(e), lay.col_wi(e));
                    let (r1, r2) = (lay.r_ohm_pf(e), lay.r_ohm_qf(e));
                    let (r3, r4) = (lay.r_ohm_pt(e), lay.r_ohm_qt(e));
                    let entries: [(usize, usize, f64); 10] = match gb {
                        GB::Conductance => [
                            (r1, cf, -1.0 / tm2),
                            (r1, cwr, tr / tm2),
                            (r1, cwi, ti / tm2),
                            (r2, cwr, -ti / tm2),
                            (r2, cwi, tr / tm2),
                            (r3, ct, -1.0),
                            (r3, cwr, tr / tm2),
                            (r3, cwi, ti / tm2),
                            (r4, cwr, ti / tm2),
                            (r4, cwi, -tr / tm2),
                        ],
                        GB::Susceptance => [
                            (r1, cwr, -ti / tm2),
                            (r1, cwi, tr / tm2),
                            (r2, cf, 1.0 / tm2),
                            (r2, cwr, -tr / tm2),
                            (r2, cwi, -ti / tm2),
                            (r3, cwr, ti / tm2),
                            (r3, cwi, -tr / tm2),
                            (r4, ct, 1.0),
                            (r4, cwr, -tr / tm2),
                            (r4, cwi, -ti / tm2),
                        ],
                    };
                    for (r, col, da) in entries {
                        rhs[(nvar + r, c)] += da * x[col];
                        rhs[(col, c)] += da * z[r];
                    }
                }
            }
            Parameter::Transformer(_) | Parameter::Switching => {
                return Err(SensError::InvalidInput(format!(
                    "socwr does not support parameter {p:?}"
                )))
            }
        }
        Ok(rhs)
    }

    fn operand_len(&self, o: Operand) -> Option<usize> {
        match o {
            // Per generator.
            Operand::Dispatch(_) => Some(self.lay.k),
            // Per bus.
            Operand::Price(_) | Operand::Voltage(VoltageKind::Squared) => Some(self.lay.n),
            // Per branch.
            Operand::Voltage(VoltageKind::ProductReal | VoltageKind::ProductImag)
            | Operand::Flow { .. } => Some(self.lay.m),
            // Polar voltage components are AC-only.
            Operand::Voltage(_) => None,
        }
    }

    /// The `[x, z]` rows the operand reads, with the reporting sign that maps the raw
    /// variable to the reported quantity: the variables (dispatch, W-space voltage,
    /// branch flows) are read straight (`+1`), while a nodal price is `−z` on its
    /// balance row (`−1`). The driver composes this with the global `−1`.
    fn operand_selector(&self, o: Operand) -> Result<Selector, SensError> {
        let lay = &self.lay;
        let nvar = self.nvar;
        let by = |rows: Vec<usize>, sign: f64| {
            let n = rows.len();
            Selector::new(rows, (0..n).collect(), sign)
        };
        let sel = match o {
            Operand::Dispatch(Power::Active) => {
                by((0..lay.k).map(|g| lay.col_pg(g)).collect(), 1.0)
            }
            Operand::Dispatch(Power::Reactive) => {
                by((0..lay.k).map(|g| lay.col_qg(g)).collect(), 1.0)
            }
            Operand::Price(Power::Active) => {
                by((0..lay.n).map(|i| nvar + lay.r_pbal(i)).collect(), -1.0)
            }
            Operand::Price(Power::Reactive) => {
                by((0..lay.n).map(|i| nvar + lay.r_qbal(i)).collect(), -1.0)
            }
            Operand::Voltage(VoltageKind::Squared) => {
                by((0..lay.n).map(|i| lay.col_w(i)).collect(), 1.0)
            }
            Operand::Voltage(VoltageKind::ProductReal) => {
                by((0..lay.m).map(|e| lay.col_wr(e)).collect(), 1.0)
            }
            Operand::Voltage(VoltageKind::ProductImag) => {
                by((0..lay.m).map(|e| lay.col_wi(e)).collect(), 1.0)
            }
            Operand::Flow { power, end } => {
                let col = match (power, end) {
                    (Power::Active, End::From) => SocWrLayout::col_pf,
                    (Power::Active, End::To) => SocWrLayout::col_pt,
                    (Power::Reactive, End::From) => SocWrLayout::col_qf,
                    (Power::Reactive, End::To) => SocWrLayout::col_qt,
                };
                by((0..lay.m).map(|e| col(lay, e)).collect(), 1.0)
            }
            _ => {
                return Err(SensError::InvalidInput(format!(
                    "socwr does not support operand {o:?}"
                )))
            }
        };
        Ok(sel)
    }

    fn solve_spec(&self) -> SolveSpec {
        SolveSpec::new(CONIC_KKT_EPS, 12, 1e-13)
    }

    fn element_id(&self, axis: Axis, index: usize) -> ElementId {
        match axis {
            Axis::Bus => ElementId::Bus(self.net.bus_ids[index]),
            Axis::Branch => ElementId::Branch(self.net.branch_ids[index]),
            Axis::Generator => ElementId::Generator(self.net.gen_ids[index]),
        }
    }

    fn unit_scale(&self, o: Operand, p: Parameter) -> f64 {
        super::served_unit_scale(o, p, self.net.base_mva)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{parse_case3_ac, parse_case9_ac};
    use crate::problem::{socwr_opf, SocWrLayout};
    use crate::sens::{sensitivity, CostTerm, End, Mode, TapKind, GB};

    const DEMAND: Parameter = Parameter::Demand(Power::Active);
    const DISPATCH: Operand = Operand::Dispatch(Power::Active);
    const PRICE: Operand = Operand::Price(Power::Active);
    const VOLTAGE: Operand = Operand::Voltage(VoltageKind::Squared);

    fn l2(v: &[f64]) -> f64 {
        v.iter().map(|x| x * x).sum::<f64>().sqrt()
    }

    fn operand_vec(net: &AcNetwork, sol: &SocWrSolution, op: Operand) -> Vec<f64> {
        match op {
            Operand::Dispatch(Power::Active) => sol.pg.clone(),
            Operand::Dispatch(Power::Reactive) => sol.qg.clone(),
            Operand::Voltage(VoltageKind::Squared) => sol.w.clone(),
            Operand::Voltage(VoltageKind::ProductReal) => sol.wr.clone(),
            Operand::Voltage(VoltageKind::ProductImag) => sol.wi.clone(),
            Operand::Price(Power::Active) => sol.lmp.clone(),
            // The conic solution exposes the active nodal price directly; the reactive
            // price is the reactive-balance dual `−z[r_qbal]`, recovered from the raw
            // dual through the layout.
            Operand::Price(Power::Reactive) => {
                let lay = SocWrLayout::new(net);
                (0..net.n).map(|i| -sol.z[lay.r_qbal(i)]).collect()
            }
            Operand::Flow {
                power: Power::Active,
                end: End::From,
            } => sol.pf.clone(),
            Operand::Flow {
                power: Power::Active,
                end: End::To,
            } => sol.pt.clone(),
            Operand::Flow {
                power: Power::Reactive,
                end: End::From,
            } => sol.qf.clone(),
            Operand::Flow {
                power: Power::Reactive,
                end: End::To,
            } => sol.qt.clone(),
            other => unreachable!("unsupported conic test operand {other:?}"),
        }
    }

    /// A copy of `net` with the `idx`-th value of `par` shifted by `d`. Voltage
    /// bounds are stored as magnitudes but parameterized in `w = |V|²`, so the shift
    /// is applied in the squared quantity.
    fn perturb(net: &AcNetwork, par: Parameter, idx: usize, d: f64) -> AcNetwork {
        let mut n = net.clone();
        match par {
            Parameter::Demand(Power::Active) => n.pd[idx] += d,
            Parameter::Demand(Power::Reactive) => n.qd[idx] += d,
            Parameter::LineLimit => n.rate_a[idx] += d,
            Parameter::VoltageBound(Bound::Max) => {
                n.vm_max[idx] = (n.vm_max[idx] * n.vm_max[idx] + d).max(0.0).sqrt()
            }
            Parameter::VoltageBound(Bound::Min) => {
                n.vm_min[idx] = (n.vm_min[idx] * n.vm_min[idx] + d).max(0.0).sqrt()
            }
            Parameter::GenBound {
                power: Power::Active,
                bound: Bound::Max,
            } => n.pmax[idx] += d,
            Parameter::GenBound {
                power: Power::Active,
                bound: Bound::Min,
            } => n.pmin[idx] += d,
            Parameter::GenBound {
                power: Power::Reactive,
                bound: Bound::Max,
            } => n.qmax[idx] += d,
            Parameter::GenBound {
                power: Power::Reactive,
                bound: Bound::Min,
            } => n.qmin[idx] += d,
            Parameter::Cost(CostTerm::Quadratic) => n.cq[idx] += d,
            Parameter::Cost(CostTerm::Linear) => n.cl[idx] += d,
            Parameter::SeriesAdmittance(GB::Conductance) => n.g[idx] += d,
            Parameter::SeriesAdmittance(GB::Susceptance) => n.b[idx] += d,
            Parameter::ShuntAdmittance(GB::Conductance) => n.gs[idx] += d,
            Parameter::ShuntAdmittance(GB::Susceptance) => n.bs[idx] += d,
            other => unreachable!("unsupported conic test parameter {other:?}"),
        }
        n
    }

    /// Central finite difference of `operand` w.r.t. the `idx`-th value of
    /// `parameter`, re-solving the conic relaxation at each perturbed point.
    fn fd_column(
        net: &AcNetwork,
        op: Operand,
        parameter: Parameter,
        idx: usize,
        eps: f64,
    ) -> Vec<f64> {
        let sp = socwr_opf(&perturb(net, parameter, idx, eps)).expect("solve +eps");
        let sm = socwr_opf(&perturb(net, parameter, idx, -eps)).expect("solve -eps");
        let (op_p, op_m) = (operand_vec(net, &sp, op), operand_vec(net, &sm, op));
        (0..op_p.len())
            .map(|i| (op_p[i] - op_m[i]) / (2.0 * eps))
            .collect()
    }

    // --- C2 Tier-1 parameters ---------------------------------------------------

    /// Worst-case relative finite-difference error for one conic cell, the conic
    /// analogue of the DC `check_parity`. Columns below 1% of the largest are skipped
    /// (insignificant against the dominant response); a cell whose whole response sits
    /// below `ZERO_FLOOR` is treated as slack and only has to show the finite
    /// difference likewise finds nothing large.
    fn check_conic_parity(
        net: &AcNetwork,
        op: Operand,
        par: Parameter,
        eps: f64,
        tol: f64,
        floor_frac: f64,
    ) {
        const ZERO_FLOOR: f64 = 5e-3;
        let sol = socwr_opf(net).expect("socwr");
        let sys = ConicKkt::new(net, &sol).expect("kkt");
        let plen = sys.parameter_len(par).expect("supported parameter");
        let m = sensitivity(&sys, op, par, None, Mode::Forward).expect("sens");
        let col = |p: usize| {
            (0..m.values.len())
                .map(|i| m.values[i][p])
                .collect::<Vec<_>>()
        };
        let norms: Vec<f64> = (0..plen).map(|p| l2(&col(p))).collect();
        let man = norms.iter().cloned().fold(0.0, f64::max);
        if man < ZERO_FLOOR {
            // The parameter is slack (or carries zero dual) for this operand: the
            // analytic column is ~0; confirm the finite difference finds nothing large.
            for p in 0..plen {
                let fnorm = l2(&fd_column(net, op, par, p, eps));
                assert!(
                    fnorm < 5e-2,
                    "{op:?}/{par:?} looks slack (max||an||={man:.2e}) but FD finds {fnorm} at col {p}"
                );
            }
            return;
        }
        let floor = floor_frac * man;
        let mut tested = false;
        // `p` is the parameter (column) index, fed to the finite difference as well as
        // indexing the precomputed norms; not a slice walk to enumerate.
        #[allow(clippy::needless_range_loop)]
        for p in 0..plen {
            if norms[p] < floor {
                continue;
            }
            let fd = fd_column(net, op, par, p, eps);
            let an = col(p);
            let diff: Vec<f64> = (0..fd.len()).map(|i| an[i] - fd[i]).collect();
            let rel = l2(&diff) / norms[p];
            assert!(
                rel < tol,
                "{op:?}/{par:?}[{p}]: rel {rel} (||an||={} ||diff||={})",
                norms[p],
                l2(&diff)
            );
            tested = true;
        }
        assert!(
            tested,
            "{op:?}/{par:?}: no significant column to finite-difference"
        );
    }

    /// case9 with every thermal limit halved, so an apparent-power SOC binds and the
    /// `LineLimit` column is exercised.
    fn case9_line_limited() -> AcNetwork {
        let mut n = parse_case9_ac();
        for e in 0..n.m {
            n.rate_a[e] *= 0.5;
        }
        n
    }

    /// case9 with the cheapest generator capped low, so it pins at `pmax` and the
    /// active upper gen-bound binds.
    fn case9_pmax_pinned() -> AcNetwork {
        let mut n = parse_case9_ac();
        n.pmax[1] = 0.5;
        n
    }

    /// case9 with an expensive generator floored high, so it pins at `pmin` and the
    /// active lower gen-bound binds.
    fn case9_pmin_pinned() -> AcNetwork {
        let mut n = parse_case9_ac();
        n.pmin[2] = 1.5;
        n
    }

    /// case9 with the reactive minima raised, so a reactive lower gen-bound binds.
    fn case9_qmin_pinned() -> AcNetwork {
        let mut n = parse_case9_ac();
        for g in 0..n.k {
            n.qmin[g] = 0.2;
        }
        n
    }

    /// Every conic parameter: the Tier-1 b-only set plus the Tier-2 cost and the
    /// Tier-3 series / shunt admittance.
    const ALL_PARAMS: [Parameter; 15] = [
        Parameter::Demand(Power::Active),
        Parameter::Demand(Power::Reactive),
        Parameter::LineLimit,
        Parameter::VoltageBound(Bound::Max),
        Parameter::VoltageBound(Bound::Min),
        Parameter::GenBound {
            power: Power::Active,
            bound: Bound::Max,
        },
        Parameter::GenBound {
            power: Power::Active,
            bound: Bound::Min,
        },
        Parameter::GenBound {
            power: Power::Reactive,
            bound: Bound::Max,
        },
        Parameter::GenBound {
            power: Power::Reactive,
            bound: Bound::Min,
        },
        Parameter::Cost(CostTerm::Quadratic),
        Parameter::Cost(CostTerm::Linear),
        Parameter::SeriesAdmittance(GB::Conductance),
        Parameter::SeriesAdmittance(GB::Susceptance),
        Parameter::ShuntAdmittance(GB::Conductance),
        Parameter::ShuntAdmittance(GB::Susceptance),
    ];

    /// Every conic operand: the C1 set plus the C3 expansion (reactive prices and
    /// dispatch, both flow ends for each power, the W-space voltage products).
    const ALL_OPERANDS: [Operand; 11] = [
        Operand::Dispatch(Power::Active),
        Operand::Dispatch(Power::Reactive),
        Operand::Price(Power::Active),
        Operand::Price(Power::Reactive),
        Operand::Voltage(VoltageKind::Squared),
        Operand::Voltage(VoltageKind::ProductReal),
        Operand::Voltage(VoltageKind::ProductImag),
        Operand::Flow {
            power: Power::Active,
            end: End::From,
        },
        Operand::Flow {
            power: Power::Active,
            end: End::To,
        },
        Operand::Flow {
            power: Power::Reactive,
            end: End::From,
        },
        Operand::Flow {
            power: Power::Reactive,
            end: End::To,
        },
    ];

    #[test]
    fn conic_adjoint_equals_forward_all_cells() {
        // The full (operand × parameter) grid, on the base case and on fixtures that
        // bind the line / gen constraints, is finite and direction-consistent. The two
        // directions are algebraically identical; the only discrepancy is the
        // regularized solve's floating point. It stays ~1e-11 for the stiff cells but
        // grows to ~2e-5 when the cell touches the Jabr cone's degenerate directions —
        // a soft operand (squared voltage or a reactive injection) or a conductance
        // parameter (the loss direction). This is a solve-consistency bound; the FD
        // parity tests pin the columns.
        for net in [parse_case9_ac(), case9_line_limited(), case9_pmin_pinned()] {
            let sol = socwr_opf(&net).expect("socwr");
            let sys = ConicKkt::new(&net, &sol).expect("kkt");
            for op in ALL_OPERANDS {
                let soft_op = matches!(
                    op,
                    Operand::Voltage(VoltageKind::Squared)
                        | Operand::Dispatch(Power::Reactive)
                        | Operand::Flow {
                            power: Power::Reactive,
                            ..
                        }
                );
                for par in ALL_PARAMS {
                    let soft_par = matches!(
                        par,
                        Parameter::SeriesAdmittance(GB::Conductance)
                            | Parameter::ShuntAdmittance(GB::Conductance)
                    );
                    let tol = if soft_op || soft_par { 1e-4 } else { 1e-10 };
                    let fwd = sensitivity(&sys, op, par, None, Mode::Forward).expect("fwd");
                    let adj = sensitivity(&sys, op, par, None, Mode::Adjoint).expect("adj");
                    for (rf, ra) in fwd.values.iter().zip(adj.values.iter()) {
                        for (a, b) in rf.iter().zip(ra.iter()) {
                            assert!(a.is_finite(), "non-finite {op:?}/{par:?}: {a}");
                            assert!(
                                (a - b).abs() < tol,
                                "{op:?}/{par:?}: forward {a} adjoint {b} (diff {})",
                                (a - b).abs()
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn conic_tier1_active_routed_parity() {
        // The line limit and the active gen bounds move active dispatch and price
        // through the objective curvature, away from the Jabr cone, so the finite
        // difference is clean to 1e-3. Each fixture binds the constraint under test.
        let ll = case9_line_limited();
        let pmax = case9_pmax_pinned();
        let pmin = case9_pmin_pinned();
        for op in [DISPATCH, PRICE] {
            check_conic_parity(&ll, op, Parameter::LineLimit, 1e-4, 1e-3, 1e-2);
            check_conic_parity(
                &pmax,
                op,
                Parameter::GenBound {
                    power: Power::Active,
                    bound: Bound::Max,
                },
                1e-4,
                1e-3,
                1e-2,
            );
            check_conic_parity(
                &pmin,
                op,
                Parameter::GenBound {
                    power: Power::Active,
                    bound: Bound::Min,
                },
                1e-4,
                1e-3,
                1e-2,
            );
        }
    }

    #[test]
    fn conic_tier1_jabr_coupled_parity() {
        // Reactive demand, the voltage bound, and the reactive gen bound move the
        // solution through the Jabr (voltage-product) cone, whose relaxed
        // complementarity is near-degenerate; a central finite difference straddles
        // that mild kink, so it confirms the column to a few e-3 rather than 1e-3 (the
        // architecture's AdjointOnly class). adjoint == forward (1e-10) pins exactness;
        // this bounds gross error and fixes the row/sign.
        let net = parse_case9_ac();
        check_conic_parity(
            &net,
            PRICE,
            Parameter::Demand(Power::Reactive),
            1e-4,
            2e-2,
            1e-2,
        );
        check_conic_parity(
            &net,
            PRICE,
            Parameter::VoltageBound(Bound::Max),
            1e-4,
            2e-2,
            1e-2,
        );
        check_conic_parity(
            &case9_qmin_pinned(),
            PRICE,
            Parameter::GenBound {
                power: Power::Reactive,
                bound: Bound::Min,
            },
            1e-4,
            2e-2,
            1e-2,
        );
    }

    #[test]
    fn conic_tier1_slack_columns_vanish() {
        // On the loose base case these constraints are slack (or carry zero dual, like
        // the costless reactive bounds), so their columns are identically zero and
        // finite — the norm-floor case of the parity contract.
        let net = parse_case9_ac();
        let sol = socwr_opf(&net).expect("socwr");
        let sys = ConicKkt::new(&net, &sol).expect("kkt");
        let slack = [
            Parameter::LineLimit,
            Parameter::VoltageBound(Bound::Min),
            Parameter::GenBound {
                power: Power::Active,
                bound: Bound::Max,
            },
            Parameter::GenBound {
                power: Power::Active,
                bound: Bound::Min,
            },
            Parameter::GenBound {
                power: Power::Reactive,
                bound: Bound::Max,
            },
            Parameter::GenBound {
                power: Power::Reactive,
                bound: Bound::Min,
            },
        ];
        for par in slack {
            for op in [DISPATCH, PRICE] {
                let m = sensitivity(&sys, op, par, None, Mode::Forward).expect("sens");
                let man = (0..m.cols.len())
                    .map(|p| {
                        l2(&(0..m.values.len())
                            .map(|i| m.values[i][p])
                            .collect::<Vec<_>>())
                    })
                    .fold(0.0, f64::max);
                assert!(
                    man < 1e-2,
                    "expected slack {op:?}/{par:?} on base case9, got max||an||={man:.2e}"
                );
                for row in &m.values {
                    for &v in row {
                        assert!(v.is_finite(), "non-finite {op:?}/{par:?}: {v}");
                    }
                }
            }
        }
    }

    // --- C3 operand expansion ---------------------------------------------------

    #[test]
    fn conic_operand_expansion_parity() {
        // The reactive nodal price, the W-space voltage products, and the active branch
        // flows respond to active demand away from the cone kink, so the finite
        // difference is clean to 1e-3 on the dominant columns.
        let net = parse_case9_ac();
        let active = Parameter::Demand(Power::Active);
        for op in [
            Operand::Price(Power::Reactive),
            Operand::Voltage(VoltageKind::ProductReal),
            Operand::Voltage(VoltageKind::ProductImag),
            Operand::Flow {
                power: Power::Active,
                end: End::From,
            },
            Operand::Flow {
                power: Power::Active,
                end: End::To,
            },
        ] {
            check_conic_parity(&net, op, active, 1e-4, 1e-3, 5e-2);
        }
    }

    #[test]
    fn conic_reactive_operands_parity() {
        // Reactive dispatch and the reactive branch flows reach demand only through the
        // Jabr cone, so the finite difference confirms the dominant columns to a few
        // percent (the AdjointOnly class) — enough to rule out a wrong selector row,
        // with adjoint == forward pinning exactness. The active-flow selectors, clean
        // to 1e-3 above, share these arms' code path (the reactive twins col_qf /
        // col_qt, and col_qg for the dispatch).
        let net = parse_case9_ac();
        let reactive = Parameter::Demand(Power::Reactive);
        for op in [
            Operand::Dispatch(Power::Reactive),
            Operand::Flow {
                power: Power::Reactive,
                end: End::From,
            },
            Operand::Flow {
                power: Power::Reactive,
                end: End::To,
            },
        ] {
            check_conic_parity(&net, op, reactive, 1e-4, 1e-1, 3e-1);
        }
    }

    #[test]
    fn conic_operand_metadata_axes() {
        // The expansion operands name the right source element: reactive dispatch per
        // generator, the reactive price per bus, the W-space products and the flows per
        // branch — the per-branch axis split is what distinguishes wr / wi from the
        // squared magnitude.
        let net = parse_case9_ac();
        let sol = socwr_opf(&net).expect("socwr");
        let sys = ConicKkt::new(&net, &sol).expect("kkt");
        let demand = Parameter::Demand(Power::Active);
        let check = |op: Operand, want_len: usize, axis: &str| {
            let m = sensitivity(&sys, op, demand, None, Mode::Forward).expect("sens");
            assert_eq!(m.rows.len(), want_len, "{op:?} row count");
            let ok = m.rows.iter().enumerate().all(|(i, r)| {
                r.index == i
                    && match (axis, r.element) {
                        ("gen", ElementId::Generator(id)) => id == net.gen_ids[i],
                        ("bus", ElementId::Bus(id)) => id == net.bus_ids[i],
                        ("branch", ElementId::Branch(id)) => id == net.branch_ids[i],
                        _ => false,
                    }
            });
            assert!(ok, "{op:?} element ids/axis ({axis})");
        };
        check(Operand::Dispatch(Power::Reactive), net.k, "gen");
        check(Operand::Price(Power::Reactive), net.n, "bus");
        check(Operand::Voltage(VoltageKind::ProductReal), net.m, "branch");
        check(Operand::Voltage(VoltageKind::ProductImag), net.m, "branch");
        check(
            Operand::Flow {
                power: Power::Reactive,
                end: End::To,
            },
            net.m,
            "branch",
        );
    }

    // --- C4 Tier-2 (cost) and Tier-3 (admittance) parameters --------------------

    #[test]
    fn conic_tier23_parity() {
        // Validated against the nodal price (the operand with the largest, best-
        // conditioned response to each parameter).
        let net = parse_case9_ac();
        // Tier 2 (objective cost) and the susceptance directions are FdClean to 1e-3.
        check_conic_parity(
            &net,
            PRICE,
            Parameter::Cost(CostTerm::Quadratic),
            1e-3,
            1e-3,
            1e-1,
        );
        check_conic_parity(
            &net,
            PRICE,
            Parameter::Cost(CostTerm::Linear),
            1e-3,
            1e-3,
            1e-1,
        );
        check_conic_parity(
            &net,
            PRICE,
            Parameter::SeriesAdmittance(GB::Susceptance),
            1e-3,
            1e-3,
            1e-1,
        );
        // Series conductance perturbs the branch loss — the relaxation's tightness — so
        // only the dominant columns are clean to 1e-3; the sub-dominant ones straddle
        // the Jabr cone (so the floor keeps to the leading columns).
        check_conic_parity(
            &net,
            PRICE,
            Parameter::SeriesAdmittance(GB::Conductance),
            1e-3,
            1e-3,
            5e-1,
        );
        // The bus shunt couples mildly through the voltage, holding to a few e-3.
        check_conic_parity(
            &net,
            PRICE,
            Parameter::ShuntAdmittance(GB::Conductance),
            1e-3,
            1e-2,
            1e-1,
        );
        check_conic_parity(
            &net,
            PRICE,
            Parameter::ShuntAdmittance(GB::Susceptance),
            1e-3,
            1e-2,
            5e-1,
        );
    }

    /// Dispatch and price sensitivities to demand match central differences within
    /// 1e-3, on the exactly-tight case3 and the near-tight case9. (The W-space
    /// voltage variables sit on the second-order cone boundary, where the relaxed
    /// complementarity is near-degenerate and the finite-difference comparison is
    /// not a clean test; the dispatch and price blocks carry objective curvature
    /// and are well conditioned, so they are the parity targets, as in the DC
    /// engine where the price column is the reference.)
    #[test]
    fn conic_sensitivity_matches_central_differences() {
        for net in [parse_case3_ac(), parse_case9_ac()] {
            let sol = socwr_opf(&net).expect("socwr");
            let buses: Vec<usize> = (0..net.n).collect();
            let sys = ConicKkt::new(&net, &sol).expect("conic kkt");
            let eps = 1e-4;
            let mut exercised = false;
            for op in [DISPATCH, PRICE] {
                let m =
                    sensitivity(&sys, op, DEMAND, Some(&buses), Mode::Forward).expect("conic sens");
                for (c, &bus) in buses.iter().enumerate() {
                    let fd = fd_column(&net, op, DEMAND, bus, eps);
                    let an: Vec<f64> = (0..fd.len()).map(|i| m.values[i][c]).collect();
                    let diff: Vec<f64> = (0..fd.len()).map(|i| an[i] - fd[i]).collect();
                    let (anorm, dnorm, fnorm) = (l2(&an), l2(&diff), l2(&fd));
                    if anorm < 1e-3 {
                        assert!(
                            fnorm < 1e-3,
                            "{op:?} d/d(pd[{bus}]): analytic ~0 but FD finds {fnorm}"
                        );
                        continue;
                    }
                    exercised = true;
                    let rel = dnorm / anorm;
                    assert!(
                        rel < 1e-3,
                        "{op:?} d/d(pd[{bus}]): rel {rel} (||a||={anorm} ||diff||={dnorm})"
                    );
                }
            }
            assert!(exercised, "conic parity check found no non-trivial columns");
        }
    }

    #[test]
    fn conic_adjoint_equals_forward() {
        let net = parse_case9_ac();
        let sol = socwr_opf(&net).expect("socwr");
        let buses: Vec<usize> = (0..net.n).collect();
        let sys = ConicKkt::new(&net, &sol).expect("conic kkt");
        for op in [VOLTAGE, DISPATCH, PRICE] {
            let fwd = sensitivity(&sys, op, DEMAND, Some(&buses), Mode::Forward).expect("fwd");
            let adj = sensitivity(&sys, op, DEMAND, Some(&buses), Mode::Adjoint).expect("adj");
            for i in 0..fwd.values.len() {
                for c in 0..buses.len() {
                    assert!(
                        (fwd.values[i][c] - adj.values[i][c]).abs() < 1e-10,
                        "{op:?}[{i}][{c}]: forward {} adjoint {}",
                        fwd.values[i][c],
                        adj.values[i][c]
                    );
                }
            }
        }
    }

    #[test]
    fn unsupported_conic_requests_surface_errors() {
        let net = parse_case9_ac();
        let sol = socwr_opf(&net).expect("socwr");
        let sys = ConicKkt::new(&net, &sol).expect("conic kkt");
        // The transformer tap / phase shift is deferred to C6; branch switching is a
        // DC-only parameter.
        assert!(matches!(
            sensitivity(
                &sys,
                DISPATCH,
                Parameter::Transformer(TapKind::Ratio),
                None,
                Mode::Auto
            ),
            Err(SensError::Unsupported { .. })
        ));
        assert!(matches!(
            sensitivity(&sys, DISPATCH, Parameter::Switching, None, Mode::Auto),
            Err(SensError::Unsupported { .. })
        ));
        // The polar voltage components are AC-only; the conic relaxation has no |V| or
        // angle, only the W-space lift.
        assert!(matches!(
            sensitivity(
                &sys,
                Operand::Voltage(VoltageKind::Magnitude),
                DEMAND,
                None,
                Mode::Auto
            ),
            Err(SensError::Unsupported { .. })
        ));
    }
}
