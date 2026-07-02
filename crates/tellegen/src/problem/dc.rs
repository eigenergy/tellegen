//! DC OPF: the B-theta linear program assembled into Clarabel standard form, its
//! dual readout, and the DC entry points. The variable and constraint layout here
//! is the contract the [`crate::sens`] KKT mirrors block-for-block.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use clarabel::solver::SupportedConeT::{NonnegativeConeT, ZeroConeT};

use crate::formulation::Dc;
use crate::model::DcNetwork;
use crate::solve::{run, RawSolution, SolveIteration};

use super::{build_opf, OpfFormulation, OpfProgram, ProgramBuilder};

/// Primal and dual solution of the DC OPF, in per unit. `nu_bal` is the LMP
/// (per-unit $/per-unit-MW); divide by `base_mva` for $/MWh.
#[derive(Clone)]
#[cfg_attr(not(feature = "sensitivity"), allow(dead_code))]
pub struct DcOpfSolution {
    pub va: Vec<f64>,
    pub pg: Vec<f64>,
    pub f: Vec<f64>,
    pub psh: Vec<f64>,
    pub nu_bal: Vec<f64>,
    pub lam_ub: Vec<f64>,
    pub lam_lb: Vec<f64>,
    pub rho_ub: Vec<f64>,
    pub rho_lb: Vec<f64>,
    pub mu_ub: Vec<f64>,
    pub mu_lb: Vec<f64>,
    pub gamma_ub: Vec<f64>,
    pub gamma_lb: Vec<f64>,
    pub objective: f64,
    pub iterations: Vec<SolveIteration>,
}

impl DcOpfSolution {
    /// LMP per bus in $/MWh (`nu_bal / base_mva`), in dense bus-index order.
    pub fn lmp_usd_per_mwh(&self, base_mva: f64) -> Vec<f64> {
        self.nu_bal.iter().map(|v| v / base_mva).collect()
    }
}

/// Row and column offsets of the DC OPF program, derived from the network sizes.
/// Variables are `x = [va(n), pg(k), f(m), psh(n)]`; constraint rows are the
/// equalities first (zero cone: power balance, flow definition, reference), then the
/// inequalities (non-negative cone: line limits, gen limits, shed bounds, phase
/// limits), each block contiguous. The assembly scatters by these offsets and the
/// readout reads the duals back by the same offsets, so the two stay in lockstep.
struct OpfLayout {
    n: usize,
    m: usize,
    k: usize,
    n_eq: usize,
    n_ineq: usize,
}

impl OpfLayout {
    fn dc(n: usize, m: usize, k: usize) -> Self {
        let n_eq = n + m + 1;
        let n_ineq = 4 * m + 2 * k + 2 * n;
        OpfLayout {
            n,
            m,
            k,
            n_eq,
            n_ineq,
        }
    }
    fn nvar(&self) -> usize {
        2 * self.n + self.k + self.m
    }
    fn ncon(&self) -> usize {
        self.n_eq + self.n_ineq
    }
    fn col_va(&self, i: usize) -> usize {
        i
    }
    fn col_pg(&self, j: usize) -> usize {
        self.n + j
    }
    fn col_f(&self, e: usize) -> usize {
        self.n + self.k + e
    }
    fn col_psh(&self, i: usize) -> usize {
        self.n + self.k + self.m + i
    }
    fn r_pb(&self, i: usize) -> usize {
        i
    }
    fn r_fd(&self, e: usize) -> usize {
        self.n + e
    }
    fn r_ref(&self) -> usize {
        self.n + self.m
    }
    fn r_lineub(&self, e: usize) -> usize {
        self.n_eq + e
    }
    fn r_linelb(&self, e: usize) -> usize {
        self.n_eq + self.m + e
    }
    fn r_genub(&self, j: usize) -> usize {
        self.n_eq + 2 * self.m + j
    }
    fn r_genlb(&self, j: usize) -> usize {
        self.n_eq + 2 * self.m + self.k + j
    }
    fn r_shedub(&self, i: usize) -> usize {
        self.n_eq + 2 * self.m + 2 * self.k + i
    }
    fn r_shedlb(&self, i: usize) -> usize {
        self.n_eq + 2 * self.m + 2 * self.k + self.n + i
    }
    fn r_phaseub(&self, e: usize) -> usize {
        self.n_eq + 2 * self.m + 2 * self.k + 2 * self.n + e
    }
    fn r_phaselb(&self, e: usize) -> usize {
        self.n_eq + 2 * self.m + 2 * self.k + 2 * self.n + self.m + e
    }
}

