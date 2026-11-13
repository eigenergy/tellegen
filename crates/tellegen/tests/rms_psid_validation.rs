//! RMS validation against PowerSimulationsDynamics.jl 0.16.2 on the WECC 240
//! case that ships with the code (`docs/src/rms-validation.md`).
//!
//! - Single-machine infinite-bus cases, one per model combination of WECC 240,
//!   built from its units (`support/smib.rs`): a line trip and three-phase
//!   faults, every channel within three times tellegen's difference at a 1 ms
//!   step in the validation study. The renewable plant's trip and shallow fault
//!   stay above its low-voltage gain range, so they check tellegen's own
//!   (PSS/E) plant; its deep fault runs with PowerSimulationsDynamics.jl's gain.
//! - WECC 240 itself as PowerSimulationsDynamics.jl's tutorial system holds it
//!   (the loads as constant power at the solved voltage), four scenarios over
//!   20 s at tellegen's 5 ms step: per quantity the channels with the largest
//!   difference in the validation study, and the response of the whole system
//!   (every machine speed and rotor angle, every bus voltage), within the
//!   bounds below.
//!
//! The reference is `tests/data/rms_examples/wecc240/psid_reference.json`:
//! PowerSimulationsDynamics.jl results of the validation study, see the
//! `SOURCES.md` next to it. Set `RMS_PSID_REPORT=1` to print every comparison.

#![cfg(feature = "sensitivity")]
// Negated comparisons are deliberate: they treat NaN as failing.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::Value;
use tellegen::rms::{LoadModel, RmsEvent, RmsOptions, RmsRequest, RmsSimulation};

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/support/smib.rs"
));

fn reference() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/rms_examples/wecc240/psid_reference.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn series(v: &Value) -> Vec<f64> {
    serde_json::from_value(v.clone()).unwrap()
}

fn report() -> bool {
    std::env::var_os("RMS_PSID_REPORT").is_some()
}

/// An angle difference folded into `(-π, π]`.
fn wrap(d: f64) -> f64 {
    let two_pi = 2.0 * std::f64::consts::PI;
    d - two_pi * (d / two_pi).round()
}

#[test]
fn smib_cases_against_psid() {
    let reference = reference();
    let cases = wecc240_smib_cases();
    let mut failures = Vec::new();
    let mut compared = 0;
    for run in reference["smib"].as_array().unwrap() {
        let id = run["case"].as_str().unwrap();
        let event = run["event"].as_str().unwrap();
        let case = cases.iter().find(|c| c.id == id).unwrap();
        let (network, dyr) = smib_study(case);
        let events = match event {
            // One of the two lines to the infinite bus opens.
            "trip" => vec![RmsEvent::BranchTrip {
                time: 1.0,
                from: 101,
                to: 102,
                circuit: "2".into(),
            }],
            // A fault at the unit through half its connection's reactance;
            // for the plant also a deep (1/8) and a shallow (8x) one.
            "fault" | "fault_v02" | "fault_v09" => {
                let x = case.x_line
                    * match event {
                        "fault" => 0.5,
                        "fault_v02" => 0.125,
                        _ => 8.0,
                    };
                vec![
                    RmsEvent::BusFault {
                        time: 1.0,
                        bus: 102,
                        r: 0.0,
                        x,
                    },
                    RmsEvent::ClearFault {
                        time: 1.1,
                        bus: 102,
                    },
                ]
            }
            other => panic!("{id}: unknown event {other}"),
        };
        let options = RmsOptions {
            psid_lv_gain: run["psid_lv_gain"].as_bool().unwrap_or(false),
            ..RmsOptions::default()
        };
        let sim =
            RmsSimulation::with_options(&network, &dyr, &[], LoadModel::ConstantImpedance, options)
                .unwrap();
        let ours = sim
            .run(&RmsRequest {
                t_end: 3.0,
                dt: 0.001,
                output_step: 0.01,
                load_model: LoadModel::ConstantImpedance,
                events,
                small_signal: false,
                record: None,
            })
            .unwrap();
        let time = series(&run["time"]);
        let idx: Vec<usize> = time.iter().map(|t| (t / 0.01).round() as usize).collect();
        let channels = &run["channels"];
        let va101 = series(&channels["va:101"]);
        for (key, tol) in run["tolerance"].as_object().unwrap() {
            let tol = tol.as_f64().unwrap();
            let r = series(&channels[key]);
            let o = ours
                .channels
                .get(key)
                .unwrap_or_else(|| panic!("{id}: tellegen records no channel {key}"));
            let angle = key.starts_with("va:") || key.starts_with("delta:");
            let d = idx
                .iter()
                .zip(&r)
                .zip(&va101)
                .map(|((&k, r), v101)| {
                    let shift = if angle {
                        ours.channels["va:101"][k] - v101
                    } else {
                        0.0
                    };
                    (o[k] - r - shift).abs()
                })
                .fold(0.0_f64, f64::max);
            compared += 1;
            if report() {
                println!("{id} {event} {key}: {d:.2e} (bound {tol:.1e})");
            }
            if !(d <= tol) {
                failures.push(format!("{id} {event} {key}: {d:.2e} > {tol:.1e}"));
            }
        }
    }
    // Six machine combinations with eight channels each, the plant's three
    // events with twelve.
    assert_eq!(compared, 12 * 8 + 3 * 12);
    assert!(failures.is_empty(), "{failures:#?}");
}

