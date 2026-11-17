//! Machine-readable error codes for errors that leave the engine.
//!
//! Most of the engine reports failures as plain message strings. The solve
//! entry points in `api.rs` attach an [`ErrorCode`] where each error is
//! created or returned, so a caller can branch on the class of failure without
//! reading the message. The message text is unchanged: an [`EngineError`]
//! displays exactly the string the older entry points return.

use serde::{Deserialize, Serialize};

/// The class of an engine failure. The wire name of each code is its
/// `snake_case` spelling (see [`ErrorCode::as_str`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ErrorCode {
    /// The case or module data is malformed or inconsistent.
    InvalidInput,
    /// The request is malformed: unreadable JSON, an unknown field, or a
    /// value that is not a usable number or id.
    InvalidRequest,
    /// The request names a bus, branch, or generator the case does not have.
    UnknownElement,
    /// An edit or option value is outside the range the model allows, such as
    /// an edit that would make demand negative.
    OutOfRange,
    /// The request is well formed, but this formulation, entry point, or
    /// build does not support it.
    Unsupported,
    /// The solver proved the problem has no feasible point.
    Infeasible,
    /// The solver found the objective unbounded.
    Unbounded,
    /// The solver stopped without reaching a solution.
    NotConverged,
    /// The caller cancelled the calculation.
    Cancelled,
    /// The engine failed in a way the caller could not have caused, including
    /// a panic caught at a boundary.
    Internal,
}

impl ErrorCode {
    /// Every code, in declaration order.
    pub const ALL: [ErrorCode; 10] = [
        Self::InvalidInput,
        Self::InvalidRequest,
        Self::UnknownElement,
        Self::OutOfRange,
        Self::Unsupported,
        Self::Infeasible,
        Self::Unbounded,
        Self::NotConverged,
        Self::Cancelled,
        Self::Internal,
    ];

    /// The wire name, such as `invalid_input` or `not_converged`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::InvalidRequest => "invalid_request",
            Self::UnknownElement => "unknown_element",
            Self::OutOfRange => "out_of_range",
            Self::Unsupported => "unsupported",
            Self::Infeasible => "infeasible",
            Self::Unbounded => "unbounded",
            Self::NotConverged => "not_converged",
            Self::Cancelled => "cancelled",
            Self::Internal => "internal",
        }
    }

    /// Classify a plain message from an entry point that does not return an
    /// [`EngineError`] yet (Study, multiconductor, RMS, and planning calls).
    ///
    /// This reads the fixed phrases the engine itself writes, and returns
    /// `None` for anything else. It exists so adapters do not each keep their
    /// own text matching; prefer the entry points that return an
    /// [`EngineError`], whose code is set where the error is created.
    pub fn from_message(message: &str) -> Option<ErrorCode> {
        const PHRASES: &[(&str, ErrorCode)] = &[
            ("solve infeasible", ErrorCode::Infeasible),
            ("solve unbounded", ErrorCode::Unbounded),
            ("did not converge", ErrorCode::NotConverged),
            ("cancelled", ErrorCode::Cancelled),
            ("bad request JSON", ErrorCode::InvalidRequest),
            ("unknown field", ErrorCode::InvalidRequest),
            ("unknown demand delta bus", ErrorCode::UnknownElement),
            ("unknown rating delta branch", ErrorCode::UnknownElement),
            ("would make demand negative", ErrorCode::OutOfRange),
            (
                "would make the line limit non-positive",
                ErrorCode::OutOfRange,
            ),
            ("does not support d(", ErrorCode::Unsupported),
            ("is not available in this build", ErrorCode::Unsupported),
            ("is unavailable in this build", ErrorCode::Unsupported),
            ("requires the `", ErrorCode::Unsupported),
            (
                "which this solve entry does not support",
                ErrorCode::Unsupported,
            ),
            ("are not supported by", ErrorCode::Unsupported),
            ("supported only by", ErrorCode::Unsupported),
            ("only supported for", ErrorCode::Unsupported),
            ("applies only to", ErrorCode::Unsupported),
            ("apply only to", ErrorCode::Unsupported),
        ];
        PHRASES
            .iter()
            .find(|(phrase, _)| message.contains(phrase))
            .map(|&(_, code)| code)
    }
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// An engine error: a stable [`ErrorCode`] and the message for people.
///
/// `Display` writes the message alone, so converting to a `String` gives the
/// same text the string-returning entry points produce.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
pub struct EngineError {
    pub code: ErrorCode,
    pub message: String,
}

