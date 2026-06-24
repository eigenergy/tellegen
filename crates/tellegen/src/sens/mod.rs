//! Sensitivities by implicit differentiation of a solved optimality/physics system.
//!
//! `d(operand)/d(parameter)` comes from a converged solution by the implicit function
//! theorem on a residual `K(z, p) = 0`: `dz/dp = -(dK/dz)^{-1} (dK/dp)`. One physical
//! vocabulary ([`Operand`]/[`Parameter`]) and one object-safe trait ([`Differentiable`])
//! span the three formulations — the DC OPF KKT ([`DcKkt`]), the AC
//! power flow Newton system ([`AcNewton`]), and the conic SOCWR KKT ([`ConicKkt`]) —
//! which differ only in how they build the Jacobian `K`, the parameter columns `dK/dp`,
//! and the operand [`Selector`]. The free [`sensitivity`] driver is the single front
//! door; it runs the shared [`forward_adjoint`] over [`solve_refined`]:
//!
//! - **forward** solves `K X = dK/dp` once per parameter and reads the operand rows;
//! - **adjoint** solves `Kᵀ Y = Sᵀ` once per operand and contracts with `dK/dp`.
//!
//! The two are algebraically identical; [`Mode::Auto`] picks whichever dimension is
//! smaller. The driver applies the leading minus of `dz/dp`, composed with the
//! selector's reporting sign, so every engine writes natural `+dK/dp` columns.

use faer::linalg::solvers::Solve;
use faer::sparse::{SparseColMat, Triplet};
use faer::Mat;

mod ac;
#[cfg(feature = "conic")]
mod conic;
mod contract;
mod dc;

pub use ac::AcNewton;
#[cfg(feature = "conic")]
pub use conic::ConicKkt;
pub use contract::{
    sensitivity, Axis, Bound, ColMeta, CostTerm, Differentiable, ElementId, End, Operand,
    Parameter, Power, RowMeta, Selector, SensError, SensitivityMatrix, SolveSpec, TapKind,
    VoltageKind, GB,
};
pub use dc::DcKkt;

pub(crate) use contract::{served_unit_scale, served_units_label};

/// Which linear systems the engine factorizes. [`Forward`](Mode::Forward) and
/// [`Adjoint`](Mode::Adjoint) return the same matrix; the choice trades a
/// factorization of `K` (cheap when parameters are few) for one of `K'` (cheap when
/// operands are few). [`Auto`](Mode::Auto) picks the smaller dimension and is the api
/// default.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum Mode {
    /// Solve `K X = rhs` once per parameter set, read the operand rows.
    Forward,
    /// Solve `K' Y = S'` once per operand set, contract with `rhs`.
    Adjoint,
    /// Pick [`Forward`](Mode::Forward) when parameters are at most operands, else
    /// [`Adjoint`](Mode::Adjoint).
    Auto,
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

/// Solve `(K + eps I) X = rhs` for every right-hand-side column by faer sparse LU.
///
/// `eps == 0` with `max_iters == 0` is a plain LU solve for a well-conditioned
/// system (the converged AC Newton Jacobian). A positive `eps` adds a Tikhonov term
/// that makes a KKT factorization with a benign nullspace (the variables with no
/// objective curvature) well posed; the refinement steps then drive the answer back
/// toward the unregularized solution against the regularized operator, which both
/// removes the Tikhonov bias and damps the nullspace direction so the iteration
/// stays bounded. Refinement stops at `tol_factor · ||rhs||` or once it stops
/// improving.
pub(super) fn solve_refined(
    dim: usize,
    triplets: &[(usize, usize, f64)],
    rhs: Mat<f64>,
    eps: f64,
    max_iters: usize,
    tol_factor: f64,
) -> Result<Mat<f64>, String> {
    let mut reg: Vec<Triplet<usize, usize, f64>> = triplets
        .iter()
        .map(|&(r, c, v)| Triplet::new(r, c, v))
        .collect();
    if eps != 0.0 {
        for d in 0..dim {
            reg.push(Triplet::new(d, d, eps));
        }
    }
    let mat = SparseColMat::<usize, f64>::try_new_from_triplets(dim, dim, &reg)
        .map_err(|e| format!("sparse system assembly failed: {e:?}"))?;
    let lu = mat
        .sp_lu()
        .map_err(|e| format!("sparse LU failed: {e:?}"))?;

    let mut x = rhs.clone();
    lu.solve_in_place(x.as_mut());
    if max_iters > 0 {
        let tol = tol_factor * max_abs(&rhs).max(1.0);
        let mut prev = f64::INFINITY;
        for _ in 0..max_iters {
            let mut r = rhs.clone();
            for tr in &reg {
                for c in 0..rhs.ncols() {
                    r[(tr.row, c)] -= tr.val * x[(tr.col, c)];
                }
            }
            let rn = max_abs(&r);
            if rn <= tol || rn >= prev {
                break;
            }
            prev = rn;
            lu.solve_in_place(r.as_mut());
            for c in 0..x.ncols() {
                for i in 0..dim {
                    x[(i, c)] += r[(i, c)];
                }
            }
        }
    }
    Ok(x)
}

