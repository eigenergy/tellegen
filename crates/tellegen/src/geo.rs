use std::collections::{BTreeMap, BTreeSet};

use powerio::format::powerworld::{aux_sections, AuxFile, PwdDisplay};
use powerio::geo::{
    geo_layer_from_pwd, pwd_mercator_to_lonlat, CoordinateSpace, CoordsKind, GeoGeometry, GeoLayer,
    GeoMeta, Location,
};
use powerio::network::{Bus, Network};

pub type Coords = BTreeMap<usize, (f64, f64)>;

/// Bus id => (lon, lat). powerio promotes the substation `Latitude:1`/
/// `Longitude:1` pair into the typed `Bus.location` at parse, so that is the
/// primary source. Two PowerWorld shapes stay tellegen-side because upstream
/// deliberately leaves them in extras: older complete cases write bare
/// `Latitude`/`Longitude` on every bus row, and later exports point each bus at
/// the aux `Substation` table through `SubNum`.
pub fn network_coords(net: &Network) -> Coords {
    let subs = match aux_sections(net) {
        Some(Ok(aux)) => substation_coords(&aux),
        _ => BTreeMap::new(),
    };
    let mut out = BTreeMap::new();
    for b in &net.buses {
        let Some(p) = b
            .location
            .map(|l| (l.x, l.y))
            .or_else(|| {
                match (
                    extra_f64(b, &["Longitude:1", "Longitude"]),
                    extra_f64(b, &["Latitude:1", "Latitude"]),
                ) {
                    (Some(lon), Some(lat)) => Some((lon, lat)),
                    _ => None,
                }
            })
            .or_else(|| {
                extra_f64(b, &["SubNum", "SubNumber"])
                    .and_then(|n| subs.get(&(n as usize)).copied())
            })
        else {
            continue;
        };
        out.insert(b.id.0, p);
    }
    out
}

/// Stamp a computed layout onto the network: each `coords` entry (bus id =>
/// lon/lat) becomes that bus's `Bus.location` with `kind` provenance, and the
/// network's geo meta becomes geographic with the same default. Buses absent
/// from `coords` keep whatever location they had. Returns how many buses were
/// placed; zero leaves the geo meta untouched.
pub fn stamp_layout(net: &mut Network, coords: &Coords, kind: CoordsKind) -> usize {
    let mut placed = 0;
    for b in &mut net.buses {
        if let Some(&(lon, lat)) = coords.get(&b.id.0) {
            b.location = Some(Location {
                x: lon,
                y: lat,
                kind: Some(kind),
            });
            placed += 1;
        }
    }
    if placed > 0 {
        net.geo = Some(GeoMeta {
            space: CoordinateSpace::Geographic { crs: None },
            kind: Some(kind),
        });
    }
    placed
}

/// The `.pwd` substation layer projected to approximate longitude/latitude:
/// [`geo_layer_from_pwd`] lifts the diagram symbols, and each point runs
/// through powerio's [`pwd_mercator_to_lonlat`] inverse so
/// `apply_substation_points` lands geographic coordinates on the case.
/// Provenance is [`CoordsKind::Derived`]: the positions come from diagram
/// geometry, not surveyed geography, and hand edited diagrams drift from the
/// projection.
pub fn pwd_lonlat_layer(display: &PwdDisplay) -> GeoLayer {
    let mut layer = geo_layer_from_pwd(display);
    for f in &mut layer.features {
        if let GeoGeometry::Point([x, y]) = f.geometry {
            let (lon, lat) = pwd_mercator_to_lonlat(x, y);
            f.geometry = GeoGeometry::Point([lon, lat]);
        }
        f.kind = Some(CoordsKind::Derived);
    }
    layer.space = CoordinateSpace::Geographic { crs: None };
    layer.kind = Some(CoordsKind::Derived);
    layer
}

pub fn complete_coords_for(case: &Network, aux: &Network, source: &str) -> Result<Coords, String> {
    let mut coords = network_coords(aux);
    spread_stacks(&mut coords);
    let missing: Vec<_> = case
        .buses
        .iter()
        .map(|b| b.id.0)
        .filter(|id| !coords.contains_key(id))
        .collect();
    if !missing.is_empty() {
        return Err(format!(
            "{source}: {} case bus(es) missing coordinates (first: {})",
            missing.len(),
            missing[0]
        ));
    }
    Ok(coords)
}

