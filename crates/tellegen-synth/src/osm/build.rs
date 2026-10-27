//! The OpenStreetMap grid build. Substations become buses, one per voltage
//! level at a site, joined by transformers; transmission ways become
//! corridors after fragmented ways are stitched, untagged voltages inferred
//! and line ends snapped to the bus of their voltage; DC lines between
//! converter stations become HVDC links; power plants become generators.
//! Electrical parameters and loads stay modelled (the cited class tables over
//! the real way lengths, population weighted demand), so the grid is mapped,
//! not surveyed.

use std::collections::BTreeMap;

use crate::build::DraftHvdc;
use crate::context::ContextData;
use crate::electrical::generators::DraftGenerator;
use crate::electrical::params::{
    bus_generation_cap_mw, fuel_params, MIN_UNIT_MW, QMAX_FRACTION, QMIN_FRACTION,
};
use crate::error::SynthError;
use crate::geom::delaunay::UnionFind;
use crate::geom::{haversine_km, lift_lon};
use crate::osm::parse::{OsmLine, OsmPlant, ParsedDocument, MAX_OSM_CIRCUITS};
use crate::report::{BusKind, ElementSource, RegionContext, Warning};
use crate::request::{Fuel, OsmIslands, OsmOptions, SynthRequest};
use crate::rng::Stream;
use crate::site::DraftBus;
use crate::topology::{corridor_degrees, Corridor, CorridorRole, TopologyOutput};

/// Length given to the short connection inside a substation that carries a
/// transformer between two of its voltage levels, km.
const SITE_TRANSFORMER_KM: f64 = 0.3;

/// Rating per circuit of an HVDC link mapped from OSM, MW. source: monopole
/// and bipole ratings of HVDC links in ENTSO-E's TYNDP project sheets and the
/// Hitachi Energy and Siemens Energy reference lists run from about 0.5 to
/// 3 GW per circuit; 1 GW is a conservative mid value for an untagged link.
pub const HVDC_MW_PER_CIRCUIT: f64 = 1000.0;

/// Capacity given to a plant OSM tags no output for: a modest share of the
/// regional demand, bounded so an untagged plant neither vanishes nor
/// dominates. A modelling choice, recorded per generator as estimated.
fn estimated_plant_mw(total_load_mw: f64) -> f64 {
    (0.04 * total_load_mw.max(1.0)).clamp(20.0, 200.0)
}

/// A generator this close to a mapped plant of the same fuel is one of its units, km.
const PLANT_UNIT_KM: f64 = 3.0;

/// Makeup units spread the shortfall over at least this many buses.
const MAKEUP_MIN_BUSES: usize = 5;

/// Counters the provenance block reports.
#[derive(Clone, Debug, Default)]
pub struct OsmCounts {
    pub substation_sites: usize,
    pub converters: usize,
    pub clipped: usize,
    pub lines_stitched: usize,
    pub lines_dropped: usize,
    pub voltage_inferred: usize,
    pub transformers: usize,
    pub interconnects: usize,
    pub hvdc_links: usize,
    pub buses_cut: usize,
    pub plants_snapped: usize,
    pub plants_estimated: usize,
    pub makeup_units: usize,
}

impl OsmCounts {
    pub fn as_map(&self) -> BTreeMap<String, u64> {
        [
            ("substation_sites", self.substation_sites),
            ("converters", self.converters),
            ("clipped", self.clipped),
            ("lines_stitched", self.lines_stitched),
            ("lines_dropped", self.lines_dropped),
            ("voltage_inferred", self.voltage_inferred),
            ("transformers", self.transformers),
            ("interconnects", self.interconnects),
            ("hvdc_links", self.hvdc_links),
            ("buses_cut", self.buses_cut),
            ("plants_snapped", self.plants_snapped),
            ("plants_estimated", self.plants_estimated),
            ("makeup_units", self.makeup_units),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v as u64))
        .collect()
    }
}

/// Greedy single-pass clustering of points within `radius_km`: each point
/// joins the first cluster whose running centre is in range, else starts one.
/// Returns, per cluster, its centre and its member indices.
pub fn cluster_points(points: &[(f64, f64)], radius_km: f64) -> Vec<((f64, f64), Vec<usize>)> {
    let mut clusters: Vec<((f64, f64), Vec<usize>)> = Vec::new();
    let dlat = radius_km / 111.0;
    for (i, &(lon, lat)) in points.iter().enumerate() {
        let hit = clusters.iter().position(|((clon, clat), _)| {
            (clat - lat).abs() <= dlat && haversine_km(lon, lat, *clon, *clat) <= radius_km
        });
        match hit {
            Some(k) => {
                let ((clon, clat), members) = &mut clusters[k];
                let n = members.len() as f64;
                *clon = (*clon * n + lon) / (n + 1.0);
                *clat = (*clat * n + lat) / (n + 1.0);
                members.push(i);
            }
            None => clusters.push(((lon, lat), vec![i])),
        }
    }
    clusters
}

/// The nearest bus to `(lon, lat)` within `tol_km`. With a voltage, the
/// nearest bus at that voltage wins when it is in range, so a 110 kV line
/// lands on the 110 kV bus of a site with several levels.
pub fn snap(
    buses: &[DraftBus],
    lon: f64,
    lat: f64,
    voltage: Option<f64>,
    tol_km: f64,
) -> Option<usize> {
    let mut best: Option<(f64, usize)> = None;
    let mut best_at_voltage: Option<(f64, usize)> = None;
    let dlat = tol_km / 111.0;
    for (i, b) in buses.iter().enumerate() {
        if (b.lat - lat).abs() > dlat {
            continue;
        }
        let d = haversine_km(lon, lat, b.lon, b.lat);
        if d > tol_km {
            continue;
        }
        if best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, i));
        }
        if voltage.is_some_and(|v| (b.base_kv - v).abs() < 1e-6)
            && best_at_voltage.is_none_or(|(bd, _)| d < bd)
        {
            best_at_voltage = Some((d, i));
        }
    }
    best_at_voltage.or(best).map(|(_, i)| i)
}

