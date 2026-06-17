//! DC pipeline: the OPF model, the Clarabel solve, and the optional dLMP/dd
//! sensitivity column ported from PowerDiff.jl.

mod api;
mod model;
#[cfg(feature = "sensitivity")]
mod sens;
mod solve;

pub use api::{
    solve_dc_json, solve_network, DcSolveOutput, DcSolveRequest, DispatchValue, DlmpDdColumn,
    FlowValue, LmpValue, SensitivityValue,
};
