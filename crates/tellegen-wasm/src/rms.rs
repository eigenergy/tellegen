//! RMS dynamic simulation for the browser: a prepared case (`RmsCase`) that
//! runs any number of times, and the buffers of a run (`RmsRunBuffers`)
//! moved to JavaScript as two flat `Float64Array`s and a JSON head.
//!
//! The numbers are the engine's (`tellegen::rms`); this layer chooses what a
//! display records ([`RecordSpec::display`]), derives the screening series
//! (COI, bus frequencies), packs the channels channel-major and drops the
//! bus angles once the bus frequencies are derived from them. The packing is
//! plain Rust ([`run_buffers`]) so a native test checks it.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use tellegen::rms::{
    LoadModel, RecordSpec, RmsGenerator, RmsOptions, RmsRequest, RmsScreening, RmsSimulation,
    ScreeningCriteria,
};
use wasm_bindgen::prelude::*;

use crate::{ensure_input_text, install_panic_hook, jserr};

/// Experimental: the engine's threads, in a build with shared memory.
#[cfg(target_feature = "atomics")]
pub mod threads;

/// The most values (channels × samples) one run moves to JavaScript; a
/// longer run records at a coarser output step and says so in its notes.
pub const MAX_VALUES: f64 = 4e6;

/// How a case is prepared: its load model and the engine's options.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct CaseOptions {
    pub load_model: LoadModel,
    #[serde(flatten)]
    pub options: RmsOptions,
}

/// A run request as the browser sends it: an [`RmsRequest`] (without
/// `record`, the display set) and the screening criteria.
#[derive(Clone, Debug, Deserialize)]
pub struct RunRequest {
    #[serde(flatten)]
    pub request: RmsRequest,
    #[serde(default)]
    pub criteria: ScreeningCriteria,
}

/// The head of a run's buffers.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Serialize)]
pub struct RunMeta {
    /// The channel of each block of `values`, in order.
    pub keys: Vec<String>,
    pub notes: Vec<String>,
    pub events: Vec<String>,
    pub generators: Vec<RmsGenerator>,
    pub sources: Vec<String>,
    pub buses: Vec<usize>,
    pub base_mva: f64,
    pub base_frequency: f64,
    pub steps: usize,
    pub newton_iterations: usize,
    /// The output step the run recorded at, s (coarser than asked when the
    /// run would move more than [`MAX_VALUES`] values; noted).
    pub output_step: f64,
    pub dt: f64,
    pub t_end: f64,
    pub screening: RmsScreening,
    /// The request as run (events, steps, load model), to reproduce it.
    pub request: RmsRequest,
    /// The relays' record, when the run had relays acting.
    #[cfg(feature = "protection")]
    #[cfg_attr(feature = "schema", schemars(skip))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protection: Option<tellegen::protection::ProtectionReport>,
}

/// A run, packed: `values` holds `keys.len()` blocks of `time.len()`
/// samples each.
pub struct RunBuffers {
    pub meta: RunMeta,
    pub time: Vec<f64>,
    pub values: Vec<f64>,
}

/// Parse case options JSON (empty: the defaults).
///
/// # Errors
/// When the JSON does not parse.
pub fn case_options(json: &str) -> Result<CaseOptions, String> {
    if json.trim().is_empty() {
        return Ok(CaseOptions::default());
    }
    serde_json::from_str(json).map_err(|e| format!("options: {e}"))
}

/// Parse a run request JSON (empty: the defaults).
///
/// # Errors
/// When the JSON does not parse.
pub fn run_request(json: &str) -> Result<RunRequest, String> {
    if json.trim().is_empty() {
        return Ok(RunRequest {
            request: RmsRequest::default(),
            criteria: ScreeningCriteria::default(),
        });
    }
    serde_json::from_str(json).map_err(|e| format!("request: {e}"))
}

/// The relays a run request carries (`protection`, a `ProtectionScheme`),
/// if any.
///
/// # Errors
/// When the JSON or the scheme does not parse.
#[cfg(feature = "protection")]
pub fn run_scheme(json: &str) -> Result<Option<tellegen::protection::ProtectionScheme>, String> {
    #[derive(Deserialize)]
    struct Relays {
        #[serde(default)]
        protection: Option<tellegen::protection::ProtectionScheme>,
    }
    if json.trim().is_empty() {
        return Ok(None);
    }
    let r: Relays = serde_json::from_str(json).map_err(|e| format!("protection: {e}"))?;
    Ok(r.protection)
}

