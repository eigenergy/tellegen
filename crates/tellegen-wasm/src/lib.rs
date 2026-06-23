//! The browser adapter: it exports the tellegen engine to JavaScript.
//!
//! Every export here is a thin wrapper. The OPF math, sensitivities, edit semantics,
//! and display-coordinate helpers live in the [`tellegen`] engine crate; this crate
//! only crosses the wasm boundary — `JsValue`/string conversion, `JsError` mapping,
//! and the case-file-drop payload shapes the frontend reads. Case files never leave
//! the machine: parsing and solving happen here, in the browser.

use std::collections::{BTreeMap, HashMap};

use powerio::{parse_display_bytes, DisplayData};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use tellegen::geo::{network_coords, spread_stacks};
use tellegen::{DcNetwork, Iterations, SolveIteration, SolveRequest, SolveResponse};

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

/// Solve the DC OPF in the browser. `network_json` is the `network` object from
/// `parse_case`; `deltas_json` is `{ deltas: { bus: mw }, sens_bus }` (or empty for
/// the base case). Returns `{ objective, lmp, flows, dispatch, dlmp_dd, iterations }`
/// in the shapes the HTTP API serves — LMPs in $/MWh keyed by bus id, flows and
/// dispatch in MW, and `dlmp_dd` the ($/MWh)/MW sensitivity column for `sens_bus`
/// (null when none is requested, or when this build lacks the sensitivity feature).
#[wasm_bindgen]
pub fn solve_dc(network_json: &str, deltas_json: &str) -> Result<String, JsError> {
    solve_dc_json(network_json, deltas_json).map_err(jserr)
}

/// The generalized solve front door: a [`SolveRequest`](tellegen::SolveRequest) JSON
/// in, a [`SolveResponse`](tellegen::SolveResponse) JSON out. The frontend migrates to
/// this once it carries `(formulation, operand, parameter)` state; until then `solve_dc`
/// is the compatibility shape.
#[wasm_bindgen]
pub fn solve_json(network_json: &str, request_json: &str) -> Result<String, JsError> {
    tellegen::solve_json(network_json, request_json).map_err(jserr)
}

/// The capability matrix as JSON: which `(formulation, operand, parameter)` cells this
/// build supports, so the UI can populate menus and grey out the rest.
#[wasm_bindgen]
pub fn capabilities_json() -> String {
    tellegen::capabilities_json()
}

// ---------------------------------------------------------------------------
// Stateful study (build once, solve many) — the reactive hot path
// ---------------------------------------------------------------------------

/// A build-once handle over the engine, exported to JS. Construct once per case (the
/// network is parsed and the model built here); then [`commit`](Study::commit)
/// exact-re-solves and [`preview`](Study::preview) returns a first-order linearization
/// at the committed point — neither re-parses the network, unlike `solve_json` / `solve_dc`
/// which rebuild it on every call. This is the path a reactive drag should use.
///
/// Arguments and results are JSON in the engine's `Study` shapes: edits are a
/// `NetworkEdit[]` (e.g. `[{"kind":"add_load","bus":2,"p_mw":50}]`), `preview` watches an
/// `Operand[]` (e.g. `[{"Price":"Active"}]`) and returns a `Preview`, `commit` returns a
/// `SolveResponse`. Only in the sensitivity build (preview needs the differentiable path).
#[cfg(feature = "sensitivity")]
#[wasm_bindgen]
pub struct Study(tellegen::Study);

#[cfg(feature = "sensitivity")]
#[wasm_bindgen]
impl Study {
    /// Build a study over `network_json` for `formulation` (`"dcopf"` or `"acpf"`),
    /// solving the base case. Errors on an unknown or not-yet-supported formulation.
    #[wasm_bindgen(constructor)]
    pub fn new(network_json: &str, formulation: &str) -> Result<Study, JsError> {
        let problem = parse_problem(formulation)?;
        tellegen::Study::new(network_json, problem)
            .map(Study)
            .map_err(jserr)
    }