/// The WECC 240 raw file with the loads as the tutorial system stores them:
/// every load record's constant power set to the tutorial's value (MW, Mvar)
/// and its current and admittance parts to zero, nothing else changed.
fn wecc240_tutorial(loads: &[Value]) -> powerio::BalancedNetwork {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/rms_examples/wecc240");
    let raw = std::fs::read_to_string(dir.join("case.raw")).unwrap();
    let mut by_bus: BTreeMap<usize, std::collections::VecDeque<(f64, f64)>> = BTreeMap::new();
    for l in loads {
        let l = l.as_array().unwrap();
        by_bus
            .entry(l[0].as_u64().unwrap() as usize)
            .or_default()
            .push_back((l[1].as_f64().unwrap(), l[2].as_f64().unwrap()));
    }
    let mut lines: Vec<String> = raw.split('\n').map(str::to_owned).collect();
    let start = lines
        .iter()
        .position(|l| l.contains("Begin Load data"))
        .unwrap()
        + 1;
    let end = lines
        .iter()
        .position(|l| l.contains("End of Load data"))
        .unwrap();
    for line in &mut lines[start..end] {
        let mut f: Vec<String> = line.split(',').map(str::to_owned).collect();
        let bus: usize = f[0].trim().parse().unwrap();
        let (p, q) = by_bus.get_mut(&bus).unwrap().pop_front().unwrap();
        f[5] = format!("{p:18.10}");
        f[6] = format!("{q:18.10}");
        for x in &mut f[7..=10] {
            *x = "     0.000".into();
        }
        *line = f.join(",");
    }
    assert!(by_bus.values().all(std::collections::VecDeque::is_empty));
    let source = powerio::Source::from_memory("case.raw", lines.join("\n").into_bytes()).unwrap();
    let module = powerio::parse(source).unwrap();
    let powerio::PioValue::BalancedNetwork(network) = module.value() else {
        panic!("not a balanced network")
    };
    network.clone()
}

/// The largest difference of a quantity tellegen may show against
/// PowerSimulationsDynamics.jl here (pu, P and Q on 100 MVA, angles in rad):
/// tellegen's own double-precision network at its 5 ms step against the
/// reference, about twice what this test observed.
const WECC_BOUNDS: &[(&str, f64)] = &[
    ("vm", 3e-5),    // observed 1.2e-5
    ("va", 1e-4),    // 5.1e-5
    ("omega", 7e-6), // 3.2e-6
    ("delta", 5e-4), // 2.2e-4
    ("pe", 8e-3),    // 3.8e-3
    ("qe", 3e-3),    // 1.2e-3
    ("tm", 7e-6),    // 3.4e-6
    ("efd", 1e-3),   // 5.3e-4
    ("plant", 1e-4), // 4.3e-5
];