/// The number of channels [`RecordSpec::display`] records for `sim`.
fn display_channels(sim: &RmsSimulation) -> usize {
    let i = sim.inspection();
    // vm and va per bus; delta, omega and pe per machine; pe per held unit
    // or plant; pl and ql per load bus.
    2 * i.buses.len()
        + 3 * i.generators.len()
        + i.held_units.len()
        + i.plants.len()
        + 2 * i.load_buses.len()
}

/// `request` recording the display channels when it names none, with the
/// output step coarsened to keep the transfer bounded (and a note saying
/// so).
fn display_request(sim: &RmsSimulation, mut request: RmsRequest) -> (RmsRequest, Option<String>) {
    let mut coarse_note = None;
    if request.record.is_none() {
        request.record = Some(RecordSpec::display());
        // Keep the transfer bounded: coarsen the output step to a multiple
        // of dt when the display set would pass MAX_VALUES values.
        let r = &mut request;
        if r.dt > 0.0 && r.output_step > 0.0 && r.t_end > 0.0 {
            let ch = display_channels(sim) as f64;
            let samples = (r.t_end / r.output_step).floor() + 1.0;
            if ch * samples > MAX_VALUES {
                let want = r.t_end * ch / (MAX_VALUES - ch).max(1.0);
                let step = (want / r.dt).ceil() * r.dt;
                coarse_note = Some(format!(
                    "the display records every {step:.4} s instead of every {:.4} s: {ch} channels × {samples} samples would pass the {MAX_VALUES:e} values a browser run keeps",
                    r.output_step
                ));
                r.output_step = step;
            }
        }
    }
    (request, coarse_note)
}

/// Run `request` on `sim` with the display channels, screen it and pack the
/// result. `progress(t)` is called before every output sample with the time
/// reached.
///
/// # Errors
/// When the run or its screening fails (the engine's message, verbatim).
pub fn run_buffers(
    sim: &RmsSimulation,
    run: RunRequest,
    progress: &mut dyn FnMut(f64),
) -> Result<RunBuffers, String> {
    let (request, coarse_note) = display_request(sim, run.request);
    let mut k = 0usize;
    let step = request.output_step;
    let result = sim.run_cancellable(&request, &mut || {
        progress((k as f64 * step).min(request.t_end));
        k += 1;
        false
    })?;
    progress(request.t_end);
    pack(sim, request, &run.criteria, result, coarse_note)
}

/// [`run_buffers`] with the relays of `scheme` acting during the run; their
/// record comes with the head (`protection`). The run reports its progress
/// once, at the end.
///
/// # Errors
/// As [`run_buffers`], or when a relay does not bind to the case.
#[cfg(feature = "protection")]
pub fn run_buffers_protected(
    sim: &RmsSimulation,
    run: RunRequest,
    scheme: &tellegen::protection::ProtectionScheme,
    progress: &mut dyn FnMut(f64),
) -> Result<RunBuffers, String> {
    let (request, coarse_note) = display_request(sim, run.request);
    let (result, report) = sim.run_with_protection(&request, scheme)?;
    progress(request.t_end);
    let mut buffers = pack(sim, request, &run.criteria, result, coarse_note)?;
    buffers.meta.protection = Some(report);
    Ok(buffers)
}

/// Screen a finished run and pack its channels.
fn pack(
    sim: &RmsSimulation,
    request: RmsRequest,
    criteria: &ScreeningCriteria,
    result: tellegen::rms::RmsResult,
    coarse_note: Option<String>,
) -> Result<RunBuffers, String> {
    let (screening, derived) = sim.screen_with_series(&request, &result, criteria)?;
    let ns = result.time.len();
    let mut keys = Vec::new();
    let mut values = Vec::with_capacity((result.channels.len() + derived.len()) * ns);
    let mut push = |key: &str, series: &[f64]| {
        keys.push(key.to_owned());
        values.extend_from_slice(series);
    };
    // The bus angles stay behind: the bus frequencies derived from them
    // are what a display shows.
    for (key, series) in result
        .channels
        .iter()
        .filter(|(k, _)| !k.starts_with("va:"))
    {
        push(key, series);
    }
    let derived: BTreeMap<String, Vec<f64>> = derived;
    for (key, series) in &derived {
        push(key, series);
    }
    let mut notes = result.notes;
    notes.extend(coarse_note);
    Ok(RunBuffers {
        meta: RunMeta {
            keys,
            notes,
            events: result.events,
            generators: result.generators,
            sources: result.sources,
            buses: result.buses,
            base_mva: result.base_mva,
            base_frequency: result.base_frequency,
            steps: result.steps,
            newton_iterations: result.newton_iterations,
            output_step: request.output_step,
            dt: request.dt,
            t_end: request.t_end,
            screening,
            request: RmsRequest {
                record: None,
                ..request
            },
            #[cfg(feature = "protection")]
            protection: None,
        },
        time: result.time,
        values,
    })
}

