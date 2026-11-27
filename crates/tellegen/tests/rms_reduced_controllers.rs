//! Independent references for controller models that PowerSimulationsDynamics.jl
//! does not implement: ESAC6A, the OELIT and UELPQ limiters at an exciter's
//! summing point, the AVR load compensator (with and without its lag), the
//! summing-point under-excitation limiter and the GAST fuel valve rate limit.
//!
//! Each model was written once more, from IEEE Std 421.5-2005 and the PSS/E
//! model descriptions or, for tellegen's own models, from their documented
//! equations, as a PowerSimulationsDynamics.jl 0.16.2 extension (a new
//! exciter or governor type on its unmodified machine, shaft and network),
//! and run closed loop on the single machine of WECC 240's GENROU + SEXS +
//! TGOV1 unit (GENROU + SEXS + GAST for the rate limit) through line trips,
//! faults and reference steps that drive the controllers through their limits
//! (`validation/rms-models/reduced-controllers`). The reference is
//! `tests/data/rms_reduced_controllers/closed_loop.json`, see the
//! `SOURCES.md` next to it; the test needs no Julia.
//!
//! Every channel stays within its bound of `tolerances.json`: three times
//! tellegen's difference at its 1 ms step, at least 1e-6. Besides, every run
//! meets fixed physical tolerances (speed, rotor angle, terminal voltage,
//! reactive power, field voltage). Set `RMS_REDUCED_REPORT=1` to print every
//! comparison.
//!
//! The ESAC6A and limiter runs need `rms-experimental` (the extension
//! records); the others use process-wide model switches (`set_vcomp`,
//! `set_uel`, `set_gast_rate`), so every test here holds one lock and turns
//! every switch off when it starts and when it ends, also when it fails.

#![cfg(feature = "rms")]
// Negated comparisons are deliberate: they treat NaN as failing.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use serde_json::Value;
use tellegen::rms::{
    self, InputSignal, LoadModel, LowVoltageLoad, RmsEvent, RmsOptions, RmsRequest, RmsSimulation,
};

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/support/psid.rs"
));
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/support/smib.rs"
));

// ---------------------------------------------------------------------------
// The model switches: one test at a time, every switch off around it.

static SWITCHES: Mutex<()> = Mutex::new(());

/// Holds the lock on the switches; every switch is off while it lives
/// (turned off on creation and again on drop, also when a test panics).
struct Switches {
    _lock: MutexGuard<'static, ()>,
}

impl Drop for Switches {
    fn drop(&mut self) {
        all_off();
    }
}

/// Takes the lock on the switches (also after a test that failed holding it).
fn switches() -> Switches {
    let lock = SWITCHES.lock().unwrap_or_else(PoisonError::into_inner);
    all_off();
    Switches { _lock: lock }
}

/// Every switch to its off value.
fn all_off() {
    rms::set_load_recovery(0.0, 0.0);
    rms::set_load_saturation(0.0, 0.0);
    rms::set_load_rise(0.0);
    rms::set_uel(HashMap::new());
    rms::set_vcomp(HashMap::new());
    rms::set_plant_q_exponent(HashMap::new());
    rms::set_gast_rate(0.0);
    rms::set_hvdc_emulation(Vec::new());
}

// ---------------------------------------------------------------------------
// Physical tolerances, met by every run whatever its regression bound.

/// Speed, pu: 1e-4 pu is 6 mHz at 60 Hz, below the governor deadbands of
/// interconnection practice (16.6 to 36 mHz).
const OMEGA_TOL: f64 = 1e-4;
/// Rotor angle, rad: 0.5°, small against what a transient stability assessment
/// judges (first swings of tens of degrees).
const DELTA_TOL: f64 = 0.5 * std::f64::consts::PI / 180.0;
/// Terminal voltage, pu: 0.1 %, below the accuracy of the voltage
/// transducers a regulator measures with (0.2 to 0.5 %).
const VM_TOL: f64 = 1e-3;
/// Reactive power, pu on 100 MVA: 1 Mvar, a fraction of a percent of the
/// unit's (873 MVA) excursions here.
const QE_TOL: f64 = 1e-2;
/// Field voltage, pu: 0.5 % of its value at rest (1.96).
const EFD_TOL: f64 = 1e-2;

