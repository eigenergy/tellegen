//! powerio parsing in the browser: case files never leave the machine.

use std::collections::BTreeMap;

use powerio::format::powerworld::{AuxFile, aux_sections};
use powerio::network::{Bus, Network};
use powerio::{DisplayData, parse_display_bytes};
use serde::Serialize;
use wasm_bindgen::prelude::*;

fn jserr(e: impl std::fmt::Display) -> JsError {
    JsError::new(&e.to_string())
}

/// Parse a case file (MATPOWER, PSS/E RAW, PowerWorld aux, PowerModels or
/// egret JSON) and return `{"network": ..., "warnings": [...]}` as JSON.
#[wasm_bindgen]
pub fn parse_case(text: &str, format: &str) -> Result<String, JsError> {
    let parsed = powerio::parse_str(text, format).map_err(jserr)?;
    serde_json::to_string(&serde_json::json!({
        "network": parsed.network,
        "warnings": parsed.warnings,
    }))
    .map_err(jserr)
}

#[derive(Serialize)]
struct ViewBus {
    id: usize,
    lon: f64,
    lat: f64,
    demand_mw: f64,
    gen_mw: f64,
}

#[derive(Serialize)]
struct ViewBranch {
    id: usize,
    from: usize,
    to: usize,
    rate_mw: f64,
    status: u8,
    path: [[f64; 2]; 2],
}

#[derive(Serialize)]
struct View {
    buses: Vec<ViewBus>,
    branches: Vec<ViewBranch>,
}

/// Everything the drop panel needs from one parse: counts, total load and
/// capacity, parse warnings, and a map-ready `view` of buses and branches in
/// the shape the tellegen backend serves, placed at the substation
/// coordinates the file carries (PowerWorld complete case aux exports).
/// `view` is null when the file has no coordinates.
#[wasm_bindgen]
pub fn ingest_case(text: &str, format: &str) -> Result<String, JsError> {
    let parsed = powerio::parse_str(text, format).map_err(jserr)?;
    let net = &parsed.network;

    let mut demand: BTreeMap<usize, f64> = BTreeMap::new();
    for l in net.loads.iter().filter(|l| l.in_service) {
        *demand.entry(l.bus.0).or_default() += l.p;
    }
    let mut gen: BTreeMap<usize, f64> = BTreeMap::new();
    for g in net.generators.iter().filter(|g| g.in_service) {
        *gen.entry(g.bus.0).or_default() += g.pmax;
    }

    let view = coords(net).map(|mut cs| {
        spread_stacks(&mut cs);
        let buses: Vec<ViewBus> = net
            .buses
            .iter()
            .map(|b| {
                let (lon, lat) = cs[&b.id.0];
                ViewBus {
                    id: b.id.0,
                    lon,
                    lat,
                    demand_mw: demand.get(&b.id.0).copied().unwrap_or(0.0),
                    gen_mw: gen.get(&b.id.0).copied().unwrap_or(0.0),
                }
            })
            .collect();
        let branches: Vec<ViewBranch> = net
            .branches
            .iter()
            .enumerate()
            .filter_map(|(i, br)| {
                let f = cs.get(&br.from.0)?;
                let t = cs.get(&br.to.0)?;
                Some(ViewBranch {
                    id: i + 1,
                    from: br.from.0,
                    to: br.to.0,
                    rate_mw: br.rate_a,
                    status: br.in_service as u8,
                    path: [[f.0, f.1], [t.0, t.1]],
                })
            })
            .collect();
        View { buses, branches }
    });

    serde_json::to_string(&serde_json::json!({
        "name": net.name,
        "base_mva": net.base_mva,
        "n_bus": net.buses.len(),
        "n_branch": net.branches.len(),
        "n_gen": net.generators.len(),
        "load_mw": demand.values().sum::<f64>(),
        "gen_mw": gen.values().sum::<f64>(),
        "has_coords": view.is_some(),
        "warnings": parsed.warnings,
        "view": view,
    }))
    .map_err(jserr)
}