    /// Apply `edits_json` (a `NetworkEdit[]`) at the committed point and exact-re-solve.
    /// Advances the committed point; returns the `SolveResponse` JSON.
    pub fn commit(&mut self, edits_json: &str) -> Result<String, JsError> {
        let edits = parse_edits(edits_json)?;
        let resp = self
            .0
            .commit(&edits, tellegen::SolveOptions::default())
            .map_err(jserr)?;
        serde_json::to_string(&resp).map_err(jserr)
    }

    /// First-order preview of `edits_json` (a `NetworkEdit[]`) for the `watched_json`
    /// operands (an `Operand[]`), at the committed point, without re-solving. Returns the
    /// `Preview` JSON.
    pub fn preview(&self, edits_json: &str, watched_json: &str) -> Result<String, JsError> {
        let edits = parse_edits(edits_json)?;
        let watched: Vec<tellegen::Operand> = serde_json::from_str(watched_json)
            .map_err(|e| jserr(format!("bad watched-operands JSON: {e}")))?;
        let prev = self.0.preview(&edits, &watched).map_err(jserr)?;
        serde_json::to_string(&prev).map_err(jserr)
    }

    /// The most recent committed solution as `SolveResponse` JSON.
    pub fn solution(&self) -> Result<String, JsError> {
        serde_json::to_string(self.0.solution()).map_err(jserr)
    }

    /// The formulation tag (`"dcopf"` / `"acpf"`).
    pub fn formulation(&self) -> String {
        serde_json::to_value(self.0.formulation())
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default()
    }
}

/// Parse a formulation tag (`Problem` is serde-lowercase: dcpf/dcopf/acpf/socwr/acopf).
#[cfg(feature = "sensitivity")]
fn parse_problem(formulation: &str) -> Result<tellegen::Problem, JsError> {
    serde_json::from_value(serde_json::Value::String(formulation.to_string()))
        .map_err(|_| jserr(format!("unknown formulation '{formulation}'")))
}

/// Parse a `NetworkEdit[]`; empty/blank is no edits.
#[cfg(feature = "sensitivity")]
fn parse_edits(edits_json: &str) -> Result<Vec<tellegen::NetworkEdit>, JsError> {
    if edits_json.trim().is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str(edits_json).map_err(|e| jserr(format!("bad edits JSON: {e}")))
}

// ---------------------------------------------------------------------------
// DC compatibility shape (the frontend's current `solveDc` contract)
// ---------------------------------------------------------------------------

/// The DC solve request as the frontend encodes it: demand deltas in MW keyed by
/// original bus id, and an optional bus to return the dLMP/dd column for.
#[derive(Deserialize, Default)]
struct JsonSolveRequest {
    #[serde(default)]
    deltas: HashMap<i64, f64>,
    // Read only in the sensitivity build; the core build leaves `dlmp_dd` null.
    #[cfg_attr(not(feature = "sensitivity"), allow(dead_code))]
    #[serde(default)]
    sens_bus: Option<i64>,
}

#[derive(Serialize)]
struct DcSolveOutput {
    objective: f64,
    lmp: Vec<LmpValue>,
    flows: Vec<FlowValue>,
    dispatch: Vec<DispatchValue>,
    dlmp_dd: Option<DlmpDdColumn>,
    /// The interior-point convergence trace, for the solve card sparkline.
    iterations: Vec<SolveIteration>,
}

#[derive(Serialize)]
struct LmpValue {
    bus: usize,
    usd_per_mwh: f64,
}

#[derive(Serialize)]
struct FlowValue {
    branch: usize,
    mw: f64,
    loading: f64,
}

#[derive(Serialize)]
struct DispatchValue {
    gen: usize,
    mw: f64,
}

#[derive(Serialize)]
struct DlmpDdColumn {
    bus: usize,
    operand: &'static str,
    parameter: &'static str,
    units: &'static str,
    values: Vec<SensitivityValue>,
}

