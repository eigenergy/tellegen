//! The browser adapter: it exports the tellegen engine to JavaScript.
//!
//! Every export here is a thin wrapper. The OPF math, sensitivities, edit semantics,
//! and display-coordinate helpers live in the [`tellegen`] engine crate; this crate
//! only crosses the wasm boundary — `JsValue`/string conversion, `JsError` mapping,
//! and the case file drop payload shapes the frontend reads. Case files never leave
//! the machine: parsing and solving happen here, in the browser.

use std::collections::{BTreeMap, HashSet};

use powerio::{Detection, JsonClass, PioValue, Source};
use serde::Serialize;
use tellegen::ir::{deserialize_module, serialize_module};
use wasm_bindgen::prelude::*;

use tellegen::geo::{
    apply_aux_substation_locations, lowered_coords, network_coords, spread_stacks,
};
#[cfg(feature = "sensitivity")]
use tellegen::SolveResponse;

mod dist;
mod geo;

const MAX_INPUT_BYTES: usize = 128 * 1024 * 1024;
const INPUT_TOO_LARGE: &str = "input exceeds 128 MiB limit";

fn jserr(e: impl std::fmt::Display) -> JsError {
    JsError::new(&e.to_string())
}

fn ensure_input_len(byte_len: usize) -> Result<(), &'static str> {
    if byte_len > MAX_INPUT_BYTES {
        Err(INPUT_TOO_LARGE)
    } else {
        Ok(())
    }
}

/// Reject an oversized payload. Note where this runs: wasm-bindgen has already
/// malloc'd and copied the argument into linear memory by the time the export
/// body is entered, so on the browser path the caller-side check in
/// `packages/engine/src/input-limit.ts` is what actually bounds the allocation.
/// This one bounds native and non-JS callers, and keeps the limit in one place
/// if the JS check is ever bypassed.
fn ensure_input_bytes(bytes: &[u8]) -> Result<(), JsError> {
    ensure_input_len(bytes.len()).map_err(jserr)
}

/// Text counterpart of [`ensure_input_bytes`]. The string entry points sit
/// beside byte ones that were already bounded; an embedder handing us a huge
/// document as text should hit the same wall.
fn ensure_input_text(text: &str) -> Result<(), JsError> {
    ensure_input_len(text.len()).map_err(jserr)
}

fn source_format_id(format: &str) -> Result<powerio::FormatId, String> {
    let info = powerio::resolve_format(format)
        .ok_or_else(|| format!("unknown PowerIO format {format:?}"))?;
    powerio::FormatId::new(info.token).map_err(|error| error.to_string())
}

#[derive(Debug, Serialize)]
struct JsonDropClassification {
    kind: &'static str,
    format: Option<&'static str>,
}

#[cfg(feature = "sensitivity")]
#[derive(Debug, Serialize)]
struct IngestedJsonDrop {
    kind: &'static str,
    format: Option<&'static str>,
    payload: serde_json::Value,
}

/// Read a `.pio.json` stored module from dropped bytes. powerio names the
/// document in its own errors, so no prefix here.
fn read_module_bytes(bytes: &[u8]) -> Result<powerio::PioModule<powerio::PioValue>, String> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| "stored module document is not valid UTF-8".to_owned())?;
    deserialize_module(text)
}

fn with_module_json(
    mut payload: serde_json::Value,
    module: powerio::PioModule<PioValue>,
) -> Result<serde_json::Value, String> {
    let module_json = serialize_module(&module)?;
    payload
        .as_object_mut()
        .ok_or("balanced ingest payload is not an object")?
        .insert(
            "module_json".to_owned(),
            serde_json::Value::String(module_json),
        );
    Ok(payload)
}

/// The balanced network owned by a stored value that can start a browser
/// Study in this build. The original dynamic module remains the Study input;
/// this borrow supplies only the render payload.
fn balanced_study_network(value: &PioValue) -> Option<&powerio::BalancedNetwork> {
    match value {
        PioValue::BalancedNetwork(network) => Some(network),
        PioValue::DcOpfInstance(instance) => Some(instance.network()),
        _ => None,
    }
}

/// Classify a dropped JSON byte buffer through powerio's one routing table —
/// classification copies powerio's rule and invents nothing. A stored module
/// is followed by the strict module reader so the caller learns whether the
/// payload is viewable without decoding in JS. The TS drop-classify layer must
/// mirror these kinds: `module` beside powerio's families.
fn classify_json_drop_value(bytes: &[u8]) -> Result<JsonDropClassification, String> {
    let classification = powerio::classify_json_bytes(bytes);
    if classification == JsonClass::Module {
        let module = read_module_bytes(bytes)?;
        return if balanced_study_network(module.value()).is_some()
            || dist::is_viewable_module_value(module.value())
        {
            Ok(JsonDropClassification {
                kind: "module",
                format: None,
            })
        } else {
            Err(format!(
                "stored module holds {}, which tellegen cannot view",
                module.value().type_name()
            ))
        };
    }

    let format = match classification {
        JsonClass::Case(Detection::Known(format)) => Some(format.name()),
        _ => None,
    };
    Ok(JsonDropClassification {
        kind: classification.family(),
        format,
    })
}

/// Classify JSON bytes for the browser drop router. Invalid UTF-8 is never
/// replaced; it follows powerio's unknown classification.
#[wasm_bindgen]
pub fn classify_json(bytes: &[u8]) -> Result<String, JsError> {
    ensure_input_bytes(bytes)?;
    install_panic_hook();
    serde_json::to_string(&classify_json_drop_value(bytes).map_err(jserr)?).map_err(jserr)
}

/// Classify and ingest one dropped JSON byte buffer. Known inputs return their
/// render-ready payload; ambiguous and unknown documents return a null payload
/// so the caller can request an explicit format without decoding the bytes a
/// second time. A stored module is read once and its value routed to the
/// balanced or multiconductor view. The TS drop-classify layer must mirror
/// the kinds (`module` and powerio's families).
#[cfg(feature = "sensitivity")]
#[wasm_bindgen]
pub fn ingest_json_drop(bytes: &[u8]) -> Result<String, JsError> {
    ensure_input_bytes(bytes)?;
    install_panic_hook();
    serde_json::to_string(&ingest_json_drop_value(bytes).map_err(jserr)?).map_err(jserr)
}

