//! Generation: units are sited where the network is strong and demand is
//! high (nodal degree, local load, voltage), one per bus, with fuels drawn
//! from the regional mix and relative sizes from a log-normal per fuel. The
//! units of an island are grouped into adequacy areas of about
//! [`ADEQUACY_AREA_MW`] of the load nearest them, and each area's
//! reserve-margin capacity is shared out among its units by their sizes. No
//! bus collects more than its voltage class can plausibly evacuate
//! ([`bus_generation_cap_mw`]): a share above the cap spills onto the area's
//! other units, and when every unit of the area is at its cap the area's next
//! ranked bus gets a plant. Costs are quadratic by fuel. The largest unit of
//! each island sets its reference bus.

use std::collections::BTreeMap;

use crate::electrical::params::{
    bus_generation_cap_mw, fuel_params, GENERATOR_BUS_SHARE, MIN_UNIT_MW, QMAX_FRACTION,
    QMIN_FRACTION,
};
use crate::report::{BusKind, RegionContext, Warning};
use crate::request::{Fuel, SynthRequest};
use crate::rng::Stream;
use crate::site::DraftBus;
use crate::topology::{corridor_degrees, Corridor};

#[derive(Clone, Debug, PartialEq)]
pub struct DraftGenerator {
    pub bus: usize,
    pub fuel: Fuel,
    pub pmax_mw: f64,
    pub pmin_mw: f64,
    pub qmax_mvar: f64,
    pub qmin_mvar: f64,
    pub c2: f64,
    pub c1: f64,
    pub c0: f64,
    pub makeup: bool,
    pub slack: bool,
    /// A synchronous condenser: voltage support with no active power
    /// (`pmax_mw` 0), placed by [`crate::electrical::reactive::place_voltage_support`].
    pub condenser: bool,
}

/// Peak load of one adequacy area, MW: an island's units are grouped into
/// areas of about this much load, and each area holds the reserve-margin
/// capacity of its own load, so the network carries balancing transfers
/// between neighbours rather than one side of a country's demand to the
/// other. source: the Lower 48 is operated by 66 balancing authorities (EIA,
/// "U.S. electric system is made up of interconnections and balancing
/// authorities", Today in Energy, 20 July 2016), about 11 GW each of its
/// 2025 peak; the area size is a modelling choice at that scale.
pub const ADEQUACY_AREA_MW: f64 = 10_000.0;

/// Smallest weight an adequacy area keeps, as a share of the mean area's
/// load, so an area far from load still receives plants of its fuels' size.
/// Modelling choice, no source.
const CATCHMENT_FLOOR: f64 = 0.05;

/// Lloyd iterations grouping units into adequacy areas.
const AREA_ITERATIONS: usize = 12;

/// For every bus of `members`, the slot in `units` of the unit nearest it.
fn nearest_unit(buses: &[DraftBus], members: &[usize], units: &[usize]) -> Vec<usize> {
    members
        .iter()
        .map(|&i| {
            let (lon, lat) = (buses[i].lon, buses[i].lat);
            let k = lat.to_radians().cos();
            units
                .iter()
                .enumerate()
                .map(|(slot, &u)| {
                    let dx = (buses[u].lon - lon) * k;
                    let dy = buses[u].lat - lat;
                    (dx * dx + dy * dy, slot)
                })
                .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
                .map_or(0, |(_, slot)| slot)
        })
        .collect()
}

