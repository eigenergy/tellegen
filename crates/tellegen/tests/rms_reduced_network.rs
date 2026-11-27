//! Network-level RMS models that PowerSimulationsDynamics.jl does not
//! implement, each on a small network written inline here against an
//! independent reference of the same network (`tests/data/rms_reduced_network`,
//! see the `SOURCES.md` there; the scripts that wrote it are in
//! `validation/rms-models/reduced-network`):
//!
//! - load recovery, load saturation and the one-sided rise of load reactive
//!   power ([`rms::set_load_recovery`], [`rms::set_load_saturation`],
//!   [`rms::set_load_rise`]), and the PSS/E `PQBRAK` breakpoint with and
//!   without its rounding (`TELLEGEN_PQBRAK_SMOOTH`), on a feeder with a
//!   classical machine through a line trip, a load shedding and a fault;
//! - the voltage exponent of a renewable plant's reactive power set point
//!   ([`rms::set_plant_q_exponent`]) through a load step and a line trip;
//! - HVDC links in AC emulation ([`rms::set_hvdc_emulation`]) between two
//!   classical machines joined by a tie, through a load step, unlimited and
//!   limited, and the inter-area mode of the linearized system.
//!
//! Every reference run holds per compared channel a tolerance: three times
//! tellegen's difference to the reference at the 1 ms step of these runs, at
//! least 1e-6, as the single-machine comparison with
//! PowerSimulationsDynamics.jl does. Besides it every channel meets a fixed
//! physical tolerance ([`physical_tolerance`]). Set `RMS_REDUCED_REPORT=1`
//! to print every comparison; `RMS_REDUCED_MEASURE=1` measures the
//! differences the tolerances come from instead ([`measuring`]).
//!
//! The model switches are globals of the process, so every test here holds
//! one lock and turns every switch off when it starts and when it ends. The
//! `PQBRAK` rounding is read once per process from the environment, so its
//! run happens in a child process of this test binary.

#![cfg(feature = "rms")]
// Negated comparisons are deliberate: they treat NaN as failing.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, MutexGuard, PoisonError};

use serde_json::Value;
use tellegen::rms::{
    self, LoadModel, LowVoltageLoad, RmsEvent, RmsOptions, RmsRequest, RmsResult, RmsSimulation,
};

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/support/psid.rs"
));

// ---------------------------------------------------------------------------
// The switches: one test at a time, every switch off around it.

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

/// Sets the switches a reference run names (`switches` of the run).
fn apply_switches(run: &Value) {
    let s = &run["switches"];
    let num = |v: &Value| v.as_f64().unwrap();
    if let Some(v) = s.get("load_recovery") {
        rms::set_load_recovery(num(&v[0]), num(&v[1]));
    }
    if let Some(v) = s.get("load_saturation") {
        rms::set_load_saturation(num(&v[0]), num(&v[1]));
    }
    if let Some(v) = s.get("load_rise") {
        rms::set_load_rise(num(v));
    }
    if let Some(v) = s.get("plant_q_exponent") {
        let map = v
            .as_object()
            .unwrap()
            .iter()
            .map(|(bus, n)| (bus.parse().unwrap(), num(n)))
            .collect();
        rms::set_plant_q_exponent(map);
    }
    if let Some(v) = s.get("hvdc_emulation") {
        let links = v
            .as_array()
            .unwrap()
            .iter()
            .map(|l| {
                (
                    l[0].as_u64().unwrap() as usize,
                    l[1].as_u64().unwrap() as usize,
                    num(&l[2]),
                    num(&l[3]),
                )
            })
            .collect();
        rms::set_hvdc_emulation(links);
    }
}

// ---------------------------------------------------------------------------
// Inline networks.