/// Coordinate key for stitching: 1e-5° (about a metre of latitude).
fn coord_key(p: (f64, f64)) -> (i64, i64) {
    ((p.0 * 1e5).round() as i64, (p.1 * 1e5).round() as i64)
}

/// Stitch fragmented ways back into whole lines. Ways of one voltage,
/// circuit count and kind join where exactly two way ends meet: a
/// pass-through. A point with three or more ends is a junction, and a point
/// within `stop_km` of a station in `stops` is where lines terminate, so
/// neither joins; a way whose two ends meet each other is left alone.
pub fn stitch_lines(lines: &[OsmLine], stops: &[(f64, f64)], stop_km: f64) -> Vec<OsmLine> {
    let at_station = |p: (f64, f64)| {
        let dlat = stop_km / 111.0;
        stops
            .iter()
            .any(|s| (s.1 - p.1).abs() <= dlat && haversine_km(p.0, p.1, s.0, s.1) <= stop_km)
    };
    let mut groups: BTreeMap<(i64, u32, bool, bool), Vec<usize>> = BTreeMap::new();
    for (i, l) in lines.iter().enumerate() {
        groups
            .entry((
                (l.kv * 1000.0).round() as i64,
                l.circuits,
                l.is_cable,
                l.is_dc,
            ))
            .or_default()
            .push(i);
    }
    let mut out = Vec::new();
    for idxs in groups.values() {
        // End touches per coordinate: (local line, is_tail).
        let mut ends: BTreeMap<(i64, i64), Vec<(usize, bool)>> = BTreeMap::new();
        for (k, &i) in idxs.iter().enumerate() {
            let g = &lines[i].geometry;
            ends.entry(coord_key(g[0])).or_default().push((k, false));
            ends.entry(coord_key(g[g.len() - 1]))
                .or_default()
                .push((k, true));
        }
        // The neighbour across each end, where it is a two-way pass-through.
        let mut next: Vec<[Option<(usize, bool)>; 2]> = vec![[None, None]; idxs.len()];
        for (key, touch) in &ends {
            if let [(a, a_tail), (b, b_tail)] = touch[..] {
                let point = (key.0 as f64 * 1e-5, key.1 as f64 * 1e-5);
                if a != b && !at_station(point) {
                    next[a][a_tail as usize] = Some((b, b_tail));
                    next[b][b_tail as usize] = Some((a, a_tail));
                }
            }
        }
        let mut used = vec![false; idxs.len()];
        // Chains start at a line with an open head or tail; what remains are rings.
        let starts: Vec<usize> = (0..idxs.len())
            .filter(|&k| next[k][0].is_none() || next[k][1].is_none())
            .chain(0..idxs.len())
            .collect();
        for start in starts {
            if used[start] {
                continue;
            }
            // Walk away from the open end (the head when both are open).
            let forward_from_head = next[start][0].is_none();
            let mut geometry = lines[idxs[start]].geometry.clone();
            if !forward_from_head {
                geometry.reverse();
            }
            let mut ids = lines[idxs[start]].ids.clone();
            used[start] = true;
            let mut cursor = (start, forward_from_head);
            loop {
                let (k, leaving_tail) = cursor;
                let Some((n, entering_tail)) = next[k][leaving_tail as usize] else {
                    break;
                };
                if used[n] {
                    break;
                }
                used[n] = true;
                let mut g = lines[idxs[n]].geometry.clone();
                if entering_tail {
                    g.reverse();
                }
                geometry.extend(g.into_iter().skip(1));
                ids.extend(lines[idxs[n]].ids.iter().copied());
                cursor = (n, !entering_tail);
            }
            let first = &lines[idxs[start]];
            out.push(OsmLine {
                ids,
                geometry,
                kv: first.kv,
                circuits: first.circuits,
                is_cable: first.is_cable,
                is_dc: first.is_dc,
            });
        }
    }
    out
}

fn polyline_km(geometry: &[(f64, f64)]) -> f64 {
    geometry
        .windows(2)
        .map(|w| haversine_km(w[0].0, w[0].1, w[1].0, w[1].1))
        .sum()
}

/// The most frequent value (largest on ties); `default` when empty.
fn most_common(values: &[f64], default: f64) -> f64 {
    let mut counts: BTreeMap<i64, usize> = BTreeMap::new();
    for v in values {
        *counts.entry((v * 1000.0).round() as i64).or_default() += 1;
    }
    counts
        .iter()
        .max_by(|a, b| a.1.cmp(b.1).then(a.0.cmp(b.0)))
        .map(|(k, _)| *k as f64 / 1000.0)
        .unwrap_or(default)
}

/// A build in progress, carried by the session between stages.
pub struct OsmBuild {
    parsed: ParsedDocument,
    options: OsmOptions,
    plants: Vec<OsmPlant>,
    /// Bus indices per substation site, highest voltage first.
    sites: Vec<Vec<usize>>,
    /// OSM way ids per corridor.
    corridor_ids: Vec<Vec<i64>>,
    hvdc: Vec<DraftHvdc>,
    pub counts: OsmCounts,
}

