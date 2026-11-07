//! Calculation settings shared by native, WASM, CLI and HTTP callers.
//! Display preferences and network edits are deliberately separate contracts.
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

pub type Options = Map<String, Value>;

/// A calculation the common settings API resolves and runs. Each variant's
/// JSON name is the wire value every interface accepts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
pub enum Calculation {
    #[serde(rename = "mc_ac_pf")]
    McAcPf,
    #[serde(rename = "dcopf")]
    DcOpf,
    #[serde(rename = "acpf")]
    AcPf,
    #[serde(rename = "dcpf")]
    DcPf,
    #[serde(rename = "socwr")]
    Socwr,
    #[serde(rename = "acopf")]
    AcOpf,
}

impl Calculation {
    /// Every calculation, in the order [`describe`] lists them.
    const ALL: [Calculation; 6] = [
        Calculation::McAcPf,
        Calculation::DcOpf,
        Calculation::AcPf,
        Calculation::DcPf,
        Calculation::Socwr,
        Calculation::AcOpf,
    ];

    /// The wire name, e.g. `"mc_ac_pf"` or `"dcopf"`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Calculation::McAcPf => "mc_ac_pf",
            Calculation::DcOpf => "dcopf",
            Calculation::AcPf => "acpf",
            Calculation::DcPf => "dcpf",
            Calculation::Socwr => "socwr",
            Calculation::AcOpf => "acopf",
        }
    }

    /// The balanced formulation this calculation runs, if it is one.
    #[cfg(feature = "sensitivity")]
    pub(crate) fn problem(self) -> Option<crate::Problem> {
        match self {
            Calculation::McAcPf => None,
            Calculation::DcOpf => Some(crate::Problem::DcOpf),
            Calculation::AcPf => Some(crate::Problem::AcPf),
            Calculation::DcPf => Some(crate::Problem::DcPf),
            Calculation::Socwr => Some(crate::Problem::Socwr),
            Calculation::AcOpf => Some(crate::Problem::Acopf),
        }
    }
}

impl std::fmt::Display for Calculation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for Calculation {
    type Err = String;

    fn from_str(name: &str) -> Result<Self, String> {
        Calculation::ALL
            .into_iter()
            .find(|calculation| calculation.as_str() == name)
            .ok_or_else(|| format!("unknown calculation {name:?}"))
    }
}

impl From<crate::Problem> for Calculation {
    fn from(problem: crate::Problem) -> Self {
        match problem {
            crate::Problem::DcPf => Calculation::DcPf,
            crate::Problem::DcOpf => Calculation::DcOpf,
            crate::Problem::AcPf => Calculation::AcPf,
            crate::Problem::Socwr => Calculation::Socwr,
            crate::Problem::Acopf => Calculation::AcOpf,
        }
    }
}

/// Which calculation to configure, and the option layers over its defaults.
/// Build one with [`SettingsRequest::new`] and the `with_*` methods.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct SettingsRequest {
    pub calculation: Calculation,
    /// Case or study defaults, applied over the engine defaults.
    #[serde(default)]
    pub baseline: Options,
    /// The caller's explicit choices, applied last.
    #[serde(default)]
    pub overrides: Options,
}

impl SettingsRequest {
    /// Engine defaults for `calculation`, with no baseline or overrides.
    pub fn new(calculation: Calculation) -> Self {
        Self {
            calculation,
            baseline: Options::new(),
            overrides: Options::new(),
        }
    }

    /// Replace the case or study baseline.
    #[must_use]
    pub fn with_baseline(mut self, baseline: Options) -> Self {
        self.baseline = baseline;
        self
    }

    /// Replace the explicit overrides.
    #[must_use]
    pub fn with_overrides(mut self, overrides: Options) -> Self {
        self.overrides = overrides;
        self
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
pub struct ResolvedSettings {
    pub version: u32,
    pub calculation: Calculation,
    pub engine_defaults: Options,
    pub baseline: Options,
    pub overrides: Options,
    pub effective: Options,
}

/// A stateless calculation over one input module. Build one with
/// [`CalculationRequest::new`] and the `with_*` methods.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct CalculationRequest {
    pub module_json: String,
    pub settings: SettingsRequest,
    /// Source-less islands are excluded only by this explicit selection.
    #[serde(default)]
    pub energized_only: bool,
    #[serde(default)]
    pub edits: crate::Edits,
}

impl CalculationRequest {
    /// Calculate `module_json` with `settings`, over every island and with no edits.
    pub fn new(module_json: impl Into<String>, settings: SettingsRequest) -> Self {
        Self {
            module_json: module_json.into(),
            settings,
            energized_only: false,
            edits: crate::Edits::default(),
        }
    }

    /// Leave out islands with no source (multiconductor AC power flow only).
    #[must_use]
    pub fn with_energized_only(mut self, energized_only: bool) -> Self {
        self.energized_only = energized_only;
        self
    }

