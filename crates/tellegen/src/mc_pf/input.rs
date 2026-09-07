//! Raw BMOPF entry points and loss checks for the multiconductor PF.
//!
//! PowerIO remains the sole BMOPF reader.  The small preflight below only
//! rejects source fields whose v0.11 reader deliberately collapses or ignores;
//! it does not recreate the BMOPF schema or duplicate its normalization.

use powerio::{McAcPfInstance, PioValue};
use serde_json::Value;

use super::{solve_mc_ac_pf_instance, McPfOptions};

/// Validate raw BMOPF semantics before the canonical PowerIO reader runs.
///
/// This catches the particularly dangerous cases where PowerIO's permissive
/// reader would return a successful canonical network with a different model:
/// unknown constant-power models, nonuniform phase taps, unknown transformer
/// subtypes, and finite source impedances that the ideal-source MC solver would
/// otherwise discard.
pub fn validate_bmopf_json(text: &str) -> Result<(), String> {
    let root: Value = serde_json::from_str(text).map_err(|e| format!("invalid BMOPF JSON: {e}"))?;
    let object = root
        .as_object()
        .ok_or_else(|| "BMOPF document must be a JSON object".to_owned())?;

    // The tagged reader iterates only object tables and silently skips an
    // array or scalar table. Reject that shape change before parsing, and do
    // the same for rows in the ordinary element tables.
    for table in [
        "bus",
        "linecode",
        "line",
        "switch",
        "shunt",
        "load",
        "generator",
        "ibr",
        "capacitor",
        "voltage_source",
        "transformer",
    ] {
        let Some(value) = object.get(table) else {
            continue;
        };
        let Some(rows) = value.as_object() else {
            return Err(format!(
                "BMOPF table `{table}` must be an object keyed by element name"
            ));
        };
        if table != "transformer" {
            for (name, row) in rows {
                if !row.is_object() {
                    return Err(format!("BMOPF {table} `{name}` must be an object"));
                }
            }
        }
    }

    let terminal_conventions = object.get("terminal_conventions");
    let mut terminal_convention_non_phase =
        terminal_convention_names(terminal_conventions, "neutral");
    terminal_convention_non_phase.extend(terminal_convention_names(terminal_conventions, "earth"));
    if let Some(loads) = object.get("load").and_then(Value::as_object) {
        for (name, load) in loads {
            let Some(load) = load.as_object() else {
                return Err(format!("load `{name}` must be an object"));
            };
            let model = load
                .get("model")
                .and_then(Value::as_str)
                .unwrap_or("constant_power")
                .to_ascii_lowercase();
            if model != "constant_power" {
                return Err(format!(
                    "load `{name}` requests model `{model}`; multiconductor fixed-point PF supports constant_power only"
                ));
            }
            if model == "zip"
                && [
                    "alpha_z", "alpha_i", "alpha_p", "beta_z", "beta_i", "beta_p",
                ]
                .iter()
                .any(|key| !load.contains_key(*key))
            {
                return Err(format!(
                    "load `{name}` requests ZIP without all six coefficient arrays; PowerIO would read it as constant power"
                ));
            }
            if model == "exponential"
                && (!load.contains_key("gamma_p") || !load.contains_key("gamma_q"))
            {
                return Err(format!(
                    "load `{name}` requests exponential voltage dependence without gamma_p and gamma_q; PowerIO would read it as constant power"
                ));
            }
            if let Some(model) = load.get("model") {
                let Some(model) = model.as_str() else {
                    return Err(format!("load `{name}` model must be a string"));
                };
                if model.trim().is_empty() {
                    return Err(format!("load `{name}` model must not be empty"));
                }
            }
            if let Some(configuration) = load.get("configuration") {
                validate_configuration(configuration, &format!("load `{name}` configuration"))?;
            }
            let configuration = load
                .get("configuration")
                .and_then(Value::as_str)
                .unwrap_or("WYE");
            if configuration.eq_ignore_ascii_case("WYE")
                && !terminal_convention_non_phase.is_empty()
            {
                let phases = load.get("p_nom").and_then(Value::as_array).map(Vec::len);
                validate_wye_terminal_order(
                    "load",
                    name,
                    "terminal_map",
                    load.get("terminal_map"),
                    &terminal_convention_non_phase,
                    phases,
                )?;
            }
        }
    }

    for (table_name, sources) in [
        (
            "voltage_source",
            object.get("voltage_source").and_then(Value::as_object),
        ),
        (
            "extras.voltage_source",
            object
                .get("extras")
                .and_then(Value::as_object)
                .and_then(|e| e.get("voltage_source"))
                .and_then(Value::as_object),
        ),
    ] {
        if let Some(sources) = sources {
            for (name, source) in sources {
                let Some(source) = source.as_object() else {
                    return Err(format!("voltage source `{name}` must be an object"));
                };
                for key in [
                    "r1", "x1", "r0", "x0", "r2", "x2", "isc1", "isc3", "mvasc", "mvasc1",
                    "mvasc3", "z_source", "z1", "z0",
                ] {
                    if source.contains_key(key) {
                        return Err(format!("{table_name} `{name}` declares finite impedance `{key}`; normalize it to an explicit Norton source before MC PF"));
                    }
                }
            }
        }
    }

    if let Some(transformers) = object.get("transformer").and_then(Value::as_object) {
        for (subtype, records) in transformers {
            if !matches!(
                subtype.as_str(),
                "single_phase" | "center_tap" | "wye_delta" | "delta_wye" | "n_winding"
            ) {
                return Err(format!("transformer subtype `{subtype}` is unknown; PowerIO reads it as a single-phase pair"));
            }
            let Some(records) = records.as_object() else {
                return Err(format!("transformer group `{subtype}` must be an object"));
            };
            for (name, record) in records {
                validate_transformer_record(subtype, name, record)?;
                validate_transformer_terminal_order(subtype, name, record, terminal_conventions)?;
            }
        }
    }
    // PowerIO folds extras.transformer back onto matching records before
    // reading. Validate the overlay as well so a lossy nested field cannot
    // bypass the canonical table checks.
    if let Some(groups) = object
        .get("extras")
        .and_then(Value::as_object)
        .and_then(|e| e.get("transformer"))
        .and_then(Value::as_object)
    {
        for (subtype, records) in groups {
            if !matches!(
                subtype.as_str(),
                "single_phase" | "center_tap" | "wye_delta" | "delta_wye" | "n_winding"
            ) {
                return Err(format!(
                    "extras.transformer subtype `{subtype}` is unknown and would be ignored by PowerIO"
                ));
            }
            let Some(records) = records.as_object() else {
                return Err(format!("extras.transformer.{subtype} must be an object"));
            };
            for (name, record) in records {
                validate_transformer_record(subtype, name, record)?;
                validate_transformer_terminal_order(
                    subtype,
                    name,
                    record,
                    object.get("terminal_conventions"),
                )?;
            }
        }
    }
    Ok(())
}