impl OsmBuild {
    pub fn new(parsed: ParsedDocument) -> Self {
        Self {
            parsed,
            options: OsmOptions::default(),
            plants: Vec::new(),
            sites: Vec::new(),
            corridor_ids: Vec::new(),
            hvdc: Vec::new(),
            counts: OsmCounts::default(),
        }
    }

    /// Buses from substations and converter stations inside the region (or
    /// within the clip buffer of it), in the region's continuous frame.
    ///
    /// When the mapped substation levels and converters make more than
    /// `limits.max_bus_count` buses, the lowest substation level present is
    /// dropped, one at a time, until they fit; the raised floor also drops
    /// the lines below it in the topology stage. A region whose highest level
    /// alone exceeds the cap is refused.
    pub fn site(
        &mut self,
        req: &SynthRequest,
        _ctx: &RegionContext,
        data: &ContextData,
        _rng: &mut Stream,
        warnings: &mut Vec<Warning>,
    ) -> Result<Vec<DraftBus>, SynthError> {
        self.options = req.osm.clone().unwrap_or_default();
        let opts = &self.options;
        let frame = data.shape.frame;
        let inside =
            |lon: f64, lat: f64| data.shape.contains_or_near(lon, lat, opts.clip_buffer_km);
        let mut clipped = 0usize;

        let mut subs: Vec<((f64, f64), Vec<f64>)> = Vec::new();
        for s in &self.parsed.substations {
            let kvs: Vec<f64> = s
                .kvs
                .iter()
                .copied()
                .filter(|kv| *kv >= opts.voltage_floor_kv)
                .collect();
            if kvs.is_empty() {
                continue;
            }
            let lon = lift_lon(s.lon, &frame);
            if !inside(lon, s.lat) {
                clipped += 1;
                continue;
            }
            subs.push(((lon, s.lat), kvs));
        }
        let mut converters: Vec<((f64, f64), f64)> = Vec::new();
        for c in &self.parsed.converters {
            let lon = lift_lon(c.lon, &frame);
            if !inside(lon, c.lat) {
                clipped += 1;
                continue;
            }
            converters.push(((lon, c.lat), c.kv));
        }
        self.plants.clear();
        for p in &self.parsed.plants {
            let lon = lift_lon(p.lon, &frame);
            if !inside(lon, p.lat) {
                clipped += 1;
                continue;
            }
            self.plants.push(OsmPlant { lon, ..p.clone() });
        }
        self.counts.clipped = clipped;
        let requested_floor = opts.voltage_floor_kv;

        // Substation levels present, lowest first: the floors a cap can raise to.
        let mut levels: Vec<f64> = subs.iter().flat_map(|s| s.1.iter().copied()).collect();
        levels.sort_by(|a, b| a.partial_cmp(b).unwrap());
        levels.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
        let cap = req.limits.max_bus_count as usize;
        let converter_buses = self.converter_buses(&converters, requested_floor, data);
        let mut buses = self.substation_buses(&subs, requested_floor, data);
        let mapped = buses.len() + converter_buses.len();
        while buses.len() + converter_buses.len() > cap {
            let floor = self.options.voltage_floor_kv;
            let Some(next) = levels.iter().copied().find(|kv| *kv > floor + 1e-9) else {
                return Err(SynthError::InvalidRequest(format!(
                    "the region's OpenStreetMap data maps to {} buses at its highest level ({floor} kV), more than limits.max_bus_count = {cap}; choose a smaller region or raise the cap",
                    buses.len() + converter_buses.len()
                )));
            };
            self.options.voltage_floor_kv = next;
            buses = self.substation_buses(&subs, next, data);
        }
        buses.extend(converter_buses);
        let opts = &self.options;
        if opts.voltage_floor_kv > requested_floor {
            warnings.push(Warning::new(
                "osm_bus_count_capped",
                format!(
                    "{mapped} OpenStreetMap buses at or above {requested_floor} kV exceed limits.max_bus_count = {cap}; substation levels below {} kV were left out, which leaves {} buses",
                    opts.voltage_floor_kv,
                    buses.len()
                ),
            ));
        }

        // Tier: rank of the bus voltage among the levels present (0 = highest).
        let mut levels: Vec<f64> = buses.iter().map(|b| b.base_kv).collect();
        levels.sort_by(|a, b| b.partial_cmp(a).unwrap());
        levels.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
        for b in &mut buses {
            b.tier = levels
                .iter()
                .position(|l| (l - b.base_kv).abs() < 1e-9)
                .unwrap_or(0);
        }
        if clipped > 0 {
            warnings.push(Warning::new(
                "osm_clipped",
                format!(
                    "{clipped} OpenStreetMap substation(s), converter(s) or plant(s) lie outside the region (beyond the {} km buffer) and were left out",
                    opts.clip_buffer_km
                ),
            ));
        }
        if buses.len() < opts.min_buses {
            return Err(SynthError::OsmInsufficient {
                buses: buses.len(),
                branches: 0,
                message: format!(
                    "fewer than {} transmission substations at or above {} kV in the region",
                    opts.min_buses, opts.voltage_floor_kv
                ),
            });
        }
        Ok(buses)
    }

