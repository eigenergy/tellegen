//! The browser adapter: it exports the tellegen engine to JavaScript.
//!
//! Every export here is a thin wrapper. The OPF math, sensitivities, edit semantics,
//! and display-coordinate helpers live in the [`tellegen`] engine crate; this crate
//! only crosses the wasm boundary — `JsValue`/string conversion, `JsError` mapping,
//! and the case-file-drop payload shapes the frontend reads. Case files never leave
//! the machine: parsing and solving happen here, in the browser.

use std::collections::BTreeMap;

use powerio::{parse_display_bytes, DisplayData};
use serde::Serialize;
use wasm_bindgen::prelude::*;

use tellegen::geo::{network_coords, spread_stacks};
#[cfg(feature = "sensitivity")]
use tellegen::SolveResponse;

mod dist;

fn jserr(e: impl std::fmt::Display) -> JsError {
    JsError::new(&e.to_string())
}

/// Route Rust panics to `console.error` (with a JS stack) once. Without this a wasm panic
/// surfaces only as the opaque `unreachable` trap; with it the engine's panic message — the
/// real failure — is visible in the browser console and in the `JsError` chain. Used by the
/// Study entry points, which are gated on `sensitivity`.
#[cfg(feature = "sensitivity")]
fn install_panic_hook() {
    use std::sync::Once;
    static HOOK: Once = Once::new();
    HOOK.call_once(console_error_panic_hook::set_once);
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

/// The stateless solve front door: a [`SolveRequest`](tellegen::SolveRequest) JSON
/// in, a [`SolveResponse`](tellegen::SolveResponse) JSON out. One-shot callers only;
/// the reactive hot path is the [`Study`].
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
/// network is parsed and the model built here); then [`replace_edits`](Study::replace_edits)
/// solves exactly at an absolute edit state and [`preview_replacement`](Study::preview_replacement)
/// returns a first-order linearization toward an absolute edit state — neither re-parses
/// the network, unlike `solve_json` which rebuilds it on every call. This is
/// the path a reactive drag should use.
///
/// Arguments and results are JSON in the engine's `Study` shapes: edits are a
/// `NetworkEdit[]` (e.g. `[{"kind":"add_load","bus":2,"p_mw":50}]`; the element key
/// also accepts the powerio row uid string `ingest_case` stamps, e.g.
/// `"bus":"buses:1"`), `preview` watches an
/// `Operand[]` (e.g. `[{"Price":"Active"}]`) and returns a `Preview`. `commit` takes the
/// edits plus a `SensRequest[]` of watched cells and returns `{ solution, iterations,
/// sensitivities }` — the committed [`SolveResponse`] plus the requested ∂operand/∂param
/// columns, computed in the *same* solve (no second round-trip). Only in the sensitivity
/// build (preview needs the differentiable path).
#[cfg(feature = "sensitivity")]
#[wasm_bindgen]
pub struct Study(tellegen::Study);

#[cfg(feature = "sensitivity")]
#[wasm_bindgen]
impl Study {
    /// Build a study over `network_json` for `formulation` (`"dcopf"` / `"acpf"`, and —
    /// in a build that includes them — `"socwr"` / `"acopf"`), solving the base case.
    /// Errors on an unknown or not-built formulation.
    #[wasm_bindgen(constructor)]
    pub fn new(network_json: &str, formulation: &str) -> Result<Study, JsError> {
        install_panic_hook();
        let problem = parse_problem(formulation)?;
        tellegen::Study::new(network_json, problem)
            .map(Study)
            .map_err(jserr)
    }

    /// Apply `edits_json` (a `NetworkEdit[]`) at the committed point and exact-re-solve,
    /// attaching the `sensitivities_json` cells (a `SensRequest[]`, or empty/blank for
    /// none) in the same solve. Advances the committed point. Returns
    /// `{ "solution": SolveResponse, "iterations": Iterations, "sensitivities":
    /// SensitivityMatrix[] }` — `solution` is the full committed response, and
    /// `iterations` / `sensitivities` mirror its convergence trace and the watched
    /// columns so the UI renders the ∂LMP/∂d column without a second solve.
    pub fn commit(
        &mut self,
        edits_json: &str,
        sensitivities_json: &str,
    ) -> Result<String, JsError> {
        install_panic_hook();
        let edits = parse_edits(edits_json)?;
        let sensitivities = parse_sensitivities(sensitivities_json)?;
        let resp = self
            .0
            .commit_with(&edits, &sensitivities, tellegen::SolveOptions::default())
            .map_err(jserr)?;
        serde_json::to_string(&commit_output(&resp)).map_err(jserr)
    }

    /// Replace the committed edit set with `edits_json` and exact-re-solve, attaching
    /// the `sensitivities_json` cells in the same solve. Use this for UI state that stores
    /// absolute demand deltas from the base case.
    pub fn replace_edits(
        &mut self,
        edits_json: &str,
        sensitivities_json: &str,
    ) -> Result<String, JsError> {
        install_panic_hook();
        let edits = parse_edits(edits_json)?;
        let sensitivities = parse_sensitivities(sensitivities_json)?;
        let resp = self
            .0
            .replace_edits_with(&edits, &sensitivities, tellegen::SolveOptions::default())
            .map_err(jserr)?;
        serde_json::to_string(&commit_output(&resp)).map_err(jserr)
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

    /// First-order preview for replacing the committed edit set with `edits_json`.
    /// This accepts absolute demand delta state and internally previews only the step
    /// from the current committed point.
    pub fn preview_replacement(
        &self,
        edits_json: &str,
        watched_json: &str,
    ) -> Result<String, JsError> {
        let edits = parse_edits(edits_json)?;
        let watched: Vec<tellegen::Operand> = serde_json::from_str(watched_json)
            .map_err(|e| jserr(format!("bad watched-operands JSON: {e}")))?;
        let prev = self
            .0
            .preview_replacement(&edits, &watched)
            .map_err(jserr)?;
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

    /// Serialize this study as a `.pio.json` package: the base network is the payload,
    /// the edit log is the study block (one commit per `commit` call, keyed by row uid),
    /// and the formulation and solve options ride under `study.app["tellegen"]`. Returns
    /// the package JSON, ready to download or hand to [`export_study`].
    pub fn save_package(&self) -> Result<String, JsError> {
        install_panic_hook();
        let package = self.0.to_package().map_err(jserr)?;
        package.to_json().map_err(jserr)
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

/// Parse a `SensRequest[]` (the watched ∂operand/∂param cells); empty/blank is none.
/// A cell is `{"operand":{"Price":"Active"},"parameter":{"Demand":"Active"},"indices":[1]}`.
#[cfg(feature = "sensitivity")]
fn parse_sensitivities(sens_json: &str) -> Result<Vec<tellegen::SensRequest>, JsError> {
    if sens_json.trim().is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str(sens_json).map_err(|e| jserr(format!("bad sensitivities JSON: {e}")))
}

/// Wrap a committed [`SolveResponse`] as `{ solution, iterations, sensitivities }`: the
/// full response under `solution`, with its convergence trace and the watched sensitivity
/// columns mirrored at the top level so the frontend reads the ∂LMP/∂d column directly off
/// the commit without a second solve.
#[cfg(feature = "sensitivity")]
fn commit_output(resp: &SolveResponse) -> serde_json::Value {
    serde_json::json!({
        "solution": resp,
        "iterations": resp.iterations,
        "sensitivities": resp.sensitivities,
    })
}

/// Restore a study from a `.pio.json` package and return everything the frontend needs
/// to rehydrate its case in one step: the drop-panel payload (summary, topology, map
/// view, base `network_json`) plus the restored `formulation`, solve `options`, and the
/// folded `deltas`/`rates` keyed by numeric element id (bus id / branch position). The
/// package is validated by the engine's [`tellegen::Study::from_package`], which fails
/// closed on a non-balanced payload, a missing or wrong-version `app["tellegen"]` blob,
/// an unknown edit kind, or an unresolved edit key.
///
/// Untrusted input: the parse and the restore return error strings (never a panic), so
/// a malformed, truncated, or oversized package rejects cleanly. Kept string-typed and
/// separate from the `#[wasm_bindgen]` wrapper so it runs in native unit tests.
#[cfg(feature = "sensitivity")]
fn load_package_bundle(package_json: &str) -> Result<String, String> {
    let package = powerio_pkg::NetworkPackage::from_json(package_json)
        .map_err(|e| format!("invalid .pio.json package: {e}"))?;
    let study = tellegen::Study::from_package(&package)?;
    // The engine already validated the payload as balanced; clone it (uids stamped) for
    // the ingest view.
    let mut net = package
        .as_balanced()
        .ok_or("package payload is not balanced")?
        .clone();
    powerio_pkg::ensure_payload_uids(&mut net);

    let mut bundle = ingest_value(&net, Vec::new())?;
    let (deltas, rates) = study.folded_deltas_by_id()?;
    let object = bundle
        .as_object_mut()
        .ok_or("ingest payload is not an object")?;
    object.insert(
        "formulation".to_owned(),
        serde_json::to_value(study.formulation()).map_err(|e| e.to_string())?,
    );
    object.insert(
        "options".to_owned(),
        serde_json::to_value(study.solve_options()).map_err(|e| e.to_string())?,
    );
    object.insert(
        "deltas".to_owned(),
        serde_json::to_value(&deltas).map_err(|e| e.to_string())?,
    );
    object.insert(
        "rates".to_owned(),
        serde_json::to_value(&rates).map_err(|e| e.to_string())?,
    );
    serde_json::to_string(&bundle).map_err(|e| e.to_string())
}

/// Restore a study saved by [`Study::save_package`]. Returns a JSON bundle the frontend
/// reads to rebuild its case, edit sliders, formulation, and solve options in one step.
/// See [`load_package_bundle`] for the shape and the fail-closed contract.
#[cfg(feature = "sensitivity")]
#[wasm_bindgen]
pub fn load_package(package_json: &str) -> Result<String, JsError> {
    install_panic_hook();
    load_package_bundle(package_json).map_err(jserr)
}

/// Export the balanced study state at commit `commit` to a powerio `format`
/// (`matpower`, `psse`, `powerio-json`, ...). Returns `{ text, warnings, format,
/// extension }` as JSON: the serialized case, the writer's fidelity warnings so the
/// frontend can surface them, and the format token and file extension. The package
/// JSON is untrusted; malformed input returns a `JsError`, never a panic.
#[cfg(feature = "sensitivity")]
#[wasm_bindgen]
pub fn export_study(package_json: &str, commit: usize, format: &str) -> Result<String, JsError> {
    install_panic_hook();
    let exported = tellegen::export_study(package_json, commit, format).map_err(jserr)?;
    serde_json::to_string(&exported).map_err(jserr)
}

// ---------------------------------------------------------------------------
// Case file ingest (the drop-panel payload)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct ViewBus {
    id: usize,
    /// powerio row uid — stamped by `ingest_case` before the view is built, so it
    /// is always present and stable across re-parses of the same file.
    uid: String,
    lon: f64,
    lat: f64,
    demand_mw: f64,
    gen_mw: f64,
}

#[derive(Serialize)]
struct ViewBranch {
    id: usize,
    /// powerio row uid, as on [`ViewBus`].
    uid: String,
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
    /// powerio row uid, as on [`ViewBus`].
    uid: String,
    demand_mw: f64,
    gen_mw: f64,
}

#[derive(Serialize)]
struct TopologyBranch {
    id: usize,
    /// powerio row uid, as on [`ViewBus`].
    uid: String,
    from: usize,
    to: usize,
    rate_mw: f64,
    status: u8,
}

/// The uid `ensure_payload_uids` stamped on this element. It fills every row of
/// every table, so absence is a broken powerio contract, surfaced as an error string
/// (mapped to `JsError` at the boundary) rather than a panic.
fn stamped_uid(uid: &Option<String>) -> Result<String, String> {
    uid.clone()
        .ok_or_else(|| "ensure_payload_uids left an element without a uid".to_owned())
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
    let mut parsed = powerio::parse_str(text, format).map_err(jserr)?;
    // Stamp row uids (`buses:0`, `branches:1`, ...) on every element that the
    // source format did not already give one (GOC3 carries its own). The stamped
    // network is what `network_json` serializes, so a Study built from this
    // payload resolves uid-keyed edits and echoes uids on its response scalars.
    powerio_pkg::ensure_payload_uids(&mut parsed.network);
    let value = ingest_value(&parsed.network, parsed.warnings).map_err(jserr)?;
    serde_json::to_string(&value).map_err(jserr)
}

/// The distribution counterpart of [`ingest_case`]: view a multiconductor case
/// with no solve. `format` is a distribution reader token (`dss`, `bmopf`,
/// `pmd`) parsed by [`powerio_dist`], or `pio` for a `.pio.json` package
/// carrying a multiconductor payload. Returns the drop-panel payload JSON:
/// name, element counts, connected load/generation (kW), parse warnings,
/// coordinate provenance, and the bus/terminal graph the frontend renders (see
/// [`dist`]).
///
/// Untrusted input: the parse and package restore return error strings (mapped
/// to `JsError` here), so a malformed, truncated, or oversized `.dss`/JSON
/// rejects cleanly and never panics the wasm instance.
#[wasm_bindgen]
pub fn ingest_dist_case(text: &str, format: &str) -> Result<String, JsError> {
    let out = match format {
        "pio" | "pio-json" | "package" => dist::ingest_dist_package(text),
        _ => dist::ingest_dist(text, format),
    };
    out.map_err(jserr)
}

/// The drop-panel payload for an already-parsed, uid-stamped network. Returns the
/// `ingest_case` object as a JSON value so [`load_package`] can reuse it and append
/// the restored study state. Errors are strings (mapped to `JsError` at the wasm
/// edge) so the same body runs in native unit tests.
fn ingest_value(
    net: &powerio::network::Network,
    mut warnings: Vec<String>,
) -> Result<serde_json::Value, String> {
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
            .map(|b| {
                Ok(TopologyBus {
                    id: b.id.0,
                    uid: stamped_uid(&b.uid)?,
                    demand_mw: demand.get(&b.id.0).copied().unwrap_or(0.0),
                    gen_mw: gen.get(&b.id.0).copied().unwrap_or(0.0),
                })
            })
            .collect::<Result<_, String>>()?,
        branches: net
            .branches
            .iter()
            .enumerate()
            .map(|(i, br)| {
                Ok(TopologyBranch {
                    id: i + 1,
                    uid: stamped_uid(&br.uid)?,
                    from: br.from.0,
                    to: br.to.0,
                    rate_mw: br.rate_a,
                    status: br.in_service as u8,
                })
            })
            .collect::<Result<_, String>>()?,
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
                    Some(stamped_uid(&b.uid).map(|uid| ViewBus {
                        id: b.id.0,
                        uid,
                        lon,
                        lat,
                        demand_mw: demand.get(&b.id.0).copied().unwrap_or(0.0),
                        gen_mw: gen.get(&b.id.0).copied().unwrap_or(0.0),
                    }))
                })
                .collect::<Result<_, String>>()?;
            let branches: Vec<ViewBranch> = net
                .branches
                .iter()
                .enumerate()
                .filter_map(|(i, br)| {
                    let f = cs.get(&br.from.0)?;
                    let t = cs.get(&br.to.0)?;
                    Some(stamped_uid(&br.uid).map(|uid| ViewBranch {
                        id: i + 1,
                        uid,
                        from: br.from.0,
                        to: br.to.0,
                        rate_mw: br.rate_a,
                        status: br.in_service as u8,
                        path: [[f.0, f.1], [t.0, t.1]],
                    }))
                })
                .collect::<Result<_, String>>()?;
            let missing_branches = net.branches.len().saturating_sub(branches.len());
            if missing_branches > 0 {
                warnings.push(format!(
                    "{missing_branches} branch(es) lacked endpoint coordinates and are omitted from the map"
                ));
            }
            Some(View { buses, branches })
        }
    };

    Ok(serde_json::json!({
        "name": net.name,
        "base_mva": net.base_mva,
        "n_bus": net.buses.len(),
        "n_branch": net.branches.len(),
        "n_gen": net.generators.iter().filter(|g| g.in_service).count(),
        "load_mw": demand.values().sum::<f64>(),
        "gen_mw": gen.values().sum::<f64>(),
        "has_coords": view.is_some(),
        "coords_kind": if view.is_some() { "file" } else { "synthetic_pending" },
        "network_json": serde_json::to_string(net).map_err(|e| e.to_string())?,
        "topology": topology,
        "warnings": warnings,
        "view": view,
    }))
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

        // `ingest_case` stamps powerio row uids before serializing anything, so the
        // topology and the network payload both carry them: uid-keyed edits resolve
        // against a Study built from this `network_json`.
        assert_eq!(v["topology"]["buses"][0]["uid"], "buses:0");
        assert_eq!(v["topology"]["branches"][1]["uid"], "branches:1");
        assert!(v["network_json"].as_str().unwrap().contains("buses:0"));
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
        let edits = parse_edits(
            r#"[{"kind":"add_load","bus":3,"p_mw":10.0},{"kind":"adjust_branch_rating","branch":2,"delta_mw":-25.0}]"#,
        )
        .unwrap();
        assert_eq!(edits.len(), 2);
        // The element key also accepts the powerio row-uid string form.
        let edits = parse_edits(
            r#"[{"kind":"add_load","bus":"buses:2","p_mw":10.0},{"kind":"adjust_branch_rating","branch":"branches:1","delta_mw":-25.0}]"#,
        )
        .unwrap();
        assert_eq!(edits.len(), 2);
        assert!(parse_edits("").unwrap().is_empty());
        assert!(parse_edits("   ").unwrap().is_empty());

        // The `commit` sensitivity argument: a `SensRequest[]`, empty/blank for none.
        let sens = parse_sensitivities(
            r#"[{"operand":{"Price":"Active"},"parameter":{"Demand":"Active"},"indices":[1]}]"#,
        )
        .unwrap();
        assert_eq!(sens.len(), 1);
        assert!(parse_sensitivities("").unwrap().is_empty());
        assert!(parse_sensitivities("   ").unwrap().is_empty());
    }

    /// CASE14 as uid-stamped powerio network JSON, ready for a `Study`.
    #[cfg(feature = "sensitivity")]
    fn case14_network_json() -> String {
        let mut parsed = powerio::parse_str(CASE14_NO_COORDS, "m").expect("parse");
        powerio_pkg::ensure_payload_uids(&mut parsed.network);
        serde_json::to_string(&parsed.network).expect("to_json")
    }

    #[cfg(feature = "sensitivity")]
    #[test]
    fn load_package_bundle_round_trips_a_saved_study() {
        // save_package -> load_package_bundle rebuilds the case, the restored formulation,
        // solve options, and the folded deltas keyed by numeric element id.
        let net = case14_network_json();
        let mut study = tellegen::Study::new(&net, tellegen::Problem::DcOpf).expect("study");
        study
            .commit(
                &[tellegen::NetworkEdit::AddLoad {
                    bus: tellegen::ElementKey::Id(2),
                    p_mw: 10.0,
                }],
                tellegen::SolveOptions::default(),
            )
            .expect("commit");
        let text = study
            .to_package()
            .expect("to_package")
            .to_json()
            .expect("json");

        let bundle: Value = serde_json::from_str(&load_package_bundle(&text).unwrap()).unwrap();
        assert_eq!(bundle["formulation"], "dcopf");
        assert_eq!(bundle["deltas"]["2"].as_f64().unwrap(), 10.0);
        assert_eq!(bundle["options"]["shed"], false);
        assert_eq!(bundle["n_bus"].as_u64().unwrap(), 14);
        assert!(bundle["network_json"].as_str().unwrap().contains("buses:1"));
    }

    #[cfg(feature = "sensitivity")]
    #[test]
    fn load_package_bundle_rejects_untrusted_input_without_panicking() {
        // Malformed, truncated, and oversized inputs must all reject as an `Err` (a wasm
        // panic would abort the instance), never a panic.
        let big_open = "{".repeat(20_000);
        let big_quote = "\"".repeat(100_000);
        let cases = [
            "",
            "   ",
            "{",
            "]",
            "not json at all",
            "null",
            "[]",
            "42",
            r#"{"schema":"https://powerio.dev/schema/pio-package/0.1"}"#,
            big_open.as_str(),
            big_quote.as_str(),
        ];
        for bad in cases {
            assert!(
                load_package_bundle(bad).is_err(),
                "expected Err for input starting {:?}",
                &bad[..bad.len().min(16)]
            );
        }
    }

    #[cfg(feature = "sensitivity")]
    #[test]
    fn export_study_rejects_untrusted_input_without_panicking() {
        let oversized = "x".repeat(50_000);
        let cases = [
            "",
            "{",
            "truncat",
            "null",
            "[1,2,3]",
            "{\"a\":",
            oversized.as_str(),
        ];
        for bad in cases {
            assert!(tellegen::export_study(bad, 0, "matpower").is_err());
            // An out-of-range commit index must also reject rather than panic.
            assert!(tellegen::export_study(bad, 9_999_999, "powerio-json").is_err());
        }
    }
}
