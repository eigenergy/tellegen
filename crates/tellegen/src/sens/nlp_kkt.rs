//! The KKT system of a solved nonlinear program with its active set held fixed.
//!
//! Both nonlinear OPF paths differentiate a converged POUNCE point the same
//! way: keep the equality rows, the inequality rows at a bound and the
//! variables at a bound, drop the rest, and differentiate the remaining
//! equations
//!
//! ```text
//!   grad f + J_A' lambda_A + I_B' nu_B = 0
//!   g_A(x) - bound_A               = 0
//!   x_B    - bound_B               = 0
//! ```
//!
//! in `(x, lambda_A, nu_B)`. This module evaluates the exact derivatives the
//! compiled program provides ([`NlpDerivatives`]), assembles the symmetric
//! matrix `[[H, J_A', I_B'], [J_A, 0, 0], [I_B, 0, 0]]`, and factors it without
//! regularization ([`KktFactor`]). The multiconductor OPF and the balanced AC
//! OPF each choose their own active set and parameter columns and share
//! everything here.
//!
//! [`KktFactor`] is the linear solver for this matrix rather than
//! [`solve_refined`](super::solve_refined): it refuses a singular or badly
//! conditioned matrix (an active set whose constraint gradients are linearly
//! dependent), where a plain or Tikhonov-shifted LU would return numbers.

// The balanced AC OPF uses the factorization to qualify its matrix and hands
// the entries to the shared sensitivity driver; the back-solves and their
// diagnostics are called by the multiconductor OPF only.
#![cfg_attr(not(feature = "mc-opf"), allow(dead_code))]

use std::collections::BTreeMap;

use faer::{
    linalg::solvers::Solve,
    sparse::{linalg::solvers::Lu, SparseColMat, Triplet},
    Mat,
};
use pounce_nl::nl_reader::{NlProblem, NlTnlp};
use pounce_rs::{SparsityRequest, TNLP};

/// Why the fixed-active-set system could not be built or solved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KktErrorKind {
    /// The compiled program did not return finite derivatives.
    DerivativeEvaluation,
    /// The multipliers of the active set do not satisfy stationarity.
    Stationarity,
    /// The matrix is singular or too badly conditioned to trust.
    Singular,
    /// A right-hand side is malformed or not finite.
    InvalidRhs,
    /// A back-solve failed its residual check after refinement.
    SolveFailure,
}

#[derive(Clone, Debug)]
pub(crate) struct KktError {
    pub(crate) kind: KktErrorKind,
    pub(crate) message: String,
    pub(crate) linear_solve: Option<KktSolveDiagnostics>,
}

impl std::fmt::Display for KktError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

fn error(kind: KktErrorKind, message: impl Into<String>) -> KktError {
    KktError {
        kind,
        message: message.into(),
        linear_solve: None,
    }
}

/// Residuals of one back-solve against the original, unscaled matrix, after
/// any refinement. A backward-error check, not a forward-error bound.
#[derive(Clone, Debug)]
pub(crate) struct KktSolveDiagnostics {
    pub(crate) residual_inf: f64,
    pub(crate) componentwise_backward_error: f64,
    /// Maximum `|residual| / (1e-10 + 1e-8 * (|rhs| + |K| |x|))`. Accepted
    /// solves have a value of at most one.
    pub(crate) residual_tolerance_ratio: f64,
    pub(crate) refinement_steps: usize,
}

/// The constraint values `g(x)` of a compiled program.
#[cfg(feature = "sensitivity")]
pub(crate) fn constraint_values(problem: &NlProblem, x: &[f64]) -> Result<Vec<f64>, KktError> {
    let failed = || {
        error(
            KktErrorKind::DerivativeEvaluation,
            "compiled constraint evaluation failed",
        )
    };
    if x.len() != problem.n {
        return Err(failed());
    }
    let mut t = NlTnlp::try_new(problem.clone())
        .map_err(|e| error(KktErrorKind::DerivativeEvaluation, e))?;
    let mut values = vec![0.; problem.m];
    if !t.eval_g(x, true, &mut values) || values.iter().any(|v| !v.is_finite()) {
        return Err(failed());
    }
    Ok(values)
}