/// The largest difference of a system response quantity tellegen may show
/// (speeds pu, angle rad, voltages pu), about twice what this test observed.
const RESPONSE_BOUNDS: &[(&str, f64)] = &[
    ("omega_min", 1.2e-6),    // observed 5.5e-7
    ("omega_max", 1.2e-6),    // 5.9e-7
    ("omega_mean_end", 1e-9), // 3.1e-10
    ("delta_swing", 5e-5),    // 2.4e-5
    ("vm_min", 1.5e-5),       // 6.9e-6
    ("vm_end_min", 3e-6),     // 1.3e-6
];

/// The response of the whole system as the reference states it: lowest and
/// highest machine speed, mean speed at the end, largest rotor angle swing
/// relative to `delta_ref`, lowest bus voltage over the run and at the end. A
/// tripped unit's channels end at its trip.
fn response(
    ours: &tellegen::rms::RmsResult,
    keys: &Value,
    delta_ref: &str,
    tripped: &BTreeMap<String, f64>,
) -> BTreeMap<&'static str, f64> {
    let keys = |q: &str| -> Vec<String> { serde_json::from_value(keys[q].clone()).unwrap() };
    let series = |k: &str| -> Vec<f64> {
        let v = &ours.channels[k];
        match tripped.get(k.split_once(':').unwrap().1) {
            Some(&t) => v
                .iter()
                .zip(&ours.time)
                .take_while(|(_, s)| **s <= t + 1e-9)
                .map(|(x, _)| *x)
                .collect(),
            None => v.clone(),
        }
    };
    let omega = keys("omega");
    let speeds: Vec<Vec<f64>> = omega.iter().map(|k| series(k)).collect();
    let in_service: Vec<&Vec<f64>> = omega
        .iter()
        .zip(&speeds)
        .filter(|(k, _)| !tripped.contains_key(k.split_once(':').unwrap().1))
        .map(|(_, s)| s)
        .collect();
    let reference_angle = series(delta_ref);
    let mut swing = 0.0_f64;
    for k in keys("delta") {
        let rel: Vec<f64> = series(&k)
            .iter()
            .zip(&reference_angle)
            .map(|(d, r)| d - r)
            .collect();
        swing = rel.iter().fold(swing, |m, x| m.max((x - rel[0]).abs()));
    }
    let voltages: Vec<Vec<f64>> = keys("vm").iter().map(|k| series(k)).collect();
    let fold = |v: &[Vec<f64>], f: fn(f64, f64) -> f64, init: f64| {
        v.iter().flatten().copied().fold(init, f)
    };
    BTreeMap::from([
        ("omega_min", fold(&speeds, f64::min, f64::INFINITY)),
        ("omega_max", fold(&speeds, f64::max, f64::NEG_INFINITY)),
        (
            "omega_mean_end",
            in_service.iter().map(|s| s[s.len() - 1]).sum::<f64>() / in_service.len() as f64,
        ),
        ("delta_swing", swing),
        ("vm_min", fold(&voltages, f64::min, f64::INFINITY)),
        (
            "vm_end_min",
            voltages
                .iter()
                .map(|s| s[s.len() - 1])
                .fold(f64::INFINITY, f64::min),
        ),
    ])
}

fn quantity(key: &str) -> &str {
    let q = key.split(':').next().unwrap();
    if q.starts_with("inv.") {
        "plant"
    } else {
        q
    }
}