#[derive(Serialize)]
struct ViewSubstation {
    number: u32,
    name: String,
    x: f64,
    y: f64,
}

#[derive(Serialize)]
struct DisplayView {
    substations: Vec<ViewSubstation>,
    canvas_width: u16,
    canvas_height: u16,
}

/// Decode a PowerWorld `.pwd` display file (binary). Returns the substation
/// symbols at the diagram coordinates the file stores (x east, y north) plus
/// the canvas size. These are diagram positions, not geography: the caller
/// projects them. A `.pwd` carries no buses or branches. `format` is "pwd".
/// Pure in-memory parsing, no filesystem, so it runs in the browser.
#[wasm_bindgen]
pub fn parse_display(bytes: &[u8], format: &str) -> Result<String, JsError> {
    match parse_display_bytes(bytes, format).map_err(jserr)? {
        DisplayData::PowerWorld(d) => serde_json::to_string(&DisplayView {
            substations: d
                .substations
                .into_iter()
                .map(|s| ViewSubstation {
                    number: s.number,
                    name: s.name,
                    x: s.x,
                    y: s.y,
                })
                .collect(),
            canvas_width: d.canvas_width,
            canvas_height: d.canvas_height,
        })
        .map_err(jserr),
        // DisplayData is #[non_exhaustive]; PowerWorld is the only arm today.
        #[allow(unreachable_patterns)]
        _ => Err(JsError::new("unsupported display format")),
    }
}

/// Bus id => (lon, lat). Two generations of PowerWorld export carry
/// substation coordinates differently: 2018-era complete cases (the ACTIVSg
/// distributions) write them on every bus row (Latitude:1 / Longitude:1);
/// later exports leave the bus columns empty and point at the Substation
/// table through SubNumber. Try the bus row first, then the join. All buses
/// must be covered: a partially placed network misleads.
fn coords(net: &Network) -> Option<BTreeMap<usize, (f64, f64)>> {
    let subs = match aux_sections(net) {
        Some(Ok(aux)) => substation_coords(&aux),
        _ => BTreeMap::new(),
    };
    let mut out = BTreeMap::new();
    for b in &net.buses {
        let p = match (
            extra_f64(b, &["Longitude:1", "Longitude"]),
            extra_f64(b, &["Latitude:1", "Latitude"]),
        ) {
            (Some(lon), Some(lat)) => (lon, lat),
            _ => *extra_f64(b, &["SubNum", "SubNumber"])
                .and_then(|n| subs.get(&(n as usize)))?,
        };
        out.insert(b.id.0, p);
    }
    (!out.is_empty()).then_some(out)
}

/// Substation number => (lon, lat) from the aux Substation table. Field
/// names span the export generations: SubNum/Number, Latitude/Longitude.
fn substation_coords(aux: &AuxFile) -> BTreeMap<usize, (f64, f64)> {
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

/// Buses at one substation share its coordinate exactly. Place each
/// co-located group on a small ring (~400 m) so every bus stays hoverable at
/// street zoom; at network zoom the group still reads as one substation.
/// Mirrors backend/src/coords.jl. Deterministic: ordered by bus id.
fn spread_stacks(cs: &mut BTreeMap<usize, (f64, f64)>) {
    const RADIUS: f64 = 0.004;
    let mut groups: BTreeMap<(u64, u64), Vec<usize>> = BTreeMap::new();
    for (&id, &(lon, lat)) in cs.iter() {
        groups
            .entry((lon.to_bits(), lat.to_bits()))
            .or_default()
            .push(id);
    }
    for ids in groups.values() {
        if ids.len() < 2 {
            continue;
        }
        let (lon0, lat0) = cs[&ids[0]];
        let lonscale = lat0.to_radians().cos().max(0.2);
        for (j, id) in ids.iter().enumerate() {
            let theta = std::f64::consts::TAU * j as f64 / ids.len() as f64;
            cs.insert(
                *id,
                (
                    lon0 + RADIUS * theta.cos() / lonscale,
                    lat0 + RADIUS * theta.sin(),
                ),
            );
        }
    }
}