impl OpfFormulation for Dc {
    fn assemble_opf(&self, dc: &DcNetwork) -> OpfProgram {
        let lay = OpfLayout::dc(dc.n, dc.m, dc.k);
        let mut prog = ProgramBuilder::new(lay.nvar(), lay.ncon());

        // Objective Hessian P (diagonal): 2 cq on pg, tau^2 on f.
        for j in 0..dc.k {
            prog.quad(lay.col_pg(j), 2.0 * dc.cq[j]);
        }
        let tau2 = dc.tau * dc.tau;
        for e in 0..dc.m {
            prog.quad(lay.col_f(e), tau2);
        }

        // Linear objective q: cl on pg, c_shed on psh.
        for j in 0..dc.k {
            prog.lin(lay.col_pg(j), dc.cl[j]);
        }
        for i in 0..dc.n {
            prog.lin(lay.col_psh(i), dc.c_shed[i]);
        }

        // Power balance: G_inc g + psh - B theta = d.
        for i in 0..dc.n {
            prog.a(lay.r_pb(i), lay.col_psh(i), 1.0);
            prog.rhs(lay.r_pb(i), dc.demand[i]);
        }
        for j in 0..dc.k {
            prog.a(lay.r_pb(dc.gen_bus[j]), lay.col_pg(j), 1.0);
        }
        // Per-branch contributions: -B (power balance), flow definition, line
        // limits, and phase-angle-difference limits.
        for e in 0..dc.m {
            let w = -dc.b[e] * dc.sw[e]; // edge weight (positive for inductive)
            let (fb, tb) = (dc.br_from[e], dc.br_to[e]);
            // -B = -A' diag(w) A
            prog.a(lay.r_pb(fb), lay.col_va(fb), -w);
            prog.a(lay.r_pb(tb), lay.col_va(tb), -w);
            prog.a(lay.r_pb(fb), lay.col_va(tb), w);
            prog.a(lay.r_pb(tb), lay.col_va(fb), w);
            // Flow definition: f - w (theta_from - theta_to) = 0
            prog.a(lay.r_fd(e), lay.col_f(e), 1.0);
            prog.a(lay.r_fd(e), lay.col_va(fb), -w);
            prog.a(lay.r_fd(e), lay.col_va(tb), w);
            // Line limits: f <= fmax and -f <= fmax
            prog.a(lay.r_lineub(e), lay.col_f(e), 1.0);
            prog.rhs(lay.r_lineub(e), dc.fmax[e]);
            prog.a(lay.r_linelb(e), lay.col_f(e), -1.0);
            prog.rhs(lay.r_linelb(e), dc.fmax[e]);
            // Phase-angle-difference limits: sw (A theta) within sw [angmin, angmax]
            let sw = dc.sw[e];
            prog.a(lay.r_phaseub(e), lay.col_va(fb), sw);
            prog.a(lay.r_phaseub(e), lay.col_va(tb), -sw);
            prog.rhs(lay.r_phaseub(e), sw * dc.angmax[e]);
            prog.a(lay.r_phaselb(e), lay.col_va(fb), -sw);
            prog.a(lay.r_phaselb(e), lay.col_va(tb), sw);
            prog.rhs(lay.r_phaselb(e), -sw * dc.angmin[e]);
        }
        // Reference bus: theta[ref] = 0
        prog.a(lay.r_ref(), lay.col_va(dc.ref_bus), 1.0);
        // Generation limits: g <= gmax and -g <= -gmin
        for j in 0..dc.k {
            prog.a(lay.r_genub(j), lay.col_pg(j), 1.0);
            prog.rhs(lay.r_genub(j), dc.gmax[j]);
            prog.a(lay.r_genlb(j), lay.col_pg(j), -1.0);
            prog.rhs(lay.r_genlb(j), -dc.gmin[j]);
        }
        // Shedding bounds: 0 <= psh <= max(d, 0) when shedding is allowed, else psh = 0
        // (pinned), so an unservable case reports infeasible instead of shedding.
        for i in 0..dc.n {
            prog.a(lay.r_shedub(i), lay.col_psh(i), 1.0);
            prog.rhs(lay.r_shedub(i), dc.shed_cap(i));
            prog.a(lay.r_shedlb(i), lay.col_psh(i), -1.0);
            prog.rhs(lay.r_shedlb(i), 0.0);
        }

        prog.finish(vec![ZeroConeT(lay.n_eq), NonnegativeConeT(lay.n_ineq)])
    }
}