#[cfg(feature = "sensitivity")]
fn ingest_json_drop_value(bytes: &[u8]) -> Result<IngestedJsonDrop, String> {
    let classification = powerio::classify_json_bytes(bytes);
    let (kind, format, payload) = match classification {
        JsonClass::Module => {
            let module = read_module_bytes(bytes)?;
            let diagnostics = module.diagnostics.clone();
            let payload = if let Some(network) = balanced_study_network(module.value()) {
                let mut payload = ingest_value(network, &diagnostics, Vec::new(), None)?;
                let module_json = std::str::from_utf8(bytes)
                    .map_err(|_| "stored module document is not valid UTF-8".to_owned())?;
                payload
                    .as_object_mut()
                    .ok_or("balanced module ingest payload is not an object")?
                    .insert(
                        "module_json".to_owned(),
                        serde_json::Value::String(module_json.to_owned()),
                    );
                payload
            } else if dist::is_viewable_module_value(module.value()) {
                dist::ingest_dist_module_value(module)?
            } else {
                return Err(format!(
                    "stored module holds {}, which tellegen cannot view",
                    module.value().type_name()
                ));
            };
            ("module", None, payload)
        }
        JsonClass::Case(Detection::Known(format)) => {
            let kind = classification.family();
            let payload = match kind {
                "transmission" => ingest_case_value(bytes, format.name())?,
                "distribution" => dist::ingest_dist_bytes_value(bytes, format.name())?,
                _ => return Err(format!("unsupported JSON family '{kind}'")),
            };
            (kind, Some(format.name()), payload)
        }
        JsonClass::Case(Detection::Ambiguous) => ("ambiguous", None, serde_json::Value::Null),
        JsonClass::Case(Detection::Unknown) => ("unknown", None, serde_json::Value::Null),
    };

    Ok(IngestedJsonDrop {
        kind,
        format,
        payload,
    })
}

/// Route Rust panics to `console.error` (with a JS stack) once. Without this a wasm panic
/// surfaces only as the opaque `unreachable` trap; with it the engine's panic message — the
/// real failure — is visible in the browser console and in the `JsError` chain. Every
/// `#[wasm_bindgen]` entry point installs it, including the ones outside `sensitivity`:
/// dropping a case file is often the first thing a session does, so that is exactly where
/// an unexplained trap costs the most.
fn install_panic_hook() {
    use std::sync::Once;
    static HOOK: Once = Once::new();
    HOOK.call_once(console_error_panic_hook::set_once);
}

/// Parse dropped case bytes through the PowerIO module route, forcing `format`: an
/// in-memory [`Source`] with the declared format, the automatic parse, and
/// typed narrowing to the balanced network. The module rides back whole so
/// callers read the diagnostics and, for formats that keep data in retained
/// source text, still hold the original bytes beside it.
fn parse_case_module(
    bytes: &[u8],
    format: &str,
) -> Result<powerio::PioModule<powerio::BalancedNetwork>, String> {
    let format = source_format_id(format)?;
    // The angle bracketed name marks an anonymous in-memory source, so the
    // reader takes the case's own name (e.g. the MATPOWER function name)
    // instead of a file stem hint.
    let source = Source::from_memory("<case>", bytes.to_vec())
        .map_err(|e| e.to_string())?
        .with_format(format);
    let module = powerio::parse(source).map_err(|e| e.to_string())?;
    tellegen::ir::balanced_module(module)
}

/// Parse a case file (MATPOWER, PSS/E RAW, PowerWorld aux, PowerModels or
/// egret JSON) and return `{"network": ..., "diagnostics": [...]}` as JSON.
///
/// Takes the upload's bytes, not decoded text: powerio refuses a text format
/// whose bytes are not UTF-8, where a browser side `File.text()` would have
/// replaced each offending byte with U+FFFD and parsed on. It also reaches
/// PowerWorld `.pwb`, which has no text form at all.
#[wasm_bindgen]
pub fn parse_case(bytes: &[u8], format: &str) -> Result<String, JsError> {
    ensure_input_bytes(bytes)?;
    install_panic_hook();
    let module = parse_case_module(bytes, format).map_err(jserr)?;
    let network = serde_json::to_value(module.value()).map_err(jserr)?;
    serde_json::to_string(&serde_json::json!({
        "network": network,
        "diagnostics": module.diagnostics,
    }))
    .map_err(jserr)
}