/// Group `units` into adequacy areas of about [`ADEQUACY_AREA_MW`] of
/// `catchment` load each: load-weighted k-means on the units' positions,
/// seeded at the largest catchment and then at the unit farthest (by
/// load-weighted squared distance) from the seeds so far. Returns each unit's
/// area, numbered from 0 with no empty area.
fn adequacy_areas(buses: &[DraftBus], units: &[usize], catchment: &[f64]) -> Vec<usize> {
    let m = units.len();
    let load: f64 = catchment.iter().sum();
    let areas = ((load / ADEQUACY_AREA_MW).round() as usize).clamp(1, m.max(1));
    if areas <= 1 {
        return vec![0; m];
    }
    let mean_lat = units.iter().map(|&u| buses[u].lat).sum::<f64>() / m as f64;
    let k = mean_lat.to_radians().cos();
    let at: Vec<(f64, f64)> = units
        .iter()
        .map(|&u| (buses[u].lon * k, buses[u].lat))
        .collect();
    let weight: Vec<f64> = catchment.iter().map(|&c| c.max(1e-6)).collect();
    let d2 = |a: (f64, f64), b: (f64, f64)| (a.0 - b.0).powi(2) + (a.1 - b.1).powi(2);
    let first = (0..m)
        .max_by(|&a, &b| weight[a].total_cmp(&weight[b]).then(b.cmp(&a)))
        .expect("a unit");
    let mut centres = vec![at[first]];
    let mut nearest: Vec<f64> = at.iter().map(|&p| d2(p, at[first])).collect();
    while centres.len() < areas {
        let next = (0..m)
            .max_by(|&a, &b| {
                (weight[a] * nearest[a])
                    .total_cmp(&(weight[b] * nearest[b]))
                    .then(b.cmp(&a))
            })
            .expect("a unit");
        if nearest[next] <= 0.0 {
            break;
        }
        centres.push(at[next]);
        for (d, &p) in nearest.iter_mut().zip(&at) {
            *d = d.min(d2(p, at[next]));
        }
    }
    let assign = |centres: &[(f64, f64)]| -> Vec<usize> {
        at.iter()
            .map(|&p| {
                (0..centres.len())
                    .min_by(|&a, &b| {
                        d2(p, centres[a])
                            .total_cmp(&d2(p, centres[b]))
                            .then(a.cmp(&b))
                    })
                    .expect("a centre")
            })
            .collect()
    };
    let mut area = assign(&centres);
    for _ in 0..AREA_ITERATIONS {
        let mut sum = vec![(0.0, 0.0, 0.0); centres.len()];
        for (u, &a) in area.iter().enumerate() {
            sum[a].0 += weight[u] * at[u].0;
            sum[a].1 += weight[u] * at[u].1;
            sum[a].2 += weight[u];
        }
        for (c, s) in centres.iter_mut().zip(&sum) {
            if s.2 > 0.0 {
                *c = (s.0 / s.2, s.1 / s.2);
            }
        }
        let next = assign(&centres);
        if next == area {
            break;
        }
        area = next;
    }
    // Number the areas that kept units from 0, in order of first appearance.
    let mut renumber = vec![usize::MAX; centres.len()];
    let mut used = 0;
    for a in area.iter_mut() {
        if renumber[*a] == usize::MAX {
            renumber[*a] = used;
            used += 1;
        }
        *a = renumber[*a];
    }
    area
}

/// Default unit count for `n` buses.
pub fn default_generator_count(n: usize) -> usize {
    let hi = n.max(1);
    ((GENERATOR_BUS_SHARE * n as f64).round() as usize).clamp(3.min(hi), hi)
}

/// Share `capacity` among units in proportion to `raw`, holding each unit at
/// or below its cap and passing the excess to the units still below theirs.
/// Returns `None` when the caps together hold less than `capacity`.
pub fn share_under_caps(capacity: f64, raw: &[f64], caps: &[f64]) -> Option<Vec<f64>> {
    let m = raw.len();
    if caps.iter().sum::<f64>() < capacity {
        return None;
    }
    let mut capped = vec![false; m];
    loop {
        let held: f64 = (0..m).filter(|&i| capped[i]).map(|i| caps[i]).sum();
        let free_raw: f64 = (0..m).filter(|&i| !capped[i]).map(|i| raw[i]).sum();
        if free_raw <= 0.0 {
            return Some(caps.to_vec());
        }
        let scale = (capacity - held) / free_raw;
        let mut changed = false;
        for i in 0..m {
            if !capped[i] && scale * raw[i] > caps[i] {
                capped[i] = true;
                changed = true;
            }
        }
        if !changed {
            return Some(
                (0..m)
                    .map(|i| if capped[i] { caps[i] } else { scale * raw[i] })
                    .collect(),
            );
        }
    }
}

