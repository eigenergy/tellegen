//! Tunable constants of the hierarchical network design and the generation
//! fleet, with their defaults. The defaults are what every caller gets; the
//! native CLI can override them for one process (`tellegen generate
//! --tuning PATH`) so a calibration run can search them without a rebuild.
//! The request never carries them: a generated grid is a function of its
//! request and the engine version alone.
//!
//! The defaults were calibrated against the California Test System (Taylor
//! et al., IEEE TEMPR 2(1), 2024) through the generic quantities below (shares
//! per ladder depth, per-level densities, capacity credit per fuel), never
//! through anything specific to one region; `docs/evidence/synth/cats/`
//! records the calibration.

use std::sync::OnceLock;

use serde::Deserialize;

/// The tunable constants.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Tuning {
    /// Share of buses at each level, highest first, for a ladder of four
    /// levels that reaches subtransmission (sited buses, taps excluded).
    pub level_shares_4: Vec<f64>,
    /// The same for three levels.
    pub level_shares_3: Vec<f64>,
    /// The same for two levels.
    pub level_shares_2: Vec<f64>,
    /// Probability that a bus at level k also steps down to level k+1 at its
    /// site (a transformer to a co-located bus), by k.
    pub step_down: Vec<f64>,
    /// Probability that it steps down to level k+2, skipping one, by k.
    pub step_skip: Vec<f64>,
    /// Spacing of the standalone sites of the top levels, as a multiple of
    /// the even spacing sqrt(area / count), by level; deeper levels are not
    /// spaced beyond the fill.
    pub level_spacing: Vec<f64>,
    /// Distinct corridors per bus of the top level's meshed network.
    pub mesh_ratio: f64,
    /// Corridors per bus beyond a spanning tree inside each subtransmission
    /// pocket, by level below the top.
    pub pocket_extra: Vec<f64>,
    /// Share of neighbouring pockets joined by one tie, by level below the top.
    pub pocket_ties: Vec<f64>,
    /// Probability that a sharp bend of a pocket tree (below 120°) is
    /// replaced by a tap junction at its Fermat point.
    pub tap_share: f64,
    /// Smallest length a tap saves before it is made, km.
    pub tap_min_saving_km: f64,
    /// Share of taps made at the substation they serve (the junction within
    /// `tap_spur_km` of it, as a line tapped at a substation fence) rather
    /// than at the Fermat point.
    pub tap_at_site_share: f64,
    /// Spur length of a tap made at its substation, km.
    pub tap_spur_km: f64,
    /// Probability that a substation a line passes straight through is
    /// tapped off it at its fence instead (T-tapped rather than looped in).
    pub tap_straight_share: f64,
    /// σ of the log-normal factor on corridor lengths when trees and meshes
    /// are chosen: routes follow terrain and rights of way, not straight
    /// lines, so a line sometimes skips its nearest neighbour.
    pub route_noise: f64,
    /// Exponent of the relative population density the fill follows (0.5 is
    /// a square root; 1 is proportional).
    pub fill_gamma: f64,
    /// Largest load one subtransmission pocket serves, in full feeders: one
    /// feeder is a corridor at the circuit cap of the pocket's class.
    /// Heavier pockets get another step-down source.
    pub pocket_feeders: f64,
    /// Farthest a pocket member may sit from its source along the level's
    /// Delaunay graph, km at 66 kV (scaled with the level's voltage); a
    /// pocket reaching farther gets another source. Subtransmission voltage
    /// drop grows with feeder length, so step-downs sit within a few tens of
    /// kilometres of the loads they serve.
    pub pocket_reach_km: f64,
    /// Share of a hierarchical region's demand placed on the bus nearest each
    /// place; the rest follows the inverse-distance weights every bus gets.
    pub nearest_load_share: f64,
    /// Peak load per sited bus a hierarchical region is sized to, MW.
    pub mw_per_bus: f64,
    /// Fill density: kernel bandwidth around places, km, and the floor
    /// relative to the densest cell, in (0, 1].
    pub fill_bandwidth_km: f64,
    pub fill_floor: f64,
    /// Offset between the buses of one site, km.
    pub site_offset_km: f64,
    /// Midpoint jitter of a line route as a share of its length.
    pub route_jitter: f64,
    /// Firm capacity credit per fuel (gas, coal, nuclear, hydro, wind, solar,
    /// storage, other): nameplate is sized so that credited capacity covers
    /// the reserve-margin target.
    pub capacity_credit: [f64; 8],
    /// Corridor-km from the nearest generator beyond which a loaded bus gets
    /// a synchronous condenser (see `electrical::params::VOLTAGE_SUPPORT_KM`).
    pub voltage_support_km: f64,
    /// Generators per sited bus in a hierarchical region.
    pub generator_bus_share: f64,
    /// In a hierarchical region, share of wind, solar and hydro units sited
    /// on the buses with the least load around them rather than with the
    /// other units at load centres (utility-scale renewables go where the
    /// land and the resource are).
    pub remote_renewables: f64,
}

