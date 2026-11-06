//! Calculation settings shared by native, WASM, CLI and HTTP callers.
//! Display preferences and network edits are deliberately separate contracts.
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

pub type Options = Map<String, Value>;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SettingsRequest {
    pub calculation: String,
    #[serde(default)]
    pub baseline: Options,
    #[serde(default)]
    pub overrides: Options,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ResolvedSettings {
    pub version: u32,
    pub calculation: String,
    pub engine_defaults: Options,
    pub baseline: Options,
    pub overrides: Options,
    pub effective: Options,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CalculationRequest {
    pub module_json: String,
    pub settings: SettingsRequest,
    /// Source-less islands are excluded only by this explicit selection.
    #[serde(default)]
    pub energized_only: bool,
    #[serde(default)]
    pub edits: crate::Edits,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct CalculationResult {
    pub settings: ResolvedSettings,
    pub result: Value,
}

fn defaults(calculation: &str) -> Result<Options, String> {
    let value = match calculation {
        #[cfg(feature = "mc-pf")]
        "mc_ac_pf" => {
            serde_json::to_value(crate::McPfOptions::default()).map_err(|e| e.to_string())?
        }
        "dcopf" => {
            serde_json::to_value(crate::LimitOptions::default()).map_err(|e| e.to_string())?
        }
        "acpf" if cfg!(feature = "sensitivity") => json!({"start":"flat"}),
        "dcpf" if cfg!(feature = "sensitivity") => json!({}),
        "socwr" if cfg!(feature = "conic") => json!({}),
        "acopf" if cfg!(feature = "acopf") => json!({}),
        _ => {
            return Err(format!(
                "calculation {calculation:?} is unavailable in this build"
            ))
        }
    };
    let mut options = value
        .as_object()
        .cloned()
        .ok_or("settings defaults must be an object")?;
    // A disabled optional algorithm has an explicit value in the common API.
    if calculation == "dcopf" {
        options.insert("lazy".into(), Value::Null);
    }
    Ok(options)
}

/// Resolve defaults, case/study baseline, then explicit overrides. Each layer
/// must itself be valid, so a later override cannot conceal corrupt saved data.
pub fn resolve(request: &SettingsRequest) -> Result<ResolvedSettings, String> {
    let engine_defaults = defaults(&request.calculation)?;
    let merge = |base: &Options, layer: &Options| -> Result<Options, String> {
        let mut value = base.clone();
        for (key, item) in layer {
            if !engine_defaults.contains_key(key) {
                return Err(format!("unsupported setting {}.{key}", request.calculation));
            }
            value.insert(key.clone(), item.clone());
        }
        validate(&request.calculation, &value)?;
        // Materialize nested defaults too, e.g. a partial lazy-enforcement object.
        if request.calculation == "dcopf" {
            let typed: crate::LimitOptions =
                serde_json::from_value(Value::Object(value)).map_err(|e| e.to_string())?;
            let mut normalized = serde_json::to_value(typed)
                .map_err(|e| e.to_string())?
                .as_object()
                .cloned()
                .ok_or("invalid limit options")?;
            normalized.entry("lazy").or_insert(Value::Null);
            Ok(normalized)
        } else {
            Ok(value)
        }
    };
    let baseline = merge(&engine_defaults, &request.baseline)?;
    let effective = merge(&baseline, &request.overrides)?;
    Ok(ResolvedSettings {
        version: 1,
        calculation: request.calculation.clone(),
        engine_defaults,
        baseline,
        overrides: request.overrides.clone(),
        effective,
    })
}

fn validate(calculation: &str, options: &Options) -> Result<(), String> {
    let value = Value::Object(options.clone());
    match calculation {
        #[cfg(feature = "mc-pf")]
        "mc_ac_pf" => {
            let options: crate::McPfOptions =
                serde_json::from_value(value).map_err(|e| e.to_string())?;
            if options.max_iterations > u32::MAX as usize {
                return Err("max_iterations exceeds portable integer range".into());
            }
            crate::mc_pf::validate_options(&options)
        }
        "dcopf" => {
            let options: crate::LimitOptions =
                serde_json::from_value(value).map_err(|e| e.to_string())?;
            if let Some(lazy) = options.lazy {
                if lazy.max_rounds == 0
                    || lazy.max_rounds > u32::MAX as usize
                    || !lazy.near_binding.is_finite()
                    || lazy.near_binding <= 0.0
                    || lazy.near_binding > 1.0
                {
                    return Err(
                        "lazy limits require max_rounds >= 1 and 0 < near_binding <= 1".into(),
                    );
                }
            }
            Ok(())
        }
        "acpf" => serde_json::from_value::<crate::PowerFlowStart>(options["start"].clone())
            .map(|_| ())
            .map_err(|e| e.to_string()),
        _ => Ok(()),
    }
}

/// JSON descriptors carry the same keys/defaults as the typed solver options.
/// Availability describes this binary, not another host's build features.
pub fn describe() -> Value {
    let calculations: Vec<Value> = ["mc_ac_pf", "dcopf", "acpf", "dcpf", "socwr", "acopf"].into_iter().map(|name| {
        match defaults(name) {
            Ok(options) => {
                let fields: Vec<Value> = options.iter().map(|(key, default)| {
                    let (label, unit, description, group) = field(key);
                    let kind = if default.is_boolean() { "boolean" } else if default.is_number() { "number" } else if default.is_string() { "enum" } else { "json" };
                    let mut value = json!({"key":key,"label":label,"unit":unit,"description":description,"group":group,"type":kind,"default":default});
                    if key == "start" { value["choices"] = json!(["flat","case"]); }
                    if key == "thermal" { value["type"] = json!("json"); }
                    if key == "max_iterations" { value["type"] = json!("integer"); value["minimum"] = json!(1); value["maximum"] = json!(u32::MAX); }
                    if matches!(key.as_str(), "tolerance" | "zero_voltage_tolerance" | "absolute_kcl_tolerance" | "v_low_pu" | "v_min_pu" | "v_max_pu" | "damping") { value["exclusive_minimum"] = json!(0); }
                    if key == "relative_kcl_tolerance" { value["minimum"] = json!(0); }
                    if key == "damping" { value["maximum"] = json!(1); }
                    value
                }).collect();
                json!({"calculation":name,"available":true,"defaults":options,"fields":fields})
            }
            Err(reason) => json!({"calculation":name,"available":false,"reason":reason,"fields":[]}),
        }
    }).collect();
    json!({"version":1,"calculations":calculations})
}

fn field(key: &str) -> (&str, &str, &str, &str) {
    match key {
        "tolerance" => ("Voltage-change tolerance", "V", "Positive maximum complex-voltage change; KCL must also pass.", "numerical"),
        "max_iterations" => ("Maximum iterations", "", "Positive integer fixed-point iteration limit.", "numerical"),
        "damping" => ("Damping", "", "Voltage-update factor in (0, 1].", "numerical"),
        "zero_voltage_tolerance" => ("Zero-voltage guard", "V", "Positive guard for load-current evaluation, not a convergence tolerance.", "numerical"),
        "absolute_kcl_tolerance" => ("Absolute KCL tolerance", "A", "Positive absolute current-mismatch floor.", "numerical"),
        "relative_kcl_tolerance" => ("Relative KCL tolerance", "", "Nonnegative allowance scaled by incident/load-current magnitude.", "numerical"),
        "voltage_envelope" => ("Use load-model envelope", "", "Changes load equations outside the normal band; results retain model qualification.", "model"),
        "v_low_pu" => ("Vlow", "pu", "Nominal-impedance breakpoint; 0 < Vlow < Vmin < Vmax, even with envelope disabled.", "model"),
        "v_min_pu" => ("Vmin", "pu", "Lower edge of original load-law band; not an engineering voltage limit.", "model"),
        "v_max_pu" => ("Vmax", "pu", "Upper edge of original load-law band; not an engineering voltage limit.", "model"),
        "angle_difference" => ("Enforce angle-difference limits", "", "Changes the constraints included in DC OPF.", "model"),
        "thermal" => ("Thermal limits", "", "all, rated, or an object with branches containing element keys.", "model"),
        "lazy" => ("Lazy thermal enforcement", "", "null disables; otherwise {max_rounds:20, near_binding:0.98}. Solves verify omitted limits.", "numerical"),
        "start" => ("Initial voltages", "", "flat or case; a stored case start that fails falls back to flat.", "numerical"),
        _ => (key, "", "", "numerical"),
    }
}

/// Stateless calculation. Settings are never written into shared hosted cases.
pub fn calculate(
    request: &CalculationRequest,
    cancel: Option<Arc<AtomicBool>>,
) -> Result<CalculationResult, String> {
    let check = || {
        if cancel.as_ref().is_some_and(|c| c.load(Ordering::Relaxed)) {
            Err("calculation cancelled".to_owned())
        } else {
            Ok(())
        }
    };
    check()?;
    let settings = resolve(&request.settings)?;
    let result = if settings.calculation == "mc_ac_pf" {
        #[cfg(feature = "mc-pf")]
        {
            if !request.edits.deltas.is_empty() || !request.edits.rates.is_empty() {
                return Err("multiconductor edits belong in the input module".into());
            }
            crate::validate_mc_module_json(&request.module_json)?;
            let input = if request.energized_only {
                {
                    let module = crate::ir::deserialize_module(&request.module_json)?;
                    let (selected, _) = crate::prepare_mc_energized_network(&module)?;
                    crate::ir::serialize_module(&selected)?
                }
            } else {
                request.module_json.clone()
            };
            let options = serde_json::from_value(Value::Object(settings.effective.clone()))
                .map_err(|e| e.to_string())?;
            crate::mc_pf::input::solve_mc_module_json_cancellable(&input, &options, cancel.clone())?
        }
        #[cfg(not(feature = "mc-pf"))]
        {
            return Err("mc_ac_pf is unavailable in this build".into());
        }
    } else {
        if request.energized_only {
            return Err("energized_only is supported only by mc_ac_pf".into());
        }
        let mut req = json!({"formulation":settings.calculation,"edits":request.edits});
        if settings.calculation == "dcopf" {
            req["limits"] = Value::Object(settings.effective.clone());
        }
        if settings.calculation == "acpf" {
            req["start"] = settings.effective["start"].clone();
        }
        crate::api::solve_module_json_cancellable(
            &request.module_json,
            &req.to_string(),
            cancel.clone(),
        )?
    };
    check()?;
    Ok(CalculationResult {
        settings,
        result: serde_json::from_str(&result).map_err(|e| e.to_string())?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn precedence_unknown_fields_and_cross_field_validation() {
        let mut request = SettingsRequest {
            calculation: "dcopf".into(),
            baseline: serde_json::from_value(json!({"angle_difference":false})).unwrap(),
            overrides: serde_json::from_value(json!({"thermal":"rated"})).unwrap(),
        };
        let resolved = resolve(&request).unwrap();
        assert_eq!(resolved.effective["angle_difference"], false);
        assert_eq!(resolved.effective["thermal"], "rated");
        assert_eq!(resolved.engine_defaults["thermal"], "all");
        request.overrides.insert("typo".into(), json!(1));
        assert!(resolve(&request)
            .unwrap_err()
            .contains("unsupported setting"));
        request.overrides = serde_json::from_value(json!({"lazy":{"near_binding":2}})).unwrap();
        assert!(resolve(&request).is_err());
    }
    #[cfg(feature = "mc-pf")]
    #[test]
    fn mc_uses_typed_defaults_and_rejects_invalid_baselines() {
        let request: SettingsRequest =
            serde_json::from_value(json!({"calculation":"mc_ac_pf"})).unwrap();
        assert_eq!(
            Value::Object(resolve(&request).unwrap().effective),
            serde_json::to_value(crate::McPfOptions::default()).unwrap()
        );
        let request: SettingsRequest = serde_json::from_value(json!({"calculation":"mc_ac_pf","baseline":{"v_min_pu":0.1},"overrides":{"v_min_pu":0.85}})).unwrap();
        assert!(resolve(&request).is_err());
    }
    #[cfg(feature = "mc-pf")]
    #[test]
    fn calculation_defaults_match_mc_solver_and_cancellation_is_explicit() {
        let module = include_str!(
            "../../../apps/web/tests/fixtures/mc-pf-browser/public/mc-pf-three-phase.pio.json"
        );
        let request:CalculationRequest=serde_json::from_value(json!({"module_json":module,"settings":{"calculation":"mc_ac_pf","overrides":{"damping":0.8}}})).unwrap();
        let actual = calculate(&request, None).unwrap();
        let options: crate::McPfOptions =
            serde_json::from_value(Value::Object(actual.settings.effective.clone())).unwrap();
        let expected: Value =
            serde_json::from_str(&crate::solve_mc_module_json(module, &options).unwrap()).unwrap();
        assert_eq!(actual.result, expected);
        assert!(calculate(&request, Some(Arc::new(AtomicBool::new(true))))
            .unwrap_err()
            .contains("cancelled"));
    }

    #[cfg(feature = "sensitivity")]
    #[test]
    fn retained_studies_keep_options_on_edits_and_save_actual_constraints() {
        let network = crate::model::parse_matpower(crate::model::CASE3).unwrap();
        let module = crate::ir::serialize_module(&powerio::PioModule::new(
            powerio::PioValue::BalancedNetwork(network),
        ))
        .unwrap();
        let settings:SettingsRequest=serde_json::from_value(json!({"calculation":"dcopf","overrides":{"angle_difference":false,"thermal":"rated","lazy":{"near_binding":0.98}}})).unwrap();
        let resolved = resolve(&settings).unwrap();
        assert_eq!(resolved.effective["lazy"]["max_rounds"], 20);
        let mut study = crate::Study::new_with_settings(&module, &settings).unwrap();
        study
            .commit(&[crate::NetworkEdit::AddLoad {
                bus: crate::ElementKey::Id(2),
                p_mw: 1.0,
            }])
            .unwrap();
        assert_eq!(study.calculation_options(), &resolved.effective);
        let saved = study.save_instance_module().unwrap();
        let typed = crate::ir::deserialize_module(&saved).unwrap();
        let powerio::PioValue::DcOpfInstance(instance) = typed.value() else {
            panic!("expected DC instance")
        };
        assert!(!instance.constraints().angle_bounds.selects("branches:0"));
        let restored = crate::Study::new(&saved, crate::Problem::DcOpf).unwrap();
        assert_eq!(restored.calculation_options(), &resolved.effective);
        assert!(
            (restored.solution().objective.unwrap() - study.solution().objective.unwrap()).abs()
                < 1e-5
        );
        study.save_solution_module().unwrap();
    }
}
