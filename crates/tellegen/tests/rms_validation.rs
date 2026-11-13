//! RMS validation against PowerSimulationsDynamics.jl, an open-source tool,
//! on its own example cases, and checks of the engine on the WECC 240 case.
//!
//! The PowerSimulationsDynamics.jl comparison runs every network and dynamic
//! data file of its `test/benchmarks/psse` in both tools. Those cases and the
//! references (PowerSimulationsDynamics.jl 0.16.2 trajectories) are data, kept outside this repository: the tests
//! read them from `$TELLEGEN_CASE_DATA/rms_validation` and
//! `$TELLEGEN_CASE_DATA/psd-reference/refs` and are skipped without them
//! (`TELLEGEN_REQUIRE_CASE_DATA=1` makes that a failure).
//!
//! - Stage A (single-machine infinite-bus ladder and the IEEE 14-bus system):
//!   every channel shared with PowerSimulationsDynamics.jl within 1e-3.
//! - Stage B (the remaining benchmark cases, one model each): the same
//!   comparison, reported but not gated on the trajectories.
//!
//! The comparison is in the time domain. No closed-source tool's output is
//! used.

#![cfg(feature = "sensitivity")]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use tellegen::rms::{LoadModel, RmsEvent, RmsRequest, RmsSimulation};

/// Stage A tolerance on every channel shared with PowerSimulationsDynamics.jl.
const PSD_TOL: f64 = 1e-3;

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/support/smib.rs"
));

/// A folder of the case data repository (`$TELLEGEN_CASE_DATA`), or `None`
/// with a line on stderr when it is not on this machine.
fn case_data(sub: &str) -> Option<PathBuf> {
    let found = std::env::var_os("TELLEGEN_CASE_DATA")
        .map(|d| PathBuf::from(d).join(sub))
        .filter(|p| p.is_dir());
    if found.is_none() {
        assert!(
            std::env::var_os("TELLEGEN_REQUIRE_CASE_DATA").is_none(),
            "case data `{sub}` is required (TELLEGEN_REQUIRE_CASE_DATA) but not found"
        );
        eprintln!("skipped: case data `{sub}` is not on this machine");
    }
    found
}

/// PowerSimulationsDynamics.jl's PSS/E benchmark cases.
fn data_dir() -> Option<PathBuf> {
    case_data("rms_validation")
}

/// PowerSimulationsDynamics.jl's results on them.
fn psd_dir() -> Option<PathBuf> {
    case_data("psd-reference/refs")
}

/// The WECC 240 case that ships with the code.
fn wecc240_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/rms_examples/wecc240")
}

fn load_model(s: &str) -> LoadModel {
    match s {
        "constant_current" => LoadModel::ConstantCurrent,
        "constant_power" => LoadModel::ConstantPower,
        _ => LoadModel::ConstantImpedance,
    }
}

/// Max abs error between a reference sampled at `t_ref` and tellegen's
/// series on its 5 ms grid, matching each reference sample to the nearest
/// tellegen sample. Samples within one step of an event are skipped: the
/// references disagree on whether they hold the pre- or post-event value.
fn compare(
    time: &[f64],
    ours: &[f64],
    t_ref: &[f64],
    reference: &[f64],
    events: &[f64],
) -> (f64, f64) {
    let dt = time[1] - time[0];
    let mut worst = 0.0_f64;
    let mut at = 0.0;
    for (t, r) in t_ref.iter().zip(reference) {
        if events.iter().any(|e| (t - e).abs() < 1.5 * dt) {
            continue;
        }
        let k = ((t - time[0]) / dt).round() as isize;
        if k < 0 || k as usize >= ours.len() {
            continue;
        }
        let err = (ours[k as usize] - r).abs();
        if err.is_nan() {
            return (f64::INFINITY, *t);
        }
        if err > worst {
            worst = err;
            at = *t;
        }
    }
    (worst, at)
}