#[derive(Serialize)]
struct SensitivityValue {
    bus: usize,
    value: f64,
}

/// Solve the DC OPF for `network_json` at `base demand + deltas` and serve the DC
/// compatibility shape over the generalized engine. Kept out of `#[wasm_bindgen]` so it
/// is testable natively; `solve_dc` wraps it.
///
/// Building the [`DcNetwork`] once gives both the bus-id → dense-index map the
/// sensitivity column needs and the cached model [`solve_prebuilt`](tellegen::solve_prebuilt)
/// reuses, so there is no duplicate model build.
pub fn solve_dc_json(network_json: &str, deltas_json: &str) -> Result<String, String> {
    let net = powerio::network::Network::from_json(network_json).map_err(|e| e.to_string())?;
    let req: JsonSolveRequest = if deltas_json.trim().is_empty() {
        JsonSolveRequest::default()
    } else {
        serde_json::from_str(deltas_json).map_err(|e| format!("bad deltas JSON: {e}"))?
    };

    let dc = DcNetwork::from_network(&net)?;

    // A default request is a base-case DC OPF; layer on the operating-point deltas.
    // The engine ignores non-positive bus ids, so the raw map crosses unchanged.
    let mut request = SolveRequest::default();
    request.edits.deltas = req.deltas;

    // The dLMP/dd column: a Price/Demand sensitivity cell over the single sens bus.
    // Only meaningful in the sensitivity build; the core build leaves `dlmp_dd` null.
    #[cfg(feature = "sensitivity")]
    if let Some(bus) = req.sens_bus.and_then(|b| (b > 0).then_some(b as usize)) {
        if let Some(idx) = dc.bus_ids.iter().position(|&id| id == bus) {
            request.sensitivities = vec![tellegen::SensRequest {
                operand: tellegen::Operand::Price(tellegen::Power::Active),
                parameter: tellegen::Parameter::Demand(tellegen::Power::Active),
                indices: Some(vec![idx]),
                mode: tellegen::Mode::Auto,
            }];
        }
    }

    let resp = tellegen::solve_prebuilt(&dc, &request)?;
    serde_json::to_string(&dc_output(&resp)).map_err(|e| e.to_string())
}

/// Reshape the generalized [`SolveResponse`] into the DC compatibility output.
fn dc_output(resp: &SolveResponse) -> DcSolveOutput {
    let lmp = resp
        .lmp
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(|s| LmpValue {
            bus: s.bus,
            usd_per_mwh: s.value,
        })
        .collect();
    let flows = resp
        .flows
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(|f| FlowValue {
            branch: f.branch,
            mw: f.pf,
            loading: f.loading,
        })
        .collect();
    let dispatch = resp
        .dispatch
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(|g| DispatchValue {
            gen: g.gen,
            mw: g.pg,
        })
        .collect();
    let iterations = match &resp.iterations {
        Some(Iterations::Ipm(trace)) => trace.clone(),
        _ => Vec::new(),
    };
    DcSolveOutput {
        objective: resp.objective.unwrap_or(0.0),
        lmp,
        flows,
        dispatch,
        dlmp_dd: dc_sensitivity(resp),
        iterations,
    }
}

/// The dLMP/dd column from the first (and only) requested sensitivity cell: rows are
/// buses, the single column is the sens bus.
#[cfg(feature = "sensitivity")]
fn dc_sensitivity(resp: &SolveResponse) -> Option<DlmpDdColumn> {
    let m = resp.sensitivities.first()?;
    let col_bus = match m.cols.first()?.element {
        tellegen::ElementId::Bus(b) => b,
        _ => return None,
    };
    let values = m
        .rows
        .iter()
        .zip(&m.values)
        .map(|(row, vals)| SensitivityValue {
            bus: match row.element {
                tellegen::ElementId::Bus(b) => b,
                _ => 0,
            },
            value: vals.first().copied().unwrap_or(0.0),
        })
        .collect();
    Some(DlmpDdColumn {
        bus: col_bus,
        operand: "lmp",
        parameter: "d",
        units: "($/MWh)/MW",
        values,
    })
}