/// Buses at one substation share its coordinate exactly. Place each co-located
/// group on a small ring so every bus stays hoverable at street zoom.
pub fn spread_stacks(coords: &mut Coords) {
    const RADIUS: f64 = 0.004;
    let mut groups: BTreeMap<(u64, u64), Vec<usize>> = BTreeMap::new();
    for (&id, &(lon, lat)) in coords.iter() {
        groups
            .entry((lon.to_bits(), lat.to_bits()))
            .or_default()
            .push(id);
    }
    for ids in groups.values_mut() {
        if ids.len() < 2 {
            continue;
        }
        ids.sort_unstable();
        let (lon0, lat0) = coords[&ids[0]];
        let lonscale = lat0.to_radians().cos().max(0.2);
        for (j, id) in ids.iter().enumerate() {
            let theta = std::f64::consts::TAU * j as f64 / ids.len() as f64;
            coords.insert(
                *id,
                (
                    lon0 + RADIUS * theta.cos() / lonscale,
                    lat0 + RADIUS * theta.sin(),
                ),
            );
        }
    }
}

pub fn synthetic_layout(net: &Network, bbox: (f64, f64, f64, f64)) -> Coords {
    let ids: Vec<_> = net.buses.iter().map(|b| b.id.0).collect();
    let index: BTreeMap<usize, usize> = ids.iter().enumerate().map(|(i, &id)| (id, i)).collect();
    let mut seen = BTreeSet::new();
    let mut edges = Vec::new();
    for br in net.branches.iter().filter(|br| br.in_service) {
        let (Some(&i), Some(&j)) = (index.get(&br.from.0), index.get(&br.to.0)) else {
            continue;
        };
        if i == j {
            continue;
        }
        let e = (i.min(j), i.max(j));
        if seen.insert(e) {
            edges.push(e);
        }
    }

    let mut pos = force_layout(ids.len(), &edges);
    normalize_points(&mut pos);
    let (lon0, lat0, lon1, lat1) = bbox;
    ids.into_iter()
        .enumerate()
        .map(|(i, id)| {
            let x = rescale(pos[i].0, lon0, lon1);
            let y = rescale(pos[i].1, lat0, lat1);
            (id, (x, y))
        })
        .collect()
}

fn substation_coords(aux: &AuxFile) -> Coords {
    let mut out = BTreeMap::new();
    for obj in aux.data_of("Substation") {
        let (Some(num), Some(lat), Some(lon)) = (
            obj.field_index("SubNum")
                .or_else(|| obj.field_index("Number")),
            obj.field_index("Latitude"),
            obj.field_index("Longitude"),
        ) else {
            continue;
        };
        for row in &obj.rows {
            let field = |i: usize| row.values.get(i).and_then(|v| v.trim().parse::<f64>().ok());
            let (Some(n), Some(la), Some(lo)) = (field(num), field(lat), field(lon)) else {
                continue;
            };
            out.insert(n as usize, (lo, la));
        }
    }
    out
}

fn extra_f64(b: &Bus, keys: &[&str]) -> Option<f64> {
    keys.iter().find_map(|k| match b.extras.get(*k) {
        Some(serde_json::Value::Number(n)) => n.as_f64(),
        Some(serde_json::Value::String(s)) => s.trim().parse().ok(),
        _ => None,
    })
}

