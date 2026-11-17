//! Modelled synchronous areas and the fixed HVDC transfers between them.
//! The geographic rules approximate published regional divisions; they are
//! not surveyed operating boundaries or a catalogue of real equipment.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::build::DraftHvdc;
use crate::context::ContextData;
use crate::electrical::generators::DraftGenerator;
use crate::electrical::planning::economic_dispatch;
use crate::geom::haversine_km;
use crate::site::DraftBus;

const US_SOURCE: &str = "https://www.eia.gov/energyexplained/electricity/delivery-to-consumers.php";
const CN_SOURCE: &str = "https://doi.org/10.1049/iet-gtd.2014.0281";
const AU_SOURCE: &str = "https://www.aemo.com.au/energy-systems/electricity/national-electricity-market-nem/about-the-national-electricity-market-nem";
const ID_SOURCE: &str = "https://www.iea.org/reports/enhancing-indonesias-power-system";

/// A generated AC area. Bus identifiers are the module's one-based bus ids.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct SynchronousArea {
    pub id: usize,
    pub name: String,
    pub bus_ids: Vec<usize>,
    pub demand_mw: f64,
    pub basis: String,
    pub source_url: Option<String>,
}

/// A synthetic, lossless DC link scheduled at a fixed active-power transfer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct PlannedHvdcLink {
    pub from_bus: usize,
    pub to_bus: usize,
    pub from_area: usize,
    pub to_area: usize,
    pub rating_mw: f64,
    pub transfer_mw: f64,
}

/// Extra JSON fields without changing existing Rust report constructors.
#[derive(Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "schema", schemars(rename = "GenerationReport"))]
pub struct GenerationReportOutput<'a> {
    #[serde(flatten)]
    pub report: &'a crate::report::GenerationReport,
    #[serde(default)]
    pub synchronous_areas: &'a [SynchronousArea],
    #[serde(default)]
    pub planned_hvdc_links: &'a [PlannedHvdcLink],
}

fn area_name(country: &str, lon: f64, lat: f64, polygon: usize) -> (String, Option<&'static str>) {
    let lon = crate::geom::wrap_lon(lon);
    let (name, source) = match country {
        "USA" => (
            if lon < -104.0 {
                "Western"
            } else if lon < -94.0 && lat < 34.5 {
                "Texas"
            } else {
                "Eastern"
            },
            US_SOURCE,
        ),
        "CHN" => (
            if lat > 40.0 && lon > 118.0 {
                "Northeast"
            } else if lon < 105.0 && lat > 32.0 {
                "Northwest"
            } else if lat < 27.5 {
                "Southern"
            } else if lon > 117.0 && lat < 36.0 {
                "Eastern"
            } else if lat > 35.0 {
                "Northern"
            } else {
                "Central"
            },
            CN_SOURCE,
        ),
        "AUS" => (
            if lat < -39.0 {
                "Tasmania"
            } else if lon < 129.0 {
                "Western Australia"
            } else if lat > -26.0 && lon < 138.0 {
                "Northern Territory"
            } else {
                "Mainland NEM"
            },
            AU_SOURCE,
        ),
        "IDN" => (
            if lon < 106.0 && lat > -6.5 {
                "Sumatra"
            } else if lat < -5.0 && lon < 116.0 {
                "Java and Bali"
            } else if lon < 119.0 {
                "Kalimantan"
            } else if lon < 125.0 && lat > -5.5 {
                "Sulawesi"
            } else if lon >= 130.0 {
                "Papua"
            } else {
                "Eastern islands"
            },
            ID_SOURCE,
        ),
        _ => return (format!("Landmass {}", polygon + 1), None),
    };
    (format!("{country} {name}"), Some(source))
}

