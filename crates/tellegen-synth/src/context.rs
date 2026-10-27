//! The context stage: parse the region, measure it, find its populated
//! places, resolve bus count, peak demand and fuel mix, and decide which
//! polygons take part (tiny islets and far islands under the `Cut` policy do
//! not).

use std::collections::BTreeMap;

use crate::demand::{CountryAreas, NationalDemand, COUNTRY_AREA_COVERAGE};
use crate::error::SynthError;
use crate::geom::region::RegionShape;
use crate::geom::{haversine_km, wrap_lon};
use crate::places::{PlacedCity, Places};
use crate::report::{CountryDemand, DemandBasis, RegionContext, VoltageBasis, Warning};
use crate::request::{DemandSpec, Density, FarIslandPolicy, SynthRequest};
use crate::voltage::{LadderSource, VoltageLadders};

/// Everything later stages need from the context stage that is not in the
/// public [`RegionContext`].
pub struct ContextData {
    pub shape: RegionShape,
    pub cities: Vec<PlacedCity>,
    /// Sampling weight per polygon (0 for excluded polygons).
    pub polygon_weights: Vec<f64>,
    /// Group index per polygon (`None` when excluded). Polygons in one group
    /// are joined by interconnects; different groups are separate islands.
    pub polygon_group: Vec<Option<usize>>,
    pub group_count: usize,
}

/// Bus count from density and area when the request states none. The square
/// root law keeps a continent at a few hundred buses and a small country at a
/// few dozen; `target_bus_count` overrides it.
pub fn density_bus_count(density: Density, area_km2: f64) -> u32 {
    let base = match density {
        Density::Low => 28.0,
        Density::Medium => 52.0,
        Density::High => 78.0,
    };
    let area_factor = ((area_km2.max(100.0) / 10_000.0).sqrt() / 2.5).clamp(0.8, 4.0);
    (base * area_factor).round().clamp(MIN_BUS_COUNT, 400.0) as u32
}

/// Fewest buses a count from area or demand gives.
const MIN_BUS_COUNT: f64 = 14.0;

/// Peak load per bus the default bus count keeps within, MW (low, high).
/// source: average load per bus in transmission models falls with the lowest
/// voltage they include. Models down to 110–220 kV carry about 30–55 MW per
/// bus (case1888rte 31 MW, case2869pegase 46 MW, case1354pegase 54 MW, from
/// the MATPOWER 8.1 data; Josz et al., arXiv:1603.01533), substation-level
/// models at 220 kV and above about 100–140 MW (PyPSA-Eur: 0.51 TW over 4,977
/// buses, 102 MW per bus and 139 MW per substation, Hörsch et al., *Energy
/// Strategy Reviews* 22, 2018; SciGRID Germany about 129 MW per bus at the
/// 2024 German peak). The band runs from the first scale to the second,
/// rounded up to 150 MW.
pub const MW_PER_BUS_BAND: (f64, f64) = (30.0, 150.0);

/// Bus count from density, area and peak demand when the request states
/// none: the area law of [`density_bus_count`], moved into the range that
/// keeps each bus within [`MW_PER_BUS_BAND`] of peak load. A country-scale
/// region then aggregates its demand the way transmission models do instead
/// of loading a few hundred buses with gigawatts each, and a large but empty
/// region does not spread a small demand over hundreds of buses.
pub fn demand_bus_count(density: Density, area_km2: f64, peak_mw: f64) -> u32 {
    let area = density_bus_count(density, area_km2) as f64;
    if !(peak_mw.is_finite() && peak_mw > 0.0) {
        return area as u32;
    }
    let fewest = (peak_mw / MW_PER_BUS_BAND.1).ceil();
    let most = (peak_mw / MW_PER_BUS_BAND.0).floor().max(MIN_BUS_COUNT);
    area.max(fewest)
        .min(most.max(fewest))
        .min(f64::from(u32::MAX)) as u32
}

/// Branches per bus by density when the request states none. source:
/// Birchfield et al. 2017 report branch/bus ratios of 1.2–1.5 for North
/// American transmission systems.
pub fn density_branch_factor(density: Density) -> f64 {
    match density {
        Density::Low => 1.18,
        Density::Medium => 1.32,
        Density::High => 1.45,
    }
}

/// Peak demand per bus when the region has no population signal. source:
/// the ACTIVSg2000 case carries 67 GW over 2000 buses, about 33 MW per bus;
/// the smaller ACTIVSg200 carries 1.5 GW over 200 buses, 7.5 MW per bus. A
/// demo region sits between.
const DEFAULT_MW_PER_BUS: f64 = 14.0;

