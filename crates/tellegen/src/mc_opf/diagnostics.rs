//! Machine-readable failures. Messages are for people, never dispatch keys.
use super::McOpfResiduals;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum McOpfErrorKind {
    InvalidInput,
    UnsupportedInput,
    Cancelled,
    NumericalFailure,
    PhysicalValidationFailure,
    SerializationFailure,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum McOpfStage {
    Input,
    Preparation,
    Compilation,
    Solver,
    Validation,
    Import,
    Export,
}

/// A portable projection of an upstream finding; module export also preserves
/// the original PowerIO diagnostic records, including source spans.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct McOpfDiagnostic {
    pub code: String,
    pub severity: powerio::DiagnosticSeverity,
    pub message: String,
    pub target: Option<String>,
}
impl From<&powerio::Diagnostic> for McOpfDiagnostic {
    fn from(d: &powerio::Diagnostic) -> Self {
        Self {
            code: d.code().into(),
            severity: d.severity(),
            message: d.message().into(),
            target: d.target().map(str::to_owned),
        }
    }
}

/// The worst failing witness in a residual family. `None` means no finite
/// residual can be computed; JSON never disguises NaN or infinity as a number.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct McOpfViolation {
    pub quantity: String,
    pub component: String,
    pub axis: Option<String>,
    pub residual: Option<f64>,
}

/// A failed solve never contains a solution. In particular, numerical failure
/// and unsupported input do not assert that the electrical problem is infeasible.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct McOpfError {
    pub kind: McOpfErrorKind,
    pub stage: McOpfStage,
    pub message: String,
    pub diagnostics: Vec<McOpfDiagnostic>,
    pub solver_status: Option<String>,
    pub residuals: Option<Box<McOpfResiduals>>,
    pub violations: Vec<McOpfViolation>,
}
impl McOpfError {
    pub fn new(kind: McOpfErrorKind, stage: McOpfStage, message: impl Into<String>) -> Self {
        Self {
            kind,
            stage,
            message: message.into(),
            diagnostics: Vec::new(),
            solver_status: None,
            residuals: None,
            violations: Vec::new(),
        }
    }

    pub fn from_powerio(stage: McOpfStage, error: powerio::Error) -> Self {
        let kind = if matches!(stage, McOpfStage::Export) {
            McOpfErrorKind::SerializationFailure
        } else if error
            .diagnostics()
            .iter()
            .any(|d| d.code().contains("UNSUPPORTED"))
        {
            McOpfErrorKind::UnsupportedInput
        } else {
            McOpfErrorKind::InvalidInput
        };
        let mut out = Self::new(kind, stage, error.to_string());
        out.diagnostics = error.diagnostics().iter().map(Into::into).collect();
        out
    }
}
impl std::fmt::Display for McOpfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}
impl std::error::Error for McOpfError {}
