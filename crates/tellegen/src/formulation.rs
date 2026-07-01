//! Formulations: the physics / variable-space axis of a power flow problem.
//!
//! A [`Formulation`] is a zero-sized type. The problem builders in [`crate::problem`]
//! (`build_opf`, `build_dc_pf`, and `ac_pf`) are generic over it, so each formulation
//! gets its own monomorphized assembly loop. Runtime selection is a single `match` from a
//! string to a concrete type at the `api` boundary; everything downstream is static.
//!
//! [`Dc`] is the linearized B-theta model; [`AcPolar`] is the full nonlinear AC
//! model in polar voltage coordinates. `SocWr` and `Sdp` join them later by
//! implementing the same trait family. The trait is deliberately not sealed:
//! third-party formulations are a goal.

/// The formulation axis. Carries the runtime tag; the assembly dispatch points
/// live on the problem-specific sub-traits, such as
/// [`crate::problem::OpfFormulation`].
///
/// Not sealed. Where a sub-trait method carries an invariant outside impls must not
/// break, seal that one method rather than the whole trait. Kept dyn compatible so a
/// `dyn` plugin registry stays open as a later option.
pub trait Formulation {
    /// Stable lowercase tag, e.g. `"dc"`. The `api` runtime match maps an input
    /// string to a concrete formulation type; this is the inverse, for payloads and
    /// diagnostics.
    fn tag(&self) -> &'static str;
}

/// The DC (linearized B-theta) formulation. Zero-sized: it selects the assembly,
/// it holds no data.
#[derive(Clone, Copy, Debug, Default)]
#[non_exhaustive]
pub struct Dc;

impl Dc {
    /// Construct the DC formulation marker.
    pub const fn new() -> Self {
        Dc
    }
}

impl Formulation for Dc {
    fn tag(&self) -> &'static str {
        "dc"
    }
}

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
pub struct AcPolar;

impl AcPolar {
    /// Construct the AC polar formulation marker.
    pub const fn new() -> Self {
        AcPolar
    }
}

impl Formulation for AcPolar {
    fn tag(&self) -> &'static str {
        "ac"
    }
}

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
pub struct SocWr;

impl SocWr {
    /// Construct the SOCWR formulation marker.
    pub const fn new() -> Self {
        SocWr
    }
}

impl Formulation for SocWr {
    fn tag(&self) -> &'static str {
        "socwr"
    }
}
