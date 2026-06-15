//! dLMP/dd sensitivity column for the DC OPF. Step 3 of issue #2: differentiate
//! the KKT system of the solved DC OPF with respect to a demand perturbation and
//! read the change in the nodal price.
//!
//! The implicit function theorem on the KKT residual `K(z, p) = 0` gives
//! `dz/dp = -(dK/dz)^{-1} (dK/dp)`. For demand `d`, the parameter Jacobian
//! `dK/dd` has two nonzeros per bus (power balance and the shedding upper
//! bound), and the LMP sensitivity is the `nu_bal` block of `dz/dd`. Both the
//! KKT Jacobian and the demand Jacobian are ported verbatim from PowerDiff's
//! `src/prob/kkt_dc_opf.jl`, including the complementarity snapping
//! (`solve!` in `src/prob/dc_opf.jl`) that puts the solution at strict
//! complementarity before the derivative is taken.
//!
//! The nonsymmetric KKT Jacobian is factorized with faer's sparse LU (pure Rust,
//! wasm-safe; validated on a real case matrix in the tests, per faer-rs#222).

use faer::linalg::solvers::Solve;
use faer::sparse::{SparseColMat, Triplet};
use faer::Mat;

use super::model::DcNetwork;
use super::solve::DcSolution;

/// Strict-complementarity / structural-zero-shed threshold. Matches PowerDiff's
/// `COMPLEMENTARITY_SNAP_TOL`.
const SNAP_TOL: f64 = 1e-6;

/// Tikhonov perturbation applied only if the KKT Jacobian factorization fails
/// (degenerate complementarity). Matches PowerDiff's `TIKHONOV_EPS`.
const TIKHONOV_EPS: f64 = 1e-10;

/// `max(d, 0) < SNAP_TOL`: the bus has no curtailable load, so its shedding
/// variable is structurally fixed at zero. Matches `_is_fixed_zero_shed`.
#[inline]
fn is_fixed_zero_shed(d: f64) -> bool {
    d.max(0.0) < SNAP_TOL
}

/// Offsets of each block in the flattened KKT variable vector, in PowerDiff's
/// order: `[va, pg, f, psh, lam_lb, lam_ub, gamma_lb, gamma_ub, rho_lb, rho_ub,
/// mu_lb, mu_ub, nu_bal, nu_flow, eta]`. Total `5n + 6m + 3k + 1`.
struct KktIdx {
    n: usize,
    m: usize,
    k: usize,
    dim: usize,
    va: usize,
    pg: usize,
    f: usize,
    psh: usize,
    lam_lb: usize,
    lam_ub: usize,
    gamma_lb: usize,
    gamma_ub: usize,
    rho_lb: usize,
    rho_ub: usize,
    mu_lb: usize,
    mu_ub: usize,
    nu_bal: usize,
    nu_flow: usize,
    eta: usize,
}

impl KktIdx {
    fn new(n: usize, m: usize, k: usize) -> Self {
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
        let lam_lb = take(m);
        let lam_ub = take(m);
        let gamma_lb = take(m);
        let gamma_ub = take(m);
        let rho_lb = take(k);
        let rho_ub = take(k);
        let mu_lb = take(n);
        let mu_ub = take(n);
        let nu_bal = take(n);
        let nu_flow = take(m);
        let eta = take(1);
        KktIdx {
            n,
            m,
            k,
            dim: o,
            va,
            pg,
            f,
            psh,
            lam_lb,
            lam_ub,
            gamma_lb,
            gamma_ub,
            rho_lb,
            rho_ub,
            mu_lb,
            mu_ub,
            nu_bal,
            nu_flow,
            eta,
        }
    }
}