    /// One bus per voltage level at or above `floor` at each substation
    /// site. Records the sites and their count.
    fn substation_buses(
        &mut self,
        subs: &[((f64, f64), Vec<f64>)],
        floor: f64,
        data: &ContextData,
    ) -> Vec<DraftBus> {
        let kept: Vec<&((f64, f64), Vec<f64>)> = subs
            .iter()
            .filter(|s| s.1.iter().any(|kv| *kv >= floor))
            .collect();
        let points: Vec<(f64, f64)> = kept.iter().map(|s| s.0).collect();
        let mut buses = Vec::new();
        let site_clusters = cluster_points(&points, self.options.cluster_radius_m / 1000.0);
        self.counts.substation_sites = site_clusters.len();
        self.sites.clear();
        for (site, ((lon, lat), members)) in site_clusters.into_iter().enumerate() {
            let mut kvs: Vec<f64> = members
                .iter()
                .flat_map(|&m| kept[m].1.iter().copied())
                .filter(|kv| *kv >= floor)
                .collect();
            kvs.sort_by(|a, b| b.partial_cmp(a).unwrap());
            kvs.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
            let mut ids = Vec::new();
            for kv in kvs {
                ids.push(buses.len());
                buses.push(osm_bus(
                    data,
                    lon,
                    lat,
                    kv,
                    format!("OSM Substation {:03} / {} kV", site + 1, kv),
                    BusKind::Substation,
                    "OpenStreetMap substation (power=substation)",
                    0.9,
                ));
            }
            self.sites.push(ids);
        }
        buses
    }

    /// One bus per converter station, at its tagged voltage or `floor`.
    fn converter_buses(
        &mut self,
        converters: &[((f64, f64), f64)],
        floor: f64,
        data: &ContextData,
    ) -> Vec<DraftBus> {
        let points: Vec<(f64, f64)> = converters.iter().map(|c| c.0).collect();
        let converter_clusters = cluster_points(&points, self.options.cluster_radius_m / 1000.0);
        self.counts.converters = converter_clusters.len();
        let mut buses = Vec::new();
        for (k, ((lon, lat), members)) in converter_clusters.into_iter().enumerate() {
            let tagged = members.iter().map(|&m| converters[m].1).fold(0.0, f64::max);
            let kv = tagged.max(floor);
            buses.push(osm_bus(
                data,
                lon,
                lat,
                kv,
                format!("OSM Converter {:03} / {} kV", k + 1, kv),
                BusKind::Converter,
                "OpenStreetMap converter station (power=converter)",
                0.85,
            ));
        }
        buses
    }

