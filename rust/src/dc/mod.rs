//! In-browser DC pipeline (issue #2): the DC OPF model, the Clarabel solve,
//! and (next) the dLMP/dd sensitivity column, ported from PowerDiff.jl.

// Some struct fields, duals, and re-exports are consumed by later steps (the
// sensitivity column, the wasm export) and the test paths, so not all of them
// are read in the current cdylib build.
#![allow(dead_code, unused_imports)]

mod model;
mod sens;
mod solve;

pub use model::DcNetwork;
pub use sens::{dlmp_dd, dlmp_dd_perunit};
pub use solve::{solve, DcSolution};