/// Expected buses below which a polygon is left out of sampling.
const MIN_EXPECTED_BUSES: f64 = 0.5;

pub fn resolve_context(
    req: &SynthRequest,
    places: &Places,
    warnings: &mut Vec<Warning>,
) -> Result<(RegionContext, ContextData), SynthError> {
    let shape = RegionShape::from_geometry(&req.region.geometry).map_err(SynthError::Region)?;
    if shape.area_km2 > req.limits.max_area_km2 {
        return Err(SynthError::InvalidRequest(format!(
            "region area {:.0} km² exceeds limits.max_area_km2 = {:.0}",
            shape.area_km2, req.limits.max_area_km2
        )));
    }
    if shape.area_km2 <= 0.0 {
        return Err(SynthError::Region(
            "region polygons enclose no area".to_owned(),
        ));
    }
    // Bus count: the request's, else area and demand. Demand for sizing counts
    // every place inside the region; the participation rules below may drop
    // a few polygons, which only lowers the demand the count was sized for.
    let (wanted, sizing_peak) = match req.target_bus_count {
        Some(count) => (count, None),
        None => {
            let mut sizing = places.in_region(&shape);
            let peak = size_demand(&req.demand, places, &mut sizing, &shape).peak_mw;
            (
                demand_bus_count(req.density, shape.area_km2, peak),
                Some(peak),
            )
        }
    };
    let n = wanted.min(req.limits.max_bus_count).max(1);
    if let Some(peak) = sizing_peak.filter(|_| wanted > n) {
        warnings.push(Warning::new(
            "bus_count_capped",
            format!(
                "a {peak:.0} MW peak needs {wanted} buses to stay within {:.0} MW per bus; limits.max_bus_count holds the network at {n} buses, {:.0} MW per bus",
                MW_PER_BUS_BAND.1,
                peak / n as f64
            ),
        ));
    }
    if let Some(g) = req.generator_count {
        if g > n {
            return Err(SynthError::InvalidRequest(format!(
                "generator_count must be no greater than generated bus count ({n})"
            )));
        }
    }

    // Polygon participation: expected bus share first, then island grouping.
    let areas = shape.polygon_areas();
    let total_area: f64 = areas.iter().sum();
    let mut kept: Vec<usize> = (0..areas.len())
        .filter(|&i| areas[i] / total_area * n as f64 >= MIN_EXPECTED_BUSES || areas.len() == 1)
        .collect();
    if kept.is_empty() {
        kept.push(
            (0..areas.len())
                .max_by(|&a, &b| areas[a].partial_cmp(&areas[b]).unwrap())
                .unwrap_or(0),
        );
    }
    let skipped_small = areas.len() - kept.len();
    if skipped_small > 0 {
        warnings.push(Warning::new(
            "small_islands_skipped",
            format!(
                "{skipped_small} polygon(s) too small for a bus at this count were left out of siting"
            ),
        ));
    }
    let groups = landmass_groups(&shape, &kept, req.islands.max_interconnect_km);
    let mut polygon_group = vec![None; areas.len()];
    let mut polygon_weights = vec![0.0; areas.len()];
    // The connected system is the group where the region's people live, not
    // the largest by area: on a coarse outline an archipelago falls apart
    // into groups, and the largest of them need not be the populated one
    // (Borneo against Java). Area decides between groups without places.
    let mut polygon_population = vec![0.0; areas.len()];
    for city in places.in_region(&shape) {
        if let Some(i) = shape.polygon_index(city.lon, city.lat) {
            polygon_population[i] += city.place.population;
        }
    }
    let main_group = groups
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| {
            let sum = |g: &[usize], v: &[f64]| g.iter().map(|&i| v[i]).sum::<f64>();
            sum(a, &polygon_population)
                .total_cmp(&sum(b, &polygon_population))
                .then(sum(a, &areas).total_cmp(&sum(b, &areas)))
        })
        .map(|(k, _)| k)
        .unwrap_or(0);
    let mut group_count = 0;
    let mut cut_area = 0.0;
    let mut cut_polygons = 0usize;
    for (k, group) in groups.iter().enumerate() {
        let retained = k == main_group || req.islands.beyond == FarIslandPolicy::Keep;
        if retained {
            for &i in group {
                polygon_group[i] = Some(group_count);
                polygon_weights[i] = areas[i];
            }
            group_count += 1;
        } else {
            cut_polygons += group.len();
            cut_area += group.iter().map(|&i| areas[i]).sum::<f64>();
        }
    }
    if cut_polygons > 0 {
        warnings.push(Warning::new(
            "islands_cut",
            format!(
                "{cut_polygons} landmass polygon(s) covering {cut_area:.0} km² lie farther than {:.0} km from the connected system and were not generated (islands.beyond = cut)",
                req.islands.max_interconnect_km
            ),
        ));
    }
    if group_count > 1 {
        warnings.push(Warning::new(
            "multiple_islands",
            format!(
                "the region forms {group_count} electrical islands; each is validated separately and the module carries one reference bus per island"
            ),
        ));
    }

    // Places: only inside retained polygons.
    let mut cities: Vec<PlacedCity> = places
        .in_region(&shape)
        .into_iter()
        .filter(|c| {
            shape
                .polygon_index(c.lon, c.lat)
                .is_some_and(|i| polygon_group[i].is_some())
        })
        .collect();
    let population: f64 = cities.iter().map(|c| c.place.population).sum();

    let sized = size_demand(&req.demand, places, &mut cities, &shape);
    let (peak_demand_mw, demand_basis) = if sized.basis == DemandBasis::TotalMw
        || sized.peak_mw > 0.0
    {
        (sized.peak_mw, sized.basis)
    } else {
        warnings.push(Warning::new(
                "no_population_signal",
                "no populated place lies inside the region; demand uses a per-bus default and loads spread uniformly",
            ));
        (DEFAULT_MW_PER_BUS * n as f64, DemandBasis::DefaultPerBus)
    };
    let fallback: Vec<&CountryDemand> = sized
        .by_country
        .iter()
        .filter(|c| c.year.is_none())
        .collect();
    if let (DemandSpec::Population { peak_factor, .. }, false) = (&req.demand, fallback.is_empty())
    {
        let codes: Vec<&str> = fallback.iter().map(|c| c.code.as_str()).collect();
        warnings.push(Warning::new(
            "demand_fallback",
            format!(
                "{} place(s) of {} have no national demand entry and draw the world average load per person ({:.2} kW at peak)",
                fallback.iter().map(|c| c.places).sum::<usize>(),
                if codes.is_empty() { "unknown countries".to_owned() } else { codes.join(", ") },
                NationalDemand::bundled().world_intensity(*peak_factor) * 1000.0
            ),
        ));
    }

    let ladder = resolve_ladder(req, &cities, warnings);
    let levels = ladder.levels_kv.clone();
    let classes: Vec<f64> = levels
        .iter()
        .map(|&kv| crate::electrical::params::line_class_for(kv).kv)
        .collect();
    let frame = shape.frame;
    let context = RegionContext {
        label: if req.region.label.trim().is_empty() {
            "Selected region".to_owned()
        } else {
            req.region.label.clone()
        },
        area_km2: shape.area_km2,
        center_lon: wrap_lon((frame.west + frame.east) / 2.0),
        center_lat: (frame.south + frame.north) / 2.0,
        frame,
        crosses_antimeridian: shape.crosses_antimeridian(),
        landmasses: polygon_group.iter().filter(|g| g.is_some()).count(),
        population_in_region: population,
        places_in_region: cities.len(),
        places_snapped: cities.iter().filter(|c| c.snapped).count(),
        target_bus_count: n,
        peak_demand_mw,
        demand_basis,
        demand_by_country: sized.by_country,
        fuel_mix: req.resolved_fuel_mix(),
        branch_factor: req
            .branch_factor
            .unwrap_or_else(|| density_branch_factor(req.density)),
        voltage_levels_kv: levels,
        voltage_classes_kv: classes,
        voltage_basis: ladder.basis,
        voltage_country: ladder.country,
        promotion_levels_kv: ladder.above_kv,
    };
    Ok((
        context,
        ContextData {
            shape,
            cities,
            polygon_weights,
            polygon_group,
            group_count,
        },
    ))
}