fn force_layout(n: usize, edges: &[(usize, usize)]) -> Vec<(f64, f64)> {
    if n == 0 {
        return Vec::new();
    }
    if n == 1 {
        return vec![(0.5, 0.5)];
    }

    let golden = std::f64::consts::PI * (3.0 - 5.0_f64.sqrt());
    let mut pos: Vec<_> = (0..n)
        .map(|i| {
            let r = 0.44 * ((i as f64 + 0.5) / n as f64).sqrt();
            let theta = i as f64 * golden;
            (
                0.5 + r * theta.cos() + 1e-4 * (0.7 * (i as f64 + 1.0)).sin(),
                0.5 + r * theta.sin() + 1e-4 * (1.3 * (i as f64 + 1.0)).cos(),
            )
        })
        .collect();
    let mut disp = vec![(0.0, 0.0); n];
    let k2 = 1.0 / n as f64;
    let k = k2.sqrt();
    let iters = if n <= 120 {
        180
    } else if n <= 600 {
        100
    } else {
        32
    };

    for iter in 0..iters {
        disp.fill((0.0, 0.0));
        if n <= 900 {
            for i in 0..n {
                for j in (i + 1)..n {
                    let dx = pos[i].0 - pos[j].0;
                    let dy = pos[i].1 - pos[j].1;
                    let f = k2 / (dx * dx + dy * dy + 1e-6);
                    disp[i].0 += dx * f;
                    disp[i].1 += dy * f;
                    disp[j].0 -= dx * f;
                    disp[j].1 -= dy * f;
                }
            }
        }
        for &(i, j) in edges {
            let dx = pos[i].0 - pos[j].0;
            let dy = pos[i].1 - pos[j].1;
            let f = (dx * dx + dy * dy).sqrt() / k;
            disp[i].0 -= dx * f;
            disp[i].1 -= dy * f;
            disp[j].0 += dx * f;
            disp[j].1 += dy * f;
        }
        let t = 0.1 * (1.0 - iter as f64 / iters as f64) + 1e-3;
        for i in 0..n {
            let d = (disp[i].0 * disp[i].0 + disp[i].1 * disp[i].1).sqrt() + 1e-9;
            let s = d.min(t) / d;
            pos[i].0 = (pos[i].0 + disp[i].0 * s).clamp(0.0, 1.0);
            pos[i].1 = (pos[i].1 + disp[i].1 * s).clamp(0.0, 1.0);
        }
    }
    pos
}

fn normalize_points(pos: &mut [(f64, f64)]) {
    if pos.is_empty() {
        return;
    }
    let (mut min_x, mut max_x) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut min_y, mut max_y) = (f64::INFINITY, f64::NEG_INFINITY);
    for &(x, y) in pos.iter() {
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }
    let sx = (max_x - min_x).max(f64::EPSILON);
    let sy = (max_y - min_y).max(f64::EPSILON);
    for p in pos {
        p.0 = 0.04 + ((p.0 - min_x) / sx) * 0.92;
        p.1 = 0.04 + ((p.1 - min_y) / sy) * 0.92;
    }
}