/// Exact first and second derivatives of a compiled program at one point.
pub(crate) struct NlpDerivatives {
    n: usize,
    /// Objective gradient.
    pub(crate) objective_gradient: Vec<f64>,
    /// Constraint Jacobian by row: `(column, value)`.
    pub(crate) jacobian: Vec<Vec<(usize, f64)>>,
    /// Hessian of `f + lambda' g`, one triangle, as the program reports it.
    hessian: Vec<(usize, usize, f64)>,
}

impl NlpDerivatives {
    /// Evaluate at `x`. `lambda` weights the constraint Hessians and must be
    /// zero on every row left out of the active set.
    pub(crate) fn evaluate(
        problem: &NlProblem,
        x: &[f64],
        lambda: &[f64],
    ) -> Result<Self, KktError> {
        let failed = || {
            error(
                KktErrorKind::DerivativeEvaluation,
                "exact compiled derivative evaluation failed",
            )
        };
        if x.len() != problem.n || lambda.len() != problem.m {
            return Err(failed());
        }
        let mut t = NlTnlp::try_new(problem.clone())
            .map_err(|e| error(KktErrorKind::DerivativeEvaluation, e))?;
        let info = t
            .get_nlp_info()
            .ok_or_else(|| error(KktErrorKind::DerivativeEvaluation, "missing sparsity shape"))?;
        let (mut jr, mut jc, mut j) = (
            vec![0; info.nnz_jac_g as usize],
            vec![0; info.nnz_jac_g as usize],
            vec![0.; info.nnz_jac_g as usize],
        );
        let (mut hr, mut hc, mut h) = (
            vec![0; info.nnz_h_lag as usize],
            vec![0; info.nnz_h_lag as usize],
            vec![0.; info.nnz_h_lag as usize],
        );
        let mut objective_gradient = vec![0.; problem.n];
        if !t.eval_grad_f(x, true, &mut objective_gradient)
            || !t.eval_jac_g(
                None,
                false,
                SparsityRequest::Structure {
                    irow: &mut jr,
                    jcol: &mut jc,
                },
            )
            || !t.eval_jac_g(Some(x), true, SparsityRequest::Values { values: &mut j })
            || !t.eval_h(
                None,
                false,
                1.,
                None,
                false,
                SparsityRequest::Structure {
                    irow: &mut hr,
                    jcol: &mut hc,
                },
            )
            || !t.eval_h(
                Some(x),
                true,
                1.,
                Some(lambda),
                true,
                SparsityRequest::Values { values: &mut h },
            )
            || objective_gradient
                .iter()
                .chain(&j)
                .chain(&h)
                .any(|v| !v.is_finite())
        {
            return Err(failed());
        }
        let mut jacobian = vec![Vec::new(); problem.m];
        for ((r, c), v) in jr.into_iter().zip(jc).zip(j) {
            jacobian[r as usize].push((c as usize, v));
        }
        let hessian = hr
            .into_iter()
            .zip(hc)
            .zip(h)
            .map(|((r, c), v)| (r as usize, c as usize, v))
            .collect();
        Ok(Self {
            n: problem.n,
            objective_gradient,
            jacobian,
            hessian,
        })
    }

    /// `grad f + J_A' lambda` per variable, and the sum of `1 + |term|` the
    /// caller scales its tolerance by. `active` maps a compiled row to its
    /// position among the active rows; rows outside it contribute nothing.
    pub(crate) fn stationarity(
        &self,
        lambda: &[f64],
        active: &BTreeMap<usize, usize>,
    ) -> (Vec<f64>, Vec<f64>) {
        let mut residual = self.objective_gradient.clone();
        let mut scale: Vec<_> = self
            .objective_gradient
            .iter()
            .map(|v| 1. + v.abs())
            .collect();
        for &r in active.keys() {
            for &(c, v) in &self.jacobian[r] {
                residual[c] += v * lambda[r];
                scale[c] += (v * lambda[r]).abs();
            }
        }
        (residual, scale)
    }

