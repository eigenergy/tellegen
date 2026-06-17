//! In-browser DC pipeline: the DC OPF model, the Clarabel solve, and the
//! dLMP/dd sensitivity column, ported from PowerDiff.jl.

mod api;
mod model;
mod sens;
mod solve;

pub use api::{
    solve_dc_json, solve_network, DcSolveOutput, DcSolveRequest, DispatchValue, DlmpDdColumn,
    FlowValue, LmpValue, SensitivityValue,
};