    /// Corridors from the mapped lines, transformers inside multi-level
    /// sites, HVDC links between converters, and the island policy.
    pub fn topology(
        &mut self,
        buses: &mut Vec<DraftBus>,
        data: &ContextData,
        warnings: &mut Vec<Warning>,
    ) -> Result<TopologyOutput, SynthError> {
        let opts = self.options.clone();
        let frame = data.shape.frame;
        let mut corridors: Vec<Corridor> = Vec::new();
        let mut ids: Vec<Vec<i64>> = Vec::new();

        for site in &self.sites {
            for pair in site.windows(2) {
                let (hi, lo) = (pair[0], pair[1]);
                corridors.push(Corridor {
                    a: hi,
                    b: lo,
                    kv: buses[hi].base_kv,
                    length_km: SITE_TRANSFORMER_KM,
                    role: CorridorRole::Internal,
                    circuits: 1,
                    route: vec![
                        (buses[hi].lon, buses[hi].lat),
                        (buses[lo].lon, buses[lo].lat),
                    ],
                    is_delaunay: false,
                });
                ids.push(Vec::new());
            }
        }
        self.counts.transformers = corridors.len();

        let lifted: Vec<OsmLine> = self
            .parsed
            .lines
            .iter()
            .map(|l| OsmLine {
                geometry: l
                    .geometry
                    .iter()
                    .map(|&(lon, lat)| (lift_lon(lon, &frame), lat))
                    .collect(),
                ..l.clone()
            })
            .collect();
        let stations: Vec<(f64, f64)> = buses.iter().map(|b| (b.lon, b.lat)).collect();
        let stitched = stitch_lines(&lifted, &stations, opts.snap_km);
        self.counts.lines_stitched = lifted.len() - stitched.len();
        let tagged_kvs: Vec<f64> = stitched
            .iter()
            .filter(|l| l.kv > 0.0 && !l.is_dc)
            .map(|l| l.kv)
            .collect();
        let default_kv = most_common(&tagged_kvs, opts.voltage_floor_kv);

        // AC corridors, parallel ways between one bus pair at one voltage aggregated.
        let mut aggregate: BTreeMap<(usize, usize, i64), usize> = BTreeMap::new();
        let mut dropped = 0usize;
        let mut inferred = 0usize;
        for line in stitched.iter().filter(|l| !l.is_dc) {
            let (head, tail) = (line.geometry[0], line.geometry[line.geometry.len() - 1]);
            let mut kv = line.kv;
            let guessed = kv <= 0.0;
            if guessed {
                let ends: Vec<f64> = [head, tail]
                    .iter()
                    .filter_map(|p| snap(buses, p.0, p.1, None, opts.snap_km))
                    .map(|i| buses[i].base_kv)
                    .collect();
                kv = ends.iter().copied().fold(f64::INFINITY, f64::min);
                if !kv.is_finite() {
                    kv = default_kv;
                }
            }
            if kv < opts.voltage_floor_kv {
                dropped += 1;
                continue;
            }
            let from = snap(buses, head.0, head.1, Some(kv), opts.snap_km);
            let to = snap(buses, tail.0, tail.1, Some(kv), opts.snap_km);
            let (Some(from), Some(to)) = (from, to) else {
                dropped += 1;
                continue;
            };
            if from == to {
                dropped += 1;
                continue;
            }
            if guessed {
                inferred += 1;
            }
            let key = (from.min(to), from.max(to), (kv * 1000.0).round() as i64);
            if let Some(&k) = aggregate.get(&key) {
                corridors[k].circuits = corridors[k]
                    .circuits
                    .saturating_add(line.circuits)
                    .min(MAX_OSM_CIRCUITS);
                ids[k].extend(line.ids.iter().copied());
                continue;
            }
            let mut route = line.geometry.clone();
            if from != key.0 {
                route.reverse();
            }
            let role = if data.shape.polyline_inside(&route) {
                CorridorRole::Internal
            } else {
                CorridorRole::TieLine
            };
            aggregate.insert(key, corridors.len());
            corridors.push(Corridor {
                a: key.0,
                b: key.1,
                kv,
                length_km: polyline_km(&route).max(1.0),
                role,
                circuits: line.circuits,
                route,
                is_delaunay: false,
            });
            ids.push(line.ids.clone());
        }

        // HVDC links: DC ways snapped with the wider converter tolerance.
        let mut dc: BTreeMap<(usize, usize), DraftHvdc> = BTreeMap::new();
        for line in stitched.iter().filter(|l| l.is_dc) {
            let (head, tail) = (line.geometry[0], line.geometry[line.geometry.len() - 1]);
            let from = snap(buses, head.0, head.1, None, opts.hvdc_snap_km);
            let to = snap(buses, tail.0, tail.1, None, opts.hvdc_snap_km);
            let (Some(from), Some(to)) = (from, to) else {
                dropped += 1;
                continue;
            };
            if from == to {
                dropped += 1;
                continue;
            }
            let link = dc
                .entry((from.min(to), from.max(to)))
                .or_insert_with(|| DraftHvdc {
                    from: from.min(to),
                    to: from.max(to),
                    rating_mw: 0.0,
                    setpoint_mw: 0.0,
                    circuits: 0,
                    osm_ids: Vec::new(),
                });
            link.rating_mw += HVDC_MW_PER_CIRCUIT * line.circuits as f64;
            link.circuits = link
                .circuits
                .saturating_add(line.circuits)
                .min(MAX_OSM_CIRCUITS);
            link.osm_ids.extend(line.ids.iter().copied());
        }
        self.hvdc = dc.into_values().collect();
        self.counts.hvdc_links = self.hvdc.len();
        self.counts.lines_dropped = dropped;
        self.counts.voltage_inferred = inferred;
        let considered = stitched.len();
        if considered > 0 && dropped * 2 >= considered {
            warnings.push(Warning::new(
                "osm_lines_dropped",
                format!(
                    "{dropped} of {considered} OpenStreetMap line(s) were dropped (below {} kV, or an end farther than {} km from any substation); the topology leans on the island policy",
                    opts.voltage_floor_kv, opts.snap_km
                ),
            ));
        }

        // Islands.
        let mut interconnects = 0;
        match opts.islands {
            OsmIslands::Connect => {
                interconnects = connect_components(buses, &mut corridors);
                ids.resize(corridors.len(), Vec::new());
            }
            OsmIslands::Cut => {
                let cut = self.cut_islands_without_plants(buses, &mut corridors, &mut ids);
                self.counts.buses_cut = cut;
                if cut > 0 {
                    warnings.push(Warning::new(
                        "osm_islands_cut",
                        format!("{cut} bus(es) on islands with no mapped power plant were cut (osm.islands = cut)"),
                    ));
                }
            }
            OsmIslands::Keep => {}
        }
        self.counts.interconnects = interconnects;

        // Electrical islands become the buses' island index.
        let mut uf = UnionFind::new(buses.len());
        for c in &corridors {
            uf.union(c.a, c.b);
        }
        let mut index: BTreeMap<usize, usize> = BTreeMap::new();
        for (i, bus) in buses.iter_mut().enumerate() {
            let root = uf.find(i);
            let next = index.len();
            bus.island = *index.entry(root).or_insert(next);
        }
        let lines = corridors.len() - self.counts.transformers.min(corridors.len());
        if buses.len() < opts.min_buses || corridors.len() < opts.min_branches || lines == 0 {
            return Err(SynthError::OsmInsufficient {
                buses: buses.len(),
                branches: corridors.len(),
                message: format!(
                    "the mapped network has {} bus(es) and {} corridor(s) ({} line(s)) after snapping",
                    buses.len(),
                    corridors.len(),
                    lines
                ),
            });
        }
        self.corridor_ids = ids;
        Ok(TopologyOutput {
            corridors,
            interconnects,
        })
    }

