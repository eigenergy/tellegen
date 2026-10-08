//! Shared library for the non-shipping benchmark binaries.
//!
//! [`mc_feeder`] generates the deterministic synthetic multiconductor feeders
//! that `mc-pf-session-bench` and the opt-in browser benchmark measure.
//! [`meshed_grid`] generates the synthetic transmission grids that
//! `dcopf-scale-bench` measures, and [`heap`] is the heap accounting both
//! binaries share.

pub mod heap;
pub mod mc_feeder;
pub mod meshed_grid;