const RAW_TAIL: &str = " 0 /End of Branch data, Begin Transformer data
 0 /End of Transformer data, Begin Area interchange data
 0 /End of Area interchange data, Begin Two-terminal dc line data
 0 /End of Two-terminal dc line data, Begin VSC dc line data
 0 /End of VSC dc line data, Begin Impedance correction table data
 0 /End of Impedance correction table data, Begin Multi-terminal dc line data
 0 /End of Multi-terminal dc line data, Begin Multi-section line data
 0 /End of Multi-section line data, Begin Zone data
 0 /End of Zone data, Begin Inter-area transfer data
 0 /End of Inter-area transfer data, Begin Owner data
 0 /End of Owner data, Begin FACTS device data
 0 /End of FACTS device data, Begin Switched shunt data
 0 /End of Switched shunt data, Begin GNE device data
 0 /End of GNE device data
Q
";

/// A bus: number, type (1 load, 2 generator, 3 slack), voltage.
type Bus = (usize, u8, f64);
/// A generator: bus, PG (MW), VS (pu), MBASE (MVA), ZSORCE reactance (pu).
type Gen = (usize, f64, f64, f64, f64);
/// A constant-power load: bus, PL (MW), QL (Mvar).
type Load = (usize, f64, f64);
/// A line: from, to, circuit, reactance (pu).
type Line = (usize, usize, &'static str, f64);

/// A PSS/E RAW case on 100 MVA, 60 Hz.
fn raw_case(buses: &[Bus], gens: &[Gen], loads: &[Load], lines: &[Line]) -> String {
    let mut r = String::from("0,   100.00,  32, 0, 1, 60.00     / reduced network test case\n\n\n");
    for &(b, kind, v) in buses {
        r.push_str(&format!(
            "{b},'B{b}', 230.0000,{kind},   1,   1,   1,{v:.5},   0.0000\n"
        ));
    }
    r.push_str(" 0 /End of Bus data, Begin Load data\n");
    for &(b, p, q) in loads {
        r.push_str(&format!(
            "{b},'1 ',1,   1,   1, {p:.3}, {q:.3}, 0.000, 0.000, 0.000, 0.000,   1,1\n"
        ));
    }
    r.push_str(" 0 /End of Load data, Begin Fixed shunt data\n");
    r.push_str(" 0 /End of Fixed shunt data, Begin Generator data\n");
    for &(b, pg, vs, mbase, zx) in gens {
        r.push_str(&format!(
            "{b},'1 ', {pg:.3}, 0.000, 9999.000, -9999.000,{vs:.5}, 0, {mbase:.3}, 0.00000E+0, {zx:.5E}, \
             0.00000E+0, 0.00000E+0,1.00000,1,  100.0, 9999.000, -9999.000,   1,1.0000\n"
        ));
    }
    r.push_str(" 0 /End of Generator data, Begin Branch data\n");
    for &(f, t, ckt, x) in lines {
        r.push_str(&format!(
            "{f}, {t},'{ckt} ', 0.00000E+0, {x:.5E}, 0.00000, 9999.00, 9999.00, 9999.00, \
             0.00000, 0.00000, 0.00000, 0.00000,1,1, 0.00, 1,1.0000\n"
        ));
    }
    r.push_str(RAW_TAIL);
    r
}

/// The simulation of a RAW text and DYR text.
fn simulation(raw: &str, dyr: &str, model: LoadModel, options: RmsOptions) -> RmsSimulation {
    let src = powerio::Source::from_memory("case.raw", raw.as_bytes().to_vec()).unwrap();
    let module = powerio::parse(src).unwrap();
    let powerio::PioValue::BalancedNetwork(net) = module.value() else {
        panic!("not a balanced network")
    };
    let dyr = powerio::parse_dyr(dyr).unwrap();
    RmsSimulation::with_options(net, &dyr, model, options).unwrap()
}

/// The feeder of the load models: an infinite source at 101 (a classical
/// machine without inertia) feeds the load bus 103 over two lines of 0.1 pu;
/// a classical machine at 102 (80 MW at 1.02 pu on 100 MVA, X'd 0.2 pu,
/// H 3 s, D 2 pu) feeds it over 0.05 pu, and 103 feeds the load bus 104 over
/// 0.05 pu. Loads: 150 MW + 30 Mvar at 103, 60 MW + 20 Mvar at 104, as
/// constant power, with the exact law at low voltage or the PSS/E `PQBRAK`
/// conversion below `v_break`.
fn feeder(v_break: Option<f64>) -> RmsSimulation {
    let raw = raw_case(
        &[(101, 3, 1.0), (102, 2, 1.02), (103, 1, 1.0), (104, 1, 1.0)],
        &[(101, 0.0, 1.0, 100.0, 1e-5), (102, 80.0, 1.02, 100.0, 0.2)],
        &[(103, 150.0, 30.0), (104, 60.0, 20.0)],
        &[
            (101, 103, "1", 0.1),
            (101, 103, "2", 0.1),
            (102, 103, "1", 0.05),
            (103, 104, "1", 0.05),
        ],
    );
    let dyr = "101 'GENCLS' 1 0.0 0.0 /\n102 'GENCLS' 1 3.0 2.0 /\n";
    let options = RmsOptions {
        reactive_limits: false,
        low_voltage_load: match v_break {
            Some(v_break) => LowVoltageLoad::Break { v_break },
            None => LowVoltageLoad::Exact,
        },
        ..RmsOptions::default()
    };
    simulation(&raw, dyr, LoadModel::ConstantPower, options)
}

/// The events of the feeder, off the 20 ms sample grid.
fn feeder_events(event: &str) -> Vec<RmsEvent> {
    match event {
        // A line from the source opens.
        "trip" => vec![RmsEvent::BranchTrip {
            time: 0.505,
            from: 101,
            to: 103,
            circuit: "1".into(),
        }],
        // 60 % of the load at 104 is shed.
        "shed" => vec![RmsEvent::LoadChange {
            time: 0.505,
            bus: 104,
            scale: 0.4,
        }],
        // A fault at 103 through 0.05 pu for 100 ms.
        "fault" => vec![
            RmsEvent::BusFault {
                time: 0.505,
                bus: 103,
                r: 0.0,
                x: 0.05,
            },
            RmsEvent::ClearFault {
                time: 0.605,
                bus: 103,
            },
        ],
        // A fault at 103 through 0.1 pu for 200 ms, which holds 103 and 104
        // near 0.69 pu, just below the PQBRAK break at 0.7 pu.
        "fault_band" => vec![
            RmsEvent::BusFault {
                time: 0.505,
                bus: 103,
                r: 0.0,
                x: 0.1,
            },
            RmsEvent::ClearFault {
                time: 0.705,
                bus: 103,
            },
        ],
        other => panic!("unknown feeder event {other}"),
    }
}

/// A renewable plant (REGCA1, REECB1, REPCA1) in reactive power control:
/// the plant controller's PI (Kp 0.5, Ki 5, no deadband) sets the reactive
/// current command.
fn plant_records(bus: usize) -> String {
    format!(
        "{bus} 'REGCA1' 1 1 0.02 10.0 0.9 0.4 1.22 1.2 0.9 0.5 -1.3 0.02 0.0 100.0 -100.0 0.7 /\n\
         {bus} 'REECB1' 1 0 0 1 0 0 -99.0 99.0 0.02 0.0 0.0 0.0 1.1 -1.1 0.0 0.05 0.44 -0.44 \
         1.05 0.9 0.0 0.01 10.0 60.0 0.02 99.0 -99.0 1.0 0.0 1.11 0.02 /\n\
         {bus} 'REPCA1' 1 0 0 0 '0 ' 0 0 1 0.02 0.5 5.0 0.0 0.05 0.0 0.0 0.0 0.02 0.1 -0.1 \
         0.0 0.0 0.43 -0.43 1.0 0.05 0.25 -1.0 1.0 99.0 -99.0 1.0 0.0 0.1 0.0 0.0 /\n"
    )
}

/// The plant feeder: the infinite source at 101 feeds the load bus 103
/// (150 MW + 30 Mvar, constant impedance) over two lines of 0.1 pu; the
/// renewable plant at 104 (50 MW on 100 MVA at 1.01 pu, so it starts
/// delivering reactive power) feeds it over 0.05 pu. The plant runs as
/// PowerSimulationsDynamics.jl 0.16.2 has it ([`psid_options`] and its
/// low-voltage active current gain).
fn plant_feeder() -> RmsSimulation {
    let raw = raw_case(
        &[(101, 3, 1.0), (103, 1, 1.0), (104, 2, 1.01)],
        &[(101, 0.0, 1.0, 100.0, 1e-5), (104, 50.0, 1.01, 100.0, 0.2)],
        &[(103, 150.0, 30.0)],
        &[
            (101, 103, "1", 0.1),
            (101, 103, "2", 0.1),
            (104, 103, "1", 0.05),
        ],
    );
    let dyr = format!("101 'GENCLS' 1 0.0 0.0 /\n{}", plant_records(104));
    let options = RmsOptions {
        psid_lv_gain: true,
        ..psid_options()
    };
    simulation(&raw, &dyr, LoadModel::ConstantImpedance, options)
}

/// The events of the plant feeder, off the 20 ms sample grid.
fn plant_events(event: &str) -> Vec<RmsEvent> {
    match event {
        // The load at 103 up by half: a lasting dip.
        "step" => vec![RmsEvent::LoadChange {
            time: 0.505,
            bus: 103,
            scale: 1.5,
        }],
        // A line from the source opens.
        "trip" => vec![RmsEvent::BranchTrip {
            time: 0.505,
            from: 101,
            to: 103,
            circuit: "1".into(),
        }],
        other => panic!("unknown plant event {other}"),
    }
}

/// Two areas joined by one weak tie: the classical machines at 1 and 2
/// (500 MVA, X'd 0.3 pu, H 4 s, no damping) feed the loads at 3 and 4
/// (250 MW each, constant impedance) over lines of 0.02 pu; the tie 3-4 has
/// 0.3 pu. Its one oscillatory mode is the inter-area mode of the two
/// machines.
fn two_areas() -> RmsSimulation {
    let raw = raw_case(
        &[(1, 3, 1.0), (2, 2, 1.0), (3, 1, 1.0), (4, 1, 1.0)],
        &[(1, 0.0, 1.0, 500.0, 0.3), (2, 200.0, 1.0, 500.0, 0.3)],
        &[(3, 250.0, 0.0), (4, 250.0, 0.0)],
        &[(1, 3, "1", 0.02), (2, 4, "1", 0.02), (3, 4, "1", 0.3)],
    );
    let dyr = "1 'GENCLS' 1 4.0 0.0 /\n2 'GENCLS' 1 4.0 0.0 /\n";
    let options = RmsOptions {
        reactive_limits: false,
        ..RmsOptions::default()
    };
    simulation(&raw, dyr, LoadModel::ConstantImpedance, options)
}

/// The event of the two areas: the load at 4 up by a fifth.
fn two_area_events(event: &str) -> Vec<RmsEvent> {
    match event {
        "step" => vec![RmsEvent::LoadChange {
            time: 0.105,
            bus: 4,
            scale: 1.2,
        }],
        other => panic!("unknown two-area event {other}"),
    }
}

// ---------------------------------------------------------------------------
// References and comparisons.

fn reference(file: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/rms_reduced_network")
        .join(file);
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn series(v: &Value) -> Vec<f64> {
    serde_json::from_value(v.clone()).unwrap()
}

fn report() -> bool {
    std::env::var_os("RMS_REDUCED_REPORT").is_some()
}

/// The step of every run here, s.
const DT: f64 = 0.001;
/// The sample step of the references, s.
const SAMPLE: f64 = 0.02;

/// A run of `sim` through `events` at the step `dt`, sampled every 20 ms.
fn run(sim: &RmsSimulation, t_end: f64, dt: f64, events: Vec<RmsEvent>) -> RmsResult {
    sim.run(&RmsRequest {
        t_end,
        dt,
        output_step: SAMPLE,
        events,
        ..RmsRequest::default()
    })
    .unwrap()
}

/// The fixed tolerance of a channel against the reference, whatever the
/// step: what the agreement means for an analysis that reads the channel.
///
/// - speed 1e-4 pu: 6 mHz at 60 Hz, below the governor deadbands of
///   interconnection practice (16.6 to 36 mHz);
/// - rotor and bus angles 0.5°, small against the first swings of tens of
///   degrees a transient stability analysis judges;
/// - voltage magnitude 1e-3 pu, a tenth of the 1 % steps a voltage analysis
///   reads;
/// - active and reactive power 1e-3 pu on 100 MVA, 0.1 MW or Mvar.
fn physical_tolerance(key: &str) -> f64 {
    match key.split(':').next().unwrap() {
        "omega" => 1e-4,
        "delta" | "va" => 0.5_f64.to_radians(),
        // Voltage magnitude and every power.
        _ => 1e-3,
    }
}

/// tellegen's channel `key`; `link:3-4` is the power the link moves out of
/// bus 3, from the balance at that bus (no unit there): what its lines and
/// its load do not take.
fn channel(ours: &RmsResult, key: &str) -> Vec<f64> {
    if key == "link:3-4" {
        let c = |k: &str| &ours.channels[k];
        let (a, b, l) = (c("p_from:3-4"), c("p_to:1-3"), c("pl:3"));
        return (0..ours.time.len())
            .map(|k| -(a[k] + b[k] + l[k]))
            .collect();
    }
    ours.channels
        .get(key)
        .unwrap_or_else(|| panic!("tellegen records no channel {key}"))
        .clone()
}

/// The largest difference of every channel of `run` (a reference run) to
/// `ours`. Angles (`va:`, `delta:`) are taken against the channel of the same
/// quantity at `angle_ref` where the case has no fixed frame.
fn differences(run: &Value, ours: &RmsResult, angle_ref: Option<usize>) -> BTreeMap<String, f64> {
    let time = series(&run["time"]);
    let idx: Vec<usize> = time.iter().map(|t| (t / SAMPLE).round() as usize).collect();
    for (&k, t) in idx.iter().zip(&time) {
        assert!((ours.time[k] - t).abs() < 1e-9, "sample {t} s");
    }
    let channels = &run["channels"];
    let mut out = BTreeMap::new();
    for (key, r) in channels.as_object().unwrap() {
        let q = key.split(':').next().unwrap();
        let angle = q == "va" || q == "delta";
        if angle && angle_ref.is_some_and(|b| *key == format!("{q}:{b}")) {
            // The angle reference itself.
            continue;
        }
        let r = series(r);
        let o = channel(ours, key);
        let base = match angle_ref {
            Some(b) if angle => {
                let bk = format!("{q}:{b}");
                Some((ours.channels[&bk].clone(), series(&channels[&bk])))
            }
            _ => None,
        };
        let d = idx
            .iter()
            .zip(&r)
            .enumerate()
            .map(|(s, (&k, r))| match &base {
                Some((ob, rb)) => ((o[k] - ob[k]) - (r - rb[s])).abs(),
                None => (o[k] - r).abs(),
            })
            .fold(0.0_f64, f64::max);
        out.insert(key.clone(), d);
    }
    out
}

/// Every channel of `run` against its tolerance and its physical tolerance;
/// the failures.
fn check(what: &str, run: &Value, diffs: &BTreeMap<String, f64>) -> Vec<String> {
    let mut failures = Vec::new();
    for (key, &d) in diffs {
        let tol = run["tolerance"][key.as_str()]
            .as_f64()
            .unwrap_or_else(|| panic!("{what}: no tolerance for {key}"));
        let p = physical_tolerance(key);
        if report() {
            println!("{what} {key}: {d:.2e} (bound {tol:.1e}, physical {p:.0e})");
        }
        if !(d <= tol) {
            failures.push(format!("{what} {key}: {d:.2e} > {tol:.1e}"));
        }
        if !(d <= p) {
            failures.push(format!(
                "{what} {key}: {d:.2e} beyond the physical tolerance {p:.0e}"
            ));
        }
    }
    failures
}

/// With `RMS_REDUCED_MEASURE` set, the tests measure instead of checking
/// the tolerances: per run they print one JSON line with tellegen's largest
/// differences at the 1 ms step (`diff`, from which the tolerances are
/// derived) and at 2 ms and 0.5 ms (`diff_2ms`, `diff_05ms`), which show
/// that the differences shrink with the step. The physical tolerances are
/// checked either way.
fn measuring() -> bool {
    std::env::var_os("RMS_REDUCED_MEASURE").is_some()
}

/// One reference run against tellegen, `simulate(dt)` running it at the
/// step `dt`; the failures.
fn assess(
    file: &str,
    (name, event, r): (&str, &str, &Value),
    angle_ref: Option<usize>,
    simulate: impl Fn(f64) -> RmsResult,
) -> (usize, Vec<String>) {
    let what = format!("{name} {event}");
    let diffs = differences(r, &simulate(DT), angle_ref);
    if !measuring() {
        return (diffs.len(), check(&what, r, &diffs));
    }
    let coarse = differences(r, &simulate(2.0 * DT), angle_ref);
    let fine = differences(r, &simulate(0.5 * DT), angle_ref);
    println!(
        "{}",
        serde_json::json!({
            "file": file, "name": name, "event": event,
            "diff": diffs, "diff_2ms": coarse, "diff_05ms": fine,
        })
    );
    let failures = diffs
        .iter()
        .filter(|(k, &d)| !(d <= physical_tolerance(k)))
        .map(|(k, d)| format!("{what} {k}: {d:.2e} beyond the physical tolerance"))
        .collect();
    (diffs.len(), failures)
}

/// The runs of a reference whose name passes `keep`, each as (name, event,
/// run).
fn runs(reference: &Value, keep: impl Fn(&str) -> bool) -> Vec<(&str, &str, &Value)> {
    reference["runs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["name"].as_str().unwrap(), r["event"].as_str().unwrap(), r))
        .filter(|(n, _, _)| keep(n))
        .collect()
}

// ---------------------------------------------------------------------------
// Load models.

/// The load models on the feeder ([`feeder`]) against the independent
/// reference: without a switch through the line trip, the load shedding and
/// the fault; load recovery (`alpha = 2`, `t = 1 s`) through the trip and the
/// fault; saturation (`c = 2` above the knee 0.99 pu, which lies above the
/// start voltage of 104 and below that of 103) and the one-sided rise
/// (`c = 2`) through the shedding (a rise) and the fault (a dip, then swings
/// to both sides of the start voltage). Each run is 4 s; every channel within
/// its tolerance.
#[test]
fn load_models_against_the_reference() {
    let _s = switches();
    let reference = reference("loads.json");
    let mut failures = Vec::new();
    let mut compared = 0;
    for case in runs(&reference, |n| !n.starts_with("pqbrak")) {
        all_off();
        apply_switches(case.2);
        let (n, f) = assess("loads.json", case, None, |dt| {
            run(&feeder(None), 4.0, dt, feeder_events(case.1))
        });
        compared += n;
        failures.extend(f);
    }
    // Nine runs of eleven channels.
    assert_eq!(compared, 9 * 11);
    assert!(failures.is_empty(), "{failures:#?}");
}

// The PQBRAK rounding, in a child process.

/// Where the child writes its run (set by the parent; without it the child
/// does nothing, so running the ignored test by hand is harmless).
const CHILD_OUT: &str = "RMS_REDUCED_CHILD_OUT";
/// The child's step, s.
const CHILD_DT: &str = "RMS_REDUCED_CHILD_DT";

/// Run by [`pqbrak_against_the_reference`] in a child process with
/// `TELLEGEN_PQBRAK_SMOOTH` set: the feeder with the `PQBRAK` conversion at
/// 0.7 pu through the fault near the break, written to
/// `RMS_REDUCED_CHILD_OUT` as bit patterns.
#[test]
#[ignore = "run by pqbrak_against_the_reference in a child process with the rounding set"]
fn child_run() {
    let Some(out) = std::env::var_os(CHILD_OUT) else {
        return;
    };
    let dt = std::env::var(CHILD_DT).unwrap().parse().unwrap();
    let ours = run(&feeder(Some(0.7)), 4.0, dt, feeder_events("fault_band"));
    let bits = |v: &[f64]| v.iter().map(|x| x.to_bits()).collect::<Vec<u64>>();
    let value = serde_json::json!({
        "time": bits(&ours.time),
        "channels": ours.channels.iter().map(|(k, v)| (k.clone(), bits(v))).collect::<BTreeMap<_, _>>(),
    });
    std::fs::write(out, serde_json::to_string(&value).unwrap()).unwrap();
}

/// [`child_run`] at the step `dt` with `TELLEGEN_PQBRAK_SMOOTH` set to `d`.
fn in_child(d: &str, dt: f64) -> RmsResult {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("rms_reduced_network");
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join(format!("{}-pqbrak.json", std::process::id()));
    let _ = std::fs::remove_file(&out);
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "child_run",
            "--exact",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD_OUT, &out)
        .env(CHILD_DT, dt.to_string())
        .env("TELLEGEN_PQBRAK_SMOOTH", d)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "the child run failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let v: Value = serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
    let _ = std::fs::remove_file(&out);
    let floats = |v: &Value| -> Vec<f64> {
        v.as_array()
            .unwrap()
            .iter()
            .map(|b| f64::from_bits(b.as_u64().unwrap()))
            .collect()
    };
    let mut ours = run(&feeder(Some(0.7)), SAMPLE, SAMPLE, Vec::new());
    ours.time = floats(&v["time"]);
    ours.channels = v["channels"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, s)| (k.clone(), floats(s)))
        .collect();
    ours
}

/// The PSS/E `PQBRAK` conversion of the constant-power loads at 0.7 pu on the
/// feeder through a fault that holds 103 and 104 near 0.69 pu for 200 ms,
/// within the rounding's band: the plain break (here) and the break rounded
/// off over ±0.1 pu (`TELLEGEN_PQBRAK_SMOOTH=0.1`, in a child process), each
/// against the independent reference. Both buses start more than 0.1 pu above
/// the break, so both are rounded.
#[test]
fn pqbrak_against_the_reference() {
    let _s = switches();
    let reference = reference("loads.json");
    let mut failures = Vec::new();
    let mut compared = 0;
    for case in runs(&reference, |n| n.starts_with("pqbrak")) {
        assert_eq!(case.1, "fault_band");
        let smooth = case.2["switches"].get("pqbrak_smooth");
        let (n, f) = assess("loads.json", case, None, |dt| match smooth {
            Some(d) => in_child(&d.as_f64().unwrap().to_string(), dt),
            None => run(&feeder(Some(0.7)), 4.0, dt, feeder_events("fault_band")),
        });
        compared += n;
        failures.extend(f);
    }
    assert_eq!(compared, 2 * 11);
    assert!(failures.is_empty(), "{failures:#?}");
}

// ---------------------------------------------------------------------------
// The voltage exponent of a plant's reactive power set point.

/// The plant feeder ([`plant_feeder`]) against PowerSimulationsDynamics.jl
/// 0.16.2 with the exponent added to its plant controller: without the
/// exponent through the load step (PowerSimulationsDynamics.jl unmodified),
/// with `n = 2` through the load step and the line trip. Each run is 5 s;
/// every channel within its tolerance.
#[test]
fn plant_q_exponent_against_the_reference() {
    let _s = switches();
    let reference = reference("plant.json");
    let mut failures = Vec::new();
    let mut compared = 0;
    for case in runs(&reference, |_| true) {
        all_off();
        apply_switches(case.2);
        let (n, f) = assess("plant.json", case, None, |dt| {
            run(&plant_feeder(), 5.0, dt, plant_events(case.1))
        });
        compared += n;
        failures.extend(f);
    }
    assert_eq!(compared, 3 * 5);
    assert!(failures.is_empty(), "{failures:#?}");
}

// ---------------------------------------------------------------------------
// HVDC links in AC emulation.

/// The two areas ([`two_areas`]) through the load step against the
/// independent reference: without a link, with a link 3-4 of
/// `K = 50 MW/degree` unlimited and limited to 20 MW (the limit is reached).
/// Angles relative to machine 1 and bus 1. Each run is 5 s; every channel
/// within its tolerance.
#[test]
fn hvdc_emulation_against_the_reference() {
    let _s = switches();
    let reference = reference("hvdc.json");
    let mut failures = Vec::new();
    let mut compared = 0;
    for case in runs(&reference, |_| true) {
        all_off();
        apply_switches(case.2);
        let (n, f) = assess("hvdc.json", case, Some(1), |dt| {
            run(&two_areas(), 5.0, dt, two_area_events(case.1))
        });
        compared += n;
        failures.extend(f);
    }
    // Eight channels without the link, nine with it (and its power).
    assert_eq!(compared, 8 + 2 * 9);
    assert!(failures.is_empty(), "{failures:#?}");
}

/// The oscillatory mode of `eig`: the pair with the largest imaginary part
/// (the system has one).
#[cfg(feature = "rms-experimental")]
fn inter_area(eig: &[[f64; 2]]) -> [f64; 2] {
    let oscillatory: Vec<&[f64; 2]> = eig.iter().filter(|e| e[1].abs() > 1e-6).collect();
    assert_eq!(oscillatory.len(), 2, "one oscillatory pair: {eig:?}");
    *eig.iter().max_by(|a, b| a[1].total_cmp(&b[1])).unwrap()
}

/// The inter-area mode of the two areas ([`two_areas`]) at the start, with a
/// link 3-4 of `K = 0, 10, 50 MW/degree`, against the reference's
/// linearization of the same system (within its tolerance: three times
/// tellegen's distance in the complex plane, at least 1e-6 rad/s). Without a
/// link, PowerSimulationsDynamics.jl 0.16.2's mode agrees with the reference
/// to 1e-8 rad/s.
///
/// The link adds synchronizing power as an AC line of matching reactance in
/// parallel to the tie does: PowerSimulationsDynamics.jl's mode with such a
/// line (whose synchronizing power `V3 V4 cos(θ3 − θ4) / x` at the start is
/// `K`) lies 0.9 % (K = 10) and 1.2 % (K = 50) above the link's, closer than
/// 2 %. The line also carries reactive power and changes the operating
/// point (it takes part of the tie's flow), which the link does not; both
/// raise the mode from 0.985 Hz without a link to 1.30 Hz (K = 10) and
/// 1.55 Hz (K = 50).
#[cfg(feature = "rms-experimental")]
#[test]
fn hvdc_emulation_inter_area_mode_against_the_reference() {
    let _s = switches();
    let reference = reference("hvdc.json");
    let mut failures = Vec::new();
    let modes = reference["inter_area_mode"].as_array().unwrap();
    assert_eq!(modes.len(), 3);
    for mode in modes {
        let k = mode["k_mw_per_degree"].as_f64().unwrap();
        all_off();
        if k > 0.0 {
            rms::set_hvdc_emulation(vec![(3, 4, k, 0.0)]);
        }
        let ours = inter_area(&two_areas().eigenvalues().unwrap());
        let theirs = series(&mode["eigenvalue"]);
        let d = (ours[0] - theirs[0]).hypot(ours[1] - theirs[1]);
        if measuring() {
            println!("{}", serde_json::json!({ "k_mw_per_degree": k, "diff": d }));
        } else {
            let tol = mode["tolerance"].as_f64().unwrap();
            if report() {
                println!("K = {k}: {ours:?} against {theirs:?}: {d:.2e} (bound {tol:.1e})");
            }
            if !(d <= tol) {
                failures.push(format!(
                    "K = {k}: {ours:?} against {theirs:?}: {d:.2e} > {tol:.1e}"
                ));
            }
        }
        if let Some(psid) = mode.get("psid") {
            let psid = series(psid);
            assert!(
                (psid[1] - theirs[1]).abs() < 1e-7,
                "{psid:?} against {theirs:?}"
            );
        }
        if let Some(line) = mode.get("psid_ac_line_matched") {
            let f_line = series(line)[1];
            let above = f_line / ours[1] - 1.0;
            eprintln!(
                "K = {k} MW/degree: {:.4} Hz with the link, {:.4} Hz with a matched AC line ({:+.2} %)",
                ours[1] / std::f64::consts::TAU,
                f_line / std::f64::consts::TAU,
                100.0 * above
            );
            assert!(
                (0.0..0.02).contains(&above),
                "K = {k}: the AC line's mode lies {:.2} % above the link's",
                100.0 * above
            );
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