fn physical_tolerance(key: &str) -> Option<f64> {
    match key.split(':').next().unwrap() {
        "omega" => Some(OMEGA_TOL),
        "delta" => Some(DELTA_TOL),
        "vm" => Some(VM_TOL),
        "qe" => Some(QE_TOL),
        "efd" => Some(EFD_TOL),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// The reference and the cases.

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/rms_reduced_controllers")
}

fn read_json(file: &str) -> Value {
    let path = data_dir().join(file);
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap())
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn series(v: &Value) -> Vec<f64> {
    serde_json::from_value(v.clone()).unwrap()
}

fn report() -> bool {
    std::env::var_os("RMS_REDUCED_REPORT").is_some()
}

/// The dyr text of a run: the case's records with its exciter replaced by
/// the run's extension records (ESAC6A, OELIT, UELPQ), if any.
fn dyr_text(case: &str, records: &serde_json::Map<String, Value>) -> String {
    let base = std::fs::read_to_string(data_dir().join(format!("{case}.dyr"))).unwrap();
    let mut out = String::new();
    for rec in base.split('/').map(str::trim).filter(|r| !r.is_empty()) {
        let model = rec.split_whitespace().nth(1).unwrap().trim_matches('\'');
        if model == "SEXS" && records.contains_key("ESAC6A") {
            continue;
        }
        out.push_str(rec);
        out.push_str(" /\n");
    }
    for (model, params) in records {
        let params: Vec<String> = series(params).iter().map(f64::to_string).collect();
        out.push_str(&format!("102 '{model}' 1 {} /\n", params.join(" ")));
    }
    out
}

/// The events of a run as tellegen's.
fn events(run: &Value) -> Vec<RmsEvent> {
    run["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            let time = e["time"].as_f64().unwrap();
            let value = e["value"].as_f64().unwrap();
            match e["kind"].as_str().unwrap() {
                // One of the two lines to the infinite bus opens.
                "trip" => RmsEvent::BranchTrip {
                    time,
                    from: 101,
                    to: 102,
                    circuit: "2".into(),
                },
                // A fault at the unit through `value` (system pu).
                "fault" => RmsEvent::BusFault {
                    time,
                    bus: 102,
                    r: 0.0,
                    x: value,
                },
                "clear" => RmsEvent::ClearFault { time, bus: 102 },
                "v_ref" => RmsEvent::SetReference {
                    time,
                    bus: 102,
                    machine_id: "1".into(),
                    signal: InputSignal::VRef,
                    value,
                },
                "p_ref" => RmsEvent::SetReference {
                    time,
                    bus: 102,
                    machine_id: "1".into(),
                    signal: InputSignal::PRef,
                    value,
                },
                other => panic!("unknown event {other}"),
            }
        })
        .collect()
}