/// The voltage levels a region is modelled at.
struct ResolvedLadder {
    levels_kv: Vec<f64>,
    above_kv: Vec<f64>,
    basis: VoltageBasis,
    country: Option<String>,
}

/// The line class the corridor plan may add above stated levels.
fn top_line_class_kv() -> f64 {
    crate::electrical::params::LINE_CLASSES[crate::electrical::params::LINE_CLASSES.len() - 1].kv
}

/// The levels of the request, or else the national ladder of the region's
/// dominant country: the one whose places carry the most demand (population
/// under a stated total). A stated list may be promoted to 765 kV above it;
/// a country's ladder only to the higher levels that country has in service.
fn resolve_ladder(
    req: &SynthRequest,
    cities: &[PlacedCity],
    warnings: &mut Vec<Warning>,
) -> ResolvedLadder {
    if !req.voltage_levels_kv.is_empty() {
        let levels = req.resolved_voltage_levels();
        let top = top_line_class_kv();
        return ResolvedLadder {
            above_kv: if levels[0] < top - 1e-6 {
                vec![top]
            } else {
                Vec::new()
            },
            levels_kv: levels,
            basis: VoltageBasis::Request,
            country: None,
        };
    }
    let demand = NationalDemand::bundled();
    let mut weight: BTreeMap<&str, f64> = BTreeMap::new();
    for city in cities {
        *weight
            .entry(demand.canonical(&city.place.iso_a3))
            .or_default() += city.demand_weight.max(0.0);
    }
    let dominant = weight
        .iter()
        .filter(|(code, _)| !code.is_empty())
        .max_by(|a, b| a.1.total_cmp(b.1).then_with(|| b.0.cmp(a.0)))
        .map(|(code, _)| (*code).to_owned());
    let table = VoltageLadders::bundled();
    let ladder = table.ladder(dominant.as_deref());
    let basis = match ladder.source {
        LadderSource::Country => VoltageBasis::Country,
        LadderSource::Regional | LadderSource::Standard => {
            let levels = ladder
                .levels_kv
                .iter()
                .map(|kv| format!("{kv}"))
                .collect::<Vec<_>>()
                .join("/");
            let reason = match (&dominant, ladder.source) {
                (Some(code), LadderSource::Regional) => format!(
                    "{code} has no entry in the voltage table; the {} members' {levels} kV ladder is used",
                    table.regional_name
                ),
                (Some(code), _) => format!(
                    "{code} has no entry in the voltage table; the {} standard {levels} kV levels are used",
                    table.standard_name
                ),
                (None, _) => format!(
                    "no populated place names the region's country; the {} standard {levels} kV levels are used",
                    table.standard_name
                ),
            };
            warnings.push(Warning::new("voltage_fallback", reason));
            VoltageBasis::Fallback
        }
    };
    ResolvedLadder {
        levels_kv: ladder.levels_kv,
        above_kv: ladder.above_kv,
        basis,
        country: dominant,
    }
}

