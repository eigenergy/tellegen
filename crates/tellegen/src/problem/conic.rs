//! Conic AC OPF: the SOCWR (Jabr) relaxation on the [`SocWr`] formulation.
//!
//! In the W-space (`w_i = |V_i|^2`, `wr_ij = Re(V_i V_j*)`, `wi_ij = Im(V_i V_j*)`)
//! the AC power flow is linear; the only nonconvexity `wr^2 + wi^2 = w_i w_j` is
//! relaxed to the second-order cone `wr^2 + wi^2 <= w_i w_j`. The whole program is
//! assembled into Clarabel's standard form with ZeroCone (power balance, Ohm),
//! Nonnegative (voltage and generator bounds), and SecondOrderCone blocks (one Jabr
//! cone and two apparent-power limits per branch). Gated behind the additive `conic`
//! feature; the conic KKT sensitivity is in [`crate::sens`], which reads this
//! program and [`SocWrLayout`] directly, so the variable/constraint/cone layout here
//! is its contract.
//!
//! Variables begin with
//! `x = [pg(k), qg(k), w(n), wr(m), wi(m), pf(m), pt(m), qf(m), qt(m)]`.
//! A generator with a piecewise linear cost adds one epigraph column.

use clarabel::solver::SupportedConeT::{self, NonnegativeConeT, SecondOrderConeT, ZeroConeT};

use crate::formulation::{Formulation, SocWr};
use crate::model::AcNetwork;
use crate::solve::{run, RawSolution, SolveIteration};

use super::{OpfProgram, ProgramBuilder};

/// Variable and constraint row layout of the SOCWR program. Rows are equalities
/// (power balance, then the four Ohm blocks), inequalities (voltage, generator,
/// angle, and piecewise cost rows), then the per branch second order cones
/// (Jabr, then from/to apparent power). Construct with [`SocWrLayout::new`].
pub(crate) struct SocWrLayout {
    pub(crate) n: usize,
    pub(crate) m: usize,
    pub(crate) k: usize,
    pub(crate) n_eq: usize,
    pub(crate) n_ineq: usize,
    pub(crate) nvar: usize,
    pub(crate) ncon: usize,
    thermal_rows: Vec<Option<(usize, usize)>>,
    cost_columns: Vec<Option<usize>>,
    cost_row_starts: Vec<Option<usize>>,
}

