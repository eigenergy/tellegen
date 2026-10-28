//! Sparse LU of a symmetrically equilibrated, unregularized KKT matrix.
use super::{error, ErrorKind, Result};
use faer::{
    linalg::solvers::Solve,
    sparse::{linalg::solvers::Lu, SparseColMat, Triplet},
    Mat,
};
use std::collections::BTreeMap;

pub(super) struct Factor {
    lu: Lu<usize, f64>,
    entries: Vec<(usize, usize, f64)>,
    scale: Vec<f64>,
}

impl Factor {
    pub fn dimension(&self) -> usize {
        self.scale.len()
    }
    pub fn new(n: usize, entries: Vec<(usize, usize, f64)>) -> Result<Self> {
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
                ErrorKind::DerivativeEvaluation,
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
                    ErrorKind::SingularKkt,
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
            .map_err(|e| error(ErrorKind::SingularKkt, format!("KKT assembly: {e:?}")))?;
        let lu = matrix.sp_lu().map_err(|e| {
            error(
                ErrorKind::SingularKkt,
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
    pub fn solve(&self, rhs: &[f64]) -> Result<Vec<f64>> {
        if rhs.len() != self.dimension() || rhs.iter().any(|v| !v.is_finite()) {
            return Err(error(
                ErrorKind::InvalidRequest,
                "nonfinite or malformed sensitivity right-hand side",
            ));
        }
        let mut x = self.backsolve(rhs);
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
            if residual
                .iter()
                .zip(&magnitude)
                .all(|(r, s)| r.is_finite() && s.is_finite() && r.abs() <= 1e-10 + 1e-8 * s)
            {
                return Ok(x);
            }
            if attempt < 2 {
                let correction = self.backsolve(&residual);
                for (x, c) in x.iter_mut().zip(correction) {
                    *x += c;
                }
            }
        }
        Err(error(
            ErrorKind::LinearSolveFailure,
            "unregularized KKT backsolve failed its componentwise residual check",
        ))
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
/// arithmetic. Costs n sparse backsolves once per view, never per query.
fn qualify_inverse(
    n: usize,
    lu: &Lu<usize, f64>,
    entries: &[(usize, usize, f64)],
    scale: &[f64],
) -> Result<()> {
    let refused = || {
        error(
            ErrorKind::SingularKkt,
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
                        matches!(Factor::new(n + 1, entries), Err(e) if e.kind == ErrorKind::SingularKkt)
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
                    let factor = Factor::new(n + 1, entries).unwrap();
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
        assert_eq!(error.kind, ErrorKind::SingularKkt);
    }

    #[test]
    fn sparse_kkt_backsolve_rejects_singular_and_unstable_systems() {
        for entries in [
            vec![(0, 0, 1.)],
            vec![(0, 0, 1.), (0, 1, 1.), (1, 0, 1.), (1, 1, 1.)],
            vec![(0, 0, 1.), (0, 1, 1.), (1, 0, 1.), (1, 1, 1. + 1e-15)],
        ] {
            assert!(matches!(Factor::new(2,entries),Err(e) if e.kind==ErrorKind::SingularKkt));
        }
        // min x^2 subject to x=theta: [2 1;1 0] [x';lambda']=[0;1].
        let f = Factor::new(2, vec![(0, 0, 2.), (0, 1, 1.), (1, 0, 1.)]).unwrap();
        let x = f.solve(&[0., 1.]).unwrap();
        assert!((x[0] - 1.).abs() < 1e-12 && (x[1] + 2.).abs() < 1e-12);
        assert!(f.solve(&[f64::NAN, 1.]).is_err());
    }
}