/// The runs of the reference with the given ids, against tellegen at a 1 ms
/// step: every channel within its bound of `tolerances.json` and within its
/// physical tolerance, and the initial references as the reference's.
/// Returns the failures; panics on a run, channel or bound that is missing.
fn compare(ids: &[&str]) -> Vec<String> {
    let reference = read_json("closed_loop.json");
    let bounds = read_json("tolerances.json");
    let mut failures = Vec::new();
    for &id in ids {
        let run = reference["runs"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == id)
            .unwrap_or_else(|| panic!("no run {id} in the reference"));
        let case = run["case"].as_str().unwrap();
        let raw = std::fs::read(data_dir().join(format!("{case}.raw"))).unwrap();
        let src = powerio::Source::from_memory("case.raw", raw).unwrap();
        let module = powerio::parse(src).unwrap();
        let powerio::PioValue::BalancedNetwork(network) = module.value() else {
            panic!("{case}: not a balanced network")
        };
        let dyr = powerio::parse_dyr(&dyr_text(case, run["records"].as_object().unwrap()))
            .unwrap_or_else(|e| panic!("{id}: {e}"));
        // The run's model switches (the lock is held by the caller).
        let sw = &run["switches"];
        if let Some(v) = sw.get("vcomp") {
            let v = series(v);
            rms::set_vcomp(HashMap::from([(102, [v[0], v[1], v[2]])]));
        }
        if let Some(v) = sw.get("uel") {
            let v = series(v);
            rms::set_uel(HashMap::from([(102, [v[0], v[1]])]));
        }
        if let Some(v) = sw.get("gast_rate") {
            rms::set_gast_rate(v.as_f64().unwrap());
        }
        let sim = RmsSimulation::with_options(
            network,
            &dyr,
            LoadModel::ConstantImpedance,
            psid_options(),
        )
        .unwrap_or_else(|e| panic!("{id}: {e}"));
        let t_end = run["t_end"].as_f64().unwrap();
        let ours = sim
            .run(&RmsRequest {
                t_end,
                dt: 0.001,
                output_step: 0.01,
                load_model: Some(LoadModel::ConstantImpedance),
                events: events(run),
                small_signal: false,
                record: None,
            })
            .unwrap_or_else(|e| panic!("{id}: {e}"));
        all_off();
        // The initial references solve the same operating point.
        let unit = &ours.generators[0];
        for (name, theirs, mine) in [
            ("v_ref", &run["v_ref0"], unit.v_ref),
            ("p_ref", &run["p_ref0"], unit.p_ref),
        ] {
            let d = (mine - theirs.as_f64().unwrap()).abs();
            if !(d <= 1e-6) {
                failures.push(format!("{id}: initial {name} differs by {d:.2e}"));
            }
        }
        let time = series(&run["time"]);
        let idx: Vec<usize> = time.iter().map(|t| (t / 0.01).round() as usize).collect();
        let channels = run["channels"].as_object().unwrap();
        for (key, r) in channels {
            let r = series(r);
            let o = ours
                .channels
                .get(key)
                .unwrap_or_else(|| panic!("{id}: tellegen records no channel {key}"));
            let d = idx
                .iter()
                .zip(&r)
                .map(|(&k, r)| (o[k] - r).abs())
                .fold(0.0_f64, f64::max);
            let tol = bounds[id][key].as_f64().unwrap_or_else(|| {
                assert!(report(), "{id}: no bound for {key}");
                f64::INFINITY
            });
            if report() {
                println!("{id} {key}: {d:.3e} (bound {tol:.2e})");
            }
            if !(d <= tol) {
                failures.push(format!("{id} {key}: {d:.2e} > {tol:.2e}"));
            }
            if let Some(p) = physical_tolerance(key) {
                if !(d <= p) {
                    failures.push(format!(
                        "{id} {key}: {d:.2e} beyond the physical tolerance {p:.1e}"
                    ));
                }
            }
        }
        // Every bound belongs to a compared channel.
        if let Some(b) = bounds[id].as_object() {
            assert_eq!(
                b.keys().collect::<Vec<_>>(),
                channels.keys().collect::<Vec<_>>(),
                "{id}"
            );
        }
    }
    failures
}

// ---------------------------------------------------------------------------
// The tests.

/// The runs of the reference, by the test that compares them.
const ESAC6A_RUNS: &[&str] = &[
    "esac6a_trip",
    "esac6a_fault",
    "esac6a_limits_trip",
    "esac6a_limits_fault",
];
const VFELIM_RUNS: &[&str] = &["esac6a_vfelim_trip"];
const LIMITER_RUNS: &[&str] = &["esac6a_oelit_uelpq"];
const COMPENSATOR_RUNS: &[&str] = &["sexs_compensator", "sexs_compensator_lag"];
const UEL_RUNS: &[&str] = &["sexs_uel"];
const GAST_RUNS: &[&str] = &["gast_rate"];

/// The case files are the single-machine cases of WECC 240 that
/// `support/smib.rs` builds (`smib_genrou_sexs_tgov1`, `smib_genrou_gast_sexs`),
/// so the reference ran on exactly the network and records tellegen runs.
#[test]
fn case_files_are_the_wecc240_single_machine_cases() {
    let cases = wecc240_smib_cases();
    for id in ["smib_genrou_sexs_tgov1", "smib_genrou_gast_sexs"] {
        let case = cases.iter().find(|c| c.id == id).unwrap();
        for (ext, text) in [("raw", &case.raw), ("dyr", &case.dyr)] {
            let file = std::fs::read_to_string(data_dir().join(format!("{id}.{ext}"))).unwrap();
            assert_eq!(&file, text, "{id}.{ext}");
        }
    }
}