impl SocWrLayout {
    pub(crate) fn new(net: &AcNetwork) -> Self {
        let (n, m, k) = (net.n, net.m, net.k);
        let n_eq = 2 * n + 4 * m;
        // 2n voltage + 4k generator + 2m angle-difference bounds, followed by
        // one epigraph inequality for each piecewise cost segment.
        let base_n_ineq = 2 * n + 4 * k + 2 * m;
        let base_nvar = 2 * k + n + 6 * m;
        let mut next_column = base_nvar;
        let mut next_cost_row = n_eq + base_n_ineq;
        let mut cost_columns = Vec::with_capacity(k);
        let mut cost_row_starts = Vec::with_capacity(k);
        for cost in &net.piecewise_costs {
            if let Some(cost) = cost {
                cost_columns.push(Some(next_column));
                cost_row_starts.push(Some(next_cost_row));
                next_column += 1;
                next_cost_row += cost.segment_count();
            } else {
                cost_columns.push(None);
                cost_row_starts.push(None);
            }
        }
        let nvar = next_column;
        let n_ineq = next_cost_row - n_eq;
        let soc_base = n_eq + n_ineq;
        let thermal_base = soc_base + 4 * m;
        let mut next_thermal = thermal_base;
        let thermal_rows = net
            .thermal_limit_active
            .iter()
            .map(|&active| {
                active.then(|| {
                    let rows = (next_thermal, next_thermal + 3);
                    next_thermal += 6;
                    rows
                })
            })
            .collect();
        let ncon = next_thermal;
        SocWrLayout {
            n,
            m,
            k,
            n_eq,
            n_ineq,
            nvar,
            ncon,
            thermal_rows,
            cost_columns,
            cost_row_starts,
        }
    }
    // Variable columns.
    pub(crate) fn col_pg(&self, g: usize) -> usize {
        g
    }
    pub(crate) fn col_qg(&self, g: usize) -> usize {
        self.k + g
    }
    pub(crate) fn col_w(&self, i: usize) -> usize {
        2 * self.k + i
    }
    pub(crate) fn col_wr(&self, e: usize) -> usize {
        2 * self.k + self.n + e
    }
    pub(crate) fn col_wi(&self, e: usize) -> usize {
        2 * self.k + self.n + self.m + e
    }
    pub(crate) fn col_pf(&self, e: usize) -> usize {
        2 * self.k + self.n + 2 * self.m + e
    }
    pub(crate) fn col_pt(&self, e: usize) -> usize {
        2 * self.k + self.n + 3 * self.m + e
    }
    pub(crate) fn col_qf(&self, e: usize) -> usize {
        2 * self.k + self.n + 4 * self.m + e
    }
    pub(crate) fn col_qt(&self, e: usize) -> usize {
        2 * self.k + self.n + 5 * self.m + e
    }
    pub(crate) fn col_cost(&self, generator: usize) -> Option<usize> {
        self.cost_columns[generator]
    }
    // Equality rows.
    pub(crate) fn r_pbal(&self, i: usize) -> usize {
        i
    }
    pub(crate) fn r_qbal(&self, i: usize) -> usize {
        self.n + i
    }
    // Ohm equality rows. Exposed to the sensitivity engine, which differentiates the
    // Ohm coefficients for the series-admittance parameter columns.
    pub(crate) fn r_ohm_pf(&self, e: usize) -> usize {
        2 * self.n + e
    }
    pub(crate) fn r_ohm_qf(&self, e: usize) -> usize {
        2 * self.n + self.m + e
    }
    pub(crate) fn r_ohm_pt(&self, e: usize) -> usize {
        2 * self.n + 2 * self.m + e
    }
    pub(crate) fn r_ohm_qt(&self, e: usize) -> usize {
        2 * self.n + 3 * self.m + e
    }
    // Inequality rows. Exposed to the sensitivity engine, which reads the bound
    // duals off these rows for the Tier-1 b-only parameter columns.
    pub(crate) fn r_wub(&self, i: usize) -> usize {
        self.n_eq + i
    }
    pub(crate) fn r_wlb(&self, i: usize) -> usize {
        self.n_eq + self.n + i
    }
    pub(crate) fn r_pgub(&self, g: usize) -> usize {
        self.n_eq + 2 * self.n + g
    }
    pub(crate) fn r_pglb(&self, g: usize) -> usize {
        self.n_eq + 2 * self.n + self.k + g
    }
    pub(crate) fn r_qgub(&self, g: usize) -> usize {
        self.n_eq + 2 * self.n + 2 * self.k + g
    }
    pub(crate) fn r_qglb(&self, g: usize) -> usize {
        self.n_eq + 2 * self.n + 3 * self.k + g
    }
    /// Angle-difference upper-bound row (`wi − tan(angmax)·wr ≤ 0`) for branch `e`.
    pub(crate) fn r_angub(&self, e: usize) -> usize {
        self.n_eq + 2 * self.n + 4 * self.k + e
    }
    /// Angle-difference lower-bound row (`tan(angmin)·wr − wi ≤ 0`) for branch `e`.
    pub(crate) fn r_anglb(&self, e: usize) -> usize {
        self.n_eq + 2 * self.n + 4 * self.k + self.m + e
    }
    pub(crate) fn r_cost_segment(&self, generator: usize, segment: usize) -> Option<usize> {
        self.cost_row_starts[generator].map(|start| start + segment)
    }
    // Second-order cone rows. soc_base is the first cone row.
    pub(crate) fn soc_base(&self) -> usize {
        self.n_eq + self.n_ineq
    }
    /// First row of the Jabr cone for branch `e` (a 4-dim cone).
    pub(crate) fn r_jabr(&self, e: usize) -> usize {
        self.soc_base() + 4 * e
    }
    /// First row of the from-side apparent-power cone for branch `e` (3-dim).
    pub(crate) fn r_sf(&self, e: usize) -> Option<usize> {
        self.thermal_rows[e].map(|rows| rows.0)
    }
    /// First row of the to-side apparent-power cone for branch `e` (3-dim).
    pub(crate) fn r_st(&self, e: usize) -> Option<usize> {
        self.thermal_rows[e].map(|rows| rows.1)
    }
    /// The cone partition in Clarabel row order.
    pub(crate) fn cones(&self) -> Vec<SupportedConeT<f64>> {
        let mut cones = vec![ZeroConeT(self.n_eq), NonnegativeConeT(self.n_ineq)];
        cones.extend((0..self.m).map(|_| SecondOrderConeT(4)));
        cones.extend(
            self.thermal_rows
                .iter()
                .filter(|rows| rows.is_some())
                .flat_map(|_| [SecondOrderConeT(3), SecondOrderConeT(3)]),
        );
        cones
    }
}

