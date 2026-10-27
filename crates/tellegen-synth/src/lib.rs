//! Geo-referenced synthetic transmission grid generation.
//!
//! Given a region (GeoJSON polygon), a seed and a handful of planning
//! parameters, the generator sites buses on populated places and blue-noise
//! fill, wires them with a minimum spanning tree plus a Gabriel proximity
//! mesh, assigns line and transformer parameters from cited class tables,
//! allocates population weighted loads and reserve-margin generation, and
//! hands the result to the tellegen engine for a DC optimal power flow and an
//! AC power flow. A recorded calibration loop then adjusts a declared parameter
//! set against a declared objective, re-solving exactly after every step.
//! Before any validation, a corridor plan sizes the circuits of every
//! corridor to the flows of the peak economic dispatch and a reactive plan
//! balances every bus, so a generated grid is feasible by construction
//! rather than by repair.
//!
//! The product is a PowerIO [`BalancedNetwork`](powerio::BalancedNetwork)
//! module carrying bus locations and branch routes, plus a
//! [`GenerationReport`] that states every verdict as the engine returned it.
//! The library has no clock, no filesystem access and no platform randomness,
//! so the same request reproduces the same grid byte for byte on one build and
//! platform. Native and WebAssembly values can differ in the last bits, since
//! the platform math libraries round `ln`, `exp`, `sin` and `cos` differently.

pub mod build;
pub mod calibrate;
pub mod context;
pub mod demand;
pub mod electrical;
pub mod error;
pub mod export;
pub mod geom;
pub mod osm;
pub mod places;
pub mod report;
pub mod request;
pub mod rng;
pub mod session;
pub mod site;
pub mod stats;
pub mod topology;
pub mod validate;
pub mod voltage;

pub use error::{ErrorPayload, SynthError};
pub use places::Places;
pub use report::{
    AcPfOutcome, AcPfReport, BusKind, CalibrationReport, CalibrationStatus, CalibrationStep,
    DcOpfReport, ElementAnnotations, GenerationReport, PlanningReport, Preview, PreviewArrays,
    Provenance, RegionContext, SolveOutcome, Stage, StageRecord, StepOutcome, TopologyStats,
    Warning,
};
pub use request::{
    CalibrationOptions, CalibrationTargets, DemandSpec, Density, FarIslandPolicy, Fidelity, Fuel,
    IslandOptions, Limits, OsmIslands, OsmOptions, Region, Source, SynthRequest, ValidationOptions,
};
pub use session::{generate, Generator, HISTORY_NAME};

/// The crate version written into module producers and report provenance.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Version of the physical parameter tables in `electrical::params`.
pub const PARAMETER_TABLE_VERSION: &str = "2026.10";