    /// Drop the buses of every island no mapped plant snaps to, then reindex.
    fn cut_islands_without_plants(
        &mut self,
        buses: &mut Vec<DraftBus>,
        corridors: &mut Vec<Corridor>,
        ids: &mut Vec<Vec<i64>>,
    ) -> usize {
        let mut uf = UnionFind::new(buses.len());
        for c in corridors.iter() {
            uf.union(c.a, c.b);
        }
        let mut powered = std::collections::BTreeSet::new();
        for p in &self.plants {
            if let Some(i) = snap(buses, p.lon, p.lat, None, self.options.plant_snap_km) {
                powered.insert(uf.find(i));
            }
        }
        let keep: Vec<bool> = (0..buses.len())
            .map(|i| powered.contains(&uf.find(i)))
            .collect();
        let cut = keep.iter().filter(|k| !**k).count();
        if cut == 0 || cut == buses.len() {
            return 0;
        }
        let mut remap = vec![None; buses.len()];
        let mut kept = Vec::new();
        for (i, bus) in buses.drain(..).enumerate() {
            if keep[i] {
                remap[i] = Some(kept.len());
                kept.push(bus);
            }
        }
        *buses = kept;
        let mut new_corridors = Vec::new();
        let mut new_ids = Vec::new();
        for (c, i) in corridors.drain(..).zip(ids.drain(..)) {
            if let (Some(a), Some(b)) = (remap[c.a], remap[c.b]) {
                new_corridors.push(Corridor { a, b, ..c });
                new_ids.push(i);
            }
        }
        *corridors = new_corridors;
        *ids = new_ids;
        for site in &mut self.sites {
            *site = site.iter().filter_map(|&i| remap[i]).collect();
        }
        self.hvdc
            .retain_mut(|h| match (remap[h.from], remap[h.to]) {
                (Some(a), Some(b)) => {
                    h.from = a;
                    h.to = b;
                    true
                }
                _ => false,
            });
        self.counts.hvdc_links = self.hvdc.len();
        cut
    }

    /// Mapped generation sites as (lon, lat, fuel, MW), `None` MW for a plant
    /// with neither an output tag nor tagged units (estimated by the caller). A
    /// generator within [`PLANT_UNIT_KM`] of a plant of the same fuel is a unit
    /// of that plant, so the two are counted once: the plant's tagged output or
    /// its units' sum, whichever is larger. Other generators stand alone; one
    /// with no output tag (mostly rooftop solar) is left out.
    fn plant_sites(&self) -> Vec<(f64, f64, Fuel, Option<f64>)> {
        let mut plants: Vec<&OsmPlant> = self.plants.iter().filter(|p| !p.is_generator).collect();
        plants.sort_by(|a, b| a.lat.total_cmp(&b.lat));
        let dlat = PLANT_UNIT_KM / 111.0;
        let mut unit_mw = vec![0.0; plants.len()];
        let mut sites = Vec::new();
        for u in self.plants.iter().filter(|p| p.is_generator) {
            let Some(mw) = u.capacity_mw else {
                continue;
            };
            let from = plants.partition_point(|p| p.lat < u.lat - dlat);
            let owner = plants[from..]
                .iter()
                .take_while(|p| p.lat <= u.lat + dlat)
                .enumerate()
                .filter(|(_, p)| p.fuel == u.fuel)
                .map(|(k, p)| (haversine_km(u.lon, u.lat, p.lon, p.lat), from + k))
                .filter(|&(d, _)| d <= PLANT_UNIT_KM)
                .min_by(|a, b| a.0.total_cmp(&b.0));
            match owner {
                Some((_, i)) => unit_mw[i] += mw,
                None => sites.push((u.lon, u.lat, u.fuel, Some(mw))),
            }
        }
        for (p, units) in plants.iter().zip(unit_mw) {
            let mw = match p.capacity_mw {
                Some(mw) => Some(mw.max(units)),
                None if units > 0.0 => Some(units),
                None => None,
            };
            sites.push((p.lon, p.lat, p.fuel, mw));
        }
        sites
    }