/// A formulation that assembles a conic AC OPF relaxation — the dispatch point the
/// generic [`build_conic_opf`] calls, the conic analogue of
/// [`OpfFormulation`](super::OpfFormulation). Not sealed.
pub trait ConicOpfFormulation: Formulation {
    /// Assemble the relaxed OPF program (Clarabel standard form) for `net`.
    fn assemble_conic_opf(&self, net: &AcNetwork) -> OpfProgram;
}

/// Build the conic OPF program for `net` under formulation `f`. Generic over the
/// formulation, like [`build_opf`](super::build_opf).
pub fn build_conic_opf<F: ConicOpfFormulation>(f: &F, net: &AcNetwork) -> OpfProgram {
    f.assemble_conic_opf(net)
}

impl ConicOpfFormulation for SocWr {
    fn assemble_conic_opf(&self, net: &AcNetwork) -> OpfProgram {
        let lay = SocWrLayout::new(net);
        let (n, m, k) = (net.n, net.m, net.k);
        let mut prog = ProgramBuilder::new(lay.nvar, lay.ncon);

        // Objective: sum_g cq_g pg^2 + cl_g pg (the constant cc is folded in at the
        // readout). P is diagonal 2 cq on pg, q is cl on pg.
        for g in 0..k {
            prog.quad(lay.col_pg(g), 2.0 * net.cq[g]);
            prog.lin(lay.col_pg(g), net.cl[g]);
            if let Some(column) = lay.col_cost(g) {
                prog.lin(column, 1.0);
            }
        }

        // Power balance: sum pg - sum(branch p leaving) - gs w = pd  (real)
        //                sum qg - sum(branch q leaving) + bs w = qd  (reactive)
        for i in 0..n {
            prog.a(lay.r_pbal(i), lay.col_w(i), -net.gs[i]);
            prog.rhs(lay.r_pbal(i), net.pd[i]);
            prog.a(lay.r_qbal(i), lay.col_w(i), net.bs[i]);
            prog.rhs(lay.r_qbal(i), net.qd[i]);
        }
        for g in 0..k {
            prog.a(lay.r_pbal(net.gen_bus[g]), lay.col_pg(g), 1.0);
            prog.a(lay.r_qbal(net.gen_bus[g]), lay.col_qg(g), 1.0);
        }
        for e in 0..m {
            let (f, t) = (net.br_from[e], net.br_to[e]);
            prog.a(lay.r_pbal(f), lay.col_pf(e), -1.0);
            prog.a(lay.r_pbal(t), lay.col_pt(e), -1.0);
            prog.a(lay.r_qbal(f), lay.col_qf(e), -1.0);
            prog.a(lay.r_qbal(t), lay.col_qt(e), -1.0);

            // Ohm's law in W-space (PowerModels constraint_ohms_yt). tr/ti are the
            // complex tap, tm2 the squared tap magnitude.
            let (g, bb) = (net.g[e], net.b[e]);
            let (gfr, bfr, gto, bto) = (net.g_fr[e], net.b_fr[e], net.g_to[e], net.b_to[e]);
            let tr = net.tap[e] * net.shift[e].cos();
            let ti = net.tap[e] * net.shift[e].sin();
            let tm2 = net.tap[e] * net.tap[e];

            // pf = (g+gfr)/tm2 w_f + (-g tr+b ti)/tm2 wr + (-b tr-g ti)/tm2 wi
            prog.a(lay.r_ohm_pf(e), lay.col_pf(e), 1.0);
            prog.a(lay.r_ohm_pf(e), lay.col_w(f), -(g + gfr) / tm2);
            prog.a(lay.r_ohm_pf(e), lay.col_wr(e), -(-g * tr + bb * ti) / tm2);
            prog.a(lay.r_ohm_pf(e), lay.col_wi(e), -(-bb * tr - g * ti) / tm2);
            // qf = -(b+bfr)/tm2 w_f - (-b tr-g ti)/tm2 wr + (-g tr+b ti)/tm2 wi
            prog.a(lay.r_ohm_qf(e), lay.col_qf(e), 1.0);
            prog.a(lay.r_ohm_qf(e), lay.col_w(f), (bb + bfr) / tm2);
            prog.a(lay.r_ohm_qf(e), lay.col_wr(e), (-bb * tr - g * ti) / tm2);
            prog.a(lay.r_ohm_qf(e), lay.col_wi(e), -(-g * tr + bb * ti) / tm2);
            // pt = (g+gto) w_t + (-g tr-b ti)/tm2 wr - (-b tr+g ti)/tm2 wi
            prog.a(lay.r_ohm_pt(e), lay.col_pt(e), 1.0);
            prog.a(lay.r_ohm_pt(e), lay.col_w(t), -(g + gto));
            prog.a(lay.r_ohm_pt(e), lay.col_wr(e), -(-g * tr - bb * ti) / tm2);
            prog.a(lay.r_ohm_pt(e), lay.col_wi(e), (-bb * tr + g * ti) / tm2);
            // qt = -(b+bto) w_t - (-b tr+g ti)/tm2 wr - (-g tr-b ti)/tm2 wi
            prog.a(lay.r_ohm_qt(e), lay.col_qt(e), 1.0);
            prog.a(lay.r_ohm_qt(e), lay.col_w(t), bb + bto);
            prog.a(lay.r_ohm_qt(e), lay.col_wr(e), (-bb * tr + g * ti) / tm2);
            prog.a(lay.r_ohm_qt(e), lay.col_wi(e), (-g * tr - bb * ti) / tm2);
        }

        // Convex piecewise linear generator costs, represented by one
        // epigraph variable and one linear inequality for every segment.
        for (generator, cost) in net.piecewise_costs.iter().enumerate() {
            let Some(cost) = cost else {
                continue;
            };
            let column = lay.col_cost(generator).expect("piecewise cost column");
            for segment in 0..cost.segment_count() {
                let row = lay
                    .r_cost_segment(generator, segment)
                    .expect("piecewise cost row");
                prog.a(row, lay.col_pg(generator), cost.slopes[segment]);
                prog.a(row, column, -1.0);
                prog.rhs(row, -cost.intercepts[segment]);
            }
        }

        // Voltage magnitude bounds vmin^2 <= w <= vmax^2.
        for i in 0..n {
            if net.voltage_bound_active[i] {
                prog.a(lay.r_wub(i), lay.col_w(i), 1.0);
                prog.rhs(lay.r_wub(i), net.vm_max[i] * net.vm_max[i]);
                prog.a(lay.r_wlb(i), lay.col_w(i), -1.0);
                prog.rhs(lay.r_wlb(i), -net.vm_min[i] * net.vm_min[i]);
            } else {
                prog.rhs(lay.r_wub(i), 1.0);
                prog.rhs(lay.r_wlb(i), 1.0);
            }
        }
        // Generator bounds.
        for g in 0..k {
            if net.generator_capability_active[g] {
                prog.a(lay.r_pgub(g), lay.col_pg(g), 1.0);
                prog.rhs(lay.r_pgub(g), net.pmax[g]);
                prog.a(lay.r_pglb(g), lay.col_pg(g), -1.0);
                prog.rhs(lay.r_pglb(g), -net.pmin[g]);
                prog.a(lay.r_qgub(g), lay.col_qg(g), 1.0);
                prog.rhs(lay.r_qgub(g), net.qmax[g]);
                prog.a(lay.r_qglb(g), lay.col_qg(g), -1.0);
                prog.rhs(lay.r_qglb(g), -net.qmin[g]);
            } else {
                prog.rhs(lay.r_pgub(g), 1.0);
                prog.rhs(lay.r_pglb(g), 1.0);
                prog.rhs(lay.r_qgub(g), 1.0);
                prog.rhs(lay.r_qglb(g), 1.0);
            }
        }

        // Branch angle-difference limits, linear in the W-space products. With
        // wr = |Vi||Vj|cos θij, wi = |Vi||Vj|sin θij (wr > 0 for |θ| < 90°), the bound
        // θij ∈ [angmin, angmax] becomes tan(angmin)·wr ≤ wi ≤ tan(angmax)·wr. The
        // normalized bounds sit within ±90°, so the tangents are finite. PowerModels
        // enforces these in its SOC; they bind on the small-angle (SAD) variant and are
        // slack on the typical case (where ±30° never binds).
        for e in 0..m {
            if net.conic_angle_bound_active(e) {
                // wi − tan(angmax)·wr ≤ 0
                prog.a(lay.r_angub(e), lay.col_wi(e), 1.0);
                prog.a(lay.r_angub(e), lay.col_wr(e), -net.angmax[e].tan());
                prog.rhs(lay.r_angub(e), 0.0);
                // tan(angmin)·wr − wi ≤ 0
                prog.a(lay.r_anglb(e), lay.col_wi(e), -1.0);
                prog.a(lay.r_anglb(e), lay.col_wr(e), net.angmin[e].tan());
                prog.rhs(lay.r_anglb(e), 0.0);
            } else {
                prog.rhs(lay.r_angub(e), 1.0);
                prog.rhs(lay.r_anglb(e), 1.0);
            }
        }

        // Jabr cone per branch: ((w_f+w_t)/2, wr, wi, (w_f-w_t)/2) in SOC(4), i.e.
        // (w_f+w_t)/2 >= ||(wr, wi, (w_f-w_t)/2)||  <=>  wr^2 + wi^2 <= w_f w_t.
        for e in 0..m {
            let (f, t) = (net.br_from[e], net.br_to[e]);
            let r = lay.r_jabr(e);
            prog.a(r, lay.col_w(f), -0.5);
            prog.a(r, lay.col_w(t), -0.5);
            prog.a(r + 1, lay.col_wr(e), -1.0);
            prog.a(r + 2, lay.col_wi(e), -1.0);
            prog.a(r + 3, lay.col_w(f), -0.5);
            prog.a(r + 3, lay.col_w(t), 0.5);
        }
        // Apparent-power limits: (rate_a, pf, qf) and (rate_a, pt, qt) in SOC(3).
        for e in 0..m {
            let Some(rf) = lay.r_sf(e) else {
                continue;
            };
            prog.rhs(rf, net.rate_a[e]);
            prog.a(rf + 1, lay.col_pf(e), -1.0);
            prog.a(rf + 2, lay.col_qf(e), -1.0);
            let rt = lay.r_st(e).expect("thermal rows are paired");
            prog.rhs(rt, net.rate_a[e]);
            prog.a(rt + 1, lay.col_pt(e), -1.0);
            prog.a(rt + 2, lay.col_qt(e), -1.0);
        }

        prog.finish(lay.cones())
    }
}