/// Assign every co-located site to one area before AC topology is built.
/// A country's table is selected from its populated places, not the label
/// supplied by the user. Existing exclusions of distant landmasses remain.
pub(crate) fn partition(buses: &mut [DraftBus], data: &ContextData) -> Vec<SynchronousArea> {
    // Generalised country outlines can contain a small neighbouring border
    // town. Do not create an extra electrical area from that town's country.
    let mut country_demand = BTreeMap::<&str, f64>::new();
    for city in &data.cities {
        *country_demand.entry(&city.place.iso_a3).or_default() += city.demand_weight;
    }
    let total: f64 = country_demand.values().sum();
    let dominant = country_demand
        .iter()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .filter(|(_, demand)| total > 0.0 && **demand >= 0.95 * total)
        .map(|(&country, _)| country);
    let mut keys = BTreeMap::<(usize, String), usize>::new();
    let mut areas = Vec::<SynchronousArea>::new();
    let mut sites = BTreeMap::<usize, usize>::new();
    for (row, bus) in buses.iter_mut().enumerate() {
        let nearest = data.cities.iter().min_by(|a, b| {
            haversine_km(bus.lon, bus.lat, a.lon, a.lat)
                .total_cmp(&haversine_km(bus.lon, bus.lat, b.lon, b.lat))
        });
        let country = dominant
            .or_else(|| nearest.map(|c| c.place.iso_a3.as_str()))
            .unwrap_or("");
        let (name, source) = area_name(
            country,
            crate::geom::wrap_lon(bus.lon),
            bus.lat,
            bus.polygon,
        );
        // For countries without a table, polygons still joined by the
        // context stage retain that grouping. Named regions stay within an
        // existing group, so this cannot reconnect a deliberately kept island.
        let key_name = if source.is_none() {
            String::new()
        } else {
            name.clone()
        };
        let next = areas.len();
        let area = *sites.entry(bus.site).or_insert_with(|| {
            *keys.entry((bus.island, key_name)).or_insert_with(|| {
                areas.push(SynchronousArea {
                    id: next,
                    name,
                    bus_ids: Vec::new(),
                    demand_mw: 0.0,
                    basis: if source.is_some() {
                        "Approximate geographic division for a synthetic network; not an operating boundary".to_owned()
                    } else {
                        "Landmass and distance grouping from the generation request".to_owned()
                    },
                    source_url: source.map(str::to_owned),
                });
                next
            })
        });
        bus.island = area;
        areas[area].bus_ids.push(row + 1);
        areas[area].demand_mw += bus.load_mw;
    }
    areas
}

/// Connect areas by their shortest high-voltage links. A common economic
/// dispatch supplies the desired net exports; flows on the area tree follow
/// from conservation. Scale the whole schedule to keep each link below 5%
/// of the smaller area's peak demand, divided by its number of neighbours.
/// Rate each link at its scheduled transfer divided by 0.8, with a 1 MW floor.
pub(crate) fn plan_links(
    buses: &[DraftBus],
    generators: &[DraftGenerator],
    areas: &[SynchronousArea],
) -> (Vec<DraftHvdc>, Vec<PlannedHvdcLink>) {
    if areas.len() < 2 {
        return (Vec::new(), Vec::new());
    }
    let candidates: Vec<Vec<usize>> = areas
        .iter()
        .map(|area| {
            let highest = area
                .bus_ids
                .iter()
                .map(|&id| buses[id - 1].base_kv)
                .fold(0.0, f64::max);
            area.bus_ids
                .iter()
                .map(|id| id - 1)
                .filter(|&row| buses[row].base_kv >= highest - 1e-6)
                .collect()
        })
        .collect();
    let mut joined = vec![false; areas.len()];
    joined[0] = true;
    let mut newest = 0;
    type LinkChoice = (f64, usize, usize, usize, usize);
    let mut closest: Vec<Option<LinkChoice>> = vec![None; areas.len()];
    let mut edges = Vec::<(usize, usize, usize, usize)>::new();
    while edges.len() + 1 < areas.len() {
        // Only the new area's distances can improve the retained choices.
        for b in 0..areas.len() {
            if joined[b] {
                continue;
            }
            for &u in &candidates[newest] {
                for &v in &candidates[b] {
                    let distance =
                        haversine_km(buses[u].lon, buses[u].lat, buses[v].lon, buses[v].lat);
                    let candidate = (distance, newest, b, u, v);
                    if closest[b]
                        .as_ref()
                        .is_none_or(|previous| candidate < *previous)
                    {
                        closest[b] = Some(candidate);
                    }
                }
            }
        }
        let best = closest
            .iter()
            .enumerate()
            .filter(|(b, _)| !joined[*b])
            .filter_map(|(_, choice)| *choice)
            .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let Some((_, a, b, u, v)) = best else { break };
        joined[b] = true;
        newest = b;
        edges.push((a, b, u, v));
    }
    let units: Vec<_> = generators
        .iter()
        .map(|g| (g.pmin_mw, g.pmax_mw, g.c1, g.c2))
        .collect();
    let dispatch = economic_dispatch(&units, areas.iter().map(|a| a.demand_mw).sum());
    let mut export: Vec<f64> = areas.iter().map(|a| -a.demand_mw).collect();
    for (g, p) in generators.iter().zip(dispatch) {
        export[buses[g.bus].island] += p;
    }
    let mut degree = vec![0usize; areas.len()];
    for &(a, b, _, _) in &edges {
        degree[a] += 1;
        degree[b] += 1;
    }
    let mut flows = vec![0.0; edges.len()];
    let mut scale = 1.0f64;
    for (k, &(a, b, _, _)) in edges.iter().enumerate().rev() {
        flows[k] = -export[b];
        export[a] += export[b];
        let budget = 0.05
            * (areas[a].demand_mw / degree[a] as f64).min(areas[b].demand_mw / degree[b] as f64);
        if flows[k].abs() > 1e-9 {
            scale = scale.min(budget / flows[k].abs());
        }
    }
    let mut links = Vec::new();
    let mut report = Vec::new();
    for ((a, b, u, v), raw) in edges.into_iter().zip(flows) {
        let transfer = raw * scale;
        let (from, to, from_area, to_area) = if transfer >= 0.0 {
            (u, v, a, b)
        } else {
            (v, u, b, a)
        };
        let transfer = transfer.abs();
        let rating = (transfer / 0.8).max(1.0);
        links.push(DraftHvdc {
            from,
            to,
            rating_mw: rating,
            setpoint_mw: transfer,
            circuits: 1,
            osm_ids: Vec::new(),
        });
        report.push(PlannedHvdcLink {
            from_bus: from + 1,
            to_bus: to + 1,
            from_area,
            to_area,
            rating_mw: rating,
            transfer_mw: transfer,
        });
    }
    (links, report)
}

