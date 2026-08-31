//! tellegen: differentiable optimal power flow and sensitivities.
//!
//! Solves PowerIO problem instances and stored modules as DC power flow, DC OPF,
//! AC power flow, or the SOCWR conic relaxation. Dense solver workspaces are
//! private implementation details; portable input and persistence use PowerIO.
//!
//! [`solve_instance`] is the typed DC OPF entry. [`Study`] accepts a stored
//! PowerIO balanced network module for interactive browser work.
//!
//! ```ignore
//! use tellegen::{solve_instance, SolveRequest};
//!
//! let module = powerio::stored::read_module(&module_json)?;
//! use powerio::IntoTypedModule;
//! let instance_module: powerio::PioModule<powerio::DcOpfInstance> =
//!     module.into_typed()?;
//! let response = solve_instance(instance_module.value(), &SolveRequest::default())?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

/// Tellegen engine package version embedded by Cargo.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

mod api;
mod emit;
mod formulation;
pub mod geo;
#[cfg(feature = "sensitivity")]
mod history;
mod model;
#[cfg(feature = "sensitivity")]
pub mod plan;
mod problem;
#[cfg(feature = "sensitivity")]
mod sens;
mod solve;
#[cfg(feature = "sensitivity")]
pub mod study;

#[cfg(feature = "conic")]
pub use api::solve_ac_instance;
pub use api::{
    capabilities_json, solve_instance, solve_instance_cancellable, solve_module_json,
    validate_canonical_identity, BranchFlow, BusInjection, BusScalar, Edits, ElementKey,
    GenDispatch, Iterations, Problem, ProblemCaps, SolveRequest, SolveResponse, SolveStatus,
};
#[cfg(feature = "sensitivity")]
pub use api::{solve_ac_pf_instance, SensRequest};
pub use emit::solve_dc_opf_instance;
#[cfg(feature = "sensitivity")]
pub use plan::{
    plan_capacity, plan_capacity_cancellable, BusWeight, CapacityPlanExecution,
    CapacityPlanIteration, CapacityPlanOutcome, CapacityPlanResultSummary, CapacityPlanSpec,
    GradientEntry, ImplicitObjective, RatingChange,
};
#[cfg(feature = "sensitivity")]
pub use sens::{
    Axis, Bound, ColMeta, CostTerm, ElementId, End, Mode, Operand, Parameter, Power, RowMeta,
    Selector, SensError, SensitivityMatrix, SolveSpec, TapKind, VoltageKind, GB,
};
pub use solve::SolveIteration;
#[cfg(feature = "sensitivity")]
pub use study::{
    apply_network_edits, ExportedCase, NetworkEdit, Preview, PreviewColumn, PreviewValue, Study,
};
