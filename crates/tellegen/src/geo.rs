use std::collections::{BTreeMap, BTreeSet};

use powerio::format::powerworld::{aux_sections, AuxFile};
use powerio::network::{Bus, Network};

pub type Coords = BTreeMap<usize, (f64, f64)>;

/// Bus id => (lon, lat). PowerWorld exports carry substation coordinates in
/// two shapes: older complete cases write latitude and longitude on every bus
/// row, while later exports point each bus at the Substation table. Try the bus
/// row first, then the join.
pub fn network_coords(net: &Network) -> Coords {
    let subs = match aux_sections(net) {
        Some(Ok(aux)) => substation_coords(&aux),
        _ => BTreeMap::new(),
    };
    let mut out = BTreeMap::new();
    for b in &net.buses {
        let Some(p) = (match (
            extra_f64(b, &["Longitude:1", "Longitude"]),
            extra_f64(b, &["Latitude:1", "Latitude"]),
        ) {
            (Some(lon), Some(lat)) => Some((lon, lat)),
            _ => extra_f64(b, &["SubNum", "SubNumber"])
                .and_then(|n| subs.get(&(n as usize)).copied()),
        }) else {
            continue;
        };
        out.insert(b.id.0, p);
    }
    out
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
    use powerio::network::{Branch, Bus, BusId, BusType, SourceFormat};

    fn test_bus(id: usize) -> Bus {
        Bus {
            id: BusId(id),
            kind: BusType::Pq,
            vm: 1.0,
            va: 0.0,
            base_kv: 115.0,
            vmax: 1.1,
            vmin: 0.9,
            evhi: None,
            evlo: None,
            area: 1,
            zone: 1,
            name: None,
            extras: Default::default(),
        }
    }

    fn test_branch(from: usize, to: usize) -> Branch {
        Branch {
            from: BusId(from),
            to: BusId(to),
            r: 0.01,
            x: 0.1,
            b: 0.0,
            rate_a: 100.0,
            rate_b: 100.0,
            rate_c: 100.0,
            tap: 0.0,
            shift: 0.0,
            in_service: true,
            angmin: -360.0,
            angmax: 360.0,
            control: None,
            extras: Default::default(),
        }
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
        let net = Network {
            name: "test".into(),
            base_mva: 100.0,
            base_frequency: 60.0,
            buses: (1..=20).map(test_bus).collect(),
            loads: Vec::new(),
            shunts: Vec::new(),
            branches: (1..20).map(|i| test_branch(i, i + 1)).collect(),
            generators: Vec::new(),
            storage: Vec::new(),
            hvdc: Vec::new(),
            transformers_3w: Vec::new(),
            areas: Vec::new(),
            solver: None,
            source_format: SourceFormat::InMemory,
            source: None,
        };
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
