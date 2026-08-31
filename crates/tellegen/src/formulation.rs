//! Formulations: the physics / variable-space axis of a power flow problem.
//!
//! A [`Formulation`] is a zero-sized type. The problem builders in [`crate::problem`]
//! (`build_opf`, `build_dc_pf`, and `ac_pf`) are generic over it, so each formulation
//! gets its own monomorphized assembly loop. Runtime selection is a single `match` from a
//! string to a concrete type at the `api` boundary; everything downstream is static.
//!
//! [`Dc`] is the linearized B-theta model; [`AcPolar`] is the full nonlinear AC
//! model in polar voltage coordinates. `SocWr` implements the same marker and
//! problem specific trait family.

/// Internal marker for the formulation axis. Problem specific traits carry
/// the assembly methods.
pub(crate) trait Formulation {}

/// The DC (linearized B-theta) formulation. Zero-sized: it selects the assembly,
/// it holds no data.
#[derive(Clone, Copy, Debug, Default)]
#[non_exhaustive]
pub(crate) struct Dc;

impl Dc {
    /// Construct the DC formulation marker.
    pub(crate) const fn new() -> Self {
        Dc
    }
}

impl Formulation for Dc {}

/// The AC formulation in polar voltage coordinates (`vm`, `va`). Zero-sized: it
/// selects the nonlinear power flow assembly and Newton driver, it holds no data.
///
/// The power flow it drives treats every non-reference bus as PQ (free voltage
/// magnitude and angle), the form under which the voltage sensitivities
/// `d(vm, va)/dp` are uniformly defined. The faer-backed Newton solve and the
/// sensitivities sit behind the `sensitivity` feature, like the rest of the
/// faer paths; the marker type itself is always available.
#[derive(Clone, Copy, Debug, Default)]
#[non_exhaustive]
#[cfg(feature = "sensitivity")]
pub(crate) struct AcPolar;

#[cfg(feature = "sensitivity")]
impl AcPolar {
    /// Construct the AC polar formulation marker.
    pub(crate) const fn new() -> Self {
        AcPolar
    }
}

#[cfg(feature = "sensitivity")]
impl Formulation for AcPolar {}

/// The SOCWR (Jabr) second-order cone relaxation of AC OPF. Zero-sized: it selects
/// the conic assembly into Clarabel's standard form, it holds no data.
///
/// In the W-space (`w_i = |V_i|²`, `wr_ij = Re(V_i V_j*)`, `wi_ij = Im(V_i V_j*)`)
/// the AC power flow equations are linear and the only nonconvexity, the rank-1
/// coupling `wr² + wi² = w_i w_j`, is relaxed to the rotated second-order cone
/// `wr² + wi² ≤ w_i w_j`. The result is a convex lower bound on AC OPF. The conic
/// solve and its KKT sensitivities sit behind the `conic` feature; the marker type
/// itself is always available.
#[derive(Clone, Copy, Debug, Default)]
#[non_exhaustive]
#[cfg(feature = "conic")]
pub(crate) struct SocWr;

#[cfg(feature = "conic")]
impl SocWr {
    /// Construct the SOCWR formulation marker.
    pub(crate) const fn new() -> Self {
        SocWr
    }
}

#[cfg(feature = "conic")]
impl Formulation for SocWr {}