/// PowerIO's typed winding has no separate neutral-terminal field. Its
/// terminal-map contract is phase coils first, followed by the WYE neutral
/// when one is explicit. Reject a raw permutation when terminal conventions
/// identify a neutral/earth elsewhere; otherwise the primitive would silently
/// use the wrong conductor as the common return.
fn validate_transformer_terminal_order(
    subtype: &str,
    name: &str,
    raw: &Value,
    terminal_conventions: Option<&Value>,
) -> Result<(), String> {
    let Some(record) = raw.as_object() else {
        return Ok(());
    };
    let neutral_names = terminal_convention_names(terminal_conventions, "neutral");
    let earth_names = terminal_convention_names(terminal_conventions, "earth");
    let phase_count = terminal_conventions
        .and_then(Value::as_object)
        .and_then(|o| o.get("phase"))
        .and_then(Value::as_array)
        .map(|phases| phases.len().max(1));
    let mut non_phase = neutral_names;
    non_phase.extend(earth_names);
    if non_phase.is_empty() {
        return Ok(());
    }
    if subtype == "n_winding" {
        let Some(windings) = record.get("windings").and_then(Value::as_array) else {
            return Ok(());
        };
        for (idx, winding) in windings.iter().enumerate() {
            let Some(winding) = winding.as_object() else {
                continue;
            };
            let configuration = winding
                .get("configuration")
                .or_else(|| winding.get("connection"))
                .and_then(Value::as_str)
                .unwrap_or("WYE");
            if configuration.eq_ignore_ascii_case("WYE") {
                validate_wye_terminal_order(
                    "transformer",
                    name,
                    &format!("windings[{idx}].terminal_map"),
                    winding.get("terminal_map"),
                    &non_phase,
                    phase_count,
                )?;
            }
        }
        return Ok(());
    }
    let from_is_wye = !matches!(subtype, "delta_wye");
    let to_is_wye = !matches!(subtype, "wye_delta");
    if from_is_wye {
        validate_wye_terminal_order(
            "transformer",
            name,
            "terminal_map_from",
            record.get("terminal_map_from"),
            &non_phase,
            Some(if matches!(subtype, "wye_delta" | "delta_wye") {
                3
            } else {
                1
            }),
        )?;
    }
    if to_is_wye {
        validate_wye_terminal_order(
            "transformer",
            name,
            "terminal_map_to",
            record.get("terminal_map_to"),
            &non_phase,
            Some(if matches!(subtype, "wye_delta" | "delta_wye") {
                3
            } else {
                1
            }),
        )?;
    }
    Ok(())
}

