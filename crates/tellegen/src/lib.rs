//! tellegen: differentiable optimal power flow and sensitivities.
//!
//! Parses a case through [`powerio`] and solves any of four formulations — DC power
//! flow, DC OPF, AC power flow, and the SOCWR (Jabr) conic relaxation — returning a
//! formulation-agnostic result: locational marginal prices, voltages, branch flows,
//! generator dispatch, and analytical KKT sensitivities of any [`Operand`] with
//! respect to any [`Parameter`]. Every path is pure Rust and compiles to native
//! targets and WebAssembly, so the same engine runs on a server and in the browser.
//!
//! [`solve_json`] is the one front door (a [`SolveRequest`] in, a [`SolveResponse`]
//! out); [`capabilities_json`] reports which `(formulation, operand, parameter)`
//! combinations a given build supports.
//!
//! ```ignore
//! use tellegen::solve_json;
//!
//! let network_json = powerio::parse_str(case_text, "matpower")?.network.to_json()?;
//! let request = r#"{
//!     "formulation": "dcopf",
//!     "edits": { "deltas": { "2": 50.0 } },
//!     "sensitivities": [
//!         { "operand": {"Price":"Active"}, "parameter": {"Demand":"Active"} }
//!     ]
//! }"#;
//! let out = solve_json(&network_json, request)?;  // { lmp, flows, dispatch, sensitivities, ... }
//! ```

mod api;
pub mod formulation;
pub mod geo;
mod model;
pub mod problem;
#[cfg(feature = "sensitivity")]
mod sens;
mod solve;
#[cfg(feature = "sensitivity")]
pub mod study;

#[cfg(feature = "sensitivity")]
pub use api::SensRequest;
pub use api::{
    capabilities_json, solve_json, solve_network, solve_prebuilt, solve_prebuilt_cancellable,
    BranchFlow, BusInjection, BusScalar, Edits, ElementKey, GenDispatch, Iterations, Problem,
    ProblemCaps, SolveOptions, SolveRequest, SolveResponse, SolveStatus,
};
pub use formulation::{AcPolar, Dc, Formulation, SocWr};
#[cfg(feature = "sensitivity")]
pub use model::AcNetwork;
pub use model::DcNetwork;
#[cfg(feature = "sensitivity")]
pub use problem::{
    ac_pf, build_dc_pf, dc_pf, AcPfFormulation, AcPfLayout, AcPfSolution, DcPfFormulation,
    DcPfSolution, DcPfSystem,
};
#[cfg(feature = "conic")]
pub use problem::{build_conic_opf, socwr_opf, ConicOpfFormulation, SocWrSolution};
pub use problem::{build_opf, DcOpfSolution, OpfFormulation, OpfProgram};
#[cfg(feature = "conic")]
pub use sens::ConicKkt;
#[cfg(feature = "sensitivity")]
pub use sens::{
    sensitivity, AcNewton, Axis, Bound, ColMeta, CostTerm, DcKkt, Differentiable, ElementId, End,
    Mode, Operand, Parameter, Power, RowMeta, Selector, SensError, SensitivityMatrix, SolveSpec,
    TapKind, VoltageKind, GB,
};
pub use solve::SolveIteration;
#[cfg(feature = "sensitivity")]
pub use study::{
    export_study, ExportedCase, NetworkEdit, Preview, PreviewColumn, PreviewValue, Study,
};
