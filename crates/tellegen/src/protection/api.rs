//! JSON entry points over a stored PowerIO module and its DYR text, for the
//! browser and the model-building tools. Each takes the case options last
//! (`load_model` and the RMS options, as the app prepares its case; empty:
//! constant-impedance loads and the defaults).

use serde::{Deserialize, Serialize};

use super::setting::{
    audit, auto_scheme, carry_ratings, estimate_ratings, AutoSetting, Finding, SettingOptions,
};
use super::testing::{standard_suite, ProtectionTest, SuiteOptions, SuiteReport};
use super::{Direction, ProtectionFunction, ProtectionScheme, RelayLocation, Topology};
use crate::rms::{prepare_module_json, LoadModel, RmsOptions, RmsSimulation};

/// How the case is prepared: the load model and the RMS options, as the
/// app prepares its RMS case (`load_model` and the [`RmsOptions`] fields).
/// Empty: constant-impedance loads and the default options.
#[derive(Clone, Debug, Deserialize)]
struct CaseOptions {
    #[serde(default = "constant_impedance")]
    load_model: LoadModel,
    #[serde(flatten)]
    options: RmsOptions,
}

fn constant_impedance() -> LoadModel {
    LoadModel::ConstantImpedance
}

fn simulation(module_json: &str, dyr_text: &str, case_json: &str) -> Result<RmsSimulation, String> {
    let (load_model, options) = if case_json.trim().is_empty() {
        (LoadModel::ConstantImpedance, RmsOptions::default())
    } else {
        let c: CaseOptions =
            serde_json::from_str(case_json).map_err(|e| format!("case options: {e}"))?;
        (c.load_model, c.options)
    };
    prepare_module_json(module_json, dyr_text, load_model, options)
}

fn parse<T: for<'de> Deserialize<'de> + Default>(json: &str, what: &str) -> Result<T, String> {
    if json.trim().is_empty() {
        Ok(T::default())
    } else {
        serde_json::from_str(json).map_err(|e| format!("{what}: {e}"))
    }
}

fn out<T: Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_string(value).map_err(|e| e.to_string())
}

/// A scheme by the setting rules, its audit and the zone spans to draw.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AutoResult {
    #[serde(flatten)]
    pub setting: AutoSetting,
    pub audit: Vec<Finding>,
    pub zones: Vec<ZoneSpan>,
}

/// # Errors
/// When the inputs do not parse or the case does not initialize.
pub fn auto_module_json(
    module_json: &str,
    dyr_text: &str,
    options_json: &str,
    case_json: &str,
) -> Result<String, String> {
    let sim = simulation(module_json, dyr_text, case_json)?;
    let opts: SettingOptions = parse(options_json, "options")?;
    // Branches the case leaves unrated are set on an estimated rating,
    // which their relays then carry.
    let mut topo = sim.topology().clone();
    let estimated = estimate_ratings(&mut topo, &sim.branch_loading());
    let mut setting = auto_scheme(&topo, &opts);
    if !estimated.is_empty() {
        carry_ratings(&mut setting.scheme, &topo, &estimated);
        setting.notes.insert(
            0,
            format!(
                "{} of {} branches carry no rating: set on the typical thermal rating of their voltage level, at least 125 % of the flow at the operating point (the rated current of their relays)",
                estimated.len(),
                topo.branches.len()
            ),
        );
    }
    let audit = audit(&topo, &setting.scheme);
    let zones = zone_spans(&topo, &setting.scheme);
    out(&AutoResult {
        setting,
        audit,
        zones,
    })
}

/// The audit and zone spans of a given scheme.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuditResult {
    pub audit: Vec<Finding>,
    pub zones: Vec<ZoneSpan>,
}

/// # Errors
/// When the inputs do not parse or the case does not initialize.
pub fn audit_module_json(
    module_json: &str,
    dyr_text: &str,
    scheme_json: &str,
    case_json: &str,
) -> Result<String, String> {
    let sim = simulation(module_json, dyr_text, case_json)?;
    let scheme: ProtectionScheme = parse(scheme_json, "scheme")?;
    out(&AuditResult {
        audit: audit(sim.topology(), &scheme),
        zones: zone_spans(sim.topology(), &scheme),
    })
}

/// The relays at the load flow operating point.
///
/// # Errors
/// When the inputs do not parse or the case does not initialize.
pub fn operating_module_json(
    module_json: &str,
    dyr_text: &str,
    scheme_json: &str,
    case_json: &str,
) -> Result<String, String> {
    let sim = simulation(module_json, dyr_text, case_json)?;
    let scheme: ProtectionScheme = parse(scheme_json, "scheme")?;
    out(&sim.relays_at_operating_point(&scheme)?)
}

/// A static protection study of one fault (`StaticFault` JSON).
///
/// # Errors
/// When the inputs do not parse, the case does not initialize or the
/// fault names a missing element.
pub fn static_module_json(
    module_json: &str,
    dyr_text: &str,
    scheme_json: &str,
    fault_json: &str,
    t_max: f64,
    case_json: &str,
) -> Result<String, String> {
    let sim = simulation(module_json, dyr_text, case_json)?;
    let scheme: ProtectionScheme = parse(scheme_json, "scheme")?;
    let fault: super::statics::StaticFault =
        serde_json::from_str(fault_json).map_err(|e| format!("fault: {e}"))?;
    out(&super::statics::static_study(
        sim.topology(),
        &scheme,
        &fault,
        if t_max > 0.0 { t_max } else { 3.0 },
    )?)
}