impl EngineError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    /// The same code with the message rewritten, for a caller that adds
    /// context such as the round a solve failed in.
    pub(crate) fn map_message(self, f: impl FnOnce(&str) -> String) -> Self {
        Self {
            code: self.code,
            message: f(&self.message),
        }
    }
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for EngineError {}

impl From<EngineError> for String {
    fn from(error: EngineError) -> Self {
        error.message
    }
}

/// Attach a code to a plain error where it is returned.
pub(crate) trait WithCode<T> {
    fn code(self, code: ErrorCode) -> Result<T, EngineError>;
}

impl<T, E: std::fmt::Display> WithCode<T> for Result<T, E> {
    fn code(self, code: ErrorCode) -> Result<T, EngineError> {
        self.map_err(|error| EngineError::new(code, error.to_string()))
    }
}

/// Shorthand constructors used where the engine creates an error.
pub(crate) fn coded(code: ErrorCode, message: impl Into<String>) -> EngineError {
    EngineError::new(code, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_names_match_serde_and_are_distinct() {
        let mut seen = std::collections::HashSet::new();
        for code in ErrorCode::ALL {
            assert_eq!(
                serde_json::to_value(code).unwrap(),
                serde_json::json!(code.as_str())
            );
            assert_eq!(code.to_string(), code.as_str());
            assert!(seen.insert(code.as_str()));
        }
    }

    #[test]
    fn an_engine_error_displays_as_its_message_alone() {
        let error = EngineError::new(ErrorCode::Infeasible, "DC OPF solve infeasible: X");
        assert_eq!(error.to_string(), "DC OPF solve infeasible: X");
        assert_eq!(String::from(error.clone()), "DC OPF solve infeasible: X");
        assert_eq!(
            serde_json::to_value(&error).unwrap(),
            serde_json::json!({"code": "infeasible", "message": "DC OPF solve infeasible: X"})
        );
    }

    #[test]
    fn plain_messages_classify_by_the_engines_fixed_phrases() {
        let cases = [
            (
                "DC OPF solve infeasible: PrimalInfeasible",
                ErrorCode::Infeasible,
            ),
            (
                "DC OPF solve unbounded: DualInfeasible",
                ErrorCode::Unbounded,
            ),
            (
                "DC OPF solve did not converge: MaxIterations",
                ErrorCode::NotConverged,
            ),
            ("calculation cancelled", ErrorCode::Cancelled),
            (
                "bad request JSON: trailing comma",
                ErrorCode::InvalidRequest,
            ),
            ("unknown demand delta bus 42", ErrorCode::UnknownElement),
            ("unknown rating delta branch 7", ErrorCode::UnknownElement),
            (
                "demand delta for bus 2 would make demand negative",
                ErrorCode::OutOfRange,
            ),
            (
                "branch rating edits are not supported by dcpf",
                ErrorCode::Unsupported,
            ),
            (
                "acpf does not support d(Price(Active))/d(Demand(Active))",
                ErrorCode::Unsupported,
            ),
            ("socwr requires the `conic` feature", ErrorCode::Unsupported),
        ];
        for (message, code) in cases {
            assert_eq!(ErrorCode::from_message(message), Some(code), "{message}");
        }
        assert_eq!(ErrorCode::from_message("something new went wrong"), None);
    }
}