    /// The symmetric fixed-active-set matrix as `(row, column, value)`
    /// entries, both triangles. Row `n + a` is active row `a`; row
    /// `n + active.len() + i` holds `bound_columns[i]` at its bound.
    pub(crate) fn kkt_entries(
        &self,
        active: &BTreeMap<usize, usize>,
        bound_columns: &[usize],
    ) -> Vec<(usize, usize, f64)> {
        let n = self.n;
        let mut entries = Vec::new();
        for (&r, &a) in active {
            for &(c, v) in &self.jacobian[r] {
                entries.push((n + a, c, v));
                entries.push((c, n + a, v));
            }
        }
        for &(r, c, v) in &self.hessian {
            entries.push((r, c, v));
            if r != c {
                entries.push((c, r, v));
            }
        }
        for (i, &c) in bound_columns.iter().enumerate() {
            let r = n + active.len() + i;
            entries.push((r, c, 1.));
            entries.push((c, r, 1.));
        }
        entries
    }
}

/// Fail when any stationarity residual exceeds `tolerance` times its scale.
pub(crate) fn check_stationarity(
    residual: &[f64],
    scale: &[f64],
    tolerance: f64,
) -> Result<(), KktError> {
    if residual
        .iter()
        .zip(scale)
        .any(|(r, s)| !r.is_finite() || !s.is_finite() || r.abs() > tolerance * s)
    {
        return Err(error(
            KktErrorKind::Stationarity,
            "stationarity failed after removing inactive multipliers",
        ));
    }
    Ok(())
}

/// Sparse LU of a symmetrically equilibrated, unregularized KKT matrix.
pub(crate) struct KktFactor {
    lu: Lu<usize, f64>,
    entries: Vec<(usize, usize, f64)>,
    scale: Vec<f64>,
}