#[cfg(not(feature = "sensitivity"))]
fn dc_sensitivity(_resp: &SolveResponse) -> Option<DlmpDdColumn> {
    None
}

// ---------------------------------------------------------------------------
// Case file ingest (the drop-panel payload)
// ---------------------------------------------------------------------------

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
/// (PowerWorld complete case aux exports). `view` is null when the file has no
/// coordinates.
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

    fn case14_json() -> String {
        powerio::parse_str(CASE14_NO_COORDS, "m")
            .expect("parse")
            .network
            .to_json()
            .expect("to_json")
    }

    #[test]
    fn solve_dc_base_shapes() {
        let out = solve_dc_json(&case14_json(), "").expect("solve_dc");
        let v: Value = serde_json::from_str(&out).unwrap();
        assert!(v["objective"].as_f64().unwrap() > 0.0);
        assert_eq!(v["lmp"].as_array().unwrap().len(), 14);
        assert_eq!(v["flows"].as_array().unwrap().len(), 20);
        assert!(!v["dispatch"].as_array().unwrap().is_empty());
        assert!(v["dlmp_dd"].is_null());
        let iters = v["iterations"].as_array().unwrap();
        assert!(!iters.is_empty(), "expected a convergence trace");
        for it in iters {
            assert!(it["inf_pr"].as_f64().unwrap().is_finite());
        }
    }

    #[test]
    fn solve_dc_deltas_shift_the_operating_point() {
        let base: Value =
            serde_json::from_str(&solve_dc_json(&case14_json(), "").unwrap()).unwrap();
        let bumped: Value = serde_json::from_str(
            &solve_dc_json(&case14_json(), r#"{"deltas": {"3": 80.0}}"#).unwrap(),
        )
        .unwrap();
        assert!(
            (bumped["objective"].as_f64().unwrap() - base["objective"].as_f64().unwrap()).abs()
                > 1e-6,
            "demand delta had no effect on the objective"
        );
    }

    #[cfg(feature = "sensitivity")]
    #[test]
    fn solve_dc_sensitivity_column_present_when_requested() {
        let out = solve_dc_json(&case14_json(), r#"{"sens_bus": 3}"#).expect("solve_dc");
        let v: Value = serde_json::from_str(&out).unwrap();
        let s = &v["dlmp_dd"];
        assert_eq!(s["bus"].as_i64().unwrap(), 3);
        assert_eq!(s["units"].as_str().unwrap(), "($/MWh)/MW");
        assert_eq!(s["operand"].as_str().unwrap(), "lmp");
        let values = s["values"].as_array().unwrap();
        assert_eq!(values.len(), 14);
        for e in values {
            assert!(e["value"].as_f64().unwrap().is_finite());
        }
    }

    #[cfg(feature = "sensitivity")]
    #[test]
    fn study_argument_parsing() {
        // The `#[wasm_bindgen] Study` struct can't be exercised natively (returning
        // an exported struct crosses the wasm ABI), and JsError can't be constructed
        // off-wasm — so the end-to-end binding is covered by the browser (Playwright) and
        // the engine `tellegen::Study` is unit-tested in the engine crate. Here we cover
        // this shim's own logic: parsing the JSON arguments (success paths).
        assert!(matches!(
            parse_problem("dcopf").unwrap(),
            tellegen::Problem::DcOpf
        ));
        assert!(matches!(
            parse_problem("acpf").unwrap(),
            tellegen::Problem::AcPf
        ));
        let edits = parse_edits(r#"[{"kind":"add_load","bus":3,"p_mw":10.0}]"#).unwrap();
        assert_eq!(edits.len(), 1);
        assert!(parse_edits("").unwrap().is_empty());
        assert!(parse_edits("   ").unwrap().is_empty());
    }
}