/// Primal-dual solution of the SOCWR relaxation, in per unit. The primals are the
/// W-space variables and the branch flows; `lmp` is the real-power-balance dual
/// (the nodal price). `objective` includes the constant cost term, so it matches a
/// reference AC OPF objective for comparison.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct SocWrSolution {
    /// Generator real and reactive dispatch (per unit), per generator.
    pub pg: Vec<f64>,
    pub qg: Vec<f64>,
    /// Squared bus voltage magnitude `w_i = |V_i|^2`.
    pub w: Vec<f64>,
    /// Oriented branch voltage products in squared per unit.
    pub wr: Vec<f64>,
    pub wi: Vec<f64>,
    /// Branch from/to active and reactive flows.
    pub pf: Vec<f64>,
    pub pt: Vec<f64>,
    pub qf: Vec<f64>,
    pub qt: Vec<f64>,
    /// Nodal price (real-power-balance dual), per bus.
    pub lmp: Vec<f64>,
    /// Reactive-power balance marginal value per unit of demand.
    pub lmp_q: Vec<f64>,
    /// Objective value (generation cost, constant term included).
    pub objective: f64,
    /// Interior-point iteration trace.
    pub iterations: Vec<SolveIteration>,
    /// Raw Clarabel primal and conic dual, kept for the conic KKT sensitivity.
    pub(crate) x: Vec<f64>,
    pub(crate) z: Vec<f64>,
}