    /// Generators at the mapped plants (aggregated per bus and fuel), topped up
    /// with makeup units on the best connected buses when the mapped capacity
    /// falls short of the reserve-margin target. With no usable plant, the
    /// synthetic siting rule places all generation and a warning says so.
    pub fn generators(
        &mut self,
        buses: &mut [DraftBus],
        corridors: &[Corridor],
        ctx: &RegionContext,
        req: &SynthRequest,
        rng: &mut Stream,
        warnings: &mut Vec<Warning>,
    ) -> Vec<DraftGenerator> {
        let total_load: f64 = buses.iter().map(|b| b.load_mw).sum();
        let mut capacity: BTreeMap<(usize, Fuel), f64> = BTreeMap::new();
        let mut estimated = 0usize;
        let mut snapped = 0usize;
        for (lon, lat, fuel, mw) in self.plant_sites() {
            let Some(bus) = snap(buses, lon, lat, None, self.options.plant_snap_km) else {
                continue;
            };
            let mw = mw.unwrap_or_else(|| {
                estimated += 1;
                estimated_plant_mw(total_load)
            });
            if mw <= 0.0 {
                continue;
            }
            snapped += 1;
            *capacity.entry((bus, fuel)).or_default() += mw;
        }
        self.counts.plants_snapped = snapped;
        self.counts.plants_estimated = estimated;
        if capacity.is_empty() {
            warnings.push(Warning::new(
                "osm_generation_synthetic",
                "no mapped power plant snapped to the network; generation was sited by the synthetic rule",
            ));
            return crate::electrical::generators::place_generators(
                buses, corridors, ctx, req, rng, warnings,
            );
        }
        let mut gens = Vec::new();
        let mut mapped_mw = 0.0;
        for (&(bus, fuel), &mw) in &capacity {
            buses[bus].kind = BusKind::Generator;
            buses[bus].annotation =
                "OpenStreetMap power plant (power=plant or generator)".to_owned();
            gens.push(unit(bus, fuel, mw, false, rng));
            mapped_mw += mw;
        }

        let target = (1.0 + req.reserve_margin) * total_load;
        if total_load > 0.0 && mapped_mw < target {
            let degree = corridor_degrees(buses.len(), corridors);
            let mut candidates: Vec<usize> = (0..buses.len())
                .filter(|&i| !matches!(buses[i].kind, BusKind::Generator | BusKind::Converter))
                .collect();
            candidates.sort_by(|&a, &b| degree[b].cmp(&degree[a]).then(a.cmp(&b)));
            let fuels: Vec<Fuel> = ctx.fuel_mix.keys().copied().collect();
            let weights: Vec<f64> = ctx.fuel_mix.values().copied().collect();
            let mut remaining = target - mapped_mw;
            let share = remaining / candidates.len().clamp(1, MAKEUP_MIN_BUSES) as f64;
            let mut makeup = 0usize;
            for &bus in &candidates {
                if remaining <= 0.0 {
                    break;
                }
                let mw = share
                    .min(bus_generation_cap_mw(buses[bus].base_kv))
                    .min(remaining)
                    .max(MIN_UNIT_MW);
                let fuel = rng
                    .weighted_index(&weights)
                    .map(|k| fuels[k])
                    .unwrap_or(Fuel::Gas);
                buses[bus].kind = BusKind::Generator;
                buses[bus].source = ElementSource::Makeup;
                buses[bus].annotation =
                    "Makeup generation (mapped plants fall short of the reserve margin)".to_owned();
                gens.push(unit(bus, fuel, mw, true, rng));
                remaining -= mw;
                makeup += 1;
            }
            if remaining > 1e-6 && makeup > 0 {
                // Every candidate is at its cap: the shortfall goes on the makeup
                // units in proportion, and the report says so.
                let first = gens.len() - makeup;
                let placed: f64 = gens[first..].iter().map(|g| g.pmax_mw).sum();
                for g in &mut gens[first..] {
                    let mw = g.pmax_mw * (1.0 + remaining / placed);
                    *g = DraftGenerator {
                        pmax_mw: mw,
                        qmax_mvar: QMAX_FRACTION * mw,
                        qmin_mvar: QMIN_FRACTION * mw,
                        ..g.clone()
                    };
                }
                warnings.push(Warning::new(
                    "generator_cap_exceeded",
                    "makeup units exceed the generation cap of their buses because the mapped network has too few buses to hold the reserve-margin target otherwise",
                ));
            }
            self.counts.makeup_units = makeup;
            if makeup > 0 {
                warnings.push(Warning::new(
                    "osm_makeup_generation",
                    format!(
                        "mapped plants hold {mapped_mw:.0} MW against a {target:.0} MW reserve-margin target; {makeup} makeup unit(s) cover the shortfall"
                    ),
                ));
            }
        }
        // The largest unit of each electrical island is its reference.
        let mut best: BTreeMap<usize, usize> = BTreeMap::new();
        for (k, g) in gens.iter().enumerate() {
            let island = buses[g.bus].island;
            match best.get(&island) {
                Some(&j) if gens[j].pmax_mw >= g.pmax_mw => {}
                _ => {
                    best.insert(island, k);
                }
            }
        }
        for k in best.into_values() {
            gens[k].slack = true;
        }
        gens
    }

    pub fn hvdc_links(&self) -> Vec<DraftHvdc> {
        self.hvdc.clone()
    }

    pub fn branch_osm_ids(
        &self,
        branches: &[crate::electrical::branches::DraftBranch],
    ) -> Vec<Vec<i64>> {
        branches
            .iter()
            .map(|b| {
                self.corridor_ids
                    .get(b.corridor)
                    .cloned()
                    .unwrap_or_default()
            })
            .collect()
    }

    pub fn makeup_flags(&self, generators: &[DraftGenerator]) -> Vec<bool> {
        generators.iter().map(|g| g.makeup).collect()
    }
}

#[allow(clippy::too_many_arguments)]
fn osm_bus(
    data: &ContextData,
    lon: f64,
    lat: f64,
    kv: f64,
    name: String,
    kind: BusKind,
    annotation: &str,
    confidence: f64,
) -> DraftBus {
    let polygon = data.shape.polygon_index(lon, lat).unwrap_or(0);
    DraftBus {
        lon,
        lat,
        name,
        place: None,
        tier: 0,
        base_kv: kv,
        kind,
        confidence,
        polygon,
        island: data
            .polygon_group
            .get(polygon)
            .copied()
            .flatten()
            .unwrap_or(0),
        source: ElementSource::Osm,
        population_weight: 0.0,
        load_mw: 0.0,
        load_mvar: 0.0,
        annotation: annotation.to_owned(),
    }
}

fn unit(bus: usize, fuel: Fuel, mw: f64, makeup: bool, rng: &mut Stream) -> DraftGenerator {
    let p = fuel_params(fuel);
    DraftGenerator {
        bus,
        fuel,
        pmax_mw: mw,
        pmin_mw: 0.0,
        qmax_mvar: QMAX_FRACTION * mw,
        qmin_mvar: QMIN_FRACTION * mw,
        c2: p.c2 * (0.8 + 0.4 * rng.uniform()),
        c1: rng.range(p.c1_range.0, p.c1_range.1),
        c0: 0.0,
        makeup,
        slack: false,
        condenser: false,
    }
}