/// The largest speed and voltage drift of a run without events, pu: a case
/// at rest stays where it starts.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Serialize)]
pub struct FlatCheck {
    pub seconds: f64,
    pub max_speed_drift: f64,
    pub max_voltage_drift: f64,
    pub steps: usize,
}

/// Run `seconds` without events and measure the drift.
///
/// # Errors
/// When the run fails.
pub fn flat_check(sim: &RmsSimulation, seconds: f64) -> Result<FlatCheck, String> {
    let request = RmsRequest {
        t_end: seconds,
        dt: 0.005,
        output_step: 0.05,
        record: Some(RecordSpec {
            channels: vec!["omega:".into(), "vm:".into()],
            ..RecordSpec::default()
        }),
        ..RmsRequest::default()
    };
    let r = sim.run(&request)?;
    let drift = |prefix: &str| {
        r.channels
            .iter()
            .filter(|(k, _)| k.starts_with(prefix))
            .map(|(_, v)| {
                let v0 = v.first().copied().unwrap_or(0.0);
                v.iter().map(|x| (x - v0).abs()).fold(0.0, f64::max)
            })
            .fold(0.0, f64::max)
    };
    Ok(FlatCheck {
        seconds,
        max_speed_drift: drift("omega:"),
        max_voltage_drift: drift("vm:"),
        steps: r.steps,
    })
}

/// Whether `text` is PSS/E dynamic data with at least one model the engine
/// reads.
#[wasm_bindgen]
pub fn rms_looks_like_dyr(text: &str) -> bool {
    install_panic_hook();
    tellegen::rms::looks_like_dyr(text)
}

/// A case prepared for RMS simulation: assembled and initialized once.
#[wasm_bindgen]
pub struct RmsCase(RmsSimulation);

#[wasm_bindgen]
impl RmsCase {
    /// Prepare `module_json` (a stored PowerIO module) with `dyr_text`;
    /// `options_json` holds `load_model` and the [`RmsOptions`] fields.
    #[wasm_bindgen(constructor)]
    pub fn new(module_json: &str, dyr_text: &str, options_json: &str) -> Result<RmsCase, JsValue> {
        install_panic_hook();
        ensure_input_text(module_json)?;
        ensure_input_text(dyr_text)?;
        let o = case_options(options_json).map_err(jserr)?;
        tellegen::rms::prepare_module_json(module_json, dyr_text, o.load_model, o.options)
            .map(RmsCase)
            .map_err(jserr)
    }

    /// The [`tellegen::rms::RmsInspection`] JSON (no run).
    pub fn inspection(&self) -> Result<String, JsValue> {
        install_panic_hook();
        serde_json::to_string(&self.0.inspection()).map_err(jserr)
    }

    /// Run `request_json` ([`RunRequest`]); `progress(t_reached)` is called
    /// at most every 50 ms of wall time. It never cancels: the host stops a
    /// run by terminating the worker.
    pub fn run(
        &self,
        request_json: &str,
        progress: &js_sys::Function,
    ) -> Result<RmsRunBuffers, JsValue> {
        install_panic_hook();
        let run = run_request(request_json).map_err(jserr)?;
        let mut last = f64::NEG_INFINITY;
        let mut report = |t: f64| {
            let now = js_sys::Date::now();
            if now - last >= 50.0 {
                last = now;
                let _ = progress.call1(&JsValue::NULL, &JsValue::from_f64(t));
            }
        };
        #[cfg(feature = "protection")]
        let b = match run_scheme(request_json).map_err(jserr)? {
            Some(scheme) => run_buffers_protected(&self.0, run, &scheme, &mut report),
            None => run_buffers(&self.0, run, &mut report),
        }
        .map_err(jserr)?;
        #[cfg(not(feature = "protection"))]
        let b = run_buffers(&self.0, run, &mut report).map_err(jserr)?;
        Ok(RmsRunBuffers {
            meta: serde_json::to_string(&b.meta).map_err(jserr)?,
            time: b.time,
            values: b.values,
        })
    }