    /// Apply demand and rating edits before solving (balanced calculations only).
    #[must_use]
    pub fn with_edits(mut self, edits: crate::Edits) -> Self {
        self.edits = edits;
        self
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
pub struct CalculationResult {
    pub settings: ResolvedSettings,
    pub result: Value,
}

fn defaults(calculation: Calculation) -> Result<Options, String> {
    let value = match calculation {
        #[cfg(feature = "mc-pf")]
        Calculation::McAcPf => {
            serde_json::to_value(crate::McPfOptions::default()).map_err(|e| e.to_string())?
        }
        Calculation::DcOpf => {
            serde_json::to_value(crate::LimitOptions::default()).map_err(|e| e.to_string())?
        }
        Calculation::AcPf if cfg!(feature = "sensitivity") => json!({"start":"flat"}),
        Calculation::DcPf if cfg!(feature = "sensitivity") => json!({}),
        Calculation::Socwr if cfg!(feature = "conic") => json!({}),
        Calculation::AcOpf if cfg!(feature = "acopf") => json!({}),
        _ => {
            return Err(format!(
                "calculation {:?} is unavailable in this build",
                calculation.as_str()
            ))
        }
    };
    let mut options = value
        .as_object()
        .cloned()
        .ok_or("settings defaults must be an object")?;
    // A disabled optional algorithm has an explicit value in the common API.
    if calculation == Calculation::DcOpf {
        options.insert("lazy".into(), Value::Null);
    }
    Ok(options)
}

/// Resolve defaults, case/study baseline, then explicit overrides. Each layer
/// must itself be valid, so a later override cannot conceal corrupt saved data.
pub fn resolve(request: &SettingsRequest) -> Result<ResolvedSettings, String> {
    let engine_defaults = defaults(request.calculation)?;
    let merge = |base: &Options, layer: &Options| -> Result<Options, String> {
        let mut value = base.clone();
        for (key, item) in layer {
            if !engine_defaults.contains_key(key) {
                return Err(format!("unsupported setting {}.{key}", request.calculation));
            }
            value.insert(key.clone(), item.clone());
        }
        validate(request.calculation, &value)?;
        // Materialize nested defaults too, e.g. a partial lazy-enforcement object.
        if request.calculation == Calculation::DcOpf {
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
        calculation: request.calculation,
        engine_defaults,
        baseline,
        overrides: request.overrides.clone(),
        effective,
    })
}

fn validate(calculation: Calculation, options: &Options) -> Result<(), String> {
    let value = Value::Object(options.clone());
    match calculation {
        #[cfg(feature = "mc-pf")]
        Calculation::McAcPf => {
            let options: crate::McPfOptions =
                serde_json::from_value(value).map_err(|e| e.to_string())?;
            if options.max_iterations > u32::MAX as usize {
                return Err("max_iterations exceeds portable integer range".into());
            }
            crate::mc_pf::validate_options(&options)
        }
        Calculation::DcOpf => {
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
        Calculation::AcPf => {
            serde_json::from_value::<crate::PowerFlowStart>(options["start"].clone())
                .map(|_| ())
                .map_err(|e| e.to_string())
        }
        _ => Ok(()),
    }
}

/// JSON descriptors carry the same keys/defaults as the typed solver options.
/// Availability describes this binary, not another host's build features.
pub fn describe() -> Value {
    let calculations: Vec<Value> = Calculation::ALL.into_iter().map(|name| {
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
    let result = if settings.calculation == Calculation::McAcPf {
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
        if settings.calculation == Calculation::DcOpf {
            req["limits"] = Value::Object(settings.effective.clone());
        }
        if settings.calculation == Calculation::AcPf {
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
    fn calculation_wire_names_are_unchanged() {
        let names = ["mc_ac_pf", "dcopf", "acpf", "dcpf", "socwr", "acopf"];
        assert_eq!(Calculation::ALL.len(), names.len());
        for (calculation, name) in Calculation::ALL.into_iter().zip(names) {
            assert_eq!(calculation.as_str(), name);
            assert_eq!(calculation.to_string(), name);
            assert_eq!(name.parse::<Calculation>().unwrap(), calculation);
            assert_eq!(serde_json::to_value(calculation).unwrap(), json!(name));
            assert_eq!(
                serde_json::from_value::<Calculation>(json!(name)).unwrap(),
                calculation
            );
        }
        assert!("DCOPF".parse::<Calculation>().is_err());
        assert!(serde_json::from_value::<SettingsRequest>(json!({"calculation":"opf"})).is_err());
        // Requests still refuse fields they do not define.
        assert!(serde_json::from_value::<SettingsRequest>(
            json!({"calculation":"dcopf","overides":{}})
        )
        .is_err());
        assert!(serde_json::from_value::<CalculationRequest>(
            json!({"module_json":"{}","settings":{"calculation":"dcopf"},"extra":1})
        )
        .is_err());
    }

    #[test]
    fn builders_match_the_wire_form() {
        let options: Options = serde_json::from_value(json!({"thermal":"rated"})).unwrap();
        let built = CalculationRequest::new(
            "{}",
            SettingsRequest::new(Calculation::DcOpf)
                .with_baseline(options.clone())
                .with_overrides(options),
        )
        .with_energized_only(true)
        .with_edits(serde_json::from_value(json!({"deltas":{"2":1.5}})).unwrap());
        let parsed: CalculationRequest = serde_json::from_value(json!({
            "module_json": "{}",
            "settings": {"calculation":"dcopf","baseline":{"thermal":"rated"},"overrides":{"thermal":"rated"}},
            "energized_only": true,
            "edits": {"deltas":{"2":1.5}}
        }))
        .unwrap();
        assert_eq!(
            serde_json::to_value(&built).unwrap(),
            serde_json::to_value(&parsed).unwrap()
        );
        let plain = CalculationRequest::new("{}", SettingsRequest::new(Calculation::AcPf));
        assert!(!plain.energized_only);
        assert!(plain.settings.baseline.is_empty() && plain.settings.overrides.is_empty());
    }

    #[test]
    fn precedence_unknown_fields_and_cross_field_validation() {
        let mut request = SettingsRequest::new(Calculation::DcOpf)
            .with_baseline(serde_json::from_value(json!({"angle_difference":false})).unwrap())
            .with_overrides(serde_json::from_value(json!({"thermal":"rated"})).unwrap());
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
