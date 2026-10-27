//! The generator's error surface. A plain enum with a `Display` so the wasm
//! edge maps it to a string and the CLI prints it; no `thiserror` dependency.

use std::fmt;

/// Why generation stopped. Every variant is a caller-visible condition, not a
/// panic: a malformed request, an unusable region, unreadable places data, an
/// Overpass document the OSM path cannot turn into a grid, an engine solve that
/// refused the network, or a session driven out of order.
#[derive(Clone, Debug, PartialEq)]
pub enum SynthError {
    /// The request failed validation (bounds, enums, geometry shape).
    InvalidRequest(String),
    /// The region geometry parsed but no bus can be placed in it.
    Region(String),
    /// The populated places table could not be read.
    Places(String),
    /// The Overpass document is malformed.
    Osm(String),
    /// The Overpass document parsed but holds too little transmission data to
    /// build a grid. The caller decides whether to rerun as `Source::Synthetic`.
    OsmInsufficient {
        buses: usize,
        branches: usize,
        message: String,
    },
    /// Assembling the PowerIO network failed its own validation.
    Build(String),
    /// The tellegen engine refused to build a problem instance. A solve that
    /// ran and reported infeasibility is a report outcome, not this error.
    Engine(String),
    /// The session was driven out of order (`step` after `Done`, `into_parts`
    /// before `Done`).
    State(String),
    /// The host cancelled the run between stages or inside a solve.
    Cancelled,
}

impl fmt::Display for SynthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SynthError::InvalidRequest(m) => write!(f, "invalid request: {m}"),
            SynthError::Region(m) => write!(f, "unusable region: {m}"),
            SynthError::Places(m) => write!(f, "populated places: {m}"),
            SynthError::Osm(m) => write!(f, "Overpass document: {m}"),
            SynthError::OsmInsufficient {
                buses,
                branches,
                message,
            } => write!(
                f,
                "OpenStreetMap data is insufficient ({buses} bus(es), {branches} branch(es)): {message}"
            ),
            SynthError::Build(m) => write!(f, "network build: {m}"),
            SynthError::Engine(m) => write!(f, "engine: {m}"),
            SynthError::State(m) => write!(f, "session: {m}"),
            SynthError::Cancelled => write!(f, "generation cancelled"),
        }
    }
}

impl std::error::Error for SynthError {}

impl SynthError {
    /// The machine readable code the wasm edge and the UI switch on.
    pub fn code(&self) -> &'static str {
        match self {
            SynthError::InvalidRequest(_) => "invalid_request",
            SynthError::Region(_) => "region",
            SynthError::Places(_) => "places",
            SynthError::Osm(_) => "osm",
            SynthError::OsmInsufficient { .. } => "osm_insufficient",
            SynthError::Build(_) => "build",
            SynthError::Engine(_) => "engine",
            SynthError::State(_) => "state",
            SynthError::Cancelled => "cancelled",
        }
    }
}

/// The error as the wasm edge and the CLI serialize it.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ErrorPayload {
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub buses: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branches: Option<usize>,
}

impl From<&SynthError> for ErrorPayload {
    fn from(error: &SynthError) -> Self {
        let (buses, branches) = match error {
            SynthError::OsmInsufficient {
                buses, branches, ..
            } => (Some(*buses), Some(*branches)),
            _ => (None, None),
        };
        Self {
            code: error.code().to_owned(),
            message: error.to_string(),
            buses,
            branches,
        }
    }
}