/// A region's peak demand from its places, with each place's demand weight
/// set to its share.
struct DemandSizing {
    peak_mw: f64,
    basis: DemandBasis,
    by_country: Vec<CountryDemand>,
}

/// Samples a region's area is split into when it is shared among the
/// countries of its places, each sample taking the country of its nearest
/// place. Modelling choice: a share to a few percent is all the demand split
/// needs, and the cost stays a few milliseconds for a continent.
const AREA_SAMPLES: f64 = 1024.0;
/// Most grid points the area sampler visits in one polygon's bounding box.
/// Real outlines need a few thousand; a sliver or self-cancelling ring would
/// otherwise ask for billions.
const MAX_AREA_GRID_CELLS: f64 = 1.0e6;

/// The region's area by country of its nearest place (table codes), km².
/// Every polygon of the selection counts, islands the network leaves out
/// included: leaving one out removes its listed places, and with them its
/// share of the country's demand, not a share by area, which would charge
/// an empty territory such as French Guiana or Alaska as if it were
/// populated. Samples sit on a regular grid in each polygon, weighted by
/// cos(lat) so they share the polygon's spherical area.
fn area_by_country(
    shape: &RegionShape,
    cities: &[PlacedCity],
    table: &NationalDemand,
) -> BTreeMap<String, f64> {
    let mut out = BTreeMap::new();
    if cities.is_empty() {
        return out;
    }
    let total = shape.area_km2;
    if total <= 0.0 {
        return out;
    }
    let nearest = |lon: f64, lat: f64| -> &str {
        let k = lat.to_radians().cos();
        let (_, j) = cities
            .iter()
            .enumerate()
            .map(|(j, c)| (((c.lon - lon) * k).powi(2) + (c.lat - lat).powi(2), j))
            .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
            .expect("a city");
        table.canonical(&cities[j].place.iso_a3)
    };
    for poly in &shape.polygons {
        // A zero-area polygon adds no area to any country.
        if poly.area_km2 <= 0.0 {
            continue;
        }
        let target = (AREA_SAMPLES * poly.area_km2 / total).max(1.0);
        let mut dlat = (poly.area_km2 / target).sqrt().max(1e-3) / 111.0;
        let bb = *poly.bbox();
        let mid = 0.5 * (bb.south + bb.north);
        let mut dlon = dlat / mid.to_radians().cos().abs().max(0.05);
        let cells = ((bb.north - bb.south) / dlat) * ((bb.east - bb.west) / dlon);
        if cells > MAX_AREA_GRID_CELLS {
            let scale = (cells / MAX_AREA_GRID_CELLS).sqrt();
            dlat *= scale;
            dlon *= scale;
        }
        let mut points = Vec::new();
        let mut lat = bb.south + 0.5 * dlat;
        while lat < bb.north {
            let mut lon = bb.west + 0.5 * dlon;
            while lon < bb.east {
                if poly.contains(lon, lat) {
                    points.push((lon, lat));
                }
                lon += dlon;
            }
            lat += dlat;
        }
        if points.is_empty() {
            points.push(poly.rings[0][0]);
        }
        let weight: f64 = points.iter().map(|p| p.1.to_radians().cos()).sum();
        for (lon, lat) in points {
            let share = if weight > 0.0 {
                lat.to_radians().cos() / weight
            } else {
                1.0
            };
            *out.entry(nearest(lon, lat).to_owned()).or_default() += poly.area_km2 * share;
        }
    }
    out
}

