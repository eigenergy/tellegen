//! The screening of runs (`tellegen::rms::screen`): verdicts on a single
//! machine against an infinite bus, on the WECC 240 validation scenarios,
//! and on an island that runs away.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::Value;
use tellegen::rms::{
    LoadModel, LowVoltageLoad, RecordSpec, RmsEvent, RmsOptions, RmsRequest, RmsSimulation,
    ScreeningCriteria, ScreeningVerdict,
};

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/support/smib.rs"
));
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/support/wecc240.rs"
));

fn smib() -> Option<(RmsSimulation, f64)> {
    shipped_wecc240()?;
    let case = &wecc240_smib_cases()[0];
    let (net, dyr) = smib_study(case);
    Some((
        RmsSimulation::new(&net, &dyr, LoadModel::PowerFlow).unwrap(),
        case.x_line,
    ))
}

fn fault_request(clear_after: f64, x: f64) -> RmsRequest {
    RmsRequest {
        t_end: 8.0,
        dt: 0.0025,
        output_step: 0.01,
        events: vec![
            RmsEvent::BusFault {
                time: 0.5,
                bus: 102,
                r: 0.0,
                x,
            },
            RmsEvent::ClearFault {
                time: 0.5 + clear_after,
                bus: 102,
            },
        ],
        record: Some(RecordSpec::display()),
        ..RmsRequest::default()
    }
}

/// A run's verdict, or loss of synchronism when the integration itself
/// fails after the machine slips (the run then has no samples to screen).
fn verdict(sim: &RmsSimulation, req: &RmsRequest) -> ScreeningVerdict {
    match sim.run(req) {
        Ok(r) => {
            sim.screen(req, &r, &ScreeningCriteria::default())
                .unwrap()
                .verdict
        }
        Err(e) => panic!("run failed: {e}"),
    }
}

#[test]
fn a_fast_clearing_is_stable_and_a_slow_one_slips_a_pole() {
    let Some((sim, x_line)) = smib() else { return };
    // A bolted-like fault at the unit's terminal.
    let x = 0.01 * x_line;
    let fast = fault_request(0.05, x);
    let r = sim.run(&fast).unwrap();
    let s = sim
        .screen(&fast, &r, &ScreeningCriteria::default())
        .unwrap();
    assert_eq!(s.verdict, ScreeningVerdict::Stable, "{:#?}", s.reasons);
    assert!(s.angle.pole_slips.is_empty());
    assert!(s.angle.first_swing_deg > 1.0, "{}", s.angle.first_swing_deg);
    assert_eq!(s.criteria_name, "tellegen-rms-screening-1");

    let slow = fault_request(0.5, x);
    let r = sim.run(&slow).unwrap();
    let s = sim
        .screen(&slow, &r, &ScreeningCriteria::default())
        .unwrap();
    assert_eq!(
        s.verdict,
        ScreeningVerdict::LossOfSynchronism,
        "{:#?}",
        s.reasons
    );
    assert_eq!(s.angle.pole_slips[0].unit, "102");
    assert!(s.angle.pole_slips[0].t > 0.5);
}