fn terminal_convention_names(conventions: Option<&Value>, role: &str) -> Vec<String> {
    conventions
        .and_then(Value::as_object)
        .and_then(|o| o.get(role))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

fn validate_wye_terminal_order(
    kind: &str,
    name: &str,
    field: &str,
    value: Option<&Value>,
    non_phase: &[String],
    expected_phases: Option<usize>,
) -> Result<(), String> {
    let Some(map) = value.and_then(Value::as_array) else {
        return Ok(());
    };
    let names: Vec<&str> = map.iter().filter_map(Value::as_str).collect();
    if names.len() != map.len() {
        return Ok(());
    }
    let is_non_phase = |terminal: &str| non_phase.iter().any(|n| n == terminal);
    let Some(neutral_position) = names.iter().position(|terminal| is_non_phase(terminal)) else {
        // An implicit ground has no named neutral in a phase-only map. If the
        // map has one extra conductor, however, it is an explicit return and
        // must be named by the declared conventions before the reader can
        // preserve it safely.
        if expected_phases.is_some_and(|phases| names.len() == phases + 1) {
            return Err(format!(
                "{kind} `{name}` field `{field}` has an explicit WYE return that is not identified as the final neutral/earth terminal"
            ));
        }
        return Ok(());
    };
    if let Some(phases) = expected_phases {
        if names.len() != phases + 1 {
            return Err(format!(
                "{kind} `{name}` field `{field}` has an explicit WYE neutral but {} terminals; expected {phases} phase terminals followed by one neutral",
                names.len()
            ));
        }
    }
    if neutral_position != names.len() - 1
        || names
            .iter()
            .take(names.len() - 1)
            .any(|terminal| is_non_phase(terminal))
    {
        return Err(format!(
            "{kind} `{name}` field `{field}` places an explicit neutral/earth terminal among phase coils; PowerIO v0.11 requires WYE phase terminals first and the neutral last"
        ));
    }
    Ok(())
}

fn validate_transformer_record(subtype: &str, name: &str, raw: &Value) -> Result<(), String> {
    let record = raw
        .as_object()
        .ok_or_else(|| format!("transformer `{name}` must be an object"))?;
    for key in [
        "tap",
        "tap_ratio",
        "tap_min",
        "tap_max",
        "tap_ratio_min",
        "tap_ratio_max",
    ] {
        if let Some(value) = record.get(key) {
            require_uniform_scalar(value, &format!("transformer `{name}` field `{key}`"))?;
        }
    }
    for key in [
        "r_neutral_from",
        "x_neutral_from",
        "r_neutral_to",
        "x_neutral_to",
    ] {
        if let Some(value) = record.get(key) {
            require_uniform_scalar(value, &format!("transformer `{name}` field `{key}`"))?;
        }
    }
    if (record.contains_key("g_no_load") || record.contains_key("b_no_load"))
        && !record.contains_key("no_load_shunt")
    {
        return Err(format!(
            "transformer `{name}` uses legacy g_no_load/b_no_load without an explicit no_load_shunt; normalize the producer profile before MC PF"
        ));
    }
    if subtype == "n_winding" && (record.contains_key("tap") || record.contains_key("tap_ratio")) {
        return Err(format!(
            "transformer `{name}` n_winding tap is not represented by PowerIO v0.11"
        ));
    }
    if subtype != "n_winding" {
        return Ok(());
    }
    let Some(windings) = record.get("windings").and_then(Value::as_array) else {
        return Ok(());
    };
    if windings.len() != 2 {
        return Err(format!(
            "transformer `{name}` n_winding has {} windings; the MC primitive supports exactly two",
            windings.len()
        ));
    }
    const RETAINED: &[&str] = &[
        "bus",
        "terminal_map",
        "v_nom",
        "v_ref",
        "r_winding",
        "s_rating",
        "configuration",
        "connection",
        "delta_roll",
        "i_max",
        "tap_ratio_min",
        "tap_ratio_max",
    ];
    for (idx, winding) in windings.iter().enumerate() {
        let winding = winding
            .as_object()
            .ok_or_else(|| format!("transformer `{name}` winding {idx} must be an object"))?;
        for key in winding.keys() {
            if !RETAINED.contains(&key.as_str()) {
                return Err(format!(
                    "transformer `{name}` winding {idx} field `{key}` is not losslessly represented by PowerIO v0.11"
                ));
            }
        }
        if let Some(roll) = winding.get("delta_roll") {
            let Some(roll) = roll.as_i64() else {
                return Err(format!(
                    "transformer `{name}` winding {idx} delta_roll must be ±1"
                ));
            };
            if !matches!(roll, -1 | 1) {
                return Err(format!(
                    "transformer `{name}` winding {idx} delta_roll must be ±1"
                ));
            }
        }
        if let Some(configuration) = winding
            .get("configuration")
            .or_else(|| winding.get("connection"))
        {
            validate_configuration(
                configuration,
                &format!("transformer `{name}` winding {idx} configuration"),
            )?;
        }
    }
    Ok(())
}

fn require_uniform_scalar(value: &Value, field: &str) -> Result<(), String> {
    let Some(values) = value.as_array() else {
        if value.is_number() {
            return Ok(());
        }
        return Err(format!(
            "{field} must be a scalar or a uniform numeric array"
        ));
    };
    let numbers: Vec<f64> = values
        .iter()
        .map(|v| {
            v.as_f64()
                .ok_or_else(|| format!("{field} contains a nonnumeric tap"))
        })
        .collect::<Result<_, _>>()?;
    if numbers.is_empty() || numbers.iter().any(|v| !v.is_finite()) {
        return Err(format!("{field} contains no finite tap value"));
    }
    if numbers
        .iter()
        .any(|v| (*v - numbers[0]).abs() > 1e-12 * (1.0 + numbers[0].abs()))
    {
        return Err(format!(
            "{field} has per-phase values that PowerIO would collapse to the first phase"
        ));
    }
    Ok(())
}

fn validate_configuration(value: &Value, field: &str) -> Result<(), String> {
    let Some(configuration) = value.as_str() else {
        return Err(format!("{field} must be WYE, DELTA, or SINGLE_PHASE"));
    };
    if !matches!(
        configuration.to_ascii_uppercase().as_str(),
        "WYE" | "DELTA" | "SINGLE_PHASE"
    ) {
        return Err(format!(
            "{field} `{configuration}` is unsupported; PowerIO would default it to WYE"
        ));
    }
    Ok(())
}

/// Parse a raw BMOPF document and retain the typed instance's prescribed
/// load/source values exactly as PowerIO supplied them.
pub fn parse_bmopf_instance(text: &str) -> Result<McAcPfInstance, String> {
    validate_bmopf_json(text)?;
    let source = powerio::Source::from_memory("input.bmopf.json", text.as_bytes().to_vec())
        .map_err(|e| e.to_string())?;
    let module = powerio::parse_with_options(
        source,
        &powerio::ParseOptions::default()
            .format("bmopf-json")
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    reject_lossy_diagnostics(&module)?;
    instance_from_value(module.value())
}

fn reject_lossy_diagnostics(module: &powerio::PioModule<PioValue>) -> Result<(), String> {
    let lossy: Vec<String> = module
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            let code = diagnostic.code();
            code.contains("UNSUPPORTED")
                || code.contains("MALFORMED")
                || code.contains("FIELD_DROPPED")
                || code.contains("RECORD_DROPPED")
        })
        .map(|diagnostic| format!("{}: {}", diagnostic.code(), diagnostic.message()))
        .collect();
    if lossy.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "PowerIO module contains unsupported or lossy diagnostics: {}",
            lossy.join("; ")
        ))
    }
}

