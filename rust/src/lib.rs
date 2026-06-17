//! powerio parsing in the browser: case files never leave the machine.

use std::collections::BTreeMap;

use powerio::{parse_display_bytes, DisplayData};
use serde::Serialize;
use wasm_bindgen::prelude::*;

pub mod dc;
pub mod geo;
#[cfg(not(target_arch = "wasm32"))]
pub mod server;

use dc::solve_dc_json;
use geo::{network_coords, spread_stacks};

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

/// Solve the DC OPF in the browser (issue #2). `network_json` is the `network`
/// object from `parse_case`; `deltas_json` is `{ deltas: { bus: mw }, sens_bus }`
/// (or empty for the base case). Returns `{ objective, lmp, flows, dispatch,
/// dlmp_dd }` in the shapes the HTTP API serves — LMPs in $/MWh keyed by
/// bus id, flows and dispatch in MW, and `dlmp_dd` the ($/MWh)/MW sensitivity
/// column for `sens_bus` (null when none is requested).
#[wasm_bindgen]
pub fn solve_dc(network_json: &str, deltas_json: &str) -> Result<String, JsError> {
    solve_dc_json(network_json, deltas_json).map_err(jserr)
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

#[derive(Serialize)]
struct TopologyBus {
    id: usize,
    demand_mw: f64,
    gen_mw: f64,
}

#[derive(Serialize)]
struct TopologyBranch {
    id: usize,
    from: usize,
    to: usize,
    rate_mw: f64,
    status: u8,
}

#[derive(Serialize)]
struct Topology {
    buses: Vec<TopologyBus>,
    branches: Vec<TopologyBranch>,
}

/// Everything the drop panel needs from one parse: counts, total load and
/// capacity, parse warnings, and a `view` of buses and branches in the shape
/// the tellegen API serves, placed at the coordinates the file carries
/// (PowerWorld complete case aux exports).
/// `view` is null when the file has no coordinates.
#[wasm_bindgen]
pub fn ingest_case(text: &str, format: &str) -> Result<String, JsError> {
    let parsed = powerio::parse_str(text, format).map_err(jserr)?;
    let mut warnings = parsed.warnings;
    let net = &parsed.network;

    let mut demand: BTreeMap<usize, f64> = BTreeMap::new();
    for l in net.loads.iter().filter(|l| l.in_service) {
        *demand.entry(l.bus.0).or_default() += l.p;
    }
    let mut gen: BTreeMap<usize, f64> = BTreeMap::new();
    for g in net.generators.iter().filter(|g| g.in_service) {
        *gen.entry(g.bus.0).or_default() += g.pmax;
    }

    let topology = Topology {
        buses: net
            .buses
            .iter()
            .map(|b| TopologyBus {
                id: b.id.0,
                demand_mw: demand.get(&b.id.0).copied().unwrap_or(0.0),
                gen_mw: gen.get(&b.id.0).copied().unwrap_or(0.0),
            })
            .collect(),
        branches: net
            .branches
            .iter()
            .enumerate()
            .map(|(i, br)| TopologyBranch {
                id: i + 1,
                from: br.from.0,
                to: br.to.0,
                rate_mw: br.rate_a,
                status: br.in_service as u8,
            })
            .collect(),
    };

    let view = {
        let mut cs = network_coords(net);
        if cs.is_empty() {
            None
        } else {
            let missing_buses = net.buses.len().saturating_sub(cs.len());
            if missing_buses > 0 {
                warnings.push(format!(
                    "{missing_buses} bus(es) lacked coordinates and are omitted from the map"
                ));
            }
            spread_stacks(&mut cs);
            let buses: Vec<ViewBus> = net
                .buses
                .iter()
                .filter_map(|b| {
                    let &(lon, lat) = cs.get(&b.id.0)?;
                    Some(ViewBus {
                        id: b.id.0,
                        lon,
                        lat,
                        demand_mw: demand.get(&b.id.0).copied().unwrap_or(0.0),
                        gen_mw: gen.get(&b.id.0).copied().unwrap_or(0.0),
                    })
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
            let missing_branches = net.branches.len().saturating_sub(branches.len());
            if missing_branches > 0 {
                warnings.push(format!(
                    "{missing_branches} branch(es) lacked endpoint coordinates and are omitted from the map"
                ));
            }
            Some(View { buses, branches })
        }
    };

    serde_json::to_string(&serde_json::json!({
        "name": net.name,
        "base_mva": net.base_mva,
        "n_bus": net.buses.len(),
        "n_branch": net.branches.len(),
        "n_gen": net.generators.iter().filter(|g| g.in_service).count(),
        "load_mw": demand.values().sum::<f64>(),
        "gen_mw": gen.values().sum::<f64>(),
        "has_coords": view.is_some(),
        "coords_kind": if view.is_some() { "file" } else { "synthetic_pending" },
        "network_json": serde_json::to_string(net).map_err(jserr)?,
        "topology": topology,
        "warnings": warnings,
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    const CASE14_NO_COORDS: &str = "\
function mpc = case14synthetic
mpc.version = '2';
mpc.baseMVA = 100;
mpc.bus = [
 1 3 0 0 0 0 1 1 0 230 1 1.1 0.9;
 2 1 21.7 12.7 0 0 1 1 0 230 1 1.1 0.9;
 3 1 94.2 19 0 0 1 1 0 230 1 1.1 0.9;
 4 1 47.8 -3.9 0 0 1 1 0 230 1 1.1 0.9;
 5 1 7.6 1.6 0 0 1 1 0 230 1 1.1 0.9;
 6 2 11.2 7.5 0 0 1 1 0 230 1 1.1 0.9;
 7 1 0 0 0 0 1 1 0 230 1 1.1 0.9;
 8 2 0 0 0 0 1 1 0 230 1 1.1 0.9;
 9 1 29.5 16.6 0 0 1 1 0 230 1 1.1 0.9;
 10 1 9 5.8 0 0 1 1 0 230 1 1.1 0.9;
 11 1 3.5 1.8 0 0 1 1 0 230 1 1.1 0.9;
 12 1 6.1 1.6 0 0 1 1 0 230 1 1.1 0.9;
 13 1 13.5 5.8 0 0 1 1 0 230 1 1.1 0.9;
 14 1 14.9 5 0 0 1 1 0 230 1 1.1 0.9;
];
mpc.gen = [
 1 232.4 0 300 -300 1 100 1 332 0 0 0 0 0 0 0 0 0 0 0 0;
 6 40 0 300 -300 1 100 1 140 0 0 0 0 0 0 0 0 0 0 0 0;
 8 0 0 300 -300 1 100 1 100 0 0 0 0 0 0 0 0 0 0 0 0;
];
mpc.branch = [
 1 2 0.01938 0.05917 0.0528 9900 0 0 0 0 1 -360 360;
 1 5 0.05403 0.22304 0.0492 9900 0 0 0 0 1 -360 360;
 2 3 0.04699 0.19797 0.0438 9900 0 0 0 0 1 -360 360;
 2 4 0.05811 0.17632 0.034 9900 0 0 0 0 1 -360 360;
 2 5 0.05695 0.17388 0.0346 9900 0 0 0 0 1 -360 360;
 3 4 0.06701 0.17103 0.0128 9900 0 0 0 0 1 -360 360;
 4 5 0.01335 0.04211 0 9900 0 0 0 0 1 -360 360;
 4 7 0 0.20912 0 9900 0 0 0.978 0 1 -360 360;
 4 9 0 0.55618 0 9900 0 0 0.969 0 1 -360 360;
 5 6 0 0.25202 0 9900 0 0 0.932 0 1 -360 360;
 6 11 0.09498 0.1989 0 9900 0 0 0 0 1 -360 360;
 6 12 0.12291 0.25581 0 9900 0 0 0 0 1 -360 360;
 6 13 0.06615 0.13027 0 9900 0 0 0 0 1 -360 360;
 7 8 0 0.17615 0 9900 0 0 0 0 1 -360 360;
 7 9 0 0.11001 0 9900 0 0 0 0 1 -360 360;
 9 10 0.03181 0.0845 0 9900 0 0 0 0 1 -360 360;
 9 14 0.12711 0.27038 0 9900 0 0 0 0 1 -360 360;
 10 11 0.08205 0.19207 0 9900 0 0 0 0 1 -360 360;
 12 13 0.22092 0.19988 0 9900 0 0 0 0 1 -360 360;
 13 14 0.17093 0.34802 0 9900 0 0 0 0 1 -360 360;
];
mpc.gencost = [
 2 0 0 3 0.043 20 0;
 2 0 0 3 0.25 20 0;
 2 0 0 3 0.01 20 0;
];
";

    #[test]
    fn matpower_without_coordinates_returns_topology_for_placement() {
        let out = ingest_case(CASE14_NO_COORDS, "m").expect("ingest case14");
        let v: Value = serde_json::from_str(&out).unwrap();

        assert_eq!(v["n_bus"].as_u64().unwrap(), 14);
        assert_eq!(v["coords_kind"].as_str().unwrap(), "synthetic_pending");
        assert!(v["view"].is_null());
        assert!(v["network_json"]
            .as_str()
            .unwrap()
            .contains("case14synthetic"));
        assert_eq!(v["topology"]["buses"].as_array().unwrap().len(), 14);
        assert_eq!(v["topology"]["branches"].as_array().unwrap().len(), 20);
        assert_eq!(
            v["topology"]["buses"][1]["demand_mw"].as_f64().unwrap(),
            21.7
        );
    }
}