fn read_socwr(net: &AcNetwork, lay: &SocWrLayout, raw: &RawSolution) -> SocWrSolution {
    let (n, m, k) = (net.n, net.m, net.k);
    let x = &raw.x;
    let objective = match net.objective {
        powerio_matrix::PreparedObjective::Feasibility => 0.0,
        powerio_matrix::PreparedObjective::NetworkGeneratorCost => (0..k)
            .map(|g| net.generator_cost(g, x[lay.col_pg(g)]))
            .sum(),
        _ => raw.objective,
    };
    SocWrSolution {
        pg: (0..k).map(|g| x[lay.col_pg(g)]).collect(),
        qg: (0..k).map(|g| x[lay.col_qg(g)]).collect(),
        w: (0..n).map(|i| x[lay.col_w(i)]).collect(),
        wr: (0..m).map(|e| x[lay.col_wr(e)]).collect(),
        wi: (0..m).map(|e| x[lay.col_wi(e)]).collect(),
        pf: (0..m).map(|e| x[lay.col_pf(e)]).collect(),
        pt: (0..m).map(|e| x[lay.col_pt(e)]).collect(),
        qf: (0..m).map(|e| x[lay.col_qf(e)]).collect(),
        qt: (0..m).map(|e| x[lay.col_qt(e)]).collect(),
        // Equality dual sign flip (nu = -z), as in the DC OPF readout, so the price
        // is the positive marginal cost of demand.
        lmp: (0..n).map(|i| -raw.z[lay.r_pbal(i)]).collect(),
        lmp_q: (0..n).map(|i| -raw.z[lay.r_qbal(i)]).collect(),
        objective,
        iterations: raw.iterations.clone(),
        x: raw.x.clone(),
        z: raw.z.clone(),
    }
}