fn instance_from_value(value: &PioValue) -> Result<McAcPfInstance, String> {
    match value {
        PioValue::MulticonductorNetwork(network) => {
            McAcPfInstance::from_network(network.clone()).map_err(|e| e.to_string())
        }
        PioValue::McAcPfInstance(instance) => Ok(instance.clone()),
        // PowerIO's explicit projection preserves the network's prescribed
        // load/source values and returns a diagnostic recording that the OPF
        // objective and active constraints were discarded.  The raw solve
        // surface is PF, so accepting the projection here is intentional.
        PioValue::McAcOpfInstance(instance) => instance
            .to_mc_ac_pf()
            .map(|(pf, _discarded)| pf)
            .map_err(|e| e.to_string()),
        other => Err(format!(
            "BMOPF parsed as `{}`; expected a multiconductor network",
            other.type_name()
        )),
    }
}

/// Parse and solve a raw BMOPF document, returning the portable JSON result.
pub fn solve_bmopf_json(text: &str, options: &McPfOptions) -> Result<String, String> {
    let instance = parse_bmopf_instance(text)?;
    let result = solve_mc_ac_pf_instance(&instance, options)?;
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

/// Solve a stored PowerIO IR module carrying a multiconductor network or
/// instance. This is the portable module counterpart of the raw BMOPF entry
/// point; the module remains typed by PowerIO throughout.
pub fn solve_mc_module_json(text: &str, options: &McPfOptions) -> Result<String, String> {
    let module = crate::ir::deserialize_module(text)?;
    reject_lossy_diagnostics(&module)?;
    let instance = instance_from_value(module.value())?;
    let result = solve_mc_ac_pf_instance(&instance, options)?;
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rejects_unknown_load_model_before_powerio_fallback() {
        let raw = json!({"load": {"l": {"model": "mystery"}}});
        let error = validate_bmopf_json(&raw.to_string()).unwrap_err();
        assert!(error.contains("requests model"), "{error}");
    }

    #[test]
    fn rejects_unknown_configuration_before_wye_default() {
        let raw = json!({
            "load": {"l": {"model": "constant_power", "configuration": "NOT_A_CONNECTION"}}
        });
        let error = validate_bmopf_json(&raw.to_string()).unwrap_err();
        assert!(error.contains("default it to WYE"), "{error}");
    }

    #[test]
    fn rejects_nonobject_element_tables_instead_of_powerio_skipping_them() {
        let raw = json!({"load": [{"bus": "b", "p_nom": [2.0]}]});
        let error = validate_bmopf_json(&raw.to_string()).unwrap_err();
        assert!(error.contains("table `load` must be an object"), "{error}");
    }

    #[test]
    fn rejects_nonuniform_transformer_taps_before_first_phase_collapse() {
        let raw = json!({
            "transformer": {"wye_delta": {
                "t": {"tap_ratio": [1.0, 1.01], "bus_from": "a", "bus_to": "b"}
            }}
        });
        let error = validate_bmopf_json(&raw.to_string()).unwrap_err();
        assert!(error.contains("per-phase values"), "{error}");
    }

    #[test]
    fn accepts_two_winding_n_winding_delta_roll_for_the_primitive() {
        let raw = json!({
            "transformer": {"n_winding": {
                "t": {"windings": [{"delta_roll": 1}, {"delta_roll": -1}]}
            }}
        });
        validate_bmopf_json(&raw.to_string()).unwrap();
    }

    #[test]
    fn rejects_explicit_neutral_before_wye_phase_coils() {
        let raw = json!({
            "terminal_conventions": {"phase": ["a", "b", "c"], "neutral": ["n"], "earth": []},
            "transformer": {"delta_wye": {
                "t": {
                    "terminal_map_from": ["a", "b", "c"],
                    "terminal_map_to": ["n", "a", "b", "c"]
                }
            }}
        });
        let error = validate_bmopf_json(&raw.to_string()).unwrap_err();
        assert!(
            error.contains("neutral/earth terminal among phase coils"),
            "{error}"
        );
    }

    #[test]
    fn accepts_named_neutral_after_wye_phase_coils() {
        let raw = json!({
            "terminal_conventions": {"phase": ["a", "b", "c"], "neutral": ["n"], "earth": []},
            "transformer": {"delta_wye": {
                "t": {
                    "terminal_map_from": ["a", "b", "c"],
                    "terminal_map_to": ["a", "b", "c", "n"]
                }
            }}
        });
        validate_bmopf_json(&raw.to_string()).unwrap();
    }

    #[test]
    fn rejects_unnamed_explicit_wye_return_when_neutral_is_declared() {
        let raw = json!({
            "terminal_conventions": {"phase": ["a", "b", "c"], "neutral": ["n"], "earth": []},
            "transformer": {"delta_wye": {
                "t": {
                    "terminal_map_from": ["a", "b", "c"],
                    "terminal_map_to": ["a", "b", "c", "return"]
                }
            }}
        });
        let error = validate_bmopf_json(&raw.to_string()).unwrap_err();
        assert!(
            error.contains("not identified as the final neutral/earth"),
            "{error}"
        );
    }

    #[test]
    fn rejects_declared_neutral_in_the_middle_of_a_wye_load_map() {
        let raw = json!({
            "terminal_conventions": {"phase": ["a", "b", "c"], "neutral": ["n"], "earth": []},
            "load": {"l": {
                "model": "constant_power",
                "configuration": "WYE",
                "terminal_map": ["a", "n", "b", "c"],
                "p_nom": [1.0, 1.0, 1.0],
                "q_nom": [0.0, 0.0, 0.0]
            }}
        });
        let error = validate_bmopf_json(&raw.to_string()).unwrap_err();
        assert!(error.contains("load `l`"), "{error}");
        assert!(
            error.contains("neutral/earth terminal among phase coils"),
            "{error}"
        );
    }
}