/// Read the raw Clarabel primal/dual vectors back into a [`DcOpfSolution`].
///
/// Equality duals carry the Clarabel sign flip (`nu = -z`); the non-negative
/// inequality duals map straight across. The g-stationarity
/// `2 cq g + cl = G_inc' nu_bal` then makes `nu_bal` the (positive) marginal cost,
/// i.e. the LMP.
fn read_dc_solution(dc: &DcNetwork, raw: &RawSolution) -> DcOpfSolution {
    let lay = OpfLayout::dc(dc.n, dc.m, dc.k);
    let (n, m, k) = (dc.n, dc.m, dc.k);
    let x = &raw.x;
    let z = &raw.z;
    DcOpfSolution {
        va: (0..n).map(|i| x[lay.col_va(i)]).collect(),
        pg: (0..k).map(|j| x[lay.col_pg(j)]).collect(),
        f: (0..m).map(|e| x[lay.col_f(e)]).collect(),
        psh: (0..n).map(|i| x[lay.col_psh(i)]).collect(),
        nu_bal: (0..n).map(|i| -z[lay.r_pb(i)]).collect(),
        lam_ub: (0..m).map(|e| z[lay.r_lineub(e)]).collect(),
        lam_lb: (0..m).map(|e| z[lay.r_linelb(e)]).collect(),
        rho_ub: (0..k).map(|j| z[lay.r_genub(j)]).collect(),
        rho_lb: (0..k).map(|j| z[lay.r_genlb(j)]).collect(),
        mu_ub: (0..n).map(|i| z[lay.r_shedub(i)]).collect(),
        mu_lb: (0..n).map(|i| z[lay.r_shedlb(i)]).collect(),
        gamma_ub: (0..m).map(|e| z[lay.r_phaseub(e)]).collect(),
        gamma_lb: (0..m).map(|e| z[lay.r_phaselb(e)]).collect(),
        objective: raw.objective,
        iterations: raw.iterations.clone(),
    }
}

/// Solve the DC OPF for `model`: build the program over [`Dc`], solve it, and read
/// back the dispatch, flows, shedding, and the full dual set (including the nodal
/// price `nu_bal`). The uncancellable convenience entry point the tests and the
/// sensitivity finite-difference checks use. The public solve entry is `api`.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn dc_opf(model: &DcNetwork) -> Result<DcOpfSolution, String> {
    dc_opf_cancellable(model, None)
}