impl KktFactor {
    pub(crate) fn dimension(&self) -> usize {
        self.scale.len()
    }
    pub(crate) fn nonzeros(&self) -> usize {
        self.entries.len()
    }
    /// The combined entries of the original matrix, duplicates summed.
    #[cfg(feature = "sensitivity")]
    pub(crate) fn entries(&self) -> &[(usize, usize, f64)] {
        &self.entries
    }
    pub(crate) fn new(n: usize, entries: Vec<(usize, usize, f64)>) -> Result<Self, KktError> {
        let mut combined = BTreeMap::<(usize, usize), f64>::new();
        for (r, c, v) in entries {
            *combined.entry((r, c)).or_default() += v;
        }
        let entries: Vec<_> = combined
            .into_iter()
            .filter(|(_, v)| *v != 0.)
            .map(|((r, c), v)| (r, c, v))
            .collect();
        if entries
            .iter()
            .any(|&(r, c, v)| r >= n || c >= n || !v.is_finite())
        {
            return Err(error(
                KktErrorKind::DerivativeEvaluation,
                "invalid KKT entries",
            ));
        }
        let mut scale = vec![1.; n];
        for _ in 0..6 {
            let mut largest = vec![0.0_f64; n];
            for &(r, c, v) in &entries {
                largest[r] = largest[r].max((v * scale[r] * scale[c]).abs());
            }
            if largest.iter().any(|&v| v == 0. || !v.is_finite()) {
                return Err(error(
                    KktErrorKind::Singular,
                    "KKT has an empty or nonfinite row",
                ));
            }
            for (d, largest) in scale.iter_mut().zip(largest) {
                *d /= largest.sqrt();
            }
        }
        let triplets: Vec<_> = entries
            .iter()
            .map(|&(r, c, v)| Triplet::new(r, c, v * scale[r] * scale[c]))
            .collect();
        let matrix = SparseColMat::<usize, f64>::try_new_from_triplets(n, n, &triplets)
            .map_err(|e| error(KktErrorKind::Singular, format!("KKT assembly: {e:?}")))?;
        let lu = matrix.sp_lu().map_err(|e| {
            error(
                KktErrorKind::Singular,
                format!("unregularized KKT factorization: {e:?}"),
            )
        })?;
        qualify_inverse(n, &lu, &entries, &scale)?;
        Ok(Self { lu, entries, scale })
    }
    fn backsolve(&self, rhs: &[f64]) -> Vec<f64> {
        let mut b = Mat::from_fn(rhs.len(), 1, |i, _| rhs[i] * self.scale[i]);
        self.lu.solve_in_place(b.as_mut());
        (0..rhs.len()).map(|i| b[(i, 0)] * self.scale[i]).collect()
    }
    #[cfg(test)]
    pub(crate) fn solve(&self, rhs: &[f64]) -> Result<Vec<f64>, KktError> {
        self.solve_with_diagnostics(rhs).map(|(x, _)| x)
    }
    pub(crate) fn solve_with_diagnostics(
        &self,
        rhs: &[f64],
    ) -> Result<(Vec<f64>, KktSolveDiagnostics), KktError> {
        if rhs.len() != self.dimension() || rhs.iter().any(|v| !v.is_finite()) {
            return Err(error(
                KktErrorKind::InvalidRhs,
                "nonfinite or malformed sensitivity right-hand side",
            ));
        }
        let mut x = self.backsolve(rhs);
        let mut last = None;
        for attempt in 0..3 {
            if x.iter().any(|v| !v.is_finite()) {
                break;
            }
            let mut residual = rhs.to_vec();
            let mut magnitude: Vec<_> = rhs.iter().map(|v| v.abs()).collect();
            for &(r, c, v) in &self.entries {
                residual[r] -= v * x[c];
                magnitude[r] += (v * x[c]).abs();
            }
            if residual.iter().chain(&magnitude).any(|v| !v.is_finite()) {
                break;
            }
            let diagnostics = KktSolveDiagnostics {
                residual_inf: residual.iter().map(|r| r.abs()).fold(0., f64::max),
                componentwise_backward_error: residual
                    .iter()
                    .zip(&magnitude)
                    .map(|(r, s)| if *s == 0. { 0. } else { r.abs() / s })
                    .fold(0., f64::max),
                residual_tolerance_ratio: residual
                    .iter()
                    .zip(&magnitude)
                    .map(|(r, s)| r.abs() / (1e-10 + 1e-8 * s))
                    .fold(0., f64::max),
                refinement_steps: attempt,
            };
            if diagnostics.residual_tolerance_ratio <= 1. {
                return Ok((x, diagnostics));
            }
            last = Some(diagnostics);
            if attempt < 2 {
                let correction = self.backsolve(&residual);
                for (x, c) in x.iter_mut().zip(correction) {
                    *x += c;
                }
            }
        }
        let mut error = error(
            KktErrorKind::SolveFailure,
            "unregularized KKT backsolve failed its componentwise residual check",
        );
        error.linear_solve = last;
        Err(error)
    }
}