    /// Run `seconds` without events: `{seconds, max_speed_drift,
    /// max_voltage_drift, steps}` JSON.
    pub fn flat_check(&self, seconds: f64) -> Result<String, JsValue> {
        install_panic_hook();
        let c = flat_check(&self.0, seconds).map_err(jserr)?;
        serde_json::to_string(&c).map_err(jserr)
    }
}

/// The packed buffers of a run (see [`RunMeta`]).
#[wasm_bindgen]
pub struct RmsRunBuffers {
    meta: String,
    time: Vec<f64>,
    values: Vec<f64>,
}

#[wasm_bindgen]
impl RmsRunBuffers {
    /// The [`RunMeta`] JSON.
    pub fn meta(&self) -> String {
        self.meta.clone()
    }
    /// The sample times, s (moved out: a second call returns nothing).
    pub fn time(&mut self) -> Vec<f64> {
        std::mem::take(&mut self.time)
    }
    /// Channel-major: `meta.keys.len()` blocks of `time.len()` samples
    /// (moved out: a second call returns nothing).
    pub fn values(&mut self) -> Vec<f64> {
        std::mem::take(&mut self.values)
    }
}

#[cfg(feature = "rms-experimental")]
#[wasm_bindgen]
impl RmsCase {
    /// Modes with participation factors at the operating point
    /// ([`tellegen::rms::LinearRequest`] JSON in, `LinearResult` JSON out).
    pub fn linear(&self, request_json: &str) -> Result<String, JsValue> {
        install_panic_hook();
        let request: tellegen::rms::LinearRequest = if request_json.trim().is_empty() {
            tellegen::rms::LinearRequest::default()
        } else {
            serde_json::from_str(request_json).map_err(jserr)?
        };
        let r = self.0.linear_analysis(&request).map_err(jserr)?;
        serde_json::to_string(&r).map_err(jserr)
    }
}

/// Give a static case's generators typical dynamic models: the DYR text.
#[cfg(feature = "rms-experimental")]
#[wasm_bindgen]
pub fn rms_dynamize(
    module_json: &str,
    dyr_text: &str,
    request_json: &str,
) -> Result<String, JsValue> {
    install_panic_hook();
    ensure_input_text(module_json)?;
    tellegen::rms::dyr_dynamize_json(module_json, dyr_text, request_json).map_err(jserr)
}

/// The model library (machines, controls, presets) as JSON.
#[cfg(feature = "rms-experimental")]
#[wasm_bindgen]
pub fn rms_library() -> String {
    install_panic_hook();
    tellegen::rms::library_json()
}

/// The parameters of every DYR record, as JSON.
#[cfg(feature = "rms-experimental")]
#[wasm_bindgen]
pub fn rms_dyr_parameters(module_json: &str, dyr_text: &str) -> Result<String, JsValue> {
    install_panic_hook();
    tellegen::rms::dyr_parameters_json(module_json, dyr_text).map_err(jserr)
}

/// Apply parameter edits to DYR text.
#[cfg(feature = "rms-experimental")]
#[wasm_bindgen]
pub fn rms_dyr_edit(dyr_text: &str, edit_json: &str) -> Result<String, JsValue> {
    install_panic_hook();
    tellegen::rms::dyr_edit_json(dyr_text, edit_json).map_err(jserr)
}

/// Schemas of the RMS records returned by the browser adapter.
#[cfg(all(feature = "schema", feature = "rms-experimental"))]
#[must_use]
pub fn output_schema() -> serde_json::Value {
    #[derive(schemars::JsonSchema)]
    #[allow(dead_code)]
    struct OutputSchema {
        run_meta: RunMeta,
        flat_check: FlatCheck,
        linear_result: tellegen::rms::LinearResult,
        library: Vec<tellegen::rms::LibraryModel>,
    }
    let schema = schemars::generate::SchemaSettings::default()
        .for_serialize()
        .into_generator()
        .into_root_schema_for::<OutputSchema>();
    serde_json::to_value(schema).expect("RMS output schema")
}