/// As [`dc_opf`], but `cancel` (when present) is polled once per interior-point
/// iteration; flipping it true halts the solve at the next iteration and returns
/// `Err`. The server uses this to drop a solve that has timed out or whose client
/// disconnected, releasing the solver permit instead of running to convergence.
pub(crate) fn dc_opf_cancellable(
    model: &DcNetwork,
    cancel: Option<Arc<AtomicBool>>,
) -> Result<DcOpfSolution, String> {
    let prog = build_opf(&Dc::new(), model);
    let raw = run(&prog, cancel)?;
    Ok(read_dc_solution(model, &raw))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{parse_case3, DcNetwork};

    #[test]
    fn uncongested_three_bus_economic_dispatch() {
        let dc = parse_case3();
        let sol = dc_opf(&dc).expect("solve");

        // DC power balance is lossless: sum(pg) + sum(psh) == sum(demand).
        let total_pg: f64 = sol.pg.iter().sum();
        let total_psh: f64 = sol.psh.iter().sum();
        let total_d: f64 = dc.demand.iter().sum();
        assert!(
            (total_pg + total_psh - total_d).abs() < 1e-6,
            "balance: pg {total_pg} + psh {total_psh} != d {total_d}"
        );
        // Ample capacity, so nothing is shed.
        for &p in &sol.psh {
            assert!(p.abs() < 1e-6, "unexpected shedding {p}");
        }

        // Uncongested: LMPs are positive, near-equal, and at the analytic
        // marginal price (~11.49 $/MWh; the small flow regularization shifts it
        // slightly).
        let lmp = sol.lmp_usd_per_mwh(dc.base_mva);
        let lo = lmp.iter().cloned().fold(f64::INFINITY, f64::min);
        let hi = lmp.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        assert!(lo > 0.0, "LMP must be positive, got {lo}");
        assert!(
            hi - lo < 0.05,
            "uncongested LMPs should be ~equal: {lo}..{hi}"
        );
        assert!((lo - 11.49).abs() < 0.5, "LMP {lo} not near analytic 11.49");

        // Flows obey f = W A theta exactly (the flow-definition equality).
        for e in 0..dc.m {
            let w = -dc.b[e] * dc.sw[e];
            let expected = w * (sol.va[dc.br_from[e]] - sol.va[dc.br_to[e]]);
            assert!(
                (sol.f[e] - expected).abs() < 1e-6,
                "flow def mismatch at {e}"
            );
        }
    }

    #[test]
    fn marginal_cost_equals_nodal_price() {
        let dc = parse_case3();
        let sol = dc_opf(&dc).expect("solve");
        // Both generators are interior (between gmin and gmax), so generator
        // stationarity reduces to 2 cq g + cl == nu_bal at the gen bus. This is
        // the LMP sign and scale, independent of the analytic dispatch.
        for j in 0..dc.k {
            let mc = 2.0 * dc.cq[j] * sol.pg[j] + dc.cl[j];
            let nu = sol.nu_bal[dc.gen_bus[j]];
            assert!((mc - nu).abs() < 1e-3, "gen {j}: MC {mc} != nu_bal {nu}");
        }
        // Inequality duals are non-negative (standard KKT convention).
        let ineq = sol
            .lam_ub
            .iter()
            .chain(&sol.lam_lb)
            .chain(&sol.rho_ub)
            .chain(&sol.rho_lb)
            .chain(&sol.mu_ub)
            .chain(&sol.mu_lb)
            .chain(&sol.gamma_ub)
            .chain(&sol.gamma_lb);
        for v in ineq {
            assert!(*v > -1e-6, "inequality dual is negative: {v}");
        }
    }

    #[test]
    fn load_shedding_when_capacity_is_short() {
        // Generation capacity (0.8 pu) below the 0.9 pu load forces the optimum to
        // shed: both generators pin at gmax and the 0.1 pu deficit is shed at the
        // load bus, the regime the served cases never reach.
        let mut dc = parse_case3();
        dc.gmax = vec![0.4, 0.4];
        let sol = dc_opf(&dc).expect("solve");

        let total_pg: f64 = sol.pg.iter().sum();
        let total_psh: f64 = sol.psh.iter().sum();
        let total_d: f64 = dc.demand.iter().sum();
        assert!(
            (total_pg + total_psh - total_d).abs() < 1e-6,
            "balance with shedding: pg {total_pg} + psh {total_psh} != d {total_d}"
        );
        assert!(
            (total_pg - 0.8).abs() < 1e-4,
            "generation should saturate at capacity, got {total_pg}"
        );
        assert!(
            (total_psh - 0.1).abs() < 1e-4,
            "should shed the 0.1 pu deficit, got {total_psh}"
        );
        // The shedding lands at the load bus (dense index 1).
        assert!(sol.psh[1] > 1e-3, "expected shedding at the load bus");
    }

    #[test]
    fn solves_a_real_case() {
        // ACTIVSg200 solve smoke test: confirms Clarabel handles a full network
        // and the power-balance identity holds. Skips if the data is absent.
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../data/ACTIVSg200/case_ACTIVSg200.m"
        );
        let Ok(text) = std::fs::read_to_string(path) else {
            eprintln!("skipping solves_a_real_case: {path} not found");
            return;
        };
        let net = powerio::parse_str(&text, "matpower")
            .expect("parse ACTIVSg200")
            .network;
        let dc = DcNetwork::from_network(&net).expect("build DcNetwork");
        let sol = dc_opf(&dc).expect("solve ACTIVSg200");

        // Power balance identity: sum(pg) + sum(psh) == sum(demand).
        let total_pg: f64 = sol.pg.iter().sum();
        let total_psh: f64 = sol.psh.iter().sum();
        let total_d: f64 = dc.demand.iter().sum();
        assert!(
            (total_pg + total_psh - total_d).abs() < 1e-5,
            "balance off: pg {total_pg} + psh {total_psh} vs d {total_d}"
        );
        // Every LMP is finite.
        let lmp = sol.lmp_usd_per_mwh(dc.base_mva);
        assert_eq!(lmp.len(), dc.n);
        for v in &lmp {
            assert!(v.is_finite(), "non-finite LMP {v}");
        }
    }
}
