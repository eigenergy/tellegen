//! Load allocation. At high fidelity each bus is weighted by the inverse
//! distance weighted demand of places within 25 km, with a floor so a
//! bus with no place nearby still carries some load; at low fidelity (or
//! without a population signal) weights are a floor plus a skewed random
//! draw. Weights are normalised to the context's peak demand and reactive
//! power follows the requested power factor. No bus carries more than the
//! load cap of the highest requested level ([`bus_load_cap_mw`], 7 GW at
//! 500 kV, sourced there): a metropolitan area beyond it is served by
//! several substations, so the excess moves to the nearest buses with room.

use crate::context::ContextData;
use crate::electrical::params::{bus_load_cap_mw, LOAD_FLOOR_MEDIAN_FRACTION, LOAD_IDW_RADIUS_KM};
use crate::geom::haversine_km;
use crate::places::demand_idw;
use crate::report::RegionContext;
use crate::request::Fidelity;
use crate::rng::Stream;
use crate::site::DraftBus;

/// What load allocation reports back.
pub struct LoadOutcome {
    /// No place was in range of any bus; the allocation used the random spread.
    pub uniform: bool,
    /// Buses moved to a higher voltage level because their load exceeded the
    /// class cap.
    pub promoted: usize,
    /// Buses whose load exceeded the highest level's cap.
    pub capped: usize,
    /// Load moved from those buses to their neighbours, MW.
    pub spilled_mw: f64,
}

/// Hold every bus at or below `cap`: the excess of each bus above it, largest
/// first, moves to the nearest buses of its island that still have room.
/// Returns how many buses were above the cap and how much load moved, MW.
fn spill_loads(buses: &mut [DraftBus], cap: f64) -> (usize, f64) {
    let mut over: Vec<usize> = (0..buses.len())
        .filter(|&i| buses[i].load_mw > cap)
        .collect();
    over.sort_by(|&a, &b| {
        buses[b]
            .load_mw
            .total_cmp(&buses[a].load_mw)
            .then(a.cmp(&b))
    });
    let mut moved = 0.0;
    for &i in &over {
        let mut excess = buses[i].load_mw - cap;
        let (lon, lat, island) = (buses[i].lon, buses[i].lat, buses[i].island);
        let mut near: Vec<(f64, usize)> = (0..buses.len())
            .filter(|&j| {
                j != i
                    && buses[j].island == island
                    && buses[j].kind != crate::report::BusKind::Converter
                    && buses[j].load_mw < cap
            })
            .map(|j| (haversine_km(lon, lat, buses[j].lon, buses[j].lat), j))
            .collect();
        near.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        for (_, j) in near {
            if excess <= 0.0 {
                break;
            }
            let take = (cap - buses[j].load_mw).min(excess);
            buses[j].load_mw += take;
            buses[i].load_mw -= take;
            excess -= take;
            moved += take;
        }
    }
    (over.len(), moved)
}

