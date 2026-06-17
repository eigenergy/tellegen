//! DC OPF solve via Clarabel (pure Rust QP, no BLAS). Step 2 of issue #2:
//! assemble the B-theta QP from a [`DcNetwork`], solve, and extract dispatch,
//! flows, and the duals — including the nodal power-balance dual `nu_bal` that
//! is the LMP.
//!
//! The formulation matches PowerDiff.jl's JuMP model exactly (same variables,
//! objective, and constraints) so the optimum and its duals agree:
//!
//! ```text
//! min  sum(cq g^2 + cl g) + (tau^2/2) ||f||^2 + sum(c_shed psh)
//! s.t. G_inc g + psh - d = B theta        (nu_bal)   <- LMP
//!      f = W A theta                       (nu_flow)
//!      -fmax <= f <= fmax                  (lam_lb, lam_ub)
//!      gmin <= g <= gmax                   (rho_lb, rho_ub)
//!      0 <= psh <= max(d, 0)               (mu_lb, mu_ub)
//!      sw .* angmin <= sw .* (A theta) <= sw .* angmax  (gamma_lb, gamma_ub)
//!      theta[ref] = 0                       (eta_ref)
//! ```
//!
//! Clarabel solves `min 1/2 x'Px + q'x  s.t.  Ax + s = b, s in K` with
//! Lagrangian `L = obj + z'(Ax + s - b)`, so an equality row written as
//! PowerDiff's residual gives `nu = -z`, while the non-negative inequality
//! duals map straight across. The g-stationarity `2 cq g + cl = G_inc' nu_bal`
//! then makes `nu_bal` the (positive) marginal cost, i.e. the LMP. See
//! PowerDiff `src/prob/kkt_dc_opf.jl` and `src/sens/lmp.jl`.

use clarabel::algebra::CscMatrix;
use clarabel::solver::{
    DefaultSettings, DefaultSolver, IPSolver, SolverStatus,
    SupportedConeT::{NonnegativeConeT, ZeroConeT},
};

use super::model::DcNetwork;

/// Primal and dual solution of the DC OPF, in per unit. Dual names and signs
/// follow PowerDiff's `DCOPFSolution`. `nu_bal` is the LMP (per-unit
/// $/per-unit-MW); divide by `base_mva` for $/MWh.
#[derive(Clone)]
#[cfg_attr(not(feature = "sensitivity"), allow(dead_code))]
pub struct DcSolution {
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
}

impl DcSolution {
    /// LMP per bus in $/MWh (`nu_bal / base_mva`), in dense bus-index order.
    pub fn lmp_usd_per_mwh(&self, base_mva: f64) -> Vec<f64> {
        self.nu_bal.iter().map(|v| v / base_mva).collect()
    }
}