/// Size demand from `spec`. Under the national basis each country's peak
/// splits into the share its listed places explain, which follows them, and
/// the unlisted rest, which the region takes in proportion to the country
/// area it covers (by nearest place) but never beyond its share of the
/// country's listed population
/// ([`crate::demand`]); a place whose country has no entry draws the world
/// average per person. With `per_capita_kw` every person draws that
/// average; a stated total leaves the population weights as they are.
fn size_demand(
    spec: &DemandSpec,
    places: &Places,
    cities: &mut [PlacedCity],
    shape: &RegionShape,
) -> DemandSizing {
    let (per_capita_kw, peak_factor) = match spec {
        DemandSpec::TotalMw { mw } => {
            return DemandSizing {
                peak_mw: *mw,
                basis: DemandBasis::TotalMw,
                by_country: Vec::new(),
            }
        }
        DemandSpec::Population {
            per_capita_kw,
            peak_factor,
        } => (*per_capita_kw, *peak_factor),
    };
    if let Some(kw) = per_capita_kw {
        for city in cities.iter_mut() {
            city.demand_weight = city.place.population * kw * peak_factor / 1000.0;
        }
        return DemandSizing {
            peak_mw: cities.iter().map(|c| c.demand_weight).sum(),
            basis: DemandBasis::Population,
            by_country: Vec::new(),
        };
    }
    let table = NationalDemand::bundled();
    let areas = CountryAreas::bundled();
    let listed = table.listed_population(places);
    let world = table.world_intensity(peak_factor);
    let covered = area_by_country(shape, cities, table);
    let mut by_country: BTreeMap<String, CountryDemand> = BTreeMap::new();
    for city in cities.iter() {
        let code = table.canonical(&city.place.iso_a3);
        let share = by_country
            .entry(code.to_owned())
            .or_insert_with(|| CountryDemand {
                code: code.to_owned(),
                year: None,
                places: 0,
                population: 0.0,
                peak_mw: 0.0,
                national_peak_mw: None,
                listed_share: None,
                area_share: None,
                unlisted_peak_mw: None,
            });
        share.places += 1;
        share.population += city.place.population;
    }
    // Each country's peak in the region, and the peak per person of its
    // places there.
    let mut intensity: BTreeMap<String, f64> = BTreeMap::new();
    for (code, share) in by_country.iter_mut() {
        let (Some(entry), Some(&total)) = (table.country(code), listed.get(code)) else {
            share.peak_mw = share.population * world;
            intensity.insert(code.clone(), world);
            continue;
        };
        if !(total > 0.0 && share.population > 0.0) {
            intensity.insert(code.clone(), 0.0);
            continue;
        }
        let national = entry.average_mw() * peak_factor;
        let explained = (total / entry.population).min(1.0);
        let held = (share.population / total).min(1.0);
        let area_share = areas
            .country_km2(table, code)
            .map(|a| covered.get(code).copied().unwrap_or(0.0) / a);
        let unlisted_share = area_share
            .map_or(held, |a| held.min(a / COUNTRY_AREA_COVERAGE))
            .min(1.0);
        let unlisted = national * (1.0 - explained) * unlisted_share;
        share.year = Some(entry.year);
        share.national_peak_mw = Some(national);
        share.listed_share = Some(explained);
        share.area_share = area_share;
        share.unlisted_peak_mw = Some(unlisted);
        share.peak_mw = national * explained * held + unlisted;
        intensity.insert(code.clone(), share.peak_mw / share.population);
    }
    for city in cities.iter_mut() {
        city.demand_weight = city.place.population * intensity[table.canonical(&city.place.iso_a3)];
    }
    let mut by_country: Vec<CountryDemand> = by_country.into_values().collect();
    by_country.sort_by(|a, b| {
        b.peak_mw
            .total_cmp(&a.peak_mw)
            .then_with(|| a.code.cmp(&b.code))
    });
    DemandSizing {
        peak_mw: cities.iter().map(|c| c.demand_weight).sum(),
        basis: DemandBasis::National,
        by_country,
    }
}