/// ESAC6A (IEEE Std 421.5-2005 AC6A with ESAC1A's AC exciter) in place of
/// the unit's SEXS, with tellegen's typical set and with its limits narrowed
/// around the operating point: through a line trip VA rests on VAMIN and VR
/// on its lower limit `Vt VRMIN`; through a fault VA reaches VAMAX, VR its
/// upper limit `Vt VRMAX` and the exciter field current limiter VH its
/// maximum.
#[cfg(feature = "rms-experimental")]
#[test]
fn esac6a_against_reference() {
    let _switches = switches();
    let failures = compare(ESAC6A_RUNS);
    assert!(failures.is_empty(), "{failures:#?}");
}

/// ESAC6A with the narrowed limits of [`esac6a_against_reference`] but its
/// exciter field current limit VFELIM at 2.95 instead of 2.93, through the
/// line trip: at rest the limiter's lead-lag `KH (VFE − VFELIM)` is −0.051,
/// below its zero floor, so VH rests at zero as with 2.93. The reference
/// starts at rest and VA and VR reach their lower limits as in
/// `esac6a_limits_trip`. Its bounds are those of `esac6a_limits_trip` (the
/// same run but for a limiter that stays at zero).
#[cfg(feature = "rms-experimental")]
#[test]
fn esac6a_field_current_limiter_below_its_floor_at_rest_against_reference() {
    let _switches = switches();
    let failures = compare(VFELIM_RUNS);
    assert!(failures.is_empty(), "{failures:#?}");
}

/// ESAC6A with OELIT and UELPQ at its summing point, through voltage
/// reference steps over 18 s: up (the over-excitation limiter holds the
/// field current at its instantaneous limit, its heating passes the inverse
/// time and the limit falls to the continuous one), down (the field current
/// falls below the pickup, the limiter winds back and cools, the
/// under-excitation limiter acts on the P–Q line) and back (it winds back).
/// The limiters' heating, limit and integrators are compared with the
/// exciter's own signals.
#[cfg(feature = "rms-experimental")]
#[test]
fn oelit_uelpq_at_the_summing_point_against_reference() {
    let _switches = switches();
    let failures = compare(LIMITER_RUNS);
    assert!(failures.is_empty(), "{failures:#?}");
}

/// The AVR load compensator (`set_vcomp`) on the unit's SEXS: the regulator
/// sees `|Vt + (Rc + j Xc) It|` (IEEE Std 421.5), and in the variant with a
/// lag `|Vt + Rc It| + Xc z` with `z` the reactive current through
/// `1/(1 + s T)`; through a line trip and a voltage reference step.
#[test]
fn load_compensator_against_reference() {
    let _switches = switches();
    let failures = compare(COMPENSATOR_RUNS);
    assert!(failures.is_empty(), "{failures:#?}");
}

/// The summing-point under-excitation limiter (`set_uel`, `[q_lim, gain]`)
/// on the unit's SEXS: a voltage reference step down drives the reactive
/// power below `−q_lim Vt²` and the limiter raises the voltage error by
/// `gain` times the excess, until a step back releases it.
#[test]
fn summing_point_uel_against_reference() {
    let _switches = switches();
    let failures = compare(UEL_RUNS);
    assert!(failures.is_empty(), "{failures:#?}");
}

/// The GAST fuel valve rate limit (`set_gast_rate`, 0.05 pu/s) on WECC 240's
/// GENROU + SEXS + GAST unit: a power reference step up and a larger one
/// down move the valve at the rate limit, then on its plain lag.
#[test]
fn gast_valve_rate_against_reference() {
    let _switches = switches();
    let failures = compare(GAST_RUNS);
    assert!(failures.is_empty(), "{failures:#?}");
}

/// Every run of the reference is compared by one of the tests above, and
/// has its bounds.
#[test]
fn every_reference_run_is_compared() {
    let reference = read_json("closed_loop.json");
    let mut ids: Vec<&str> = reference["runs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    ids.sort_unstable();
    let mut compared: Vec<&str> = [
        ESAC6A_RUNS,
        VFELIM_RUNS,
        LIMITER_RUNS,
        COMPENSATOR_RUNS,
        UEL_RUNS,
        GAST_RUNS,
    ]
    .concat();
    compared.sort_unstable();
    assert_eq!(ids, compared);
    let bounds = read_json("tolerances.json");
    let bound_ids: Vec<&str> = bounds
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(ids, bound_ids);
}