/// The standard tests of a scheme (not run).
///
/// # Errors
/// When the inputs do not parse or the case does not initialize.
pub fn tests_module_json(
    module_json: &str,
    dyr_text: &str,
    scheme_json: &str,
    options_json: &str,
    case_json: &str,
) -> Result<String, String> {
    let sim = simulation(module_json, dyr_text, case_json)?;
    let scheme: ProtectionScheme = parse(scheme_json, "scheme")?;
    let opts: SuiteOptions = parse(options_json, "options")?;
    out(&standard_suite(sim.topology(), &scheme, &opts))
}

/// Run tests (all standard tests when `tests_json` is empty) in RMS.
///
/// # Errors
/// When the inputs do not parse or the case does not initialize.
pub fn suite_module_json(
    module_json: &str,
    dyr_text: &str,
    scheme_json: &str,
    tests_json: &str,
    options_json: &str,
    case_json: &str,
) -> Result<String, String> {
    let sim = simulation(module_json, dyr_text, case_json)?;
    let scheme: ProtectionScheme = parse(scheme_json, "scheme")?;
    let opts: SuiteOptions = parse(options_json, "options")?;
    let tests: Vec<ProtectionTest> = if tests_json.trim().is_empty() {
        standard_suite(sim.topology(), &scheme, &opts)
    } else {
        serde_json::from_str(tests_json).map_err(|e| format!("tests: {e}"))?
    };
    let outcomes = super::testing::run_tests(&sim, &scheme, &tests);
    let passed = outcomes.iter().filter(|o| o.passed).count();
    out(&SuiteReport {
        audit: audit(sim.topology(), &scheme),
        failed: outcomes.len() - passed,
        passed,
        tests: outcomes,
    })
}

/// A stretch of a branch inside a distance zone, for drawing reaches:
/// `start`/`end` as fractions from the branch's from bus.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ZoneSpan {
    pub relay: String,
    pub zone: String,
    pub from: usize,
    pub to: usize,
    pub circuit: String,
    pub start: f64,
    pub end: f64,
    pub delay: f64,
}

/// The stretches each forward distance zone reaches, walking the line
/// impedance from the relay (no infeed), two branches deep.
#[must_use]
pub fn zone_spans(topo: &Topology, scheme: &ProtectionScheme) -> Vec<ZoneSpan> {
    let mut spans = Vec::new();
    for r in scheme.relays.iter().filter(|r| r.enabled) {
        let RelayLocation::Line {
            from,
            to,
            circuit,
            at,
        } = &r.location
        else {
            continue;
        };
        let Ok(e) = topo.branch(*from, *to, circuit) else {
            continue;
        };
        let Ok(here) = topo.bus(*at) else { continue };
        let zb = topo.z_base(here);
        for f in &r.functions {
            let ProtectionFunction::Distance(d) = f else {
                continue;
            };
            for z in d.zones.iter().filter(|z| z.direction == Direction::Forward) {
                let reach = z.reach / zb;
                walk(topo, e, here, reach, 2, &mut |k, a, b| {
                    let br = &topo.branches[k];
                    spans.push(ZoneSpan {
                        relay: r.id.clone(),
                        zone: z.name.clone(),
                        from: br.from_number,
                        to: br.to_number,
                        circuit: br.circuit.clone(),
                        start: a,
                        end: b,
                        delay: z.delay,
                    });
                });
            }
        }
    }
    spans
}

/// Cover `reach` (pu impedance) from bus `entry` along branch `e`, then on
/// into the branches beyond.
fn walk(
    topo: &Topology,
    e: usize,
    entry: usize,
    reach: f64,
    depth: usize,
    emit: &mut dyn FnMut(usize, f64, f64),
) {
    let br = &topo.branches[e];
    let z = br.z.norm().max(1e-9);
    let share = (reach / z).min(1.0);
    let forward = br.from == entry;
    if forward {
        emit(e, 0.0, share);
    } else {
        emit(e, 1.0 - share, 1.0);
    }
    let rest = reach - z;
    if rest <= 0.0 || depth == 0 {
        return;
    }
    let far = if forward { br.to } else { br.from };
    for (k, b) in topo.branches.iter().enumerate() {
        if k != e && (b.from == far || b.to == far) {
            walk(topo, k, far, rest, depth - 1, emit);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_json_round_trip_on_wecc240() {
        // WECC 240 ships with the code (tests/data/rms_examples/README.md).
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data/rms_examples/wecc240");
        let module = powerio::parse(powerio::Source::open(dir.join("case.raw")).unwrap()).unwrap();
        let module_json = crate::ir::serialize_module(&module).unwrap();
        let dyr = std::fs::read_to_string(dir.join("case.dyr")).unwrap();
        let auto: AutoResult =
            serde_json::from_str(&auto_module_json(&module_json, &dyr, "", "").unwrap()).unwrap();
        assert!(auto.setting.scheme.relays.len() > 40);
        assert!(!auto.zones.is_empty());
        let scheme = serde_json::to_string(&auto.setting.scheme).unwrap();
        let tests: Vec<ProtectionTest> =
            serde_json::from_str(&tests_module_json(&module_json, &dyr, &scheme, "", "").unwrap())
                .unwrap();
        let first = serde_json::to_string(&tests[..2]).unwrap();
        let report: SuiteReport = serde_json::from_str(
            &suite_module_json(&module_json, &dyr, &scheme, &first, "", "").unwrap(),
        )
        .unwrap();
        assert_eq!(report.passed, 2);
    }
}