fn rescale(v: f64, lo: f64, hi: f64) -> f64 {
    lo + v * (hi - lo)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An n-bus chain network built through the parser: powerio's data structs
    /// are `#[non_exhaustive]`, so tests construct networks from case text.
    fn chain_network(n: usize) -> Network {
        let mut m = String::from(
            "function mpc = chain\nmpc.version = '2';\nmpc.baseMVA = 100;\nmpc.bus = [\n",
        );
        for i in 1..=n {
            let kind = if i == 1 { 3 } else { 1 };
            m += &format!(" {i} {kind} 0 0 0 0 1 1 0 115 1 1.1 0.9;\n");
        }
        m += "];\nmpc.gen = [\n 1 0 0 300 -300 1 100 1 250 10 0 0 0 0 0 0 0 0 0 0 0;\n];\nmpc.branch = [\n";
        for i in 1..n {
            m += &format!(" {i} {} 0.01 0.1 0 100 100 100 0 0 1 -360 360;\n", i + 1);
        }
        m += "];\nmpc.gencost = [\n 2 0 0 3 0.1 5 0;\n];\n";
        powerio::parse_str(&m, "matpower")
            .expect("parse chain")
            .network
    }

    #[test]
    fn typed_location_wins_over_extras_fallbacks() {
        let mut net = chain_network(3);
        // A typed location (what powerio promotes from `Latitude:1`/`Longitude:1`)
        // and a conflicting bare-extras pair on the same bus: the typed one wins.
        net.buses[0].location = Some(Location {
            x: -84.4,
            y: 33.7,
            kind: None,
        });
        net.buses[0]
            .extras
            .insert("Latitude".to_owned(), serde_json::json!(1.0));
        net.buses[0]
            .extras
            .insert("Longitude".to_owned(), serde_json::json!(2.0));
        // Bus 2 has only the bare pair, the shape upstream leaves in extras.
        net.buses[1]
            .extras
            .insert("Latitude".to_owned(), serde_json::json!(35.5));
        net.buses[1]
            .extras
            .insert("Longitude".to_owned(), serde_json::json!(-80.1));
        let coords = network_coords(&net);
        assert_eq!(coords[&1], (-84.4, 33.7));
        assert_eq!(coords[&2], (-80.1, 35.5));
        assert!(!coords.contains_key(&3));
    }

    #[test]
    fn stamp_layout_places_buses_with_provenance() {
        let mut net = chain_network(3);
        let coords = BTreeMap::from([(1, (-84.0, 33.0)), (2, (-84.1, 33.1))]);
        let placed = stamp_layout(&mut net, &coords, CoordsKind::Synthetic);
        assert_eq!(placed, 2);
        let loc = net.buses[0].location.expect("bus 1 placed");
        assert_eq!((loc.x, loc.y), (-84.0, 33.0));
        assert_eq!(loc.kind, Some(CoordsKind::Synthetic));
        assert!(net.buses[2].location.is_none());
        let geo = net.geo.as_ref().expect("geo meta stamped");
        assert_eq!(geo.kind, Some(CoordsKind::Synthetic));
        assert!(matches!(
            geo.space,
            CoordinateSpace::Geographic { crs: None }
        ));
        // Locations survive the network JSON round trip, so a package built from
        // this payload carries the layout.
        let json = net.to_json().expect("to_json");
        let back = Network::from_json(&json).expect("from_json");
        assert_eq!(back.buses[0].location, net.buses[0].location);

        // An empty layout stamps nothing and leaves the meta untouched.
        let mut untouched = chain_network(2);
        assert_eq!(
            stamp_layout(&mut untouched, &BTreeMap::new(), CoordsKind::Manual),
            0
        );
        assert!(untouched.geo.is_none());
    }

    #[test]
    fn pwd_layer_projects_to_lonlat_with_derived_provenance() {
        use powerio::format::powerworld::PwdSubstation;
        let display = PwdDisplay {
            canvas_width: 100,
            canvas_height: 100,
            stamp: 0,
            substations: vec![PwdSubstation {
                number: 7,
                name: "North".to_owned(),
                x: -45_000.0,
                y: 21_000.0,
            }],
        };
        let layer = pwd_lonlat_layer(&display);
        assert!(matches!(
            layer.space,
            CoordinateSpace::Geographic { crs: None }
        ));
        assert_eq!(layer.kind, Some(CoordsKind::Derived));
        assert_eq!(layer.features.len(), 1);
        let f = &layer.features[0];
        assert_eq!(f.kind, Some(CoordsKind::Derived));
        let GeoGeometry::Point([lon, lat]) = f.geometry else {
            panic!("expected a point");
        };
        let (want_lon, want_lat) = pwd_mercator_to_lonlat(-45_000.0, 21_000.0);
        assert_eq!((lon, lat), (want_lon, want_lat));
        assert!(lon.abs() <= 180.0 && lat.abs() <= 90.0);
    }

    #[test]
    fn stack_spreading_is_deterministic() {
        let mut coords = BTreeMap::from([(2, (-90.0, 40.0)), (1, (-90.0, 40.0))]);
        spread_stacks(&mut coords);
        assert_ne!(coords[&1], coords[&2]);
        assert!(coords[&1].0 > coords[&2].0);
    }

    #[test]
    fn synthetic_layout_fills_bbox() {
        let net = chain_network(20);
        let bbox = (-82.9, 33.3, -79.9, 35.0);
        let coords = synthetic_layout(&net, bbox);
        assert_eq!(coords.len(), 20);
        assert!(coords
            .values()
            .all(|p| bbox.0 <= p.0 && p.0 <= bbox.2 && bbox.1 <= p.1 && p.1 <= bbox.3));
        let unique: BTreeSet<_> = coords
            .values()
            .map(|p| (p.0.to_bits(), p.1.to_bits()))
            .collect();
        assert_eq!(unique.len(), 20);
    }
}
