//! Protection relays for the browser: a scheme set by rule with its audit
//! and distance-zone spans, the standard test cases and their RMS runs, the
//! relays at the operating point and a static study of one fault.
//!
//! JSON in, JSON out; the numbers are the engine's
//! (`tellegen::protection::api`). Every entry point prepares the case from
//! the stored PowerIO module and its dynamic data, prepared as the app prepares its RMS case
//! (`case_json`: `load_model` and the RMS options; empty: constant-impedance
//! loads and the defaults).
//!
//! @block Protection entry points

use wasm_bindgen::prelude::*;

use crate::{ensure_input_text, install_panic_hook, jserr};
use tellegen::protection::api;

fn inputs(module_json: &str, dyr_text: &str) -> Result<(), JsValue> {
    install_panic_hook();
    ensure_input_text(module_json)?;
    ensure_input_text(dyr_text)
}

/// A scheme set by rule (`SettingOptions` JSON; empty: the defaults), with
/// its notes, audit and the distance-zone spans to draw.
#[wasm_bindgen]
pub fn protection_auto(
    module_json: &str,
    dyr_text: &str,
    options_json: &str,
    case_json: &str,
) -> Result<String, JsValue> {
    inputs(module_json, dyr_text)?;
    api::auto_module_json(module_json, dyr_text, options_json, case_json).map_err(jserr)
}

/// The audit and the distance-zone spans of a scheme (`ProtectionScheme`
/// JSON).
#[wasm_bindgen]
pub fn protection_audit(
    module_json: &str,
    dyr_text: &str,
    scheme_json: &str,
    case_json: &str,
) -> Result<String, JsValue> {
    inputs(module_json, dyr_text)?;
    api::audit_module_json(module_json, dyr_text, scheme_json, case_json).map_err(jserr)
}

/// The standard test cases of a scheme, not run (`SuiteOptions` JSON;
/// empty: the defaults).
#[wasm_bindgen]
pub fn protection_tests(
    module_json: &str,
    dyr_text: &str,
    scheme_json: &str,
    options_json: &str,
    case_json: &str,
) -> Result<String, JsValue> {
    inputs(module_json, dyr_text)?;
    api::tests_module_json(module_json, dyr_text, scheme_json, options_json, case_json)
        .map_err(jserr)
}

/// Run test cases in RMS (`ProtectionTest` JSON list; empty: every
/// standard test): the `SuiteReport` JSON.
#[wasm_bindgen]
pub fn protection_suite(
    module_json: &str,
    dyr_text: &str,
    scheme_json: &str,
    tests_json: &str,
    options_json: &str,
    case_json: &str,
) -> Result<String, JsValue> {
    inputs(module_json, dyr_text)?;
    api::suite_module_json(
        module_json,
        dyr_text,
        scheme_json,
        tests_json,
        options_json,
        case_json,
    )
    .map_err(jserr)
}

/// The relays at the load flow operating point: what each measures and
/// which functions pick up.
#[wasm_bindgen]
pub fn protection_operating(
    module_json: &str,
    dyr_text: &str,
    scheme_json: &str,
    case_json: &str,
) -> Result<String, JsValue> {
    inputs(module_json, dyr_text)?;
    api::operating_module_json(module_json, dyr_text, scheme_json, case_json).map_err(jserr)
}

/// A static study of one fault (`StaticFault` JSON) on the short circuit:
/// breakers open in sequence until the fault is isolated or `t_max` s pass
/// (0: 3 s).
#[wasm_bindgen]
pub fn protection_static(
    module_json: &str,
    dyr_text: &str,
    scheme_json: &str,
    fault_json: &str,
    t_max: f64,
    case_json: &str,
) -> Result<String, JsValue> {
    inputs(module_json, dyr_text)?;
    api::static_module_json(
        module_json,
        dyr_text,
        scheme_json,
        fault_json,
        t_max,
        case_json,
    )
    .map_err(jserr)
}
