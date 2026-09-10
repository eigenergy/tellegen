//! Retained complex sparse linear solves used by the multiconductor PF.

use num_complex::Complex64;

type SolveFn = dyn Fn(&[Complex64]) -> Result<Vec<Complex64>, String>;

/// A numeric factorization retained for all initial and fixed-point solves.
///
/// The factor itself is kept inside a closure because faer's sparse-LU type is
/// intentionally not part of Tellegen's public API.  The matrix is assembled
/// once and the closure only receives new right hand sides.
pub(crate) struct RetainedComplexLu {
    dim: usize,
    nonzeros: usize,
    factorization_count: usize,
    solve: Box<SolveFn>,
}

impl std::fmt::Debug for RetainedComplexLu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RetainedComplexLu")
            .field("dim", &self.dim)
            .field("nonzeros", &self.nonzeros)
            .field("factorization_count", &self.factorization_count)
            .finish_non_exhaustive()
    }
}

impl RetainedComplexLu {
    pub(crate) fn factor(
        dim: usize,
        triplets: &[(usize, usize, Complex64)],
    ) -> Result<Self, String> {
        if dim == 0 {
            return Err("the unknown terminal system has zero dimension".to_owned());
        }
        if triplets
            .iter()
            .any(|(_, _, z)| !z.re.is_finite() || !z.im.is_finite())
        {
            return Err("the global admittance contains a non-finite entry".to_owned());
        }
        let entries: Vec<faer::sparse::Triplet<usize, usize, Complex64>> = triplets
            .iter()
            .map(|&(r, c, z)| faer::sparse::Triplet::new(r, c, z))
            .collect();
        let matrix = faer::sparse::SparseColMat::<usize, Complex64>::try_new_from_triplets(
            dim, dim, &entries,
        )
        .map_err(|e| format!("complex sparse assembly failed: {e:?}"))?;
        let lu = matrix
            .sp_lu()
            .map_err(|e| format!("complex sparse LU failed: {e:?}"))?;
        let solve = move |rhs: &[Complex64]| {
            if rhs.len() != dim {
                return Err(format!("RHS has {} entries; expected {dim}", rhs.len()));
            }
            if rhs.iter().any(|z| !z.re.is_finite() || !z.im.is_finite()) {
                return Err("linear solve RHS contains a non-finite entry".to_owned());
            }
            let mut x = faer::Mat::<Complex64>::zeros(dim, 1);
            for (i, value) in rhs.iter().enumerate() {
                x[(i, 0)] = *value;
            }
            faer::linalg::solvers::Solve::solve_in_place(&lu, x.as_mut());
            let result: Vec<_> = (0..dim).map(|i| x[(i, 0)]).collect();
            if result
                .iter()
                .any(|z| !z.re.is_finite() || !z.im.is_finite())
            {
                return Err("complex sparse LU returned a non-finite solution".to_owned());
            }
            Ok(result)
        };
        Ok(Self {
            dim,
            nonzeros: triplets.len(),
            factorization_count: 1,
            solve: Box::new(solve),
        })
    }

    pub(crate) fn dim(&self) -> usize {
        self.dim
    }
    pub(crate) fn nonzeros(&self) -> usize {
        self.nonzeros
    }
    pub(crate) fn factorization_count(&self) -> usize {
        self.factorization_count
    }
    pub(crate) fn solve(&self, rhs: &[Complex64]) -> Result<Vec<Complex64>, String> {
        (self.solve)(rhs)
    }
}