/// Check every inverse column, with O(n + nnz) workspace. Fixed dense
/// probes can all be orthogonal to a null direction, and a componentwise
/// backward error can be tiny for an enormous, meaningless solution.
///
/// For the equilibrated matrix B and computed inverse X, ||I-BX||_1 < 1
/// implies nonsingularity and ||B^-1||_1 <= ||X||_1/(1-||I-BX||_1).
/// Allow at most 1/2 residual and a 1e12 condition bound. Include a floating
/// point accumulation allowance; this is a numerical check, not interval
/// arithmetic. Costs n sparse backsolves once per factorization, never per
/// query.
fn qualify_inverse(
    n: usize,
    lu: &Lu<usize, f64>,
    entries: &[(usize, usize, f64)],
    scale: &[f64],
) -> Result<(), KktError> {
    let refused = || {
        error(
            KktErrorKind::Singular,
            "singular or numerically unstable equilibrated KKT inverse",
        )
    };
    if n == 0 {
        return Err(refused());
    }
    let scaled: Vec<_> = entries
        .iter()
        .map(|&(r, c, v)| (r, c, v * scale[r] * scale[c]))
        .collect();
    let mut column_sums = vec![0.; n];
    for &(_, c, v) in &scaled {
        column_sums[c] += v.abs();
    }
    // At most n products/subtractions in each residual row and n terms in
    // each norm sum. Use a conservative combined rounding allowance.
    let roundoff = (3. * n as f64 + 4.) * f64::EPSILON;
    if roundoff >= 0.5 {
        return Err(refused());
    }
    let inflation = 1. / (1. - roundoff);
    let gamma = roundoff * inflation;
    let matrix_norm = column_sums.into_iter().fold(0., f64::max) * inflation;
    let mut inverse_norm = 0.0_f64;
    let mut residual_norm = 0.0_f64;
    let mut x = Mat::zeros(n, 1);
    let mut residual = vec![0.; n];
    let mut magnitude = vec![0.; n];
    for column in 0..n {
        x.fill(0.);
        x[(column, 0)] = 1.;
        lu.solve_in_place(x.as_mut());
        let norm = (0..n).map(|i| x[(i, 0)].abs()).sum::<f64>() * inflation;
        if !norm.is_finite() {
            return Err(refused());
        }
        inverse_norm = inverse_norm.max(norm);
        residual.fill(0.);
        magnitude.fill(0.);
        residual[column] = 1.;
        magnitude[column] = 1.;
        for &(r, c, v) in &scaled {
            let product = v * x[(c, 0)];
            residual[r] -= product;
            magnitude[r] += product.abs();
        }
        let bound = residual
            .iter()
            .zip(&magnitude)
            .map(|(r, m)| r.abs() + gamma * m)
            .sum::<f64>()
            * inflation;
        if !bound.is_finite() || bound >= 0.5 {
            return Err(refused());
        }
        residual_norm = residual_norm.max(bound);
        let condition_bound = matrix_norm * inverse_norm / (1. - residual_norm);
        if !condition_bound.is_finite() || condition_bound > 1e12 {
            return Err(refused());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    type Entries = Vec<(usize, usize, f64)>;

    fn saddle(n: usize, shift: f64, reordered: bool) -> (Entries, Vec<f64>, Vec<f64>) {
        // H=nI-vv^T, A=ones: A has full row rank but [v;0] is
        // an exact null vector when shift=0. Both old dense probes are
        // orthogonal to it. Adding shift*I gives x=v/shift for rhs=[v;0].
        let v: Vec<_> = (0..n).map(|i| [1., -1., -1., 1.][i % 4]).collect();
        let index = |i| if reordered { n - i } else { i };
        let unit = |i: usize| {
            if reordered {
                [1e-3, 2., 5e2, 0.2, 10.][i % 5]
            } else {
                1.
            }
        };
        let mut entries = Vec::new();
        let mut rhs = vec![0.; n + 1];
        let mut solution = vec![0.; n + 1];
        for i in 0..n {
            rhs[index(i)] = v[i] * unit(i);
            solution[index(i)] = v[i] / shift / unit(i);
            for j in 0..n {
                let value = if i == j { n as f64 + shift } else { 0. } - v[i] * v[j];
                entries.push((index(i), index(j), value * unit(i) * unit(j)));
            }
            entries.push((index(i), index(n), unit(i) * unit(n)));
            entries.push((index(n), index(i), unit(i) * unit(n)));
        }
        (entries, rhs, solution)
    }

    #[test]
    fn inverse_qualification_rejects_null_directions_missed_by_dense_probes() {
        for n in [4, 8, 12] {
            for reordered in [false, true] {
                for shift in [0., 1e-14] {
                    let (entries, _, _) = saddle(n, shift, reordered);
                    assert!(
                        matches!(KktFactor::new(n + 1, entries), Err(e) if e.kind == KktErrorKind::Singular)
                    );
                }
            }
        }
    }

    #[test]
    fn inverse_qualification_preserves_regular_indefinite_kkt_solutions() {
        for n in [4, 8, 12] {
            for reordered in [false, true] {
                for shift in [-1e-4, 1e-4, 0.25] {
                    let (entries, rhs, expected) = saddle(n, shift, reordered);
                    let factor = KktFactor::new(n + 1, entries).unwrap();
                    let actual = factor.solve(&rhs).unwrap();
                    for (x, reference) in actual.iter().zip(expected) {
                        assert!(
                            (x - reference).abs() <= 1e-6 * (1. + reference.abs()),
                            "{x} != {reference}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn inverse_qualification_checks_residual_as_well_as_inverse_size() {
        let matrix = SparseColMat::<usize, f64>::try_new_from_triplets(
            2,
            2,
            &[Triplet::new(0, 0, 1.), Triplet::new(1, 1, 1.)],
        )
        .unwrap();
        let lu = matrix.sp_lu().unwrap();
        // Finite, small solves from the wrong factors must not pass. Inverse
        // norms or pivot sizes alone cannot establish this check.
        let error = qualify_inverse(2, &lu, &[(0, 0, 2.), (1, 1, 1.)], &[1., 1.]).unwrap_err();
        assert_eq!(error.kind, KktErrorKind::Singular);
    }

    #[test]
    fn sparse_kkt_backsolve_rejects_singular_and_unstable_systems() {
        for entries in [
            vec![(0, 0, 1.)],
            vec![(0, 0, 1.), (0, 1, 1.), (1, 0, 1.), (1, 1, 1.)],
            vec![(0, 0, 1.), (0, 1, 1.), (1, 0, 1.), (1, 1, 1. + 1e-15)],
        ] {
            assert!(
                matches!(KktFactor::new(2, entries), Err(e) if e.kind == KktErrorKind::Singular)
            );
        }
        // min x^2 subject to x=theta: [2 1;1 0] [x';lambda']=[0;1].
        let f = KktFactor::new(2, vec![(0, 0, 2.), (0, 1, 1.), (1, 0, 1.)]).unwrap();
        let x = f.solve(&[0., 1.]).unwrap();
        assert!((x[0] - 1.).abs() < 1e-12 && (x[1] + 2.).abs() < 1e-12);
        assert!(f.solve(&[f64::NAN, 1.]).is_err());
    }

    #[test]
    fn backsolve_diagnostics_measure_the_original_matrix_and_report_failed_refinement() {
        let entries = vec![(0, 0, 2e6), (0, 1, 3.), (1, 0, 3.), (1, 1, -4e-6)];
        let f = KktFactor::new(2, entries.clone()).unwrap();
        let rhs = [1., -2.];
        let (x, d) = f.solve_with_diagnostics(&rhs).unwrap();
        let mut residual = rhs;
        let mut magnitude = rhs.map(f64::abs);
        for (r, c, v) in entries {
            residual[r] -= v * x[c];
            magnitude[r] += (v * x[c]).abs();
        }
        assert_eq!(
            d.residual_inf,
            residual.into_iter().map(f64::abs).fold(0., f64::max)
        );
        assert_eq!(
            d.componentwise_backward_error,
            (0..2)
                .map(|i| residual[i].abs() / magnitude[i])
                .fold(0., f64::max)
        );
        assert!(d.residual_tolerance_ratio <= 1.);
        // Simulate inaccurate factor solves: refinement must be checked against
        // the retained matrix and refused rather than returning an unchecked x.
        let mut f = KktFactor::new(1, vec![(0, 0, 1.)]).unwrap();
        f.entries[0].2 = 2.;
        let error = f.solve_with_diagnostics(&[1.]).unwrap_err();
        assert_eq!(error.kind, KktErrorKind::SolveFailure);
        let d = error.linear_solve.unwrap();
        assert_eq!(d.refinement_steps, 2);
        assert!(d.residual_tolerance_ratio > 1.);
        assert!(d.residual_inf.is_finite());
    }
}
