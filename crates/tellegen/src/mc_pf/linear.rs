//! Retained complex sparse linear solves used by the multiconductor PF.

use std::cell::RefCell;

use num_complex::Complex64;

type SparseLu = faer::sparse::linalg::solvers::Lu<usize, Complex64>;

/// A numeric factorization retained for all initial and fixed-point solves.
///
/// faer's sparse-LU type stays private to this module. The matrix is
/// assembled once; each solve copies its right hand side into one retained
/// column workspace, so repeated solves allocate nothing on this side and
/// keep faer's aligned column layout (and therefore its exact arithmetic).
pub(crate) struct RetainedComplexLu {
    dim: usize,
    nonzeros: usize,
    factorization_count: usize,
    lu: SparseLu,
    workspace: RefCell<faer::Mat<Complex64>>,
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
        Ok(Self {
            dim,
            nonzeros: triplets.len(),
            factorization_count: 1,
            lu,
            workspace: RefCell::new(faer::Mat::<Complex64>::zeros(dim, 1)),
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

    /// Solve into a caller-owned buffer of the system dimension.
    pub(crate) fn solve_into(
        &self,
        rhs: &[Complex64],
        out: &mut [Complex64],
    ) -> Result<(), String> {
        let dim = self.dim;
        if rhs.len() != dim {
            return Err(format!("RHS has {} entries; expected {dim}", rhs.len()));
        }
        if out.len() != dim {
            return Err(format!(
                "solution buffer has {} entries; expected {dim}",
                out.len()
            ));
        }
        if rhs.iter().any(|z| !z.re.is_finite() || !z.im.is_finite()) {
            return Err("linear solve RHS contains a non-finite entry".to_owned());
        }
        let mut x = self.workspace.borrow_mut();
        for (i, value) in rhs.iter().enumerate() {
            x[(i, 0)] = *value;
        }
        faer::linalg::solvers::Solve::solve_in_place(&self.lu, x.as_mut());
        for (i, value) in out.iter_mut().enumerate() {
            *value = x[(i, 0)];
        }
        if out.iter().any(|z| !z.re.is_finite() || !z.im.is_finite()) {
            return Err("complex sparse LU returned a non-finite solution".to_owned());
        }
        Ok(())
    }
}