#[test]
fn rms_validation() {
    let (Some(data), Some(refs)) = (data_dir(), psd_dir()) else {
        return;
    };
    let manifest: Value =
        serde_json::from_str(&std::fs::read_to_string(data.join("cases.json")).unwrap()).unwrap();
    let only: Option<Vec<String>> = std::env::var("RMS_CASES")
        .ok()
        .map(|s| s.split(',').map(str::to_owned).collect());
    let mut summary: Vec<Value> = Vec::new();
    let mut failures: Vec<String> = Vec::new();
    for case in manifest["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        if only.as_ref().is_some_and(|o| !o.iter().any(|x| x == id)) {
            continue;
        }
        let dir = data.join(id);
        let module = powerio::parse(powerio::Source::open(dir.join("case.raw")).unwrap()).unwrap();
        let powerio::PioValue::BalancedNetwork(network) = module.value() else {
            panic!("{id}: not a balanced network");
        };
        let dyr =
            powerio::parse_dyr(&std::fs::read_to_string(dir.join("case.dyr")).unwrap()).unwrap();
        let lm = load_model(case["load_model"].as_str().unwrap_or("constant_impedance"));
        let ev = &case["event"];
        let event: RmsEvent = serde_json::from_value(ev.clone()).unwrap();
        let event_time = event.time();
        let request = RmsRequest {
            t_end: case["t_end"].as_f64().unwrap(),
            dt: std::env::var("RMS_DT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.0025),
            output_step: 0.005,
            load_model: lm,
            events: vec![event],
            small_signal: false,
            record: None,
        };
        let sim = match RmsSimulation::new(network, &dyr, lm) {
            Ok(sim) => sim,
            Err(e) => {
                failures.push(format!("{id}: initialization failed: {e}"));
                continue;
            }
        };
        let result = match sim.run(&request) {
            Ok(r) => r,
            Err(e) => {
                failures.push(format!("{id}: simulation failed: {e}"));
                continue;
            }
        };

        // PowerSimulationsDynamics.jl, every shared channel.
        let mut psd_metrics: BTreeMap<String, Value> = BTreeMap::new();
        if let Ok(text) = std::fs::read_to_string(refs.join(format!("{id}.json"))) {
            let psd: Value = serde_json::from_str(&text).unwrap();
            let t_ref: Vec<f64> = serde_json::from_value(psd["time"].clone()).unwrap();
            // After a trip, PowerSimulationsDynamics.jl's branch-flow post-processing
            // still evaluates the opened line's pi model; its flows are not compared.
            let tripped = match &request.events[0] {
                RmsEvent::BranchTrip {
                    from, to, circuit, ..
                } => {
                    if circuit == "1" {
                        format!(":{from}-{to}")
                    } else {
                        format!(":{from}-{to}#{circuit}")
                    }
                }
                _ => String::new(),
            };
            let excluded = case["psd_exclude"].as_object().cloned().unwrap_or_default();
            for (key, series) in psd["channels"].as_object().unwrap() {
                if (!tripped.is_empty() && key.ends_with(&tripped)) || excluded.contains_key(key) {
                    continue;
                }
                let Some(ours) = result.channels.get(key) else {
                    continue;
                };
                let reference: Vec<f64> = serde_json::from_value(series.clone()).unwrap();
                let (err, at) = compare(&result.time, ours, &t_ref, &reference, &[event_time]);
                psd_metrics.insert(key.clone(), json!({"max_abs_error": err, "at": at}));
                if case["stage"] == "A" && err > PSD_TOL {
                    failures.push(format!("{id}: {key} differs from PowerSimulationsDynamics.jl by {err:.3e} at t = {at:.3} s"));
                }
            }
        }

        let record = json!({
            "id": id,
            "stage": case["stage"],
            "rung": case["rung"],
            "title": case["title"],
            "models": case["models"],
            "steps": result.steps,
            "newton_iterations": result.newton_iterations,
            "psd": psd_metrics,
            "notes": result.notes,
        });
        summary.push(record);
    }
    for s in &summary {
        let psd_worst = s["psd"].as_object().map_or(0.0, |m| {
            m.values()
                .map(|v| v["max_abs_error"].as_f64().unwrap())
                .fold(0.0, f64::max)
        });
        eprintln!(
            "{:<26} stage {} psd {:>9.2e}",
            s["id"].as_str().unwrap(),
            s["stage"].as_str().unwrap(),
            psd_worst
        );
    }
    assert!(
        failures.is_empty(),
        "{} validation failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// The browser path: the case reaches the engine as PowerIO IR JSON, which
/// must keep the PSS/E machine ids and ZSORCE the DYR linking reads, and
/// give the same trajectory as the direct path.
#[test]
fn module_json_path_matches_direct_path() {
    // GENROU + SEXS + TGOV1 against an infinite bus, built from WECC 240.
    let case = &wecc240_smib_cases()[0];
    let source = powerio::Source::from_memory("case.raw", case.raw.as_bytes().to_vec()).unwrap();
    let module = powerio::parse(source).unwrap();
    let module_json = tellegen::ir::serialize_module(&module).unwrap();
    let dyr = case.dyr.clone();
    let request = RmsRequest {
        t_end: 3.0,
        events: vec![
            RmsEvent::BusFault {
                time: 1.0,
                bus: 102,
                r: 0.0,
                x: case.x_line / 2.0,
            },
            RmsEvent::ClearFault {
                time: 1.1,
                bus: 102,
            },
        ],
        ..RmsRequest::default()
    };
    let via_json: tellegen::rms::RmsResult = serde_json::from_str(
        &tellegen::rms::simulate_module_json(
            &module_json,
            &dyr,
            &serde_json::to_string(&request).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let powerio::PioValue::BalancedNetwork(network) = module.value() else {
        panic!()
    };
    let direct = tellegen::rms::simulate(network, &dyr, &request).unwrap();
    assert_eq!(via_json.generators.len(), 1);
    assert_eq!(via_json.sources.len(), 1);
    for (key, series) in &direct.channels {
        let other = &via_json.channels[key];
        let worst = series
            .iter()
            .zip(other)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f64::max);
        assert!(worst < 1e-12, "{key} differs by {worst}");
    }
    let inspection: tellegen::rms::RmsInspection = serde_json::from_str(
        &tellegen::rms::inspect_module_json(&module_json, &dyr, LoadModel::ConstantImpedance)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(inspection.dyr_records, 4);
    assert!(
        inspection.eigenvalues.iter().all(|e| e[0] < 1e-6),
        "stable operating point"
    );
    assert!(tellegen::rms::looks_like_dyr(&dyr));
    assert!(!tellegen::rms::looks_like_dyr(
        "0, 100.0, 33 / not a dyr file"
    ));
}

/// Every single branch trip of the IEEE 14-bus system runs to completion,
/// including line 1-2, after which the slack machine's ESAC1A (KC = 0.2)
/// chatters between its rectifier and limiter bounds.
#[test]
fn ieee14_every_branch_trip_completes() {
    let Some(data) = data_dir() else { return };
    let dir = data.join("ieee14");
    let module = powerio::parse(powerio::Source::open(dir.join("case.raw")).unwrap()).unwrap();
    let powerio::PioValue::BalancedNetwork(network) = module.value() else {
        panic!()
    };
    let dyr = powerio::parse_dyr(&std::fs::read_to_string(dir.join("case.dyr")).unwrap()).unwrap();
    let sim = RmsSimulation::new(network, &dyr, LoadModel::ConstantImpedance).unwrap();
    for (from, to, circuit) in sim.branches() {
        let request = RmsRequest {
            t_end: 10.0,
            events: vec![RmsEvent::BranchTrip {
                time: 1.0,
                from,
                to,
                circuit: circuit.clone(),
            }],
            small_signal: false,
            ..RmsRequest::default()
        };
        let r = sim
            .run(&request)
            .unwrap_or_else(|e| panic!("trip {from}-{to}: {e}"));
        assert!(r.channels["omega:1"].iter().all(|w| w.is_finite()));
    }
}

/// Small-signal analysis: modes match the eigenvalues, participation sums to
/// one, and a frequency response of the SMIB exciter loop is finite.
#[test]
fn linear_analysis_smib() {
    let (network, dyr) = smib_study(&wecc240_smib_cases()[0]);
    let sim = RmsSimulation::new(&network, &dyr, LoadModel::ConstantImpedance).unwrap();
    let request: tellegen::rms::LinearRequest = serde_json::from_value(serde_json::json!({
        "bode": [{"input_bus": 102, "input": "v_ref", "output": "vm", "output_bus": 102},
                 {"input_bus": 102, "input": "p_ref", "output": "omega", "output_bus": 102}],
        "points": 50
    }))
    .unwrap();
    let lin = sim.linear_analysis(&request).unwrap();
    assert_eq!(lin.modes.len(), sim.eigenvalues().unwrap().len());
    for m in &lin.modes {
        let total: f64 = m.participation.iter().map(|p| p.factor).sum();
        assert!(total <= 1.0 + 1e-9);
    }
    let swing = lin
        .modes
        .iter()
        .find(|m| m.freq_hz > 0.3)
        .expect("an electromechanical mode");
    assert!(
        swing
            .participation
            .iter()
            .any(|p| p.state.contains("omega") || p.state.contains("delta")),
        "{:?}",
        swing.participation
    );
    // Static gain of V_ref -> |V| is positive and finite: the low-frequency phase is near 0.
    let v = &lin.bode[0];
    assert!(v.mag_db.iter().all(|m| m.is_finite()));
    assert!(v.phase_deg[0].abs() < 30.0, "{}", v.phase_deg[0]);
    eprintln!(
        "swing mode {:.3} Hz zeta {:.3}: {:?}",
        swing.freq_hz,
        swing.damping,
        swing.participation.iter().take(3).collect::<Vec<_>>()
    );
}

/// The IEEE 14-bus line 1-2 trip, after which the slack machine's ESAC1A
/// (KC = 0.2) chatters between its rectifier and limiter bounds, against
/// PowerSimulationsDynamics.jl (reference generated by `generate.jl` with
/// IDA, retried without dtmax as its own EXST1 test does).
#[test]
fn ieee14_line_1_2_against_psd() {
    let (Some(data), Some(refs)) = (data_dir(), psd_dir()) else {
        return;
    };
    let psd: Value =
        serde_json::from_str(&std::fs::read_to_string(refs.join("ieee14_line12.json")).unwrap())
            .unwrap();
    let dir = data.join("ieee14");
    let module = powerio::parse(powerio::Source::open(dir.join("case.raw")).unwrap()).unwrap();
    let powerio::PioValue::BalancedNetwork(network) = module.value() else {
        panic!()
    };
    let r = tellegen::rms::simulate(
        network,
        &std::fs::read_to_string(dir.join("case.dyr")).unwrap(),
        &RmsRequest {
            t_end: 10.0,
            events: vec![RmsEvent::BranchTrip {
                time: 1.0,
                from: 1,
                to: 2,
                circuit: "1".into(),
            }],
            small_signal: false,
            ..RmsRequest::default()
        },
    )
    .unwrap();
    let t_ref: Vec<f64> = serde_json::from_value(psd["time"].clone()).unwrap();
    let mut worst_v: f64 = 0.0;
    let mut worst_w: f64 = 0.0;
    for (key, series) in psd["channels"].as_object().unwrap() {
        let Some(ours) = r.channels.get(key) else {
            continue;
        };
        let reference: Vec<f64> = serde_json::from_value(series.clone()).unwrap();
        let (err, _) = compare(&r.time, ours, &t_ref, &reference, &[1.0]);
        if key.starts_with("vm:") {
            worst_v = worst_v.max(err);
        } else if key.starts_with("omega:") {
            worst_w = worst_w.max(err);
        }
    }
    eprintln!("IEEE 14 line 1-2: max |ΔV| {worst_v:.2e}, max |Δω| {worst_w:.2e}");
    assert!(worst_v < 1e-3 && worst_w < 1e-4);
}

/// A live session reproduces the batch run when the same events are applied
/// at the same times.
#[test]
fn live_session_matches_batch_run() {
    let dir = wecc240_dir();
    let module = powerio::parse(powerio::Source::open(dir.join("case.raw")).unwrap()).unwrap();
    let powerio::PioValue::BalancedNetwork(network) = module.value() else {
        panic!()
    };
    let dyr = powerio::parse_dyr(&std::fs::read_to_string(dir.join("case.dyr")).unwrap()).unwrap();
    let sim = RmsSimulation::new(network, &dyr, LoadModel::ConstantImpedance).unwrap();
    let mut live = sim.live(0.0025, 0.005);
    let mut time = Vec::new();
    let mut omega = Vec::new();
    let mut take = |c: tellegen::rms::RmsChunk| {
        time.extend(c.time);
        omega.extend(c.channels["omega:1032#C"].clone());
    };
    take(live.advance(1.0).unwrap());
    live.apply(RmsEvent::LoadChange {
        time: 0.0,
        bus: 1101,
        scale: 1.1,
    })
    .unwrap();
    take(live.advance(0.5).unwrap());
    live.apply(RmsEvent::SetReference {
        time: 0.0,
        bus: 1032,
        machine_id: "C".into(),
        signal: tellegen::rms::InputSignal::VRef,
        value: 1.03,
    })
    .unwrap();
    take(live.advance(1.5).unwrap());
    let batch = sim
        .run(&RmsRequest {
            t_end: 3.0,
            events: live.event_log().to_vec(),
            small_signal: false,
            ..RmsRequest::default()
        })
        .unwrap();
    assert_eq!(time.len(), batch.time.len(), "sample count");
    let worst = omega
        .iter()
        .zip(&batch.channels["omega:1032#C"])
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f64::max);
    assert!(worst < 1e-9, "live and batch differ by {worst}");
}

/// The WECC 240-bus case initializes and runs through a fault: every value
/// finite, every channel one series on the time grid (a line and a
/// transformer between the same buses must not share a key).
#[test]
fn wecc240_runs() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/rms_examples/wecc240");
    let module = powerio::parse(powerio::Source::open(dir.join("case.raw")).unwrap()).unwrap();
    let powerio::PioValue::BalancedNetwork(network) = module.value() else {
        panic!()
    };
    let dyr = powerio::parse_dyr(&std::fs::read_to_string(dir.join("case.dyr")).unwrap()).unwrap();
    let sim = RmsSimulation::new(network, &dyr, LoadModel::ConstantImpedance).unwrap();
    let branch = sim.branches()[10].clone();
    let r = sim
        .run(&RmsRequest {
            t_end: 3.0,
            events: vec![
                RmsEvent::BusFault {
                    time: 1.0,
                    bus: branch.0,
                    r: 0.0,
                    x: 0.01,
                },
                RmsEvent::ClearFault {
                    time: 1.1,
                    bus: branch.0,
                },
            ],
            small_signal: false,
            ..RmsRequest::default()
        })
        .unwrap();
    assert!(r.channels.values().all(|v| v.iter().all(|x| x.is_finite())));
    assert!(
        r.channels.values().all(|v| v.len() == r.time.len()),
        "a channel's length differs from the time grid"
    );
}

/// The frequency response agrees with the nonlinear simulation: its
/// low-frequency gain V_ref → |V| equals the steady-state change after a small
/// V_ref step, and its resonance sits at the swing mode's frequency.
#[test]
fn frequency_response_matches_time_domain() {
    let (network, dyr) = smib_study(&wecc240_smib_cases()[0]);
    let sim = RmsSimulation::new(&network, &dyr, LoadModel::ConstantImpedance).unwrap();
    let lin = sim
        .linear_analysis(&serde_json::from_value(serde_json::json!({
            "bode": [{"input_bus": 102, "input": "v_ref", "output": "vm", "output_bus": 102},
                     {"input_bus": 102, "input": "p_ref", "output": "omega", "output_bus": 102}],
            "f_min": 0.0005, "f_max": 5.0, "points": 400
        }))
        .unwrap())
        .unwrap();
    let v_ref = sim
        .run(&RmsRequest {
            t_end: 0.01,
            ..RmsRequest::default()
        })
        .unwrap()
        .generators[0]
        .v_ref;
    let step = 1e-4;
    let r = sim
        .run(&RmsRequest {
            t_end: 60.0,
            dt: 0.005,
            output_step: 0.01,
            events: vec![RmsEvent::SetReference {
                time: 1.0,
                bus: 102,
                machine_id: "1".into(),
                signal: tellegen::rms::InputSignal::VRef,
                value: v_ref + step,
            }],
            small_signal: false,
            ..RmsRequest::default()
        })
        .unwrap();
    let vm = &r.channels["vm:102"];
    let gain_td = (vm[vm.len() - 1] - vm[0]) / step;
    let gain_fr = 10f64.powf(lin.bode[0].mag_db[0] / 20.0)
        * lin.bode[0].phase_deg[0].to_radians().cos().signum();
    eprintln!(
        "V_ref -> |V|: static gain from the step {gain_td:.6}, from H(j·0.0005 Hz) {gain_fr:.6}"
    );
    assert!(
        (gain_td - gain_fr).abs() < 1e-3 * gain_td.abs().max(1e-3),
        "{gain_td} vs {gain_fr}"
    );
    // Resonance of P_ref -> ω at the swing mode (searched above 0.1 Hz).
    let curve = &lin.bode[1];
    let peak = (1..curve.mag_db.len())
        .filter(|&k| curve.freq_hz[k] > 0.1)
        .max_by(|&a, &b| curve.mag_db[a].total_cmp(&curve.mag_db[b]))
        .unwrap();
    let swing = lin
        .modes
        .iter()
        .filter(|m| m.participation.iter().any(|p| p.state.ends_with("delta")))
        .map(|m| m.freq_hz)
        .fold(0.0, f64::max);
    eprintln!(
        "P_ref -> ω: peak at {:.3} Hz, swing mode {swing:.3} Hz",
        curve.freq_hz[peak]
    );
    assert!((curve.freq_hz[peak] - swing).abs() < 0.1 * swing);
}