/// Solve the SOCWR (Jabr) conic relaxation of AC OPF for `net`: assemble the
/// program over [`SocWr`], solve it with Clarabel, and read back the W-space
/// primals, branch flows, nodal prices, and objective. The relaxation is a convex
/// lower bound on AC OPF.
pub fn socwr_opf(net: &AcNetwork) -> Result<SocWrSolution, String> {
    let lay = SocWrLayout::new(net);
    let prog = build_conic_opf(&SocWr::new(), net);
    let raw = run(&prog, None)?;
    Ok(read_socwr(net, &lay, &raw))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::parse_case9_ac;

    #[test]
    fn convex_piecewise_cost_uses_the_declared_socwr_epigraph() {
        let text = crate::model::CASE3
            .replace(" 2 0 0 3 0.11  5   0;", " 1 0 0 3 0 0 50 5 250 405;")
            .replace(" 2 0 0 3 0.085 1.2 0;", " 1 0 0 2 0 0 270 135;");
        let network = crate::model::parse_matpower(&text).expect("parse piecewise case3");
        let model =
            crate::model::AcNetwork::from_network(&network).expect("prepare piecewise case3");
        let layout = SocWrLayout::new(&model);
        let solution = socwr_opf(&model).expect("solve piecewise case3");

        let declared_objective: f64 = (0..model.k)
            .map(|generator| model.generator_cost(generator, solution.pg[generator]))
            .sum();
        assert!(
            (solution.objective - declared_objective).abs() < 1e-6,
            "objective {}, declared {declared_objective}",
            solution.objective
        );
        for generator in 0..model.k {
            let epigraph = solution.x[layout.col_cost(generator).expect("cost column")];
            let declared = model.generator_cost(generator, solution.pg[generator]);
            assert!(
                (epigraph - declared).abs() < 1e-5,
                "generator {generator}: epigraph {epigraph}, declared {declared}"
            );
            let cost = model.piecewise_costs[generator]
                .as_ref()
                .expect("piecewise cost");
            let dual_sum: f64 = (0..cost.segment_count())
                .map(|segment| {
                    solution.z[layout
                        .r_cost_segment(generator, segment)
                        .expect("segment row")]
                })
                .sum();
            assert!(
                (dual_sum - 1.0).abs() < 1e-6,
                "generator {generator}: segment dual sum {dual_sum}"
            );
        }
    }

    #[test]
    fn inactive_ac_constraint_families_are_absent_from_the_conic_program() {
        let mut model = crate::model::parse_case3_ac();
        model.voltage_bound_active.fill(false);
        model.generator_capability_active.fill(false);
        model.thermal_limit_active.fill(false);
        model.angle_bound_active.fill(false);

        // These values would make the case infeasible if their families were
        // still active. The masks, not sentinel bounds, remove the constraints.
        model.vm_min.fill(2.0);
        model.vm_max.fill(0.5);
        model.pmin.fill(0.0);
        model.pmax.fill(0.0);
        model.qmin.fill(0.0);
        model.qmax.fill(0.0);
        model.rate_a.fill(1e-6);
        model.angmin.fill(0.0);
        model.angmax.fill(0.0);

        let layout = SocWrLayout::new(&model);
        assert!((0..model.m).all(|branch| layout.r_sf(branch).is_none()));
        assert!((0..model.m).all(|branch| layout.r_st(branch).is_none()));
        let solution = socwr_opf(&model).expect("unconstrained family solve");
        assert!(solution.objective.is_finite());
    }

    /// Branch angle-difference limits tighten the SOCWR relaxation (the small-angle /
    /// SAD case): clamping every branch below its natural angle spread raises the
    /// objective — a strictly higher lower bound on AC OPF — and every branch angle
    /// `atan2(wi, wr)` then respects the limit, with at least one binding.
    #[test]
    fn angle_difference_limits_bind_and_tighten() {
        let base = parse_case9_ac();
        let base_sol = socwr_opf(&base).expect("base socwr");
        let angle = |s: &SocWrSolution, e: usize| s.wi[e].atan2(s.wr[e]);
        let mut sorted: Vec<f64> = (0..base.m).map(|e| angle(&base_sol, e).abs()).collect();
        sorted.sort_by(|a, b| b.total_cmp(a));
        assert!(
            sorted[0] > 1e-3,
            "case9 has a nonzero angle spread to clamp"
        );

        // Clamp just below the peak (midway to the runner-up), so only the single
        // most-loaded branch must give and the meshed network reroutes — feasible, and
        // the peak binds at the limit.
        let lim = (sorted[0] + sorted[1]) / 2.0;
        let mut tight = base.clone();
        for e in 0..tight.m {
            tight.angmin[e] = -lim;
            tight.angmax[e] = lim;
        }
        let tight_sol = socwr_opf(&tight).expect("tight socwr");

        // The base optimum violated the new limit, so a convex re-solve cannot cost less.
        assert!(
            tight_sol.objective >= base_sol.objective - 1e-6,
            "tighter angle limits cannot lower the objective: {} vs {}",
            tight_sol.objective,
            base_sol.objective
        );
        // Every branch angle now respects the limit, and the peak binds it.
        let mut max_bound = 0.0_f64;
        for e in 0..tight.m {
            let theta = angle(&tight_sol, e);
            assert!(
                theta.abs() <= lim + 1e-6,
                "branch {e} angle {theta} exceeds ±{lim}"
            );
            max_bound = max_bound.max(theta.abs());
        }
        assert!(
            max_bound > lim - 1e-4,
            "expected a branch to bind the ±{lim} angle limit, max {max_bound}"
        );
    }

    /// Regression for the near-zero-impedance-jumper bug (see
    /// `model::ac::tests::near_zero_impedance_jumper_is_a_tie_not_an_open_circuit`, and
    /// `model::dc::tests::near_zero_impedance_jumper_is_a_tie_not_an_open_circuit` for the
    /// DC analogue). 11 of CaliforniaTestSystem.m's branches — bus-splitting jumpers with
    /// real but tiny impedance (z^2 as low as ~1.5e-12) — fell below the old
    /// `MIN_Z_SQUARED = 1e-10` guard and were dropped to series `g = b = 0`, an open
    /// circuit, wrongly disconnecting the two buses they tie. Reference value from a
    /// fresh PowerModels.jl `SOCWRPowerModel` + Ipopt solve on the same file ("Optimal
    /// Solution Found", scaled and unscaled NLP error ~2.6e-7): $785,165.38. Before the
    /// fix tellegen reported $785,627.36 here (5.9e-4 relative, close to the ~$785,627
    /// logged when the jumper-charging fix, PR #13, first made SOCWR feasible on CATS —
    /// that number was never wrong about feasibility, only about severing the jumpers).
    /// After the fix it reports $785,195.50 (3.8e-5 relative), a >15x tighter match,
    /// consistent with the residual being ordinary conic-solver tolerance rather than a
    /// remaining modeling gap.
    #[test]
    fn cats_socwr_objective_matches_reference_after_the_jumper_fix() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../data/CATS/CaliforniaTestSystem.m"
        );
        let Ok(text) = std::fs::read_to_string(path) else {
            eprintln!(
                "skipping cats_socwr_objective_matches_reference_after_the_jumper_fix: {path} not found"
            );
            return;
        };
        let net = crate::model::parse_matpower(&text).expect("parse CATS");
        let ac = crate::model::AcNetwork::from_network(&net).expect("build AcNetwork");
        let sol = socwr_opf(&ac).expect("solve CATS SOCWR");

        let reference = 785_165.38;
        let rel = (sol.objective - reference).abs() / reference;
        assert!(
            rel < 1e-4,
            "objective {} vs PowerModels.jl/Ipopt reference {reference} (rel err {rel})",
            sol.objective
        );
    }

    /// A literal zero-impedance jumper has undefined series admittance. Omitting
    /// the row would also silently omit its topology and charging, so AC/SOCWR
    /// model construction rejects the case instead.
    #[test]
    fn zero_impedance_jumper_is_rejected_before_socwr() {
        const CASE: &str = "\
function mpc = jumpertest
mpc.version = '2';
mpc.baseMVA = 100;
mpc.bus = [
 1 3 0  0  0 0 1 1 0 230 1 1.1 0.9;
 2 1 90 30 0 0 1 1 0 230 1 1.1 0.9;
 3 2 0  0  0 0 1 1 0 230 1 1.1 0.9;
 4 1 0  0  0 0 1 1 0 230 1 1.1 0.9;
];
mpc.gen = [
 1 0  0 300 -300 1 100 1 250 10 0 0 0 0 0 0 0 0 0 0 0;
 3 60 0 300 -300 1 100 1 270 10 0 0 0 0 0 0 0 0 0 0 0;
];
mpc.branch = [
 1 2 0.01 0.1 0    250 250 250 0 0 1 -360 360;
 1 3 0.01 0.1 0    250 250 250 0 0 1 -360 360;
 2 3 0.01 0.1 0    250 250 250 0 0 1 -360 360;
 3 4 0    0   0.05 250 250 250 0 0 1 -360 360;
];
mpc.gencost = [
 2 0 0 3 0.11  5   0;
 2 0 0 3 0.085 1.2 0;
];
";
        let net = crate::model::parse_matpower(CASE).expect("parse");
        let error = crate::model::AcNetwork::from_network(&net).unwrap_err();
        assert!(error.contains("zero matrix denominator"), "{error}");
    }
}