/// Run the forward or adjoint solve for one `(operand, parameter)` sensitivity.
///
/// `k` is the Jacobian as `(row, col, value)` triplets, `forward_rhs` the dense
/// `dim × nparam` parameter right-hand side, and `op_map` the operand's linear
/// functionals — one per reported row, each a list of `(solution row, weight)` (a unit
/// row is `[(r, 1.0)]`; a derived operand like a branch flow is a weighted sum). `sign`
/// is the scalar each engine picks so the result reads in reported units. Returns
/// `out[operand][param]`. `solve` factorizes and back-solves (the engine threads in its
/// own regularization through the closure).
pub(super) fn forward_adjoint(
    dim: usize,
    k: &[(usize, usize, f64)],
    forward_rhs: Mat<f64>,
    op_map: &[Vec<(usize, f64)>],
    sign: f64,
    mode: Mode,
    solve: impl Fn(&[(usize, usize, f64)], Mat<f64>) -> Result<Mat<f64>, String>,
) -> Result<Vec<Vec<f64>>, String> {
    let nparam = forward_rhs.ncols();
    match mode {
        // The driver resolves Auto to a concrete direction before calling.
        Mode::Auto => unreachable!("Mode::Auto is resolved before forward_adjoint"),
        Mode::Forward => {
            let x = solve(k, forward_rhs)?;
            Ok(op_map
                .iter()
                .map(|entries| {
                    (0..nparam)
                        .map(|c| {
                            let mut acc = 0.0;
                            for &(r, w) in entries {
                                acc += w * x[(r, c)];
                            }
                            sign * acc
                        })
                        .collect()
                })
                .collect())
        }
        Mode::Adjoint => {
            let kt: Vec<(usize, usize, f64)> = k.iter().map(|&(r, c, v)| (c, r, v)).collect();
            // The adjoint right-hand side is the weighted operand functionals as columns.
            let mut sel = Mat::<f64>::zeros(dim, op_map.len());
            for (o, entries) in op_map.iter().enumerate() {
                for &(r, w) in entries {
                    sel[(r, o)] += w;
                }
            }
            let y = solve(&kt, sel)?;
            // `forward_rhs` (dK/dp) is sparse — a handful of nonzeros per column — so
            // collect each column's nonzeros once and contract the adjoint solution only
            // over them. Skipping exact zeros is bit-identical to the dense loop but
            // turns each cell from O(dim) into O(nnz per column).
            let cols_nnz: Vec<Vec<(usize, f64)>> = (0..nparam)
                .map(|c| {
                    (0..dim)
                        .filter_map(|r| {
                            let v = forward_rhs[(r, c)];
                            (v != 0.0).then_some((r, v))
                        })
                        .collect()
                })
                .collect();
            Ok((0..op_map.len())
                .map(|o| {
                    cols_nnz
                        .iter()
                        .map(|nnz| {
                            let mut acc = 0.0;
                            for &(r, v) in nnz {
                                acc += v * y[(r, o)];
                            }
                            sign * acc
                        })
                        .collect()
                })
                .collect())
        }
    }
}