/// Join every component to the one holding bus 0 with the shortest tie,
/// preferring a tie between buses of one voltage, Prim style: O(n·k) for k
/// components. Ties are tagged `interconnect`. Returns how many were added.
pub fn connect_components(buses: &[DraftBus], corridors: &mut Vec<Corridor>) -> usize {
    let n = buses.len();
    if n < 2 {
        return 0;
    }
    let mut uf = UnionFind::new(n);
    for c in corridors.iter() {
        uf.union(c.a, c.b);
    }
    if uf.components(n) <= 1 {
        return 0;
    }
    let weight = |i: usize, j: usize| {
        haversine_km(buses[i].lon, buses[i].lat, buses[j].lon, buses[j].lat)
            + if (buses[i].base_kv - buses[j].base_kv).abs() < 1e-6 {
                0.0
            } else {
                1e6
            }
    };
    let root0 = uf.find(0);
    let mut in_tree: Vec<bool> = (0..n).map(|i| uf.find(i) == root0).collect();
    let mut best: Vec<(f64, usize)> = vec![(f64::INFINITY, usize::MAX); n];
    let relax = |t: usize, in_tree: &[bool], best: &mut [(f64, usize)]| {
        for i in 0..n {
            if in_tree[i] {
                continue;
            }
            let w = weight(i, t);
            if w < best[i].0 || (w == best[i].0 && t < best[i].1) {
                best[i] = (w, t);
            }
        }
    };
    for t in 0..n {
        if in_tree[t] {
            relax(t, &in_tree, &mut best);
        }
    }
    let mut added = 0;
    loop {
        let pick = (0..n)
            .filter(|&i| !in_tree[i] && best[i].1 != usize::MAX)
            .min_by(|&a, &b| best[a].0.partial_cmp(&best[b].0).unwrap().then(a.cmp(&b)));
        let Some(i) = pick else { break };
        let j = best[i].1;
        let (a, b) = (i.min(j), i.max(j));
        let route = vec![(buses[a].lon, buses[a].lat), (buses[b].lon, buses[b].lat)];
        corridors.push(Corridor {
            a,
            b,
            kv: buses[a].base_kv.max(buses[b].base_kv),
            length_km: haversine_km(buses[a].lon, buses[a].lat, buses[b].lon, buses[b].lat)
                .max(1.0),
            role: CorridorRole::Interconnect,
            circuits: 1,
            route,
            is_delaunay: false,
        });
        added += 1;
        // The whole component of `i` joins the tree.
        let root = uf.find(i);
        let newly: Vec<usize> = (0..n)
            .filter(|&k| !in_tree[k] && uf.find(k) == root)
            .collect();
        for &k in &newly {
            in_tree[k] = true;
        }
        for &k in &newly {
            relax(k, &in_tree, &mut best);
        }
    }
    added
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(id: i64, pts: &[(f64, f64)]) -> OsmLine {
        OsmLine {
            ids: vec![id],
            geometry: pts.to_vec(),
            kv: 220.0,
            circuits: 1,
            is_cable: false,
            is_dc: false,
        }
    }

    #[test]
    fn stitching_joins_pass_throughs_but_not_junctions_or_self_loops() {
        // a-b-c chain split in two, plus a third way meeting at d with two others (a junction).
        let lines = vec![
            line(1, &[(0.0, 0.0), (1.0, 0.0)]),
            line(2, &[(2.0, 0.0), (1.0, 0.0)]),
            line(3, &[(5.0, 0.0), (6.0, 0.0)]),
            line(4, &[(6.0, 0.0), (7.0, 0.0)]),
            line(5, &[(6.0, 0.0), (6.0, 1.0)]),
            line(6, &[(9.0, 0.0), (9.5, 0.5), (9.0, 0.0)]),
        ];
        let out = stitch_lines(&lines, &[], 0.0);
        let joined = out.iter().find(|l| l.ids.contains(&1)).unwrap();
        assert_eq!(joined.ids.len(), 2);
        assert_eq!(joined.geometry.len(), 3);
        assert_eq!(joined.geometry[1], (1.0, 0.0));
        // The three ways meeting at (6, 0) stay apart.
        assert_eq!(
            out.iter()
                .filter(|l| l.ids.len() == 1 && [3, 4, 5].contains(&l.ids[0]))
                .count(),
            3
        );
        assert!(out.iter().any(|l| l.ids == vec![6]));
        // Different voltages never join.
        let mut other = line(7, &[(1.0, 0.0), (1.0, 2.0)]);
        other.kv = 110.0;
        let out = stitch_lines(&[lines[0].clone(), other], &[], 0.0);
        assert_eq!(out.len(), 2);
        // Lines that meet at a substation end there instead of passing through.
        let out = stitch_lines(&lines[..2], &[(1.0, 0.0)], 2.0);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn clustering_and_voltage_aware_snapping() {
        let clusters = cluster_points(&[(0.0, 0.0), (0.001, 0.001), (1.0, 1.0)], 0.5);
        assert_eq!(clusters.len(), 2);
        assert_eq!(clusters[0].1, vec![0, 1]);
        let bus = |lon: f64, kv: f64| DraftBus {
            lon,
            lat: 0.0,
            name: String::new(),
            place: None,
            tier: 0,
            base_kv: kv,
            kind: BusKind::Substation,
            confidence: 1.0,
            polygon: 0,
            island: 0,
            source: ElementSource::Osm,
            population_weight: 0.0,
            load_mw: 0.0,
            load_mvar: 0.0,
            annotation: String::new(),
        };
        let buses = vec![bus(0.0, 380.0), bus(0.0001, 110.0), bus(0.5, 110.0)];
        assert_eq!(snap(&buses, 0.0, 0.0, Some(110.0), 2.0), Some(1));
        assert_eq!(snap(&buses, 0.0, 0.0, Some(220.0), 2.0), Some(0));
        assert_eq!(snap(&buses, 0.2, 0.0, None, 2.0), None);
        assert_eq!(most_common(&[220.0, 110.0, 220.0], 110.0), 220.0);
        assert_eq!(most_common(&[], 110.0), 110.0);
    }
}