/// The corridor and reactive plans see exactly the fixed injections that
/// the validation solve will use. The saved loads retain their original values.
pub(crate) fn apply_transfers(buses: &mut [DraftBus], links: &[DraftHvdc], sign: f64) {
    for link in links {
        buses[link.from].load_mw += sign * link.setpoint_mw;
        buses[link.to].load_mw -= sign * link.setpoint_mw;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::{BusKind, ElementSource};
    use crate::request::Fuel;

    fn bus(lon: f64, island: usize) -> DraftBus {
        DraftBus {
            lon,
            lat: 35.0,
            name: "Test bus".into(),
            place: None,
            tier: 0,
            base_kv: 345.0,
            kind: BusKind::Load,
            confidence: 1.0,
            polygon: 0,
            island,
            source: ElementSource::Synthetic,
            population_weight: 0.5,
            load_mw: 100.0,
            load_mvar: 20.0,
            annotation: String::new(),
            site: island,
        }
    }

    fn generator(bus: usize, cost: f64) -> DraftGenerator {
        DraftGenerator {
            bus,
            fuel: Fuel::Gas,
            pmax_mw: 300.0,
            pmin_mw: 0.0,
            qmax_mvar: 100.0,
            qmin_mvar: -100.0,
            c2: 0.01,
            c1: cost,
            c0: 0.0,
            makeup: false,
            slack: true,
            condenser: false,
        }
    }

    fn areas(count: usize) -> Vec<SynchronousArea> {
        (0..count)
            .map(|i| SynchronousArea {
                id: i,
                name: format!("Area {i}"),
                bus_ids: vec![i + 1],
                demand_mw: 100.0,
                basis: "Test".into(),
                source_url: None,
            })
            .collect()
    }

    #[test]
    fn known_interior_locations_use_separate_regions() {
        for (country, locations) in [
            ("USA", vec![(-118.2, 34.1), (-97.7, 30.3), (-74.0, 40.7)]),
            (
                "CHN",
                vec![(126.6, 45.7), (87.6, 43.8), (113.3, 23.1), (121.5, 31.2)],
            ),
            (
                "AUS",
                vec![
                    (115.9, -31.9),
                    (130.8, -12.5),
                    (151.2, -33.9),
                    (147.3, -42.9),
                ],
            ),
            (
                "IDN",
                vec![(98.7, 3.6), (106.8, -6.2), (109.3, 0.0), (119.4, -5.1)],
            ),
        ] {
            let names: std::collections::BTreeSet<_> = locations
                .iter()
                .map(|&(lon, lat)| area_name(country, lon, lat, 0).0)
                .collect();
            assert_eq!(names.len(), locations.len(), "{country}");
        }
    }

    #[test]
    fn continuous_longitudes_and_minor_border_towns_do_not_merge_or_split_regions() {
        use crate::places::{Place, PlacedCity};
        let locations = [
            ("USA", -118.2, 34.1, 1000.0),
            ("USA", -97.7, 30.3, 1000.0),
            ("USA", -74.0, 40.7, 1000.0),
            ("CAN", -84.3, 46.5, 1.0),
        ];
        let data = ContextData {
            shape: crate::geom::region::RegionShape::from_geometry(
                &crate::Region::bbox(-125.0, 25.0, -68.0, 49.0, "arbitrary label").geometry,
            )
            .unwrap(),
            cities: locations
                .iter()
                .map(|&(country, lon, lat, demand)| PlacedCity {
                    lon,
                    lat,
                    demand_weight: demand,
                    snapped: false,
                    place: Place {
                        name: country.into(),
                        lon,
                        lat,
                        population: demand,
                        iso_a3: country.into(),
                        subdivision: None,
                    },
                })
                .collect(),
            polygon_weights: vec![1.0],
            polygon_group: vec![Some(0)],
            group_count: 1,
        };
        let mut buses: Vec<_> = locations
            .iter()
            .enumerate()
            .map(|(i, &(_, lon, lat, _))| {
                let mut b = bus(lon + 360.0, 0);
                b.lat = lat;
                b.site = i;
                b
            })
            .collect();
        let areas = partition(&mut buses, &data);
        assert_eq!(areas.len(), 3);
        assert_eq!(areas[buses[0].island].name, "USA Western");
        assert_eq!(areas[buses[1].island].name, "USA Texas");
        assert_eq!(areas[buses[2].island].name, "USA Eastern");
        assert_eq!(buses[2].island, buses[3].island);
    }

    #[test]
    fn transfer_direction_follows_cost_and_planning_loads_balance() {
        let buses = vec![bus(-105.0, 0), bus(-103.0, 1)];
        for costs in [[5.0, 50.0], [50.0, 5.0]] {
            let generators = vec![generator(0, costs[0]), generator(1, costs[1])];
            let (links, report) = plan_links(&buses, &generators, &areas(2));
            assert_eq!(links.len(), 1);
            let link = &links[0];
            assert_eq!(link.from, usize::from(costs[0] > costs[1]));
            assert!((link.setpoint_mw - 5.0).abs() < 1e-9);
            assert!((link.rating_mw - 6.25).abs() < 1e-9);
            assert_eq!(report[0].from_bus, link.from + 1);
            let mut planning = buses.clone();
            apply_transfers(&mut planning, &links, 1.0);
            assert_eq!(planning[link.from].load_mw, 105.0);
            assert_eq!(planning[link.to].load_mw, 95.0);
            assert_eq!(planning[0].load_mvar, buses[0].load_mvar);
            apply_transfers(&mut planning, &links, -1.0);
            assert_eq!(planning, buses);
        }
    }

    #[test]
    fn tree_schedule_keeps_every_area_within_its_trade_budget() {
        let buses = vec![bus(-110.0, 0), bus(-100.0, 1), bus(-90.0, 2)];
        let generators = vec![generator(0, 5.0), generator(1, 30.0), generator(2, 50.0)];
        let (links, report) = plan_links(&buses, &generators, &areas(3));
        assert_eq!(links.len(), 2);
        assert_eq!(plan_links(&buses, &generators, &areas(3)).1, report);
        let mut traded = [0.0; 3];
        for link in &links {
            traded[buses[link.from].island] += link.setpoint_mw;
            traded[buses[link.to].island] += link.setpoint_mw;
            assert!(link.setpoint_mw <= link.rating_mw);
            assert_ne!(buses[link.from].island, buses[link.to].island);
        }
        assert!(traded.iter().all(|&mw| mw <= 5.0 + 1e-9));
    }
}
