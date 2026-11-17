//! The multiconductor view of the shared KKT factorization in
//! [`crate::sens::nlp_kkt`], reporting failures as multiconductor errors.
use super::{error, ErrorKind, McOpfLinearSolveDiagnostics, McOpfSensitivityError, Result};
use crate::sens::nlp_kkt::{KktError, KktErrorKind, KktFactor, KktSolveDiagnostics};

fn diagnostics(d: KktSolveDiagnostics) -> McOpfLinearSolveDiagnostics {
    McOpfLinearSolveDiagnostics {
        residual_inf: d.residual_inf,
        componentwise_backward_error: d.componentwise_backward_error,
        residual_tolerance_ratio: d.residual_tolerance_ratio,
        refinement_steps: d.refinement_steps,
    }
}

/// Stationarity failures are reported by the caller, which owns the dual
/// status and rank report that accompany them.
pub(super) fn convert(e: KktError) -> McOpfSensitivityError {
    let kind = match e.kind {
        KktErrorKind::DerivativeEvaluation => ErrorKind::DerivativeEvaluation,
        KktErrorKind::Stationarity => ErrorKind::UnreliableDuals,
        KktErrorKind::Singular => ErrorKind::SingularKkt,
        KktErrorKind::InvalidRhs => ErrorKind::InvalidRequest,
        KktErrorKind::SolveFailure => ErrorKind::LinearSolveFailure,
    };
    let mut converted = error(kind, e.message);
    converted.linear_solve = e.linear_solve.map(diagnostics);
    converted
}

pub(super) struct Factor(KktFactor);

impl Factor {
    pub fn dimension(&self) -> usize {
        self.0.dimension()
    }
    pub fn nonzeros(&self) -> usize {
        self.0.nonzeros()
    }
    pub fn new(n: usize, entries: Vec<(usize, usize, f64)>) -> Result<Self> {
        KktFactor::new(n, entries).map(Self).map_err(convert)
    }
    #[cfg(test)]
    pub fn solve(&self, rhs: &[f64]) -> Result<Vec<f64>> {
        self.solve_with_diagnostics(rhs).map(|(x, _)| x)
    }
    pub fn solve_with_diagnostics(
        &self,
        rhs: &[f64],
    ) -> Result<(Vec<f64>, McOpfLinearSolveDiagnostics)> {
        self.0
            .solve_with_diagnostics(rhs)
            .map(|(x, d)| (x, diagnostics(d)))
            .map_err(convert)
    }
}