/// Allocate `ctx.peak_demand_mw` over the buses (converter stations are
/// connection nodes and carry none), then, with `promote`, move any bus whose
/// load exceeds its voltage class cap to the lowest level that holds it. A
/// mapped (OSM) bus keeps its real voltage, so its caller passes false.
pub fn assign_loads(
    buses: &mut [DraftBus],
    ctx: &RegionContext,
    data: &ContextData,
    fidelity: Fidelity,
    power_factor: f64,
    promote: bool,
    rng: &mut Stream,
) -> LoadOutcome {
    if buses.is_empty() {
        return LoadOutcome {
            uniform: false,
            promoted: 0,
            capped: 0,
            spilled_mw: 0.0,
        };
    }
    let points: Vec<(f64, f64)> = buses.iter().map(|b| (b.lon, b.lat)).collect();
    let raw = if fidelity == Fidelity::High {
        demand_idw(&points, &data.cities, LOAD_IDW_RADIUS_KM)
    } else {
        vec![0.0; buses.len()]
    };
    let max_w = raw.iter().cloned().fold(0.0, f64::max);
    let uniform = max_w <= 0.0;
    let weights: Vec<f64> = if uniform {
        buses
            .iter()
            .map(|_| 0.45 + rng.uniform().powf(1.8))
            .collect()
    } else {
        let mut nonzero: Vec<f64> = raw.iter().copied().filter(|w| *w > 0.0).collect();
        nonzero.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let median = nonzero[nonzero.len() / 2];
        let floor = LOAD_FLOOR_MEDIAN_FRACTION * median;
        raw.iter()
            .map(|w| w.max(floor) * (0.9 + 0.2 * rng.uniform()))
            .collect()
    };
    let weights: Vec<f64> = weights
        .iter()
        .zip(buses.iter())
        .map(|(w, b)| {
            if b.kind == crate::report::BusKind::Converter {
                0.0
            } else {
                *w
            }
        })
        .collect();
    let total: f64 = weights.iter().sum::<f64>().max(1e-12);
    let tan_phi = power_factor.clamp(1e-6, 1.0).acos().tan();
    for (bus, w) in buses.iter_mut().zip(&weights) {
        bus.population_weight = w / total;
        bus.load_mw = ctx.peak_demand_mw * w / total;
    }
    let levels = &ctx.voltage_levels_kv;
    let (capped, spilled_mw) = match levels.first() {
        Some(&top) if promote => spill_loads(buses, bus_load_cap_mw(top)),
        _ => (0, 0.0),
    };
    for bus in buses.iter_mut() {
        if spilled_mw > 0.0 {
            bus.population_weight = bus.load_mw / ctx.peak_demand_mw;
        }
        bus.load_mvar = bus.load_mw * tan_phi;
    }
    let mut promoted = 0;
    for bus in buses.iter_mut().filter(|_| promote) {
        let before = bus.tier;
        while bus.tier > 0 && bus.load_mw > bus_load_cap_mw(levels[bus.tier]) {
            bus.tier -= 1;
        }
        if bus.tier != before {
            bus.base_kv = levels[bus.tier];
            promoted += 1;
            bus.annotation = format!(
                "{} (promoted to {} kV for a {:.0} MW load)",
                bus.annotation, bus.base_kv, bus.load_mw
            );
        }
    }
    LoadOutcome {
        uniform,
        promoted,
        capped,
        spilled_mw,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::{BusKind, ElementSource};

    fn bus(lon: f64, load_mw: f64, island: usize) -> DraftBus {
        DraftBus {
            lon,
            lat: 0.0,
            name: String::new(),
            place: None,
            tier: 0,
            base_kv: 500.0,
            kind: BusKind::Load,
            confidence: 1.0,
            polygon: 0,
            island,
            source: ElementSource::Synthetic,
            population_weight: 0.0,
            load_mw,
            load_mvar: 0.0,
            annotation: String::new(),
        }
    }

    #[test]
    fn spill_holds_the_cap_moves_load_to_the_nearest_room_and_conserves_it() {
        let mut buses = vec![
            bus(0.0, 2_500.0, 0),
            bus(0.1, 900.0, 0),
            bus(0.2, 100.0, 0),
            bus(5.0, 0.0, 0),
            bus(0.05, 0.0, 1),
        ];
        let before: f64 = buses.iter().map(|b| b.load_mw).sum();
        let (capped, moved) = spill_loads(&mut buses, 1_000.0);
        let after: f64 = buses.iter().map(|b| b.load_mw).sum();
        assert_eq!(capped, 1);
        assert!((moved - 1_500.0).abs() < 1e-9);
        assert!((after - before).abs() < 1e-9);
        assert!(buses.iter().all(|b| b.load_mw <= 1_000.0 + 1e-9));
        // Nearest first within the island: 100 MW to the bus 11 km away,
        // 900 to the next, the remaining 500 to the far bus; none crosses to
        // the other island.
        let loads: Vec<f64> = buses.iter().map(|b| b.load_mw).collect();
        assert_eq!(loads, [1_000.0, 1_000.0, 1_000.0, 500.0, 0.0]);
    }
}