/// Assemble and solve the DC OPF for `dc`. Returns the primal dispatch, flows,
/// shedding, and the full dual set.
pub fn solve(dc: &DcNetwork) -> Result<DcSolution, String> {
    let (n, m, k) = (dc.n, dc.m, dc.k);

    // Variable vector x = [va(n), pg(k), f(m), psh(n)].
    let nvar = 2 * n + k + m;
    let col_va = |i: usize| i;
    let col_pg = |j: usize| n + j;
    let col_f = |e: usize| n + k + e;
    let col_psh = |i: usize| n + k + m + i;

    // Constraint rows: equalities first (zero cone), then inequalities
    // (non-negative cone), each block contiguous.
    let n_eq = n + m + 1;
    let r_pb = |i: usize| i; // power balance
    let r_fd = |e: usize| n + e; // flow definition
    let r_ref = n + m; // reference bus
    let base_ineq = n_eq;
    let r_lineub = |e: usize| base_ineq + e;
    let r_linelb = |e: usize| base_ineq + m + e;
    let r_genub = |j: usize| base_ineq + 2 * m + j;
    let r_genlb = |j: usize| base_ineq + 2 * m + k + j;
    let r_shedub = |i: usize| base_ineq + 2 * m + 2 * k + i;
    let r_shedlb = |i: usize| base_ineq + 2 * m + 2 * k + n + i;
    let r_phaseub = |e: usize| base_ineq + 2 * m + 2 * k + 2 * n + e;
    let r_phaselb = |e: usize| base_ineq + 2 * m + 2 * k + 2 * n + m + e;
    let n_ineq = 4 * m + 2 * k + 2 * n;
    let ncon = n_eq + n_ineq;

    // Objective Hessian P (diagonal): 2 cq on pg, tau^2 on f.
    let mut pi = Vec::with_capacity(k + m);
    let mut pj = Vec::with_capacity(k + m);
    let mut pv = Vec::with_capacity(k + m);
    for j in 0..k {
        let v = 2.0 * dc.cq[j];
        if v != 0.0 {
            pi.push(col_pg(j));
            pj.push(col_pg(j));
            pv.push(v);
        }
    }
    let tau2 = dc.tau * dc.tau;
    for e in 0..m {
        pi.push(col_f(e));
        pj.push(col_f(e));
        pv.push(tau2);
    }
    let p_mat = CscMatrix::new_from_triplets(nvar, nvar, pi, pj, pv);

    // Linear objective q: cl on pg, c_shed on psh.
    let mut q = vec![0.0; nvar];
    for j in 0..k {
        q[col_pg(j)] = dc.cl[j];
    }
    for i in 0..n {
        q[col_psh(i)] = dc.c_shed[i];
    }

    // Constraint matrix A and rhs b.
    let mut ai: Vec<usize> = Vec::new();
    let mut aj: Vec<usize> = Vec::new();
    let mut av: Vec<f64> = Vec::new();
    let mut b = vec![0.0; ncon];
    macro_rules! a {
        ($r:expr, $c:expr, $v:expr) => {{
            let v = $v;
            if v != 0.0 {
                ai.push($r);
                aj.push($c);
                av.push(v);
            }
        }};
    }

    // Power balance: G_inc g + psh - B theta = d.
    for i in 0..n {
        a!(r_pb(i), col_psh(i), 1.0);
        b[r_pb(i)] = dc.demand[i];
    }
    for j in 0..k {
        a!(r_pb(dc.gen_bus[j]), col_pg(j), 1.0);
    }
    // Per-branch contributions: -B (power balance), flow definition, line
    // limits, and phase-angle-difference limits.
    for e in 0..m {
        let w = -dc.b[e] * dc.sw[e]; // edge weight (positive for inductive)
        let (fb, tb) = (dc.br_from[e], dc.br_to[e]);
        // -B = -A' diag(w) A
        a!(r_pb(fb), col_va(fb), -w);
        a!(r_pb(tb), col_va(tb), -w);
        a!(r_pb(fb), col_va(tb), w);
        a!(r_pb(tb), col_va(fb), w);
        // Flow definition: f - w (theta_from - theta_to) = 0
        a!(r_fd(e), col_f(e), 1.0);
        a!(r_fd(e), col_va(fb), -w);
        a!(r_fd(e), col_va(tb), w);
        // Line limits: f <= fmax and -f <= fmax
        a!(r_lineub(e), col_f(e), 1.0);
        b[r_lineub(e)] = dc.fmax[e];
        a!(r_linelb(e), col_f(e), -1.0);
        b[r_linelb(e)] = dc.fmax[e];
        // Phase-angle-difference limits: sw (A theta) within sw [angmin, angmax]
        let sw = dc.sw[e];
        a!(r_phaseub(e), col_va(fb), sw);
        a!(r_phaseub(e), col_va(tb), -sw);
        b[r_phaseub(e)] = sw * dc.angmax[e];
        a!(r_phaselb(e), col_va(fb), -sw);
        a!(r_phaselb(e), col_va(tb), sw);
        b[r_phaselb(e)] = -sw * dc.angmin[e];
    }
    // Reference bus: theta[ref] = 0
    a!(r_ref, col_va(dc.ref_bus), 1.0);
    // Generation limits: g <= gmax and -g <= -gmin
    for j in 0..k {
        a!(r_genub(j), col_pg(j), 1.0);
        b[r_genub(j)] = dc.gmax[j];
        a!(r_genlb(j), col_pg(j), -1.0);
        b[r_genlb(j)] = -dc.gmin[j];
    }
    // Shedding bounds: psh <= max(d, 0) and -psh <= 0
    for i in 0..n {
        a!(r_shedub(i), col_psh(i), 1.0);
        b[r_shedub(i)] = dc.demand[i].max(0.0);
        a!(r_shedlb(i), col_psh(i), -1.0);
        b[r_shedlb(i)] = 0.0;
    }
    let a_mat = CscMatrix::new_from_triplets(ncon, nvar, ai, aj, av);

    let cones = [ZeroConeT(n_eq), NonnegativeConeT(n_ineq)];

    // Tighten tolerances past Clarabel's 1e-8 defaults for accurate dual
    // recovery, which the sensitivity column (step 3) needs.
    let settings: DefaultSettings<f64> = DefaultSettings {
        verbose: false,
        tol_gap_abs: 1e-9,
        tol_gap_rel: 1e-9,
        tol_feas: 1e-9,
        ..DefaultSettings::default()
    };

    let mut solver = DefaultSolver::new(&p_mat, &q, &a_mat, &b, &cones, settings)
        .map_err(|e| format!("Clarabel setup failed: {e:?}"))?;
    solver.solve();

    let status = solver.solution.status;
    if !matches!(status, SolverStatus::Solved | SolverStatus::AlmostSolved) {
        return Err(format!("DC OPF solve did not converge: {status:?}"));
    }

    let x = &solver.solution.x;
    let z = &solver.solution.z;

    // Equality duals carry the Clarabel-to-PowerDiff sign flip (nu = -z); the
    // non-negative inequality duals map straight across.
    let sol = DcSolution {
        va: (0..n).map(|i| x[col_va(i)]).collect(),
        pg: (0..k).map(|j| x[col_pg(j)]).collect(),
        f: (0..m).map(|e| x[col_f(e)]).collect(),
        psh: (0..n).map(|i| x[col_psh(i)]).collect(),
        nu_bal: (0..n).map(|i| -z[r_pb(i)]).collect(),
        lam_ub: (0..m).map(|e| z[r_lineub(e)]).collect(),
        lam_lb: (0..m).map(|e| z[r_linelb(e)]).collect(),
        rho_ub: (0..k).map(|j| z[r_genub(j)]).collect(),
        rho_lb: (0..k).map(|j| z[r_genlb(j)]).collect(),
        mu_ub: (0..n).map(|i| z[r_shedub(i)]).collect(),
        mu_lb: (0..n).map(|i| z[r_shedlb(i)]).collect(),
        gamma_ub: (0..m).map(|e| z[r_phaseub(e)]).collect(),
        gamma_lb: (0..m).map(|e| z[r_phaselb(e)]).collect(),
        objective: solver.solution.obj_val,
    };
    Ok(sol)
}

#[cfg(test)]
mod tests {
    use super::super::model::{parse_case3, DcNetwork};
    use super::*;

    #[test]
    fn uncongested_three_bus_economic_dispatch() {
        let dc = parse_case3();
        let sol = solve(&dc).expect("solve");

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
        let sol = solve(&dc).expect("solve");
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
        let sol = solve(&dc).expect("solve ACTIVSg200");

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
