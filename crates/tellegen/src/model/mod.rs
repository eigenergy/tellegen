//! Network models built from a powerio `Network`: the [`DcNetwork`] B-theta model
//! and the [`AcNetwork`] pi-model admittance form. Both normalize through
//! `Network::to_normalized` + `IndexedNetwork` (per unit, radians, filtered, densely
//! reindexed, reference inferred), then layer on the solver-prep each formulation
//! needs.
//!
//! The two formulations split into [`mod@dc`] and [`mod@ac`]; the dense-reindex /
//! id-reconstruction step they share lives here in [`reconstruct_ids`].

use std::collections::HashSet;

use powerio::network::{BusType, Network};
use powerio::IndexedNetwork;

#[cfg(feature = "sensitivity")]
mod ac;
#[cfg(test)]
mod cases;
mod dc;

#[cfg(feature = "sensitivity")]
pub use ac::AcNetwork;
pub use dc::DcNetwork;

#[cfg(all(test, feature = "conic"))]
pub(crate) use cases::parse_case3_ac;
#[cfg(all(test, feature = "sensitivity"))]
pub(crate) use cases::parse_case9_ac;
#[cfg(test)]
pub(crate) use cases::{parse_case3, CASE3};

/// Below this squared impedance a branch is treated as open (zero admittance) — the
/// near-zero-impedance guard, shared by the DC and AC models.
pub(super) const MIN_Z_SQUARED: f64 = 1e-10;

/// A leading gen-cost polynomial coefficient at or below this magnitude is treated as a
/// rounding artifact and stripped, so a curve meant to be linear is not read as quadratic
/// because its quadratic term came in as e.g. `1e-17` rather than exactly `0.0`. Real
/// (per unit) cost coefficients sit far above this. Shared by the DC and AC cost readers.
pub(super) const LEADING_COST_COEFF_TOL: f64 = 1e-12;

/// Default angle-difference bounds (radians in, radians out). A `>= pi/2` half-window
/// (the MATPOWER "unconstrained"
/// +-360 degree default, or the zero/zero "unset" case) collapses to the +-60 degree
/// MATPOWER/PowerModels convention. Shared by the DC OPF (which carries these) and the
/// AC model (the AC OPF angle-difference limits and the conic angle constraints).
pub(super) fn normalize_angle_bounds(mut amin: f64, mut amax: f64) -> (f64, f64) {
    let pad = 60.0_f64.to_radians();
    if amin <= -std::f64::consts::FRAC_PI_2 {
        amin = -pad;
    }
    if amax >= std::f64::consts::FRAC_PI_2 {
        amax = pad;
    }
    if amin == 0.0 && amax == 0.0 {
        return (-pad, pad);
    }
    (amin, amax)
}

/// Dense sizes and the dense-index -> source-id maps recovered from a normalized
/// network. `to_normalized` keeps non-isolated buses (and in-service, attached
/// branches/generators) in source order and reassigns dense ids in that order, so
/// the k-th surviving raw element is dense index k.
pub(super) struct Ids {
    n: usize,
    m: usize,
    k: usize,
    bus_ids: Vec<usize>,
    branch_ids: Vec<usize>,
    gen_ids: Vec<usize>,
}

/// Reconstruct the dense sizes and source-id maps for `view`, the shared first step
/// of both [`DcNetwork::from_network`] and [`AcNetwork::from_network`]. Errors if a
/// reconstructed id list does not match the normalized count (the dense-reindex
/// assumption broke) or the network has no in-service generators.
pub(super) fn reconstruct_ids(raw: &Network, view: &IndexedNetwork) -> Result<Ids, String> {
    let n = view.n();
    let bus_ids: Vec<usize> = raw
        .buses
        .iter()
        .filter(|b| b.kind != BusType::Isolated)
        .map(|b| b.id.0)
        .collect();
    if bus_ids.len() != n {
        return Err(format!(
            "bus id reconstruction mismatch: {} non-isolated raw buses vs {} normalized",
            bus_ids.len(),
            n
        ));
    }
    let active: HashSet<usize> = bus_ids.iter().copied().collect();

    let m = view.branches().len();
    let branch_ids: Vec<usize> = raw
        .branches
        .iter()
        .enumerate()
        .filter(|(_, br)| br.in_service && active.contains(&br.from.0) && active.contains(&br.to.0))
        .map(|(i, _)| i + 1)
        .collect();
    if branch_ids.len() != m {
        return Err(format!(
            "branch id reconstruction mismatch: {} active raw branches vs {} normalized",
            branch_ids.len(),
            m
        ));
    }

    let k = view.generators().len();
    if k == 0 {
        return Err("network has no in-service generators".into());
    }
    let gen_ids: Vec<usize> = raw
        .generators
        .iter()
        .enumerate()
        .filter(|(_, g)| g.in_service && active.contains(&g.bus.0))
        .map(|(i, _)| i + 1)
        .collect();
    if gen_ids.len() != k {
        return Err(format!(
            "generator id reconstruction mismatch: {} active raw generators vs {} normalized",
            gen_ids.len(),
            k
        ));
    }

    Ok(Ids {
        n,
        m,
        k,
        bus_ids,
        branch_ids,
        gen_ids,
    })
}