pub fn place_generators(
    buses: &mut [DraftBus],
    corridors: &[Corridor],
    ctx: &RegionContext,
    req: &SynthRequest,
    rng: &mut Stream,
    warnings: &mut Vec<Warning>,
) -> Vec<DraftGenerator> {
    let n = buses.len();
    if n == 0 {
        return Vec::new();
    }
    let deg = corridor_degrees(n, corridors);
    let max_deg = deg.iter().copied().max().unwrap_or(1).max(1) as f64;
    let max_load = buses.iter().map(|b| b.load_mw).fold(1e-6, f64::max);
    let max_kv = buses.iter().map(|b| b.base_kv).fold(1e-6, f64::max);
    let score: Vec<f64> = (0..n)
        .map(|i| {
            deg[i] as f64 / max_deg
                + 0.6 * buses[i].load_mw / max_load
                + 0.2 * buses[i].base_kv / max_kv
        })
        .collect();

    let requested = req.generator_count.map(|g| g as usize);
    let total_count = requested
        .unwrap_or_else(|| default_generator_count(n))
        .min(n);
    if total_count == 0 {
        return Vec::new();
    }

    // Islands: load share decides the unit count, every island gets one.
    let mut island_load: BTreeMap<usize, f64> = BTreeMap::new();
    let mut island_buses: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (i, b) in buses.iter().enumerate() {
        *island_load.entry(b.island).or_default() += b.load_mw;
        island_buses.entry(b.island).or_default().push(i);
    }
    let total_load: f64 = island_load.values().sum::<f64>().max(1e-9);
    let islands: Vec<usize> = island_buses.keys().copied().collect();
    let mut counts: BTreeMap<usize, usize> = islands
        .iter()
        .map(|&k| {
            let share = island_load[&k] / total_load;
            let c = ((total_count as f64 * share).round() as usize).max(1);
            (k, c.min(island_buses[&k].len()))
        })
        .collect();
    // Trim or pad to the requested total where island sizes allow.
    let mut assigned: usize = counts.values().sum();
    let mut guard = 0;
    while assigned != total_count && guard < 10_000 {
        guard += 1;
        if assigned > total_count {
            if let Some((&k, _)) = counts
                .iter()
                .filter(|(_, c)| **c > 1)
                .max_by_key(|(_, c)| **c)
            {
                *counts.get_mut(&k).unwrap() -= 1;
                assigned -= 1;
            } else {
                break;
            }
        } else if let Some((&k, _)) = counts
            .iter()
            .filter(|(k, c)| **c < island_buses[k].len())
            .max_by(|(a, _), (b, _)| island_load[a].partial_cmp(&island_load[b]).unwrap())
        {
            *counts.get_mut(&k).unwrap() += 1;
            assigned += 1;
        } else {
            break;
        }
    }

    let fuels: Vec<Fuel> = ctx.fuel_mix.keys().copied().collect();
    let fuel_weights: Vec<f64> = ctx.fuel_mix.values().copied().collect();
    let draw = |rng: &mut Stream| -> (Fuel, f64) {
        let fuel = rng
            .weighted_index(&fuel_weights)
            .map(|k| fuels[k])
            .unwrap_or(Fuel::Gas);
        let p = fuel_params(fuel);
        (fuel, rng.log_normal_bounded(p.size_mu, p.size_sigma))
    };
    let cap_of = |b: &DraftBus| bus_generation_cap_mw(b.base_kv);
    let mut spilled = 0usize;
    let mut over_cap = 0usize;
    let mut gens = Vec::with_capacity(total_count);
    for &island in &islands {
        let count = counts[&island];
        if count == 0 {
            continue;
        }
        let mut ranked: Vec<usize> = island_buses[&island].clone();
        ranked.sort_by(|&a, &b| score[b].partial_cmp(&score[a]).unwrap().then(a.cmp(&b)));
        let capacity = (1.0 + req.reserve_margin) * island_load[&island].max(1.0);
        let mut drafts: Vec<(usize, Fuel, f64)> = Vec::with_capacity(count);
        for &bus in ranked.iter().take(count) {
            let (fuel, raw) = draw(rng);
            drafts.push((bus, fuel, raw));
        }
        // Regional adequacy: units are grouped into areas of about
        // ADEQUACY_AREA_MW of the load nearest them, and each area's units
        // share the reserve-margin capacity of that load by their drawn sizes.
        // An area whose units all reach their bus caps grows onto its next
        // ranked bus; capacity no area can hold is shared over the island.
        let members = &island_buses[&island];
        let units: Vec<usize> = drafts.iter().map(|d| d.0).collect();
        let owner = nearest_unit(buses, members, &units);
        let mut catchment = vec![0.0; units.len()];
        for (&i, &slot) in members.iter().zip(&owner) {
            catchment[slot] += buses[i].load_mw;
        }
        let mut area = adequacy_areas(buses, &units, &catchment);
        let area_count = area.iter().max().map_or(1, |&a| a + 1);
        let mut area_load = vec![0.0; area_count];
        for (slot, &a) in area.iter().enumerate() {
            area_load[a] += catchment[slot];
        }
        let mean = area_load.iter().sum::<f64>() / area_count as f64;
        let weights: Vec<f64> = area_load
            .iter()
            .map(|&l| {
                if mean > 0.0 {
                    l.max(CATCHMENT_FLOOR * mean)
                } else {
                    1.0
                }
            })
            .collect();
        let weight_total: f64 = weights.iter().sum();
        // Each island bus belongs to the area of its nearest unit; an area
        // grows onto its own buses only.
        let bus_area: BTreeMap<usize, usize> = members
            .iter()
            .zip(&owner)
            .map(|(&i, &slot)| (i, area[slot]))
            .collect();
        let mut hosts: std::collections::BTreeSet<usize> = units.iter().copied().collect();
        let mut sizes = vec![0.0; drafts.len()];
        let mut leftover = 0.0;
        for (a, weight) in weights.iter().enumerate() {
            let target = capacity * weight / weight_total;
            loop {
                let slots: Vec<usize> = (0..drafts.len()).filter(|&k| area[k] == a).collect();
                let raw: Vec<f64> = slots.iter().map(|&k| drafts[k].2).collect();
                let caps: Vec<f64> = slots.iter().map(|&k| cap_of(&buses[drafts[k].0])).collect();
                if let Some(shares) = share_under_caps(target, &raw, &caps) {
                    for (&k, v) in slots.iter().zip(shares) {
                        sizes[k] = v;
                    }
                    break;
                }
                // Every unit of the area sits at its bus's cap. A default
                // count grows onto the area's next ranked bus; a requested
                // count is honoured and the units exceed their caps, which
                // the report says.
                if requested.is_some() {
                    let raw_total: f64 = raw.iter().sum::<f64>().max(1e-9);
                    for (&k, r) in slots.iter().zip(&raw) {
                        sizes[k] = target * r / raw_total;
                    }
                    break;
                }
                let next = ranked
                    .iter()
                    .copied()
                    .find(|b| bus_area.get(b) == Some(&a) && !hosts.contains(b));
                if let Some(bus) = next {
                    let (fuel, raw) = draw(rng);
                    drafts.push((bus, fuel, raw));
                    area.push(a);
                    sizes.push(0.0);
                    hosts.insert(bus);
                    spilled += 1;
                    continue;
                }
                for (&k, c) in slots.iter().zip(&caps) {
                    sizes[k] = *c;
                }
                leftover += target - caps.iter().sum::<f64>();
                break;
            }
        }
        // Capacity no area could hold goes to the island's units with room,
        // then onto the island's next ranked buses.
        if leftover > 1e-9 {
            loop {
                let raw: Vec<f64> = drafts.iter().map(|d| d.2).collect();
                let room: Vec<f64> = drafts
                    .iter()
                    .zip(&sizes)
                    .map(|(d, s)| (cap_of(&buses[d.0]) - s).max(0.0))
                    .collect();
                if let Some(extra) = share_under_caps(leftover, &raw, &room) {
                    for (s, e) in sizes.iter_mut().zip(extra) {
                        *s += e;
                    }
                    break;
                }
                if let Some(bus) = ranked.iter().copied().find(|b| !hosts.contains(b)) {
                    let (fuel, raw) = draw(rng);
                    drafts.push((bus, fuel, raw));
                    area.push(0);
                    sizes.push(0.0);
                    hosts.insert(bus);
                    spilled += 1;
                    continue;
                }
                let raw_total: f64 = raw.iter().sum::<f64>().max(1e-9);
                for (s, r) in sizes.iter_mut().zip(&raw) {
                    *s += leftover * r / raw_total;
                }
                break;
            }
        }
        over_cap += drafts
            .iter()
            .zip(&sizes)
            .filter(|(d, s)| **s > cap_of(&buses[d.0]) + 1e-6)
            .count();
        let first = gens.len();
        for ((bus, fuel, _), size) in drafts.into_iter().zip(sizes) {
            let pmax = size.max(MIN_UNIT_MW);
            let p = fuel_params(fuel);
            let c1 = rng.range(p.c1_range.0, p.c1_range.1);
            let c2 = p.c2 * (0.8 + 0.4 * rng.uniform());
            buses[bus].kind = BusKind::Generator;
            buses[bus].annotation = "Generator sited on a high-degree, high-demand bus".to_owned();
            gens.push(DraftGenerator {
                bus,
                fuel,
                pmax_mw: pmax,
                pmin_mw: 0.0,
                qmax_mvar: QMAX_FRACTION * pmax,
                qmin_mvar: QMIN_FRACTION * pmax,
                c2,
                c1,
                c0: 0.0,
                makeup: false,
                slack: false,
                condenser: false,
            });
        }
        // The island's largest unit is its reference.
        if let Some(k) = (first..gens.len()).max_by(|&a, &b| {
            gens[a]
                .pmax_mw
                .partial_cmp(&gens[b].pmax_mw)
                .unwrap()
                .then(b.cmp(&a))
        }) {
            gens[k].slack = true;
        }
    }
    if spilled > 0 {
        warnings.push(Warning::new(
            "generation_spread",
            format!(
                "{spilled} generator bus(es) beyond the default count: the capacity target exceeds what the default buses can collect at their voltage class caps"
            ),
        ));
    }
    if over_cap > 0 {
        warnings.push(Warning::new(
            "generator_cap_exceeded",
            format!(
                "{over_cap} unit(s) exceed the generation cap of their bus's voltage class because the requested generator count cannot hold the capacity target otherwise"
            ),
        ));
    }
    gens
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::resolve_context;
    use crate::places::Places;
    use crate::report::ElementSource;
    use crate::request::{DemandSpec, Region};
    use crate::rng::StreamTag;

    #[test]
    fn each_adequacy_area_holds_its_own_reserve_margin() {
        // Two load centres 2,000 km apart, 30 GW each on 40 buses of 750 MW:
        // the units near each must hold 1 + reserve margin of its load, and
        // a centre whose first units reach their caps grows onto its own
        // buses, not the other centre's.
        let mut req = SynthRequest::new(Region::bbox(0.0, 0.0, 21.0, 1.0, "two centres"));
        req.demand = DemandSpec::TotalMw { mw: 60_000.0 };
        req.target_bus_count = Some(80);
        let (ctx, _) = resolve_context(&req, &Places::empty(), &mut Vec::new()).unwrap();
        // Equal scores rank by index, so the centres alternate.
        let mut buses: Vec<DraftBus> = (0..80)
            .map(|i| DraftBus {
                lon: if i % 2 == 0 { 0.0 } else { 20.0 } + 0.15 * ((i / 2) % 8) as f64,
                lat: 0.2 * ((i / 2) / 8) as f64,
                name: format!("b{i}"),
                place: None,
                tier: 0,
                base_kv: 500.0,
                kind: BusKind::Load,
                confidence: 1.0,
                polygon: 0,
                island: 0,
                source: ElementSource::Synthetic,
                population_weight: 1.0 / 80.0,
                load_mw: 750.0,
                load_mvar: 0.0,
                annotation: String::new(),
            })
            .collect();
        let mut rng = Stream::new(7, StreamTag::Generators);
        let gens = place_generators(&mut buses, &[], &ctx, &req, &mut rng, &mut Vec::new());
        let held = |west: bool| -> f64 {
            gens.iter()
                .filter(|g| (buses[g.bus].lon < 10.0) == west)
                .map(|g| g.pmax_mw)
                .sum()
        };
        let need = (1.0 + req.reserve_margin) * 30_000.0;
        for west in [true, false] {
            assert!(
                (held(west) - need).abs() < 1e-6 * need,
                "{} vs {need}",
                held(west)
            );
        }
        let cap = bus_generation_cap_mw(500.0);
        assert!(gens.iter().all(|g| g.pmax_mw <= cap + 1e-6));
    }

    #[test]
    fn shares_respect_caps_and_spill() {
        let s = share_under_caps(100.0, &[1.0, 1.0, 2.0], &[1000.0, 1000.0, 1000.0]).unwrap();
        assert!((s[2] - 50.0).abs() < 1e-9 && (s[0] - 25.0).abs() < 1e-9);
        let s = share_under_caps(100.0, &[1.0, 1.0, 8.0], &[100.0, 100.0, 40.0]).unwrap();
        assert!((s[2] - 40.0).abs() < 1e-9);
        assert!((s[0] - 30.0).abs() < 1e-9 && (s[1] - 30.0).abs() < 1e-9);
        assert!(share_under_caps(100.0, &[1.0, 1.0], &[40.0, 40.0]).is_none());
    }
}