/// Snap the solution to strict complementarity before differentiating, matching
/// PowerDiff's `solve!`: zero the non-binding phase-angle duals (and clamp the
/// binding side), and canonicalize the shedding duals / value by regime
/// (degenerate, lower-bound active, upper-bound active, interior).
fn snap(dc: &DcNetwork, sol: &DcSolution) -> DcSolution {
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
        let cap = dc.demand[i].max(0.0);
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

/// Assemble the KKT Jacobian `dK/dz` as `(row, col, value)` triplets, ported
/// column by column from PowerDiff's `calc_kkt_jacobian`. `s` must already be
/// snapped to strict complementarity.
fn kkt_triplets(dc: &DcNetwork, s: &DcSolution, idx: &KktIdx) -> Vec<Triplet<usize, usize, f64>> {
    let (n, m, k) = (dc.n, dc.m, dc.k);
    let tau2 = dc.tau * dc.tau;
    let inc = incidence_by_bus(dc);
    let gens = gens_by_bus(dc);
    let bcols = susceptance_cols(dc);

    let mut t: Vec<Triplet<usize, usize, f64>> = Vec::new();
    macro_rules! e {
        ($r:expr, $c:expr, $v:expr) => {{
            let v = $v;
            if v != 0.0 {
                t.push(Triplet::new($r, $c, v));
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
    }

    // f columns: flow regularization, line-bound stationarity, flow def.
    for e in 0..m {
        let col = idx.f + e;
        e!(idx.f + e, col, tau2);
        e!(idx.lam_lb + e, col, s.lam_lb[e]);
        e!(idx.lam_ub + e, col, -s.lam_ub[e]);
        e!(idx.nu_flow + e, col, 1.0);
    }

    // psh columns: shedding-bound stationarity (degenerate buses use I) and
    // power balance.
    for i in 0..n {
        let col = idx.psh + i;
        if is_fixed_zero_shed(dc.demand[i]) {
            e!(idx.mu_lb + i, col, 1.0);
        } else {
            e!(idx.mu_lb + i, col, s.mu_lb[i]);
            e!(idx.mu_ub + i, col, -s.mu_ub[i]);
        }
        e!(idx.nu_bal + i, col, 1.0);
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
        if !is_fixed_zero_shed(dc.demand[i]) {
            e!(idx.mu_lb + i, col, s.psh[i]);
        }
    }
    for i in 0..n {
        let col = idx.mu_ub + i;
        e!(idx.psh + i, col, 1.0);
        if is_fixed_zero_shed(dc.demand[i]) {
            e!(idx.mu_ub + i, col, 1.0);
        } else {
            e!(idx.mu_ub + i, col, dc.demand[i].max(0.0) - s.psh[i]);
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

    t
}

/// Demand parameter Jacobian columns `dK/dd` for the requested buses, packed as
/// the right-hand side of the sensitivity solve. Two nonzeros per bus: `-1` in
/// power balance, and the snapped `mu_ub` (times the shed-capacity derivative)
/// in the shedding upper bound. Matches `calc_kkt_jacobian_demand_column`.
fn demand_rhs(dc: &DcNetwork, s: &DcSolution, idx: &KktIdx, buses: &[usize]) -> Mat<f64> {
    let mut rhs = Mat::<f64>::zeros(idx.dim, buses.len());
    for (c, &j) in buses.iter().enumerate() {
        rhs[(idx.nu_bal + j, c)] = -1.0;
        let dcap = if is_fixed_zero_shed(dc.demand[j]) { 0.0 } else { 1.0 };
        rhs[(idx.mu_ub + j, c)] = s.mu_ub[j] * dcap;
    }
    rhs
}

/// Largest absolute entry of a dense matrix.
fn max_abs(m: &Mat<f64>) -> f64 {
    let mut mx = 0.0f64;
    for c in 0..m.ncols() {
        for r in 0..m.nrows() {
            mx = mx.max(m[(r, c)].abs());
        }
    }
    mx
}

/// Residual `b - K x` from the KKT triplets (duplicates summed, matching the
/// assembled matrix), for every right-hand-side column.
fn residual(triplets: &[Triplet<usize, usize, f64>], x: &Mat<f64>, b: &Mat<f64>) -> Mat<f64> {
    let mut r = b.clone();
    for c in 0..b.ncols() {
        for tr in triplets {
            r[(tr.row, c)] -= tr.val * x[(tr.col, c)];
        }
    }
    r
}

/// Solve `K x = b` (all right-hand-side columns) for the sensitivity.
///
/// The differentiated KKT is singular at the optimum: the zero Hessian on the
/// angle and shedding variables leaves a benign nullspace (one that does not
/// reach the `nu_bal` / LMP block). PowerDiff hits the same singularity and
/// regularizes, so we always add a small Tikhonov term `eps*I` — enough to make
/// faer's sparse LU well-posed without perturbing the LMP sensitivity (verified
/// against finite differences in the tests). A few guarded refinement steps
/// against the regularized operator clean up the remaining ill-conditioning.
fn solve_kkt(
    triplets: &[Triplet<usize, usize, f64>],
    idx: &KktIdx,
    b: Mat<f64>,
) -> Result<Mat<f64>, String> {
    let mut reg = triplets.to_vec();
    for d in 0..idx.dim {
        reg.push(Triplet::new(d, d, TIKHONOV_EPS));
    }
    let kmat = SparseColMat::<usize, f64>::try_new_from_triplets(idx.dim, idx.dim, &reg)
        .map_err(|err| format!("KKT assembly failed: {err:?}"))?;
    let lu = kmat
        .sp_lu()
        .map_err(|err| format!("KKT LU failed: {err:?}"))?;

    let mut x = b.clone();
    lu.solve_in_place(x.as_mut());
    // Iterative refinement against the regularized operator; stop at tolerance or
    // once it stops improving (the nullspace direction will not converge).
    let tol = 1e-12 * max_abs(&b).max(1.0);
    let mut prev = f64::INFINITY;
    for _ in 0..8 {
        let r = residual(&reg, &x, &b);
        let rn = max_abs(&r);
        if rn <= tol || rn >= prev {
            break;
        }
        prev = rn;
        let mut dx = r;
        lu.solve_in_place(dx.as_mut());
        for c in 0..x.ncols() {
            for i in 0..x.nrows() {
                x[(i, c)] += dx[(i, c)];
            }
        }
    }
    Ok(x)
}

/// dLMP/dd in per unit: column `c` of the result is `d(nu_bal_i)/d(d_j)` over all
/// buses `i`, for the perturbed bus `j = buses[c]`. One sparse factorization,
/// one batched back-solve over all requested buses.
pub fn dlmp_dd_perunit(
    dc: &DcNetwork,
    sol: &DcSolution,
    buses: &[usize],
) -> Result<Vec<Vec<f64>>, String> {
    let s = snap(dc, sol);
    let idx = KktIdx::new(dc.n, dc.m, dc.k);
    let triplets = kkt_triplets(dc, &s, &idx);
    let rhs = demand_rhs(dc, &s, &idx, buses);
    // x = K^{-1} (dK/dd); dz/dd = -x; the LMP sensitivity is its nu_bal block.
    let x = solve_kkt(&triplets, &idx, rhs)?;
    Ok((0..buses.len())
        .map(|c| (0..dc.n).map(|i| -x[(idx.nu_bal + i, c)]).collect())
        .collect())
}

/// dLMP/dd in served units, ($/MWh)/MW: the per-unit sensitivity divided by
/// `base_mva^2` (both LMP and demand are per unit), as the backend serves it.
pub fn dlmp_dd(dc: &DcNetwork, sol: &DcSolution, buses: &[usize]) -> Result<Vec<Vec<f64>>, String> {
    let b2 = dc.base_mva * dc.base_mva;
    Ok(dlmp_dd_perunit(dc, sol, buses)?
        .into_iter()
        .map(|col| col.into_iter().map(|v| v / b2).collect())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::super::model::{parse_case3, DcNetwork};
    use super::super::solve::solve;
    use super::*;

    /// Central finite difference of `nu_bal` w.r.t. demand at bus `j`.
    fn central_fd(dc: &DcNetwork, j: usize, eps: f64) -> Vec<f64> {
        let mut dp = dc.clone();
        dp.demand[j] += eps;
        let mut dm = dc.clone();
        dm.demand[j] -= eps;
        let sp = solve(&dp).expect("solve +eps");
        let sm = solve(&dm).expect("solve -eps");
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
        let sol = solve(&dc).expect("solve");
        let buses: Vec<usize> = (0..dc.n).collect();
        let analytic = dlmp_dd_perunit(&dc, &sol, &buses).expect("dlmp/dd");
        for j in 0..dc.n {
            let fd = central_fd(&dc, j, 1e-4);
            for i in 0..dc.n {
                let a = analytic[j][i];
                let f = fd[i];
                let rel = (a - f).abs() / f.abs().max(1.0);
                assert!(
                    rel < 1e-3,
                    "d(nu_bal[{i}])/d(d[{j}]): analytic {a}, fd {f}, rel {rel}"
                );
            }
        }
        // Uncongested: the price rises with demand everywhere (sensitivity > 0).
        for col in &analytic {
            for &v in col {
                assert!(v > 0.0, "expected positive price sensitivity, got {v}");
            }
        }
    }

    /// Parity check mirroring the backend's `runtests.jl`: compute the full
    /// dLMP/dd matrix, take the three columns with the largest norm (the most
    /// significant sensitivities, away from near-kink buses), and compare each to
    /// central differences with the same 1 MW step. Returns the worst relative
    /// column error `norm(fd - exact)/norm(exact)`, or `None` if the case file is
    /// absent.
    fn parity_vs_finite_differences(casefile: &str) -> Option<f64> {
        let text = std::fs::read_to_string(casefile).ok()?;
        let net = powerio::parse_str(&text, "matpower").expect("parse").network;
        let dc = DcNetwork::from_network(&net).expect("model");
        let sol = solve(&dc).expect("solve");

        let all: Vec<usize> = (0..dc.n).collect();
        let exact = dlmp_dd_perunit(&dc, &sol, &all).expect("dlmp/dd"); // exact[j][i]
        let norm = |c: &[f64]| c.iter().map(|v| v * v).sum::<f64>().sqrt();
        let mut order: Vec<usize> = (0..dc.n).collect();
        order.sort_by(|&a, &b| norm(&exact[b]).total_cmp(&norm(&exact[a])));

        let h = 1e-2; // 1 MW at 100 MVA base, as in runtests.jl
        let mut worst = 0.0f64;
        for &j in order.iter().take(3) {
            let fd = central_fd(&dc, j, h);
            let diff: Vec<f64> = (0..dc.n).map(|i| fd[i] - exact[j][i]).collect();
            let rel = norm(&diff) / norm(&exact[j]).max(f64::EPSILON);
            worst = worst.max(rel);
        }
        Some(worst)
    }

    #[test]
    fn parity_with_finite_differences_activsg200() {
        // The backend's exact-sensitivity criterion (runtests.jl) on the Rust
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

    // ACTIVSg500 and ACTIVSg2000 build the full sensitivity matrix (the 2000-bus
    // case allocates a ~26500 x 2000 solve), so they are heavy: run explicitly
    // with `cargo test --release -- --ignored`.
    #[test]
    #[ignore = "heavy: run with --release --ignored"]
    fn parity_with_finite_differences_large_cases() {
        for case in ["ACTIVSg500", "ACTIVSg2000"] {
            let path = format!(
                "{}/../data/{case}/case_{case}.m",
                env!("CARGO_MANIFEST_DIR")
            );
            match parity_vs_finite_differences(&path) {
                Some(rel) => assert!(rel < 1e-3, "{case} dLMP/dd vs FD rel {rel}"),
                None => eprintln!("skipping {case} parity: {path} not found"),
            }
        }
    }
}