#[test]
fn the_verdict_changes_once_with_the_clearing_time() {
    let Some((sim, x_line)) = smib() else { return };
    let x = 0.01 * x_line;
    let stable =
        |c: f64| verdict(&sim, &fault_request(c, x)) != ScreeningVerdict::LossOfSynchronism;
    // Bisect the critical clearing time between 0.05 s and 0.5 s.
    let (mut lo, mut hi) = (0.05, 0.5);
    assert!(stable(lo) && !stable(hi));
    for _ in 0..6 {
        let mid = 0.5 * (lo + hi);
        if stable(mid) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    // Monotonic: every clearing before the critical time holds, every one
    // after slips.
    for k in 0..=8 {
        let c = 0.05 + (lo - 0.05) * f64::from(k) / 8.0;
        assert!(stable(c), "clearing at {c} s (critical {lo}..{hi})");
    }
    for k in 0..=4 {
        let c = hi + (0.5 - hi) * f64::from(k) / 4.0;
        assert!(!stable(c), "clearing at {c} s (critical {lo}..{hi})");
    }
}

#[test]
fn screening_names_the_channels_it_lacks() {
    let Some((sim, x_line)) = smib() else { return };
    let mut req = fault_request(0.05, 0.01 * x_line);
    req.t_end = 1.0;
    req.record = Some(RecordSpec {
        channels: vec!["vm:".into(), "omega:".into()],
        ..RecordSpec::default()
    });
    let r = sim.run(&req).unwrap();
    let e = sim
        .screen(&req, &r, &ScreeningCriteria::default())
        .unwrap_err();
    assert!(e.contains("delta:") && e.contains("va:"), "{e}");
}

#[test]
fn derived_frequencies_rest_at_nominal_without_events() {
    let Some((sim, _)) = smib() else { return };
    let req = RmsRequest {
        t_end: 3.0,
        output_step: 0.01,
        record: Some(RecordSpec::display()),
        ..RmsRequest::default()
    };
    let r = sim.run(&req).unwrap();
    let (s, series) = sim
        .screen_with_series(&req, &r, &ScreeningCriteria::default())
        .unwrap();
    assert_eq!(s.verdict, ScreeningVerdict::Stable, "{:#?}", s.reasons);
    let f0 = r.base_frequency;
    for key in ["coi:f", "f:101", "f:102"] {
        assert!(
            series[key].iter().all(|f| (f - f0).abs() < 1e-6),
            "{key}: {:?}",
            &series[key][..5]
        );
    }
}

/// The WECC 240 raw file with the loads of the validation study (see
/// `rms_psid_validation.rs`).
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
    let source = powerio::Source::from_memory("case.raw", lines.join("\n").into_bytes()).unwrap();
    let module = powerio::parse(source).unwrap();
    let powerio::PioValue::BalancedNetwork(network) = module.value() else {
        panic!("not a balanced network")
    };
    network.clone()
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "four 20 s WECC 240 runs: `just rms-validation` runs them in release"
)]
fn the_validated_wecc240_scenarios_screen_stable_and_agree_with_the_reference() {
    let Some(dir) = shipped_wecc240() else { return };
    let reference: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("psid_reference.json")).unwrap())
            .unwrap();
    let wecc = &reference["wecc240"];
    let network = wecc240_tutorial(wecc["loads_mw_mvar"].as_array().unwrap());
    let dyr = powerio::parse_dyr(&std::fs::read_to_string(dir.join("case.dyr")).unwrap()).unwrap();
    // The validation study's options (docs/src/rms-validation.md).
    let options = RmsOptions {
        psid_lv_gain: true,
        reactive_limits: false,
        psid_saturation: true,
        psid_converter_ramp: true,
        low_voltage_load: LowVoltageLoad::Exact,
        ..RmsOptions::default()
    };
    let sim =
        RmsSimulation::with_options(&network, &dyr, LoadModel::ConstantImpedance, options).unwrap();
    for run in wecc["runs"].as_array().unwrap() {
        let scenario = run["scenario"].as_str().unwrap();
        let events: Vec<RmsEvent> = run["events"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| serde_json::from_value(e.clone()).unwrap())
            .collect();
        let req = RmsRequest {
            t_end: 20.0,
            dt: 0.005,
            output_step: 0.01,
            load_model: Some(LoadModel::ConstantImpedance),
            events: events.clone(),
            record: Some(RecordSpec::display()),
            ..RmsRequest::default()
        };
        let r = sim.run(&req).unwrap();
        let s = sim.screen(&req, &r, &ScreeningCriteria::default()).unwrap();
        if std::env::var_os("RMS_SCREEN_REPORT").is_some() {
            eprintln!("{scenario}: {}", serde_json::to_string_pretty(&s).unwrap());
        }
        assert_eq!(
            s.verdict,
            ScreeningVerdict::Stable,
            "{scenario}: {:#?}",
            s.reasons
        );
        assert!(s.frequency.settled && s.angle.settled, "{scenario}");
        let resp = &run["response"];
        let f0 = r.base_frequency;
        let omega_min = s.frequency.lowest_unit_hz / f0;
        let d = (omega_min - resp["omega_min"].as_f64().unwrap()).abs();
        assert!(d < 1.2e-6, "{scenario}: lowest speed differs by {d:e}");
        let d = (s.voltage.min_pu - resp["vm_min"].as_f64().unwrap()).abs();
        assert!(d < 1.5e-5, "{scenario}: lowest voltage differs by {d:e}");
        // A generator trip leaves the tripped unit out of the COI after it.
        if let Some(RmsEvent::GeneratorTrip {
            bus, machine_id, ..
        }) = events.first()
        {
            let key = format!("{bus}#{machine_id}");
            let series = sim
                .derived_series(&req, &r, &ScreeningCriteria::default())
                .unwrap();
            let k = r.time.iter().position(|&t| t > 1.5).unwrap();
            let frozen = r.channels[&format!("omega:{key}")][k];
            let coi = series["coi:omega"][k];
            // The tripped unit's frozen speed is not part of the mean: the
            // others' weighted mean equals the COI.
            let mut num = 0.0;
            let mut den = 0.0;
            for g in &r.generators {
                let gk = if g.machine_id == "1" {
                    g.bus.to_string()
                } else {
                    format!("{}#{}", g.bus, g.machine_id)
                };
                if gk == key {
                    continue;
                }
                num += g.h * g.mbase * r.channels[&format!("omega:{gk}")][k];
                den += g.h * g.mbase;
            }
            assert!((coi - num / den).abs() < 1e-12, "{coi} vs {}", num / den);
            assert!((frozen - coi).abs() > 1e-9);
        }
    }
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "a 6 s WECC 240 run: `just rms-validation` runs it in release"
)]
fn an_island_without_load_runs_away() {
    let Some(net) = shipped_wecc240_network() else {
        return;
    };
    let dir = shipped_wecc240().unwrap();
    let dyr = powerio::parse_dyr(&std::fs::read_to_string(dir.join("case.dyr")).unwrap()).unwrap();
    let sim = RmsSimulation::new(&net, &dyr, LoadModel::PowerFlow).unwrap();
    let req = RmsRequest {
        t_end: 6.0,
        dt: 0.0025,
        output_step: 0.01,
        events: vec![RmsEvent::BranchTrip {
            time: 1.0,
            from: 1004,
            to: 1034,
            circuit: "1".into(),
        }],
        record: Some(RecordSpec::display()),
        ..RmsRequest::default()
    };
    let r = sim.run(&req).unwrap();
    let s = sim.screen(&req, &r, &ScreeningCriteria::default()).unwrap();
    // The island of bus 1034 runs away; the rest of the network, which lost
    // its generation, may slip poles besides (the verdict names that first).
    assert!(
        matches!(
            s.verdict,
            ScreeningVerdict::IslandRunaway | ScreeningVerdict::LossOfSynchronism
        ),
        "{:#?}",
        s.reasons
    );
    assert!(
        s.reasons.iter().any(|r| r.code == "island_runaway"),
        "{:#?}",
        s.reasons
    );
    if std::env::var_os("RMS_SCREEN_REPORT").is_some() {
        eprintln!("{:#?}", s.angle);
    }
    assert!(s.islands.len() >= 2, "{:?}", s.islands);
    assert!(s.islands.iter().any(|i| i.runaway));
}
