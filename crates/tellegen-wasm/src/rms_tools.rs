//! The experimental RMS editing tools and the live session for the browser:
//! controller block graphs (template, check), a unit's models (set a model,
//! a published unit), and a simulation that advances on demand and takes
//! events at its current time ([`RmsLiveCase`]).
//!
//! The numbers are the engine's (`tellegen::rms`); this layer moves text and
//! flat buffers.

use serde::Serialize;
use tellegen::rms::{LoadModel, RecordSpec, RmsEvent, RmsLive};
use wasm_bindgen::prelude::*;

use crate::{ensure_input_text, install_panic_hook, jserr};

/// The controller record starting on `line` of the dynamic data as a block
/// graph (`BlockGraph` JSON).
#[wasm_bindgen]
pub fn rms_model_template(dyr_text: &str, line: usize) -> Result<String, JsValue> {
    install_panic_hook();
    ensure_input_text(dyr_text)?;
    tellegen::rms::template_json(dyr_text, line).map_err(jserr)
}

/// Check a block graph: `{"ok": true, "states": [...]}` or `{"ok": false,
/// "error": ...}`.
#[wasm_bindgen]
pub fn rms_check_graph(graph_json: &str) -> String {
    install_panic_hook();
    tellegen::rms::check_graph_json(graph_json)
}

/// DYR text with the `role` model of unit `(bus, machine_id)` set to `model`
/// (typical parameters), or removed when `model` is empty.
#[wasm_bindgen]
pub fn rms_dyr_set_model(
    dyr_text: &str,
    bus: usize,
    machine_id: &str,
    role: &str,
    model: &str,
) -> Result<String, JsValue> {
    install_panic_hook();
    ensure_input_text(dyr_text)?;
    let model = (!model.trim().is_empty()).then_some(model.trim());
    tellegen::rms::dyr_set_model(dyr_text, bus, machine_id, role, model).map_err(jserr)
}

/// DYR text giving unit `(bus, machine_id)` the models of a published unit.
#[wasm_bindgen]
pub fn rms_dyr_apply_preset(
    dyr_text: &str,
    bus: usize,
    machine_id: &str,
    preset: &str,
) -> Result<String, JsValue> {
    install_panic_hook();
    ensure_input_text(dyr_text)?;
    tellegen::rms::dyr_apply_preset(dyr_text, bus, machine_id, preset).map_err(jserr)
}

/// The published units (`UnitPreset` JSON list).
#[wasm_bindgen]
pub fn rms_unit_presets() -> String {
    install_panic_hook();
    tellegen::rms::presets::unit_presets_json()
}

/// The head of a live slice.
#[derive(Serialize)]
struct SliceMeta<'a> {
    /// Simulated time reached, s.
    t: f64,
    time: &'a [f64],
    /// The channel of each block of `values`, in order.
    keys: Vec<&'a str>,
    /// Events applied since the previous slice.
    events: &'a [String],
}

/// A simulation that advances on demand and takes events at its current
/// time.
#[wasm_bindgen]
pub struct RmsLiveCase(RmsLive);

#[wasm_bindgen]
impl RmsLiveCase {
    /// Start at the initial operating point of `module_json` (a stored
    /// PowerIO module) with `dyn_text` (DYR, or `{"dyr", "models"}` JSON).
    #[wasm_bindgen(constructor)]
    pub fn new(
        module_json: &str,
        dyn_text: &str,
        load_model: &str,
        dt: f64,
        output_step: f64,
    ) -> Result<RmsLiveCase, JsValue> {
        install_panic_hook();
        ensure_input_text(module_json)?;
        ensure_input_text(dyn_text)?;
        let load_model: LoadModel = if load_model.trim().is_empty() {
            LoadModel::default()
        } else {
            serde_json::from_value(serde_json::Value::String(load_model.into())).map_err(jserr)?
        };
        tellegen::rms::live_from_module_json(module_json, dyn_text, load_model, dt, output_step)
            .map(RmsLiveCase)
            .map_err(jserr)
    }

    /// Simulated time reached, s.
    pub fn time(&self) -> f64 {
        self.0.time()
    }

    /// Integrate with a step of about `dt` s from now on; returns the step
    /// taken.
    pub fn set_step(&mut self, dt: f64) -> Result<f64, JsValue> {
        self.0.set_step(dt).map_err(jserr)?;
        Ok(self.0.step())
    }

    /// Record only the channels of `spec_json` (`RecordSpec`) from now on;
    /// empty records every channel.
    pub fn set_record(&mut self, spec_json: &str) -> Result<(), JsValue> {
        let spec: Option<RecordSpec> = if spec_json.trim().is_empty() {
            None
        } else {
            Some(serde_json::from_str(spec_json).map_err(jserr)?)
        };
        self.0.set_record(spec);
        Ok(())
    }

    /// Advance `seconds`: the samples since the previous slice, packed as
    /// [`RmsLiveSlice`].
    pub fn advance(&mut self, seconds: f64) -> Result<RmsLiveSlice, JsValue> {
        install_panic_hook();
        let chunk = self.0.advance(seconds).map_err(jserr)?;
        let n = chunk.time.len();
        let mut values = Vec::with_capacity(chunk.channels.len() * n);
        for series in chunk.channels.values() {
            values.extend_from_slice(series);
        }
        let meta = SliceMeta {
            t: chunk.t,
            time: &chunk.time,
            keys: chunk.channels.keys().map(String::as_str).collect(),
            events: &chunk.events,
        };
        Ok(RmsLiveSlice {
            meta: serde_json::to_string(&meta).map_err(jserr)?,
            values,
        })
    }

    /// Apply `event_json` (`RmsEvent`) now; its time is the session's.
    /// Returns the engine's description of what it did.
    pub fn apply(&mut self, event_json: &str) -> Result<String, JsValue> {
        install_panic_hook();
        let event: RmsEvent = serde_json::from_str(event_json).map_err(jserr)?;
        self.0.apply(event).map_err(jserr)
    }

    /// Channels matching keys or `prefix:` patterns (`patterns_json`, a list;
    /// empty: all) at the current time, as a JSON object.
    pub fn snapshot(&mut self, patterns_json: &str) -> Result<String, JsValue> {
        let patterns: Vec<String> = if patterns_json.trim().is_empty() {
            Vec::new()
        } else {
            serde_json::from_str(patterns_json).map_err(jserr)?
        };
        serde_json::to_string(&self.0.snapshot(&patterns)).map_err(jserr)
    }

    /// Every event applied so far, stamped with its simulated time (JSON).
    pub fn event_log(&self) -> Result<String, JsValue> {
        serde_json::to_string(self.0.event_log()).map_err(jserr)
    }
}

/// The samples of one live slice: `values` holds `keys.len()` blocks of
/// `time.len()` samples each (see the `meta` JSON).
#[wasm_bindgen]
pub struct RmsLiveSlice {
    meta: String,
    values: Vec<f64>,
}

#[wasm_bindgen]
impl RmsLiveSlice {
    /// `{t, time, keys, events}` JSON.
    pub fn meta(&self) -> String {
        self.meta.clone()
    }
    /// Channel-major samples (moved out: a second call returns nothing).
    pub fn values(&mut self) -> Vec<f64> {
        std::mem::take(&mut self.values)
    }
}