#[test]
fn wecc240_scenarios_against_psid() {
    let reference = reference();
    let wecc = &reference["wecc240"];
    let network = wecc240_tutorial(wecc["loads_mw_mvar"].as_array().unwrap());
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/rms_examples/wecc240");
    let dyr = powerio::parse_dyr(&std::fs::read_to_string(dir.join("case.dyr")).unwrap()).unwrap();
    // The renewable plants as PowerSimulationsDynamics.jl 0.16.2 has them
    // (its low-voltage active current gain), as in the validation study.
    let options = RmsOptions {
        psid_lv_gain: true,
        ..RmsOptions::default()
    };
    let sim =
        RmsSimulation::with_options(&network, &dyr, &[], LoadModel::ConstantImpedance, options)
            .unwrap();
    let bounds: BTreeMap<&str, f64> = WECC_BOUNDS.iter().copied().collect();
    let response_bounds: BTreeMap<&str, f64> = RESPONSE_BOUNDS.iter().copied().collect();
    let mut response_failures = Vec::new();
    let mut worst: BTreeMap<String, (f64, String)> = BTreeMap::new();
    for run in wecc["runs"].as_array().unwrap() {
        let scenario = run["scenario"].as_str().unwrap();
        let mut events = Vec::new();
        let mut tripped: BTreeMap<String, f64> = BTreeMap::new();
        for e in run["events"].as_array().unwrap() {
            let event: RmsEvent = serde_json::from_value(e.clone()).unwrap();
            if let RmsEvent::GeneratorTrip {
                time,
                bus,
                machine_id,
            } = &event
            {
                let unit = if machine_id == "1" {
                    bus.to_string()
                } else {
                    format!("{bus}#{machine_id}")
                };
                tripped.insert(unit, *time);
            }
            events.push(event);
        }
        let ours = sim
            .run(&RmsRequest {
                t_end: 20.0,
                dt: 0.005,
                output_step: 0.01,
                load_model: LoadModel::ConstantImpedance,
                events,
                small_signal: false,
                record: None,
            })
            .unwrap();
        let time = series(&run["time"]);
        let idx: Vec<usize> = time.iter().map(|t| (t / 0.01).round() as usize).collect();
        let channels = &run["channels"];
        let va_ref = run["angle_reference"]["va"].as_str().unwrap();
        let delta_ref = run["angle_reference"]["delta"].as_str().unwrap();
        for (key, r) in channels.as_object().unwrap() {
            if key == va_ref || key == delta_ref {
                continue;
            }
            let r = series(r);
            let o = ours
                .channels
                .get(key.as_str())
                .unwrap_or_else(|| panic!("{scenario}: tellegen records no channel {key}"));
            let base = if key.starts_with("va:") {
                Some(va_ref)
            } else if key.starts_with("delta:") {
                Some(delta_ref)
            } else {
                None
            };
            let unit = key.split_once(':').unwrap().1;
            let until = tripped
                .get(unit)
                .filter(|_| !key.starts_with("vm:") && !key.starts_with("va:"));
            let mut d = 0.0_f64;
            for (s, (&k, r)) in idx.iter().zip(&r).enumerate() {
                if until.is_some_and(|&t| time[s] > t + 1e-9) {
                    break;
                }
                let e = match base {
                    Some(b) => {
                        let rb = series(&channels[b])[s];
                        wrap((o[k] - ours.channels[b][k]) - (r - rb))
                    }
                    None => o[k] - r,
                };
                d = d.max(e.abs());
            }
            if report() {
                println!("{scenario} {key}: {d:.2e}");
            }
            let q = quantity(key).to_owned();
            let entry = worst.entry(q).or_insert((0.0, String::new()));
            if !(d <= entry.0) {
                *entry = (d, format!("{scenario} {key}"));
            }
        }
        // The response of the whole system, on the 10 ms grid.
        let ours_response = response(&ours, &run["response_channels"], delta_ref, &tripped);
        for (name, theirs) in run["response"].as_object().unwrap() {
            let d = (ours_response[name.as_str()] - theirs.as_f64().unwrap()).abs();
            let bound = response_bounds[name.as_str()];
            println!("{scenario} {name}: {d:.2e} (bound {bound:.0e})");
            if !(d <= bound) {
                response_failures.push(format!("{scenario} {name}: {d:.2e} > {bound:.0e}"));
            }
        }
    }
    assert!(response_failures.is_empty(), "{response_failures:#?}");
    let mut failures = Vec::new();
    for (q, (d, at)) in &worst {
        let bound = bounds[q.as_str()];
        println!("{q:6} {d:.2e} at {at} (bound {bound:.0e})");
        if !(d <= &bound) {
            failures.push(format!("{q}: {d:.2e} at {at} > {bound:.0e}"));
        }
    }
    // Every quantity was compared.
    assert_eq!(
        worst.keys().map(String::as_str).collect::<Vec<_>>(),
        bounds.keys().copied().collect::<Vec<_>>()
    );
    assert!(failures.is_empty(), "{failures:#?}");
}