impl Default for Tuning {
    fn default() -> Self {
        Self {
            level_shares_4: vec![0.015, 0.12, 0.24, 0.625],
            level_shares_3: vec![0.08, 0.29, 0.63],
            level_shares_2: vec![0.12, 0.88],
            step_down: vec![0.5, 0.25, 0.18],
            step_skip: vec![0.03, 0.12],
            level_spacing: vec![0.8, 0.45],
            mesh_ratio: 1.25,
            pocket_extra: vec![0.08, 0.08, 0.08],
            pocket_ties: vec![0.35, 0.3, 0.25],
            tap_share: 0.85,
            tap_min_saving_km: 0.3,
            tap_at_site_share: 0.95,
            tap_spur_km: 0.15,
            tap_straight_share: 0.6,
            route_noise: 0.35,
            fill_gamma: 0.5,
            pocket_feeders: 1.5,
            pocket_reach_km: 60.0,
            nearest_load_share: 0.75,
            mw_per_bus: 10.0,
            fill_bandwidth_km: 24.0,
            fill_floor: 0.06,
            site_offset_km: 0.08,
            route_jitter: 0.06,
            capacity_credit: [0.95, 0.95, 0.95, 0.8, 0.2, 0.35, 0.9, 0.9],
            voltage_support_km: crate::electrical::params::VOLTAGE_SUPPORT_KM,
            generator_bus_share: 0.25,
            remote_renewables: 0.8,
        }
    }
}

static TUNING: OnceLock<Tuning> = OnceLock::new();

/// The constants in effect for this process.
pub fn tuning() -> &'static Tuning {
    TUNING.get_or_init(Tuning::default)
}

/// Override the constants for this process, before the first generation.
/// Fails once they have been read or set.
pub fn set_tuning(t: Tuning) -> Result<(), String> {
    TUNING
        .set(t)
        .map_err(|_| "the tuning constants were already in use".to_owned())
}

/// Shares for a ladder of `depth` levels, normalised, highest first.
pub fn level_shares(depth: usize) -> Vec<f64> {
    let t = tuning();
    let raw: Vec<f64> = match depth {
        0 => Vec::new(),
        1 => vec![1.0],
        2 => t.level_shares_2.clone(),
        3 => t.level_shares_3.clone(),
        4 => t.level_shares_4.clone(),
        // Deeper ladders extend the four-level profile geometrically.
        d => {
            let mut v = t.level_shares_4.clone();
            while v.len() < d {
                let last = *v.last().unwrap_or(&1.0);
                v.push(last * 1.5);
            }
            v
        }
    };
    let raw: Vec<f64> = raw.into_iter().take(depth).map(|s| s.max(0.0)).collect();
    let total: f64 = raw.iter().sum();
    if raw.len() != depth || total <= 0.0 {
        return vec![1.0 / depth as f64; depth];
    }
    raw.iter().map(|s| s / total).collect()
}

/// The `k`th entry of a per-level list, or its last entry beyond it.
pub fn per_level(list: &[f64], k: usize) -> f64 {
    list.get(k).or(list.last()).copied().unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shares_are_normalised_for_every_depth() {
        for d in 1..=6 {
            let s = level_shares(d);
            assert_eq!(s.len(), d);
            assert!((s.iter().sum::<f64>() - 1.0).abs() < 1e-12);
        }
        assert_eq!(per_level(&[0.1, 0.2], 5), 0.2);
        assert_eq!(per_level(&[], 0), 0.0);
        let parsed: Tuning = serde_json::from_str(r#"{"mesh_ratio": 1.4}"#).unwrap();
        assert_eq!(parsed.mesh_ratio, 1.4);
        assert_eq!(parsed.tap_share, Tuning::default().tap_share);
        assert!(serde_json::from_str::<Tuning>(r#"{"nope": 1}"#).is_err());
    }
}
