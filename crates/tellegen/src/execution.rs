//! Execution choices kept outside portable power-system declarations.
use serde::{Deserialize, Serialize};

/// Solver used for DC OPF. Other formulations keep their own solver.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DcSolver {
    /// Pure Rust Clarabel interior-point solver.
    #[default]
    Clarabel,
    /// Pure Rust Moreau interior-point solver with QDLDL.
    Moreau,
}

/// Derivative policy for DC OPF.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DcDerivatives {
    /// Tellegen's specialized KKT system.
    #[default]
    Tellegen,
    /// Moreau for demand/rating columns and weighted rating gradients;
    /// Tellegen for other derivative operations.
    MoreauSelected,
}

/// Independent solver and derivative choices retained by a live engine operation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExecutionOptions {
    /// DC OPF optimization implementation.
    pub dc_solver: DcSolver,
    /// DC OPF sensitivity policy.
    pub dc_derivatives: DcDerivatives,
}

impl ExecutionOptions {
    /// Reject choices unavailable in the compiled engine.
    pub fn validate(&self) -> Result<(), String> {
        if !cfg!(feature = "moreau")
            && (self.dc_solver == DcSolver::Moreau
                || self.dc_derivatives == DcDerivatives::MoreauSelected)
        {
            return Err("Moreau requires the `moreau` feature".into());
        }
        if !cfg!(feature = "sensitivity") && self.dc_derivatives == DcDerivatives::MoreauSelected {
            return Err("Moreau derivatives require the `sensitivity` feature".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_backend_availability_and_defaults() {
        assert_eq!(
            serde_json::from_str::<ExecutionOptions>("{}").unwrap(),
            ExecutionOptions::default()
        );
        assert!(serde_json::from_str::<ExecutionOptions>(r#"{"dc_solver":"unknown"}"#).is_err());
        assert!(serde_json::from_str::<ExecutionOptions>(r#"{"solver":"moreau"}"#).is_err());
        let moreau = ExecutionOptions {
            dc_solver: DcSolver::Moreau,
            ..Default::default()
        };
        assert_eq!(moreau.validate().is_ok(), cfg!(feature = "moreau"));
    }
}
