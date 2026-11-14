//! Read the frozen BMOPF fixtures, written in an older BMOPF draft, under
//! PowerIO 0.12. The fixture bytes stay as their pinned references record;
//! these helpers adjust the parsed JSON only.

use serde_json::Value;

/// Spell load model tokens as the current schema does (`CONSTANT_POWER`).
///
/// The older draft spelled them in lowercase. PowerIO 0.12.0 reads either
/// spelling, but its semantic check compares the token case-sensitively and
/// asks a lowercase `constant_power` load for `v_nom`.
#[allow(dead_code)]
pub fn current_load_tokens(case: &mut Value) {
    for load in case["load"]
        .as_object_mut()
        .into_iter()
        .flat_map(|loads| loads.values_mut())
    {
        if let Some(model) = load.get_mut("model") {
            if let Some(token) = model.as_str() {
                *model = Value::from(token.to_ascii_uppercase());
            }
        }
    }
}

/// Restate the fields the pinned BMOPFTools references read the way PowerIO
/// 0.11 prepared them.
///
/// PowerIO 0.12 follows the BMOPF schema and OpenDSS: a positive inverter
/// `pf` puts Q on the same side as P (it was the opposite side), and a type-A
/// regulator's `tap_ratio` is the regulated over source ratio, as for type B
/// (it was the inverse). The restated document describes the circuit the
/// references solved.
#[allow(dead_code)]
pub fn restate_in_current_conventions(case: &mut Value) {
    for profile in case["control_profile"]
        .as_object_mut()
        .into_iter()
        .flat_map(|profiles| profiles.values_mut())
    {
        if let Some(pf) = profile["power_factor"]["pf"].as_f64() {
            profile["power_factor"]["pf"] = (-pf).into();
        }
    }
    for transformer in case["transformer"]
        .as_object_mut()
        .into_iter()
        .flat_map(|kinds| kinds.values_mut())
        .filter_map(Value::as_object_mut)
        .flat_map(|records| records.values_mut())
        .filter(|record| record["regulator_type"] == "A")
    {
        let inverse = |key: &str| transformer[key].as_f64().map(f64::recip);
        let (ratio, low, high) = (
            inverse("tap_ratio"),
            inverse("tap_ratio_max"),
            inverse("tap_ratio_min"),
        );
        for (key, value) in [
            ("tap_ratio", ratio),
            ("tap_ratio_min", low),
            ("tap_ratio_max", high),
        ] {
            if let Some(value) = value {
                transformer[key] = value.into();
            }
        }
    }
}

/// Whether a parse diagnostic is the single notice that the fixture predates
/// the current schema.
#[allow(dead_code)]
pub fn is_legacy_notice(diagnostic: &powerio::Diagnostic) -> bool {
    diagnostic.code() == "READ.BMOPF.SCHEMA_LEGACY"
        && diagnostic.severity() == powerio::DiagnosticSeverity::Warning
}