/// The stateless solve front door: a retained PowerIO module and a
/// [`tellegen::SolveRequest`] JSON in, a [`tellegen::SolveResponse`] JSON out. One-shot callers only;
/// the reactive hot path is the [`Study`].
#[wasm_bindgen]
pub fn solve_module(module_json: &str, request_json: &str) -> Result<String, JsError> {
    install_panic_hook();
    tellegen::solve_module_json(module_json, request_json).map_err(jserr)
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

/// A retained handle over the engine, exported to JS. Construct once per PowerIO module;
/// then [`replace_edits`](Study::replace_edits)
/// solves exactly at an absolute edit state and [`preview_replacement`](Study::preview_replacement)
/// returns a first order linearization toward an absolute edit state.
///
/// Arguments and results are JSON in the engine's `Study` shapes: edits are a
/// `NetworkEdit[]` (e.g. `[{"kind":"add_load","bus":2,"p_mw":50}]`; the element key
/// also accepts a powerio row uid string when the source format carries uids,
/// e.g. `"bus":"buses:1"`), `preview` watches an
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
    /// Build a study over `module_json` for `formulation` (`"dcopf"` / `"acpf"`, and —
    /// in a build that includes them — `"socwr"` / `"acopf"`), solving the base case.
    /// Bare model JSON and unknown or unavailable formulations are rejected.
    #[wasm_bindgen(constructor)]
    pub fn new(module_json: &str, formulation: &str) -> Result<Study, JsError> {
        install_panic_hook();
        let problem = parse_problem(formulation)?;
        tellegen::Study::new(module_json, problem)
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
        let resp = self.0.commit_with(&edits, &sensitivities).map_err(jserr)?;
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
            .replace_edits_with(&edits, &sensitivities)
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

    /// Serialize the committed operating point as a stored PowerIO module:
    /// the materialized network with one descriptive `Edit` history entry per
    /// committed edit, ready to download as `.pio.json`. Any PowerIO reader
    /// understands the file; reloading it starts a fresh session.
    pub fn save_module(&self) -> Result<String, JsError> {
        install_panic_hook();
        self.0.save_module().map_err(jserr)
    }

    /// Save the committed exact DC OPF result as a PowerIO solution module.
    /// The embedded instance contains the materialized case that was solved.
    pub fn save_solution_module(&self) -> Result<String, JsError> {
        install_panic_hook();
        self.0.save_solution_module().map_err(jserr)
    }

    /// Export the committed operating point to a powerio `format` (`matpower`,
    /// `psse`, ...). Returns `{ text, diagnostics, format,
    /// extension }` as JSON: the serialized case, this output operation's
    /// structured diagnostics, and the format token and file extension.
    pub fn export(&self, format: &str) -> Result<String, JsError> {
        ensure_input_text(format)?;
        install_panic_hook();
        let exported = self.0.export(format).map_err(jserr)?;
        serde_json::to_string(&exported).map_err(jserr)
    }

    /// Run a bounded differentiable planning search over the committed
    /// operating point: the implicit gradient of the serializable outer
    /// objective through the exact DC OPF KKT orders capacity trials
    /// whose every step is verified by an exact re-solve. Read only — the
    /// committed case, edits, and revision are untouched. `spec_json` is a
    /// [`tellegen::plan::CapacityPlanSpec`]; the returned JSON is the complete
    /// [`tellegen::plan::CapacityPlanOutcome`] trace with the unapplied
    /// proposal. A non-DC formulation returns a typed error.
    pub fn plan(&self, spec_json: &str) -> Result<String, JsError> {
        ensure_input_text(spec_json)?;
        install_panic_hook();
        let spec: tellegen::plan::CapacityPlanSpec = serde_json::from_str(spec_json)
            .map_err(|e| jserr(format!("unreadable planning specification: {e}")))?;
        let outcome = self.0.plan(&spec).map_err(jserr)?;
        serde_json::to_string(&outcome).map_err(jserr)
    }

    /// Apply a geographic layer (a canonical `.geo.json` from [`parse_geo`] or
    /// [`apply_layout`]) onto this study's base network, so a later
    /// [`save_module`](Study::save_module) or export carries the coordinates
    /// on screen. Locations are metadata the model never reads; nothing
    /// re-solves. Returns the apply report JSON.
    pub fn apply_geo(&mut self, layer_geojson: &str) -> Result<String, JsError> {
        install_panic_hook();
        let layer = crate::geo::parse_layer(layer_geojson).map_err(jserr)?;
        let report = self.0.apply_geo_layer(&layer);
        serde_json::to_string(&crate::geo::report_value(&report)).map_err(jserr)
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

// ---------------------------------------------------------------------------
// Case file ingest (the drop-panel payload)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct ViewBus {
    id: usize,
    /// powerio row uid when the source format carries one (GOC3 does), null
    /// otherwise. The numeric `id` remains the edit key. The TS layer mirrors
    /// this shape.
    uid: Option<String>,
    lon: f64,
    lat: f64,
    demand_mw: f64,
    gen_mw: f64,
    /// False for a display-only bus synthesized by analysis lowering.
    editable: bool,
}

#[derive(Serialize)]
struct ViewBranch {
    id: usize,
    /// powerio row uid, as on [`ViewBus`].
    uid: Option<String>,
    from: usize,
    to: usize,
    rate_mw: f64,
    status: u8,
    /// False for a display-only branch synthesized by analysis lowering.
    editable: bool,
    /// The rendered polyline: the branch's `route` when a geographic file
    /// carried one (already endpoint-to-endpoint in file order), else the
    /// straight segment between the bus points.
    path: Vec<[f64; 2]>,
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
    uid: Option<String>,
    demand_mw: f64,
    gen_mw: f64,
    editable: bool,
}

#[derive(Serialize)]
struct TopologyBranch {
    id: usize,
    /// powerio row uid, as on [`ViewBus`].
    uid: Option<String>,
    from: usize,
    to: usize,
    rate_mw: f64,
    status: u8,
    editable: bool,
}

/// One optional uid per rendered analysis row, in row order.
type AnalysisUids = Vec<Option<String>>;

/// Deterministic identities for rendered analysis rows. Source rows keep their
/// source uid when the format carries one. Lowered rows receive display-only
/// ids chosen outside every canonical bus and branch uid, so forwarding one to
/// an edit API always fails closed instead of aliasing a source element with
/// an unfortunate custom uid.
fn analysis_uids(
    source: &powerio::BalancedNetwork,
    analysis: &powerio::BalancedNetwork,
) -> Result<(AnalysisUids, AnalysisUids), String> {
    let mut reserved: HashSet<String> = source
        .buses()
        .iter()
        .filter_map(|bus| bus.uid.clone())
        .chain(
            source
                .branches()
                .iter()
                .filter_map(|branch| branch.uid.clone()),
        )
        .collect();

    let mut unique_synthetic = |family: &str, index: usize| -> Result<String, String> {
        let stem = format!("analysis:{family}:{index}");
        if reserved.insert(stem.clone()) {
            return Ok(stem);
        }
        let mut suffix = 1usize;
        loop {
            let candidate = format!("{stem}:{suffix}");
            if reserved.insert(candidate.clone()) {
                return Ok(candidate);
            }
            suffix = suffix
                .checked_add(1)
                .ok_or_else(|| format!("{family} analysis uid space exhausted"))?;
        }
    };

    let mut buses = Vec::with_capacity(analysis.buses().len());
    for (index, bus) in analysis.buses().iter().enumerate() {
        buses.push(if index < source.buses().len() {
            bus.uid.clone()
        } else {
            Some(unique_synthetic("buses", index)?)
        });
    }
    let mut branches = Vec::with_capacity(analysis.branches().len());
    for (index, branch) in analysis.branches().iter().enumerate() {
        branches.push(if index < source.branches().len() {
            branch.uid.clone()
        } else {
            Some(unique_synthetic("branches", index)?)
        });
    }
    Ok((buses, branches))
}

#[derive(Serialize)]
struct Topology {
    buses: Vec<TopologyBus>,
    branches: Vec<TopologyBranch>,
}

/// Everything the drop panel needs from one parse: counts, total load and
/// capacity, parse diagnostics, and a `view` of buses and branches in the shape
/// the tellegen API serves, placed at the coordinates the file carries
/// (PowerWorld complete case aux exports). `view` is null when the file has no
/// coordinates.
///
/// Takes the dropped file's bytes, as [`parse_case`] does and for the same
/// reason.
#[wasm_bindgen]
pub fn ingest_case(bytes: &[u8], format: &str) -> Result<String, JsError> {
    ensure_input_bytes(bytes)?;
    install_panic_hook();
    serde_json::to_string(&ingest_case_value(bytes, format).map_err(jserr)?).map_err(jserr)
}

fn ingest_case_value(bytes: &[u8], format: &str) -> Result<serde_json::Value, String> {
    let module = parse_case_module(bytes, format)?;
    // The aux reader retains the complete source table beside the typed
    // network. Materialize its substation join on the module value so the map
    // and every later save see the same coordinates.
    let is_aux = module.sources().iter().any(|source| {
        source
            .format()
            .is_some_and(|format| matches!(format.as_str(), "aux" | "powerworld"))
    });
    let mut module = module;
    if is_aux {
        let source_text = std::str::from_utf8(bytes)
            .map_err(|_| "PowerWorld aux source is not valid UTF-8".to_owned())?;
        apply_aux_substation_locations(module.value_mut(), source_text)?;
    }
    let payload = ingest_value(module.value(), &module.diagnostics, Vec::new(), None)?;
    with_module_json(payload, module.map_value(PioValue::BalancedNetwork))
}

/// The distribution counterpart of [`ingest_case`]: view a multiconductor case
/// with no solve. `format` is a distribution reader token (`dss`, `bmopf`,
/// `pmd`) parsed by [`powerio_dist`], or `pio` for a `.pio.json` stored module
/// holding a supported multiconductor network, instance, or solution. Returns
/// the drop-panel payload JSON:
/// name, element counts, connected load/generation (kW), parse diagnostics,
/// coordinate provenance, and the bus/terminal graph the frontend renders (see
/// the private `dist` adapter).
///
/// Untrusted input: the parse and module restore return error strings (mapped
/// to `JsError` here), so a malformed, truncated, or oversized `.dss`/JSON
/// rejects cleanly and never panics the wasm instance.
#[wasm_bindgen]
pub fn ingest_dist_case(text: &str, format: &str) -> Result<String, JsError> {
    ensure_input_text(text)?;
    install_panic_hook();
    let out = match format {
        "pio" | "pio-json" | "package" => dist::ingest_dist_module(text),
        _ => dist::ingest_dist(text, format),
    };
    out.map_err(jserr)
}

/// Byte counterpart of [`ingest_dist_case`] for dropped files.
#[wasm_bindgen]
pub fn ingest_dist_case_bytes(bytes: &[u8], format: &str) -> Result<String, JsError> {
    ensure_input_bytes(bytes)?;
    install_panic_hook();
    let out = match format {
        "pio" | "pio-json" | "package" => dist::ingest_dist_module_bytes(bytes),
        _ => dist::ingest_dist_bytes(bytes, format),
    };
    out.map_err(jserr)
}

/// The drop-panel payload for an already-parsed network, as a JSON value.
/// `source_text` is the original case text
/// for formats that keep coordinate data in retained source (PowerWorld aux
/// substation tables); callers without it pass None. Errors are strings
/// (mapped to `JsError` at the wasm edge) so the same body runs in native
/// unit tests.
pub(crate) fn ingest_value(
    net: &powerio::BalancedNetwork,
    diagnostics: &[powerio::Diagnostic],
    mut warnings: Vec<String>,
    source_text: Option<&str>,
) -> Result<serde_json::Value, String> {
    net.validate().map_err(|error| error.to_string())?;
    // Reject a duplicate or numeric-looking source uid before the browser
    // receives an ambiguous edit/persistence key.
    tellegen::validate_canonical_identity(net)?;
    let power_scale = if net.is_normalized() {
        net.check_base_mva().map_err(|error| error.to_string())?;
        net.base_mva()
    } else {
        1.0
    };
    let indexed = powerio_tx::IndexedNetwork::new(net);
    let analysis = indexed.network();
    let (analysis_bus_uids, analysis_branch_uids) = analysis_uids(net, analysis)?;
    let editable_bus_ids: HashSet<usize> = net
        .buses()
        .iter()
        .filter(|bus| bus.kind != powerio::BusType::Isolated)
        .map(|bus| bus.id.0)
        .collect();
    let editable_branch_rows: HashSet<usize> = net
        .branches()
        .iter()
        .enumerate()
        .filter(|(_, branch)| {
            branch.in_service
                && branch.from != branch.to
                && branch.r * branch.r + branch.x * branch.x > 0.0
                && editable_bus_ids.contains(&branch.from.0)
                && editable_bus_ids.contains(&branch.to.0)
        })
        .map(|(row, _)| row)
        .collect();
    let mut demand: BTreeMap<usize, f64> = BTreeMap::new();
    for l in analysis.loads().iter().filter(|l| l.in_service) {
        *demand.entry(l.bus.0).or_default() += l.p * power_scale;
    }
    let mut gen: BTreeMap<usize, f64> = BTreeMap::new();
    for g in analysis.generators().iter().filter(|g| g.in_service) {
        *gen.entry(g.bus.0).or_default() += g.pmax * power_scale;
    }

    let topology = Topology {
        buses: analysis
            .buses()
            .iter()
            .enumerate()
            .map(|(i, b)| {
                Ok(TopologyBus {
                    id: b.id.0,
                    uid: analysis_bus_uids[i].clone(),
                    demand_mw: demand.get(&b.id.0).copied().unwrap_or(0.0),
                    gen_mw: gen.get(&b.id.0).copied().unwrap_or(0.0),
                    editable: i < net.buses().len() && editable_bus_ids.contains(&b.id.0),
                })
            })
            .collect::<Result<_, String>>()?,
        branches: analysis
            .branches()
            .iter()
            .enumerate()
            .map(|(i, br)| {
                Ok(TopologyBranch {
                    id: i + 1,
                    uid: analysis_branch_uids[i].clone(),
                    from: br.from.0,
                    to: br.to.0,
                    rate_mw: br.rate_a * power_scale,
                    status: br.in_service as u8,
                    editable: editable_branch_rows.contains(&i),
                })
            })
            .collect::<Result<_, String>>()?,
    };

    let view = {
        let source_coords = network_coords(net, source_text);
        let mut cs = lowered_coords(net, &source_coords);
        if cs.is_empty() {
            None
        } else {
            let missing_buses = analysis.buses().len().saturating_sub(cs.len());
            if missing_buses > 0 {
                warnings.push(format!(
                    "{missing_buses} bus(es) lacked coordinates and are omitted from the map"
                ));
            }
            spread_stacks(&mut cs);
            let buses: Vec<ViewBus> = analysis
                .buses()
                .iter()
                .enumerate()
                .filter_map(|(i, b)| {
                    let &(lon, lat) = cs.get(&b.id.0)?;
                    Some(Ok(ViewBus {
                        id: b.id.0,
                        uid: analysis_bus_uids[i].clone(),
                        lon,
                        lat,
                        demand_mw: demand.get(&b.id.0).copied().unwrap_or(0.0),
                        gen_mw: gen.get(&b.id.0).copied().unwrap_or(0.0),
                        editable: i < net.buses().len() && editable_bus_ids.contains(&b.id.0),
                    }))
                })
                .collect::<Result<_, String>>()?;
            let branches: Vec<ViewBranch> = analysis
                .branches()
                .iter()
                .enumerate()
                .filter_map(|(i, br)| {
                    let f = cs.get(&br.from.0)?;
                    let t = cs.get(&br.to.0)?;
                    let path = match &br.route {
                        Some(route) if route.len() >= 2 => {
                            route.iter().map(|p| [p.x, p.y]).collect()
                        }
                        _ => vec![[f.0, f.1], [t.0, t.1]],
                    };
                    Some(Ok(ViewBranch {
                        id: i + 1,
                        uid: analysis_branch_uids[i].clone(),
                        from: br.from.0,
                        to: br.to.0,
                        rate_mw: br.rate_a * power_scale,
                        status: br.in_service as u8,
                        editable: editable_branch_rows.contains(&i),
                        path,
                    }))
                })
                .collect::<Result<_, String>>()?;
            let missing_branches = analysis.branches().len().saturating_sub(branches.len());
            if missing_branches > 0 {
                warnings.push(format!(
                    "{missing_branches} branch(es) lacked endpoint coordinates and are omitted from the map"
                ));
            }
            Some(View { buses, branches })
        }
    };

    Ok(serde_json::json!({
        "name": net.name(),
        "base_mva": net.base_mva(),
        "n_bus": net.buses().len(),
        "n_branch": net.branches().len(),
        "n_analysis_bus": analysis.buses().len(),
        "n_analysis_branch": analysis.branches().len(),
        "n_gen": net.generators().iter().filter(|g| g.in_service).count(),
        "load_mw": demand.values().sum::<f64>(),
        "gen_mw": gen.values().sum::<f64>(),
        "has_coords": view.is_some(),
        "coords_kind": coords_kind_token(net, view.is_some()),
        "topology": topology,
        "diagnostics": diagnostics,
        "warnings": warnings,
        "view": view,
    }))
}

/// The coordinate provenance token the frontend placement logic reads:
/// `synthetic`/`manual` echo a saved PowerIO module's layout provenance, so a
/// reloaded case comes back placed with the same badge,
/// `file` is any other located case, and `synthetic_pending` awaits placement.
fn coords_kind_token(net: &powerio::BalancedNetwork, has_view: bool) -> &'static str {
    if !has_view {
        return "synthetic_pending";
    }
    match net.geo().as_ref().and_then(|g| g.kind) {
        Some(powerio::geo::CoordsKind::Synthetic) => "synthetic",
        Some(powerio::geo::CoordsKind::Manual) => "manual",
        _ => "file",
    }
}

#[derive(Serialize)]
struct ViewSubstation {
    number: u32,
    name: String,
    x: f64,
    y: f64,
    /// Approximate longitude/latitude via powerio's inverse of the projection
    /// PowerWorld's auto generated layouts use, so the frontend never
    /// reimplements the Mercator constant.
    lon: f64,
    lat: f64,
}

#[derive(Serialize)]
struct DisplayView {
    substations: Vec<ViewSubstation>,
    canvas_width: f64,
    canvas_height: f64,
}

/// Decode a PowerWorld `.pwd` display file (binary). Returns the substation
/// symbols at the diagram coordinates the file stores (x east, y north) plus
/// the canvas size, each with the approximate `lon`/`lat` projection
/// (`to_lonlat_from_pwd_mercator`; hand edited diagrams drift from it). A `.pwd`
/// carries no buses or branches. `format` is "pwd". Pure in-memory parsing,
/// no filesystem, so it runs in the browser.
#[wasm_bindgen]
pub fn parse_display(bytes: &[u8], format: &str) -> Result<String, JsError> {
    ensure_input_bytes(bytes)?;
    if !format.eq_ignore_ascii_case("pwd") {
        return Err(JsError::new("unsupported display format"));
    }
    install_panic_hook();
    let source = Source::from_memory("display.pwd", bytes.to_vec()).map_err(jserr)?;
    let module = powerio::parse(source).map_err(jserr)?;
    let PioValue::GeoLayer(layer) = module.into_value() else {
        return Err(JsError::new(
            "PowerWorld display did not parse as a geographic layer",
        ));
    };
    let (canvas_width, canvas_height) = match &layer.space {
        powerio::CoordinateSpace::Diagram {
            canvas: Some(canvas),
        } => (canvas.width.unwrap_or(0.0), canvas.height.unwrap_or(0.0)),
        _ => (0.0, 0.0),
    };
    let substations = layer
        .features
        .into_iter()
        .filter_map(|feature| {
            if feature.target != powerio::GeoTarget::Substation {
                return None;
            }
            let powerio::GeoGeometry::Point([x, y]) = feature.geometry else {
                return None;
            };
            let number = feature.key.id?.parse().ok()?;
            let (lon, lat) = powerio::to_lonlat_from_pwd_mercator(x, y);
            Some(ViewSubstation {
                number,
                name: feature.key.name.unwrap_or_default(),
                x,
                y,
                lon,
                lat,
            })
        })
        .collect();
    serde_json::to_string(&DisplayView {
        substations,
        canvas_width,
        canvas_height,
    })
    .map_err(jserr)
}

// ---------------------------------------------------------------------------
// Geographic sidecars, layouts, and `.pwd` promotion (see [`geo`])
// ---------------------------------------------------------------------------

/// Parse a geographic sidecar (buscoords CSV, aliased CSV/JSON records,
/// GeoJSON) from raw dropped bytes; `hint` is the file name ("" to sniff).
/// Returns `{ layer, diagnostics, n_points, n_routes }` — the layer as its
/// canonical `.geo.json` document, ready for [`apply_geo`]. Untrusted input
/// rejects as a `JsError`, never a panic.
#[wasm_bindgen]
pub fn parse_geo(bytes: &[u8], hint: &str) -> Result<String, JsError> {
    ensure_input_bytes(bytes)?;
    install_panic_hook();
    geo::parse_geo_impl(bytes, hint).map_err(jserr)
}

/// Apply a [`parse_geo`] layer onto a case network: matched bus points land in
/// `Bus.location`, matched routes in `Branch.route` (matching by uid, external
/// id, name, and the unordered endpoint pair — upstream semantics). Returns
/// the refreshed `ingest_case` payload plus a `report` of matched/unmatched
/// counts; errors when nothing matched.
#[wasm_bindgen]
pub fn apply_geo(module_json: &str, layer_geojson: &str) -> Result<String, JsError> {
    ensure_input_text(module_json)?;
    install_panic_hook();
    geo::apply_geo_impl(module_json, layer_geojson).map_err(jserr)
}

/// Stamp a computed layout (bus id => `[lon, lat]`) onto a case network with
/// `kind` provenance (`synthetic` or `manual`). Returns the refreshed
/// `ingest_case` payload plus `layer`, the stamped layout as a canonical
/// `.geo.json` document.
#[wasm_bindgen]
pub fn apply_layout(module_json: &str, coords_json: &str, kind: &str) -> Result<String, JsError> {
    ensure_input_text(module_json)?;
    install_panic_hook();
    geo::apply_layout_impl(module_json, coords_json, kind).map_err(jserr)
}

/// A case's coordinates as a canonical `.geo.json` document (one point per
/// located bus, one route per routed branch, provenance preserved). Errors
/// when the case carries none.
#[wasm_bindgen]
pub fn extract_geo(module_json: &str) -> Result<String, JsError> {
    ensure_input_text(module_json)?;
    install_panic_hook();
    geo::extract_geo_impl(module_json).map_err(jserr)
}

/// Fill case coordinates from a PowerWorld `.pwd` sibling: substation symbols
/// project to approximate longitude/latitude and join onto buses through the
/// `SubNum` extras key. Returns the refreshed `ingest_case` payload plus a
/// `report`; errors when no bus joined.
#[wasm_bindgen]
pub fn apply_display_geo(module_json: &str, bytes: &[u8]) -> Result<String, JsError> {
    ensure_input_text(module_json)?;
    ensure_input_bytes(bytes)?;
    install_panic_hook();
    geo::apply_display_geo_impl(module_json, bytes).map_err(jserr)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    /// Parse in-memory MATPOWER text over the module route (tests only).
    fn parse_matpower(text: &str) -> powerio::BalancedNetwork {
        parse_case_module(text.as_bytes(), "matpower")
            .expect("parse matpower")
            .into_value()
    }

    #[test]
    fn byte_input_limit_is_inclusive_without_allocating_the_boundary() {
        assert_eq!(ensure_input_len(MAX_INPUT_BYTES), Ok(()));
        assert_eq!(ensure_input_len(MAX_INPUT_BYTES + 1), Err(INPUT_TOO_LARGE));
    }

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
        let out = ingest_case(CASE14_NO_COORDS.as_bytes(), "m").expect("ingest case14");
        let v: Value = serde_json::from_str(&out).unwrap();

        assert_eq!(v["n_bus"].as_u64().unwrap(), 14);
        assert_eq!(v["coords_kind"].as_str().unwrap(), "synthetic_pending");
        assert!(v["view"].is_null());
        assert!(v["module_json"]
            .as_str()
            .unwrap()
            .contains("case14synthetic"));
        assert_eq!(v["topology"]["buses"].as_array().unwrap().len(), 14);
        assert_eq!(v["topology"]["branches"].as_array().unwrap().len(), 20);
        assert_eq!(
            v["topology"]["buses"][1]["demand_mw"].as_f64().unwrap(),
            21.7
        );

        // A source without uids carries the ones PowerIO assigned, the bus
        // number and the branch terminals; the numeric id stays an edit key
        // and neither payload invents a row key.
        assert_eq!(v["topology"]["buses"][0]["uid"], "1");
        assert_eq!(v["topology"]["branches"][1]["uid"], "1-5");
        assert!(!v["module_json"].as_str().unwrap().contains("buses:0"));
    }

    #[test]
    fn normalized_ingest_still_reports_native_mw() {
        let raw = parse_matpower(CASE14_NO_COORDS);
        let normalized = raw.to_normalized().expect("normalize case14");

        let source = ingest_value(&raw, &[], Vec::new(), None).expect("ingest raw");
        let derived = ingest_value(&normalized, &[], Vec::new(), None).expect("ingest normalized");
        let close = |left: &Value, right: &Value| {
            let delta = left.as_f64().unwrap() - right.as_f64().unwrap();
            assert!(delta.abs() < 1e-9, "{left} != {right}");
        };
        close(&source["load_mw"], &derived["load_mw"]);
        close(&source["gen_mw"], &derived["gen_mw"]);
        close(
            &source["topology"]["buses"][1]["demand_mw"],
            &derived["topology"]["buses"][1]["demand_mw"],
        );
        close(
            &source["topology"]["branches"][0]["rate_mw"],
            &derived["topology"]["branches"][0]["rate_mw"],
        );
    }

    #[test]
    fn ingest_rejects_duplicate_source_uids() {
        let mut net = parse_matpower(CASE14_NO_COORDS);
        // Two rows carrying the same source uid would make an ambiguous
        // edit/persistence key; the ingest refuses before the browser sees it.
        net.buses_mut()[0].uid = Some("same-bus".into());
        net.buses_mut()[1].uid = Some("same-bus".into());
        let error = ingest_value(&net, &[], Vec::new(), None).unwrap_err();
        assert!(error.contains("duplicate bus uid"), "{error}");
    }

    #[test]
    fn three_winding_transformer_topology_is_closed_but_canonical_payload_stays_typed() {
        let mut net = parse_matpower(CASE14_NO_COORDS);
        let windings = [1, 2, 3].map(|id| powerio::Winding::new(powerio::BusId(id)));
        let impedance = powerio::Impedance::new(0.02, 0.2, net.base_mva());
        net.transformers_3w_mut()
            .push(powerio::Transformer3W::new(windings, [impedance; 3]));

        let value = ingest_value(&net, &[], Vec::new(), None).expect("ingest 3W case");
        assert_eq!(value["n_bus"], 14);
        assert_eq!(value["n_branch"], 20);
        assert_eq!(value["topology"]["buses"].as_array().unwrap().len(), 15);
        assert_eq!(value["topology"]["branches"].as_array().unwrap().len(), 23);
        let star = &value["topology"]["buses"][14];
        assert_eq!(star["editable"], false);
        assert!(star["uid"].as_str().unwrap().starts_with("analysis:buses:"));
        for branch in &value["topology"]["branches"].as_array().unwrap()[20..] {
            assert_eq!(branch["to"], 15);
            assert_eq!(branch["editable"], false);
            assert!(branch["uid"]
                .as_str()
                .unwrap()
                .starts_with("analysis:branches:"));
        }
        let dynamic = powerio::PioModule::new(PioValue::BalancedNetwork(net));
        let module_json = serialize_module(&dynamic).expect("PowerIO IR");
        let canonical = tellegen::ir::balanced_module(
            deserialize_module(&module_json).expect("read PowerIO IR"),
        )
        .expect("balanced network");
        assert_eq!(canonical.value().buses().len(), 14);
        assert_eq!(canonical.value().branches().len(), 20);
        assert_eq!(canonical.value().transformers_3w().len(), 1);
    }

    #[test]
    fn topology_marks_only_solver_rows_editable() {
        let mut net = parse_matpower(CASE14_NO_COORDS);
        net.buses_mut()[0].kind = powerio::BusType::Isolated;
        net.branches_mut()[0].in_service = false;
        let self_loop_from = net.branches()[1].from;
        net.branches_mut()[1].to = self_loop_from;

        let value = ingest_value(&net, &[], Vec::new(), None).expect("ingest filtered rows");
        assert_eq!(value["topology"]["buses"][0]["editable"], false);
        assert_eq!(value["topology"]["branches"][0]["editable"], false);
        assert_eq!(value["topology"]["branches"][1]["editable"], false);
    }

    #[test]
    fn json_drop_classification_comes_from_powerio_bytes() {
        let transmission = classify_json_drop_value(PM_NO_QMAX.as_bytes()).expect("classify PM");
        assert_eq!(transmission.kind, "transmission");
        assert_eq!(transmission.format, Some("powermodels-json"));

        let distribution = classify_json_drop_value(br#"{"data_model":"ENGINEERING","bus":{}}"#)
            .expect("classify PMD");
        assert_eq!(distribution.kind, "distribution");
        assert_eq!(distribution.format, Some("pmd-json"));

        let net = parse_matpower(CASE14_NO_COORDS);
        let module = serialize_module(&powerio::PioModule::new(
            powerio::PioValue::BalancedNetwork(net),
        ))
        .expect("module JSON");
        let mut bom_module = b"\xef\xbb\xbf".to_vec();
        bom_module.extend_from_slice(module.as_bytes());
        let module_class =
            classify_json_drop_value(&bom_module).expect("classify BOM-prefixed module");
        assert_eq!(module_class.kind, "module");
        assert_eq!(module_class.format, None);

        let mut invalid = br#"{"base_mva":100,"buses":[]"#.to_vec();
        invalid.push(0xff);
        invalid.push(b'}');
        let unknown = classify_json_drop_value(&invalid).expect("invalid UTF-8 classifies unknown");
        assert_eq!(unknown.kind, "unknown");
        assert!(std::str::from_utf8(&invalid).is_err());
        assert!(dist::ingest_dist_bytes(&invalid, "bmopf-json").is_err());
    }

    #[cfg(feature = "sensitivity")]
    #[test]
    fn unified_json_drop_ingests_bare_families_and_preserves_unresolved_bytes() {
        let transmission =
            ingest_json_drop_value(PM_NO_QMAX.as_bytes()).expect("ingest PowerModels JSON");
        assert_eq!(transmission.kind, "transmission");
        assert_eq!(transmission.format, Some("powermodels-json"));
        assert_eq!(transmission.payload["n_bus"], 2);
        assert!(transmission.payload["diagnostics"].is_array());
        assert!(transmission.payload["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .all(serde_json::Value::is_object));

        let distribution =
            ingest_json_drop_value(include_bytes!("../tests/fixtures/dist/micro_pmd.json"))
                .expect("ingest PMD JSON");
        assert_eq!(distribution.kind, "distribution");
        assert_eq!(distribution.format, Some("pmd-json"));
        assert_eq!(distribution.payload["model"], "multiconductor");
        assert_eq!(distribution.payload["n_bus"], 1);
        assert!(distribution.payload["diagnostics"].is_array());

        let ambiguous =
            ingest_json_drop_value(br#"{"baseMVA":100.0,"bus":{},"data_model":"ENGINEERING"}"#)
                .expect("ambiguous JSON remains unresolved");
        assert_eq!(ambiguous.kind, "ambiguous");
        assert_eq!(ambiguous.format, None);
        assert!(ambiguous.payload.is_null());

        let unknown = ingest_json_drop_value(b"{\"unrecognized\":true}")
            .expect("unknown JSON remains unresolved");
        assert_eq!(unknown.kind, "unknown");
        assert_eq!(unknown.format, None);
        assert!(unknown.payload.is_null());

        let invalid_utf8 = ingest_json_drop_value(b"{\"base_mva\":100,\"buses\":[]\xff}")
            .expect("invalid UTF-8 remains unresolved");
        assert_eq!(invalid_utf8.kind, "unknown");
        assert!(invalid_utf8.payload.is_null());

        let serialized = serde_json::to_value(transmission).expect("serialize drop result");
        let keys = serialized.as_object().expect("drop result object");
        assert_eq!(keys.len(), 3);
        assert!(keys.contains_key("kind"));
        assert!(keys.contains_key("format"));
        assert!(keys.contains_key("payload"));
    }

    #[test]
    fn module_reader_error_is_not_prefixed_twice() {
        // A document that is not PowerIO IR is refused by the reader; its
        // message must reach the caller once, with no prefix stacked on by
        // this crate.
        let error = dist::ingest_dist_module(r#"{"model_kind":"multiconductor","model":{}}"#)
            .expect_err("malformed module");
        assert_eq!(error.matches("not PowerIO IR").count(), 1, "{error}");
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

    #[cfg(feature = "sensitivity")]
    fn case14_module_json() -> String {
        let module = powerio::PioModule::new(powerio::PioValue::BalancedNetwork(parse_matpower(
            CASE14_NO_COORDS,
        )));
        serialize_module(&module).expect("module JSON")
    }

    #[cfg(feature = "sensitivity")]
    #[test]
    fn a_saved_module_ingests_as_a_fresh_case_at_the_committed_point() {
        // Study::save_module -> the unified JSON drop: the saved file is a
        // plain stored PowerIO module holding the materialized network, so it
        // ingests under the `module` kind as a fresh base with no sliders to
        // restore. The committed edit rides along as descriptive history.
        let net = case14_module_json();
        let mut study = tellegen::Study::new(&net, tellegen::Problem::DcOpf).expect("study");
        study
            .commit(&[tellegen::NetworkEdit::AddLoad {
                bus: tellegen::ElementKey::Id(2),
                p_mw: 10.0,
            }])
            .expect("commit");
        let text = study.save_module().expect("save_module");

        let value: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["schema"], "pio-ir");
        assert_eq!(value["version"], 2);
        assert_eq!(value["history"][0]["name"], "tellegen.add_load");

        let restored = ingest_json_drop_value(text.as_bytes()).expect("ingest saved module");
        assert_eq!(restored.kind, "module");
        assert_eq!(restored.format, None);
        assert_eq!(restored.payload["n_bus"], 14);
        assert_eq!(restored.payload["module_json"], text);
    }

    #[cfg(feature = "sensitivity")]
    #[test]
    fn unified_json_drop_routes_each_module_kind() {
        // A stored module holding a balanced network ingests like a plain
        // network drop under the one `module` kind.
        let balanced_json = serialize_module(&powerio::PioModule::new(
            powerio::PioValue::BalancedNetwork(parse_matpower(CASE14_NO_COORDS)),
        ))
        .expect("balanced module JSON");
        let balanced =
            ingest_json_drop_value(balanced_json.as_bytes()).expect("ingest balanced module");
        assert_eq!(balanced.kind, "module");
        assert_eq!(balanced.format, None);
        assert_eq!(balanced.payload["n_bus"], 14);

        // A declared problem module remains the browser Study input. Its
        // network supplies the render payload without replacing the instance.
        let dc_instance = powerio::DcOpfInstance::from_network(parse_matpower(CASE14_NO_COORDS))
            .expect("DC OPF instance");
        let dc_instance_json = serialize_module(&powerio::PioModule::new(
            powerio::PioValue::DcOpfInstance(dc_instance),
        ))
        .expect("DC OPF instance module JSON");
        let dc_class = classify_json_drop_value(dc_instance_json.as_bytes())
            .expect("classify DC OPF instance module");
        assert_eq!(dc_class.kind, "module");
        let dc_drop = ingest_json_drop_value(dc_instance_json.as_bytes())
            .expect("ingest DC OPF instance module");
        assert_eq!(dc_drop.payload["n_bus"], 14);
        assert_eq!(dc_drop.payload["module_json"], dc_instance_json);

        // The browser has no AC PF or AC OPF formulation selector. Keep the
        // declared calculation intact and refuse it instead of opening its
        // network under the default DC OPF formulation.
        let ac_opf = powerio::AcOpfInstance::from_network(parse_matpower(CASE14_NO_COORDS))
            .expect("AC OPF instance");
        let (ac_pf, _) = ac_opf.to_ac_pf().expect("AC PF instance");
        for value in [
            powerio::PioValue::AcPfInstance(ac_pf),
            powerio::PioValue::AcOpfInstance(ac_opf),
        ] {
            let kind = value.type_name().to_owned();
            let module_json =
                serialize_module(&powerio::PioModule::new(value)).expect("AC instance module JSON");
            let classify_error = classify_json_drop_value(module_json.as_bytes())
                .expect_err("AC instance must not classify as viewable");
            assert!(classify_error.contains(&kind), "{classify_error}");
            let ingest_error = ingest_json_drop_value(module_json.as_bytes())
                .expect_err("AC instance must not ingest as a DC study");
            assert!(ingest_error.contains(&kind), "{ingest_error}");
        }

        // One holding a multiconductor network routes to the dist view.
        let dist_source = Source::from_memory(
            "micro.json",
            include_bytes!("../tests/fixtures/dist/micro_pmd.json").to_vec(),
        )
        .expect("dist source")
        .with_format(powerio::FormatId::new("pmd").expect("pmd format id"));
        let dist_module = powerio::parse(dist_source).expect("parse distribution module");
        let dist_json = serialize_module(&dist_module).expect("multiconductor network module JSON");
        let multiconductor =
            ingest_json_drop_value(dist_json.as_bytes()).expect("ingest multiconductor module");
        assert_eq!(multiconductor.kind, "module");
        assert_eq!(multiconductor.format, None);
        assert_eq!(multiconductor.payload["model"], "multiconductor");
        assert_eq!(multiconductor.payload["n_bus"], 1);

        // A BMOPF document with no calculation section parses to the
        // multiconductor network; the view reads it like any other
        // distribution module.
        let bmopf_source = Source::from_memory(
            "micro.json",
            include_bytes!("../tests/fixtures/dist/micro_bmopf.json").to_vec(),
        )
        .expect("BMOPF source")
        .with_format(powerio::FormatId::new("bmopf").expect("BMOPF format id"));
        let bmopf_module = powerio::parse(bmopf_source).expect("parse BMOPF instance module");
        assert!(
            matches!(bmopf_module.value(), PioValue::MulticonductorNetwork(_)),
            "{}",
            bmopf_module.value().type_name()
        );
        let bmopf_json = serialize_module(&bmopf_module).expect("BMOPF instance module JSON");
        let classification =
            classify_json_drop_value(bmopf_json.as_bytes()).expect("classify BMOPF module");
        assert_eq!(classification.kind, "module");
        let bmopf =
            ingest_json_drop_value(bmopf_json.as_bytes()).expect("ingest BMOPF instance module");
        assert_eq!(bmopf.kind, "module");
        assert_eq!(bmopf.payload["model"], "multiconductor");
        assert_eq!(bmopf.payload["n_bus"], 2);
    }

    #[test]
    fn aux_ingest_materializes_substation_locations_in_the_retained_module() {
        let aux = b"DATA (Substation, [SubNum, Latitude, Longitude])\n{\n12 35.0 -81.0\n}\n\
                    DATA (Bus, [BusNum, BusName, BusNomVolt, BusSlack, SubNum])\n{\n\
                    1 \"ALPHA\" 138 \"YES\" 12\n}\n";

        for format in ["aux", "powerworld"] {
            let payload = ingest_case_value(aux, format).expect("ingest PowerWorld aux");
            assert_eq!(payload["view"]["buses"][0]["lon"], -81.0);
            assert_eq!(payload["view"]["buses"][0]["lat"], 35.0);

            let module_json = payload["module_json"]
                .as_str()
                .expect("retained module JSON");
            let module = deserialize_module(module_json).expect("read retained module");
            let module: powerio::PioModule<powerio::BalancedNetwork> =
                tellegen::ir::balanced_module(module).expect("retained balanced network");
            let location = module.value().buses()[0]
                .location
                .expect("materialized bus location");
            assert_eq!((location.x, location.y), (-81.0, 35.0));
        }
    }

    #[cfg(feature = "sensitivity")]
    #[test]
    fn unified_json_drop_rejects_a_malformed_module_after_classification() {
        let malformed = br#"{"schema":"pio-ir","version":2}"#;
        assert_eq!(powerio::classify_json_bytes(malformed), JsonClass::Module);
        assert!(ingest_json_drop_value(malformed).is_err());
    }

    #[cfg(feature = "sensitivity")]
    #[test]
    fn a_retired_study_document_is_not_a_module() {
        // The tellegen.study format is gone: classification is powerio's rule
        // alone, so the retired header is ordinary unrecognized JSON — never
        // a module, never a restored session.
        let retired = br#"{"schema":"tellegen.study","version":1,"module":{},"formulation":"dcopf","options":{},"commits":[]}"#;
        let classified = classify_json_drop_value(retired).expect("classification");
        assert_ne!(classified.kind, "module");
        let ingested = ingest_json_drop_value(retired).expect("ingest");
        assert_ne!(ingested.kind, "module");
        assert!(ingested.payload.is_null(), "payload: {}", ingested.payload);
    }

    #[cfg(feature = "sensitivity")]
    #[test]
    fn export_rejects_an_unknown_format_without_panicking() {
        let net = case14_module_json();
        let study = tellegen::Study::new(&net, tellegen::Problem::DcOpf).expect("study");
        assert!(study.export("nonesuch").is_err());
        assert!(study.export("").is_err());
    }

    /// A PowerModels generator that omits `qmax` reads as an unbounded reactive
    /// limit, which powerio carries as `+Inf`. JSON has no `Inf` literal, so
    /// powerio spells it as the string `"Infinity"` and reads it back the same
    /// way: the generation-2 PowerIO IR this payload hands the Study round-trips
    /// the unbounded limit, and the case both views and studies.
    #[test]
    fn a_case_with_an_unbounded_reactive_limit_reaches_a_study() {
        let out = ingest_case(PM_NO_QMAX.as_bytes(), "powermodels-json").expect("ingest");
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["n_bus"].as_u64().unwrap(), 2);

        let module_json = v["module_json"].as_str().unwrap();
        assert!(
            module_json.contains("\"qmax\": \"Infinity\"")
                || module_json.contains("\"qmax\":\"Infinity\""),
            "expected a string-spelled qmax in PowerIO IR, got: {module_json}"
        );
        let module = tellegen::ir::balanced_module(
            deserialize_module(module_json).expect("read PowerIO IR"),
        )
        .expect("balanced network");
        let qmax = module.value().generators()[0].qmax;
        assert!(
            qmax.is_infinite() && qmax.is_sign_positive(),
            "expected qmax to read back as +Inf, got: {qmax}"
        );

        #[cfg(feature = "sensitivity")]
        tellegen::Study::new(module_json, tellegen::Problem::DcOpf).expect("study");
    }

    /// Two buses, one generator with no `qmax`/`qmin`.
    const PM_NO_QMAX: &str = r#"{
      "name": "nq",
      "baseMVA": 100.0,
      "per_unit": true,
      "bus": {
        "1": {"index": 1, "bus_i": 1, "bus_type": 3, "vm": 1.0, "va": 0.0, "base_kv": 230.0, "vmax": 1.1, "vmin": 0.9},
        "2": {"index": 2, "bus_i": 2, "bus_type": 1, "vm": 1.0, "va": 0.0, "base_kv": 230.0, "vmax": 1.1, "vmin": 0.9}
      },
      "gen": {
        "1": {"index": 1, "gen_bus": 1, "pg": 0.5, "qg": 0.0, "pmax": 2.0, "pmin": 0.0, "vg": 1.0, "mbase": 100.0,
              "gen_status": 1, "model": 2, "ncost": 3, "cost": [0.0, 10.0, 0.0]}
      },
      "load": {"1": {"index": 1, "load_bus": 2, "pd": 0.5, "qd": 0.1, "status": 1}},
      "branch": {
        "1": {"index": 1, "f_bus": 1, "t_bus": 2, "br_r": 0.01, "br_x": 0.1, "b_fr": 0.0, "b_to": 0.0,
              "g_fr": 0.0, "g_to": 0.0, "tap": 1.0, "shift": 0.0, "br_status": 1, "rate_a": 2.0,
              "angmin": -0.5, "angmax": 0.5, "transformer": false}
      },
      "shunt": {}, "storage": {}, "switch": {}, "dcline": {}
    }"#;
}
