//! The Clarabel solve driver: run an assembled [`OpfProgram`] through Clarabel's
//! pure-Rust interior-point QP solver (no BLAS), recording the iteration trace and
//! honoring an optional cancel flag.
//!
//! Clarabel solves `min 1/2 x'Px + q'x  s.t. Ax + s = b, s in K` and returns the
//! primal `x` and the conic dual `z`. The program assembly lives in
//! [`crate::problem`]; the sign conventions that map `z` back to named duals live
//! with the readout there.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use clarabel::solver::{DefaultInfo, DefaultSettings, DefaultSolver, IPSolver, SolverStatus};
use serde::Serialize;

use crate::problem::OpfProgram;

/// One interior-point iterate: the iteration index, the primal objective, and
/// the primal and dual residuals. Collected once per Clarabel iteration to draw
/// the convergence plot. Shape matches the frontend `SolveIteration`.
#[derive(Clone, Debug, Serialize)]
pub struct SolveIteration {
    pub iter: u32,
    pub objective: f64,
    pub inf_pr: f64,
    pub inf_du: f64,
}

/// The raw output of a solve: the primal `x`, the conic dual `z`, the objective,
/// and the iteration trace. A formulation-specific readout (in [`crate::problem`])
/// turns this into a named solution.
pub(crate) struct RawSolution {
    pub x: Vec<f64>,
    pub z: Vec<f64>,
    pub objective: f64,
    pub iterations: Vec<SolveIteration>,
}

/// Solve `prog` with Clarabel and return the raw primal/dual vectors.
///
/// `cancel` (when present) is polled once per interior-point iteration through
/// Clarabel's termination callback; flipping it true halts the solve at the next
/// iteration and returns `Err`. Tolerances are tightened past Clarabel's 1e-8
/// defaults so the dual recovery the sensitivity column needs is accurate.
pub(crate) fn run(
    prog: &OpfProgram,
    cancel: Option<Arc<AtomicBool>>,
) -> Result<RawSolution, String> {
    let settings: DefaultSettings<f64> = DefaultSettings {
        verbose: false,
        tol_gap_abs: 1e-9,
        tol_gap_rel: 1e-9,
        tol_feas: 1e-9,
        ..DefaultSettings::default()
    };

    let mut solver = DefaultSolver::new(&prog.p, &prog.q, &prog.a, &prog.b, &prog.cones, settings)
        .map_err(|e| format!("Clarabel setup failed: {e:?}"))?;
    // Record every interior-point iterate for the convergence plot, and (when a
    // cancel flag is present) stop the solve at the next iteration if it flips.
    let trace: Arc<Mutex<Vec<SolveIteration>>> = Arc::new(Mutex::new(Vec::new()));
    {
        let trace = trace.clone();
        solver.set_termination_callback(move |info: &DefaultInfo<f64>| {
            trace.lock().unwrap().push(SolveIteration {
                iter: info.iterations,
                objective: info.cost_primal,
                inf_pr: info.res_primal,
                inf_du: info.res_dual,
            });
            cancel.as_ref().is_some_and(|f| f.load(Ordering::Relaxed))
        });
    }
    solver.solve();

    let status = solver.solution.status;
    if matches!(status, SolverStatus::CallbackTerminated) {
        return Err("DC OPF solve cancelled".into());
    }
    // Classify the solver status ourselves so the message carries a stable keyword
    // regardless of how Clarabel spells the enum — callers (and the benchmark harness,
    // which separates a correctly-detected infeasible case from a hard failure) key on the
    // word, not the enum naming. Primal infeasibility means the load is unservable. Dual
    // infeasibility means the primal is unbounded below (a modeling or bound bug, not an
    // unservable case), so give it a distinct word the harness will not fold into the
    // "infeasible" bucket and silently count as agreement with the reference.
    if matches!(status, SolverStatus::PrimalInfeasible) {
        return Err(format!("DC OPF solve infeasible: {status:?}"));
    }
    if matches!(status, SolverStatus::DualInfeasible) {
        return Err(format!("DC OPF solve unbounded: {status:?}"));
    }
    if !matches!(status, SolverStatus::Solved | SolverStatus::AlmostSolved) {
        return Err(format!("DC OPF solve did not converge: {status:?}"));
    }
    let iterations = std::mem::take(&mut *trace.lock().unwrap());

    Ok(RawSolution {
        x: solver.solution.x.clone(),
        z: solver.solution.z.clone(),
        objective: solver.solution.obj_val,
        iterations,
    })
}

/// faer sparse-LU driver for square linear systems — the non-optimization solve
/// path (DC power flow now; the sensitivity engine keeps its own regularized
/// factorization in [`crate::sens`]). Gated with `faer` behind the `sensitivity`
/// feature, like every other faer-backed path, so the Clarabel-only core build
/// stays free of faer's wasm kernels.
#[cfg(feature = "sensitivity")]
mod linsolve {
    use faer::linalg::solvers::Solve;
    use faer::sparse::{SparseColMat, Triplet};
    use faer::Mat;

    /// Solve the square sparse system `A x = b` by LU and return `x`. `triplets`
    /// are the `(row, col, value)` entries of `A` (duplicates summed); `rhs` is
    /// `b`, length `dim`. Errors if `A` is structurally singular (a disconnected
    /// DC network grounds to a zero row, for instance).
    pub(crate) fn solve_sparse(
        dim: usize,
        triplets: &[(usize, usize, f64)],
        rhs: &[f64],
    ) -> Result<Vec<f64>, String> {
        let entries: Vec<Triplet<usize, usize, f64>> = triplets
            .iter()
            .map(|&(r, c, v)| Triplet::new(r, c, v))
            .collect();
        let mat = SparseColMat::<usize, f64>::try_new_from_triplets(dim, dim, &entries)
            .map_err(|e| format!("sparse assembly failed: {e:?}"))?;
        let lu = mat
            .sp_lu()
            .map_err(|e| format!("sparse LU failed: {e:?}"))?;
        let mut x = Mat::<f64>::zeros(dim, 1);
        for (i, &v) in rhs.iter().enumerate() {
            x[(i, 0)] = v;
        }
        lu.solve_in_place(x.as_mut());
        Ok((0..dim).map(|i| x[(i, 0)]).collect())
    }
}
#[cfg(feature = "sensitivity")]
pub(crate) use linsolve::solve_sparse;