/// Single-linkage groups of the kept polygons: two polygons join when some
/// pair of their outer-ring vertices is within `max_km`. Rings are subsampled
/// to at most 200 vertices and bounding boxes reject distant pairs first.
fn landmass_groups(shape: &RegionShape, kept: &[usize], max_km: f64) -> Vec<Vec<usize>> {
    let samples: Vec<Vec<(f64, f64)>> = kept
        .iter()
        .map(|&i| {
            let ring = &shape.polygons[i].rings[0];
            let step = (ring.len() / 200).max(1);
            ring.iter().step_by(step).copied().collect()
        })
        .collect();
    let mut uf = crate::geom::delaunay::UnionFind::new(kept.len());
    for a in 0..kept.len() {
        for b in a + 1..kept.len() {
            let (ba, bb) = (
                shape.polygons[kept[a]].bbox(),
                shape.polygons[kept[b]].bbox(),
            );
            let lat_gap = (ba.south.max(bb.south) - ba.north.min(bb.north)).max(0.0) * 111.0;
            if lat_gap > max_km {
                continue;
            }
            let lon_gap = (ba.west.max(bb.west) - ba.east.min(bb.east)).max(0.0);
            let lat_mid = (ba.south + ba.north + bb.south + bb.north) / 4.0;
            if lon_gap * 111.0 * lat_mid.to_radians().cos().abs() > max_km * 1.2 {
                continue;
            }
            let mut near = false;
            'outer: for &p in &samples[a] {
                for &q in &samples[b] {
                    if haversine_km(p.0, p.1, q.0, q.1) <= max_km {
                        near = true;
                        break 'outer;
                    }
                }
            }
            if near {
                uf.union(a, b);
            }
        }
    }
    let mut by_root: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (k, &poly) in kept.iter().enumerate() {
        by_root.entry(uf.find(k)).or_default().push(poly);
    }
    by_root.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demand::DEFAULT_PEAK_FACTOR;
    use crate::report::VoltageBasis;
    use crate::request::Region;

    #[test]
    fn density_counts_follow_area_and_clamp() {
        assert_eq!(density_bus_count(Density::High, 373_000.0), 191);
        assert!(density_bus_count(Density::Low, 1.0) >= 14);
        assert!(density_bus_count(Density::High, 1.0e8) <= 400);
        assert!(
            density_bus_count(Density::Medium, 50_000.0)
                < density_bus_count(Density::High, 50_000.0)
        );
    }

    #[test]
    fn far_islands_are_cut_by_default_and_kept_on_request() {
        // A mainland box and an island box 600 km east.
        let geom = serde_json::json!({"type":"MultiPolygon","coordinates":[
            [[[0.0,0.0],[3.0,0.0],[3.0,3.0],[0.0,3.0],[0.0,0.0]]],
            [[[9.0,0.0],[11.0,0.0],[11.0,2.0],[9.0,2.0],[9.0,0.0]]]]});
        let mut req = SynthRequest::new(Region {
            geometry: geom,
            label: "two".into(),
        });
        req.target_bus_count = Some(40);
        let places = Places::empty();
        let mut warnings = Vec::new();
        let (ctx, data) = resolve_context(&req, &places, &mut warnings).unwrap();
        assert_eq!(data.group_count, 1);
        assert_eq!(ctx.landmasses, 1);
        assert!(warnings.iter().any(|w| w.code == "islands_cut"));
        assert_eq!(data.polygon_weights[1], 0.0);

        req.islands.beyond = FarIslandPolicy::Keep;
        let mut warnings = Vec::new();
        let (ctx, data) = resolve_context(&req, &places, &mut warnings).unwrap();
        assert_eq!(data.group_count, 2);
        assert_eq!(ctx.landmasses, 2);
        assert!(warnings.iter().any(|w| w.code == "multiple_islands"));

        req.islands.max_interconnect_km = 1000.0;
        let (_, data) = resolve_context(&req, &places, &mut Vec::new()).unwrap();
        assert_eq!(data.group_count, 1);
        assert!(data.polygon_weights[1] > 0.0);
    }

    #[test]
    fn a_region_holding_every_place_of_a_country_reproduces_its_national_peak() {
        let places = Places::parse(include_bytes!("../data/populated_places.json")).unwrap();
        // Mainland Portugal, the Azores and Madeira, with parts of Spain and
        // Morocco: one polygon, so no island is cut.
        let req = SynthRequest::new(Region::bbox(-31.5, 32.3, -6.0, 42.2, "Portugal box"));
        let (ctx, _) = resolve_context(&req, &places, &mut Vec::new()).unwrap();
        assert_eq!(ctx.demand_basis, DemandBasis::National);
        let prt = ctx
            .demand_by_country
            .iter()
            .find(|c| c.code == "PRT")
            .unwrap();
        let table = NationalDemand::bundled();
        let national = table.country("PRT").unwrap().average_mw() * DEFAULT_PEAK_FACTOR;
        assert_eq!(prt.national_peak_mw, Some(national));
        assert!((prt.peak_mw - national).abs() < 1e-6 * national, "{prt:?}");
        assert_eq!(prt.year, Some(table.country("PRT").unwrap().year));
        let total: f64 = ctx.demand_by_country.iter().map(|c| c.peak_mw).sum();
        assert!((total - ctx.peak_demand_mw).abs() < 1e-6 * total);
    }

    #[test]
    fn a_country_without_an_entry_draws_the_world_average_and_says_so() {
        let places = Places::parse(
            br#"[{"n":"A","lo":10.2,"la":10.2,"p":2000000,"c":"ZZZ"},
                 {"n":"B","lo":10.5,"la":10.6,"p":500000,"c":"PRT"}]"#,
        )
        .unwrap();
        let req = SynthRequest::new(Region::bbox(10.0, 10.0, 11.0, 11.0, "box"));
        let mut warnings = Vec::new();
        let (ctx, _) = resolve_context(&req, &places, &mut warnings).unwrap();
        let fallback = warnings
            .iter()
            .find(|w| w.code == "demand_fallback")
            .expect("a fallback warning");
        assert!(fallback.message.contains("ZZZ"), "{}", fallback.message);
        let zzz = ctx
            .demand_by_country
            .iter()
            .find(|c| c.code == "ZZZ")
            .unwrap();
        assert_eq!((zzz.year, zzz.national_peak_mw), (None, None));
        let world = NationalDemand::bundled().world_intensity(DEFAULT_PEAK_FACTOR);
        assert!((zzz.peak_mw - 2_000_000.0 * world).abs() < 1e-6 * zzz.peak_mw);
        // PRT's only place in this table is in the box, but the box covers
        // a sliver of Portugal's area: it takes the place's listed share and
        // that sliver of the unlisted rest, not Portugal's whole peak.
        let prt = ctx
            .demand_by_country
            .iter()
            .find(|c| c.code == "PRT")
            .unwrap();
        let (national, listed) = (prt.national_peak_mw.unwrap(), prt.listed_share.unwrap());
        let unlisted =
            national * (1.0 - listed) * (prt.area_share.unwrap() / COUNTRY_AREA_COVERAGE);
        assert!((prt.unlisted_peak_mw.unwrap() - unlisted).abs() < 1e-6 * national);
        assert!((prt.peak_mw - (national * listed + unlisted)).abs() < 1e-6 * national);
        assert!(prt.peak_mw < 0.2 * national, "{prt:?}");
    }

    #[test]
    fn levels_follow_the_dominant_country_unless_the_request_states_them() {
        let places = Places::parse(include_bytes!("../data/populated_places.json")).unwrap();
        // Most of Germany, with slivers of its neighbours.
        let mut req = SynthRequest::new(Region::bbox(6.5, 47.6, 14.5, 54.5, "Germany box"));
        let (ctx, _) = resolve_context(&req, &places, &mut Vec::new()).unwrap();
        assert_eq!(ctx.voltage_basis, VoltageBasis::Country);
        assert_eq!(ctx.voltage_country.as_deref(), Some("DEU"));
        assert_eq!(ctx.voltage_levels_kv, vec![380.0, 220.0]);
        assert!(ctx.promotion_levels_kv.is_empty());
        // Stated levels are used as given, with 765 kV above them.
        req.voltage_levels_kv = vec![230.0, 500.0];
        let (ctx, _) = resolve_context(&req, &places, &mut Vec::new()).unwrap();
        assert_eq!(ctx.voltage_basis, VoltageBasis::Request);
        assert_eq!(ctx.voltage_levels_kv, vec![500.0, 230.0]);
        assert_eq!(ctx.promotion_levels_kv, vec![765.0]);
        assert_eq!(ctx.voltage_country, None);
    }

    #[test]
    fn a_country_without_a_ladder_falls_back_and_says_so() {
        let fallback = |code: &str| {
            let places = Places::parse(
                format!(r#"[{{"n":"A","lo":10.5,"la":10.5,"p":2000000,"c":"{code}"}}]"#).as_bytes(),
            )
            .unwrap();
            let req = SynthRequest::new(Region::bbox(10.0, 10.0, 11.0, 11.0, "box"));
            let mut warnings = Vec::new();
            let (ctx, _) = resolve_context(&req, &places, &mut warnings).unwrap();
            let warning = warnings
                .into_iter()
                .find(|w| w.code == "voltage_fallback")
                .expect("a voltage_fallback warning");
            (ctx, warning.message)
        };
        // An ENTSO-E member without its own entry takes the members' ladder.
        let (ctx, message) = fallback("LUX");
        assert_eq!(ctx.voltage_basis, VoltageBasis::Fallback);
        assert_eq!(ctx.voltage_country.as_deref(), Some("LUX"));
        assert_eq!(ctx.voltage_levels_kv, vec![400.0, 220.0]);
        assert!(ctx.promotion_levels_kv.is_empty());
        assert!(message.contains("ENTSO-E"), "{message}");
        // Any other country takes IEC 60038's standard levels.
        let (ctx, message) = fallback("ZZZ");
        assert_eq!(ctx.voltage_levels_kv, vec![400.0, 220.0]);
        assert!(message.contains("IEC 60038"), "{message}");
    }

    #[test]
    fn far_islands_are_cut_from_the_populated_system_not_the_largest() {
        // A large empty landmass and a small populated one 1,000 km east,
        // too far apart to join: the populated one is the system kept.
        let geom = serde_json::json!({"type":"MultiPolygon","coordinates":[
            [[[0.0,0.0],[6.0,0.0],[6.0,6.0],[0.0,6.0],[0.0,0.0]]],
            [[[15.0,2.0],[16.0,2.0],[16.0,3.0],[15.0,3.0],[15.0,2.0]]]]});
        let req = SynthRequest::new(Region {
            geometry: geom,
            label: "two".into(),
        });
        let places =
            Places::parse(br#"[{"n":"Port","lo":15.5,"la":2.5,"p":3000000,"c":"PRT"}]"#).unwrap();
        let mut warnings = Vec::new();
        let (ctx, data) = resolve_context(&req, &places, &mut warnings).unwrap();
        assert_eq!(data.polygon_weights[0], 0.0);
        assert!(data.polygon_weights[1] > 0.0);
        assert_eq!(data.cities.len(), 1);
        assert!(ctx.peak_demand_mw > 0.0);
        assert!(warnings.iter().any(|w| w.code == "islands_cut"));
    }

    #[test]
    fn population_demand_and_default_fallback() {
        let places = Places::parse(include_bytes!("../tests/fixtures/places_small.json")).unwrap();
        let req = SynthRequest::new(Region::bbox(42.1, 31.1, 48.5, 36.8, "Iraq box"));
        let mut w = Vec::new();
        let (ctx, data) = resolve_context(&req, &places, &mut w).unwrap();
        assert_eq!(ctx.demand_basis, DemandBasis::National);
        assert!(ctx.population_in_region > 5_000_000.0);
        assert_eq!(data.cities[0].place.name, "Baghdad");
        // Every Iraqi and Iranian place of this small table lies in the box.
        // Their listed shares follow them; the box takes the unlisted rest in
        // proportion to the area it covers: much of Iraq, a sliver of Iran.
        let country = |code: &str| {
            ctx.demand_by_country
                .iter()
                .find(|c| c.code == code)
                .unwrap()
        };
        for code in ["IRQ", "IRN"] {
            let c = country(code);
            let (national, listed) = (c.national_peak_mw.unwrap(), c.listed_share.unwrap());
            let unlisted = national
                * (1.0 - listed)
                * (c.area_share.unwrap() / COUNTRY_AREA_COVERAGE).min(1.0);
            assert!((c.peak_mw - (national * listed + unlisted)).abs() < 1e-6 * national);
        }
        let (irq, irn) = (country("IRQ"), country("IRN"));
        assert!((0.4..0.8).contains(&irq.area_share.unwrap()), "{irq:?}");
        assert!(irn.area_share.unwrap() < 0.15, "{irn:?}");
        assert!(
            irn.peak_mw < 0.15 * irn.national_peak_mw.unwrap(),
            "{irn:?}"
        );
        assert!(ctx.peak_demand_mw / ctx.target_bus_count as f64 <= MW_PER_BUS_BAND.1 + 1e-9);

        let req = SynthRequest::new(Region::bbox(-30.0, -50.0, -10.0, -30.0, "South Atlantic"));
        let mut w = Vec::new();
        let (ctx, _) = resolve_context(&req, &places, &mut w).unwrap();
        assert_eq!(ctx.demand_basis, DemandBasis::DefaultPerBus);
        assert!(w.iter().any(|x| x.code == "no_population_signal"));
    }
}
