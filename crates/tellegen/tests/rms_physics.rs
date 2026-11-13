//! Physics and numerics of the RMS engine, case by case:
//!
//! - the initial point is a steady state (`‖f(x0)‖∞ < 1e-8`) on every
//!   single-machine case of WECC 240 and on WECC 240 itself;
//! - the per-unit bases: a case restated on another system base simulates
//!   the same machine trajectories (the network-device frame is checked in
//!   `rms::init`'s unit tests);
//! - static loads keep the law the AC power flow solved them with (constant
//!   power, ZIP, exponential), at rest and through a voltage dip, and convert
//!   to constant admittance below the `PQBRAK` breakpoint;
//! - saturation and `ESAC1A` stay finite through a bolted fault;
//! - the trapezoidal rule is of second order (halving the step quarters the
//!   difference), which states the accuracy of the default step;
//! - events are checked before a run; small-signal analysis is off unless
//!   asked for.
//!
//! The cases are built from the shipped WECC 240 data; without it (the
//! published crate) every test here is skipped.

#![cfg(feature = "sensitivity")]
// Negated comparisons are deliberate: they treat NaN as failing.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use std::collections::BTreeMap;

use powerio::LoadVoltageModel;
use tellegen::rms::{
    LoadModel, LowVoltageLoad, RmsEvent, RmsOptions, RmsRequest, RmsResult, RmsSimulation,
};

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/support/smib.rs"
));
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/support/wecc240.rs"
));

fn network(raw: &str) -> powerio::BalancedNetwork {
    let src = powerio::Source::from_memory("case.raw", raw.as_bytes().to_vec()).unwrap();
    let module = powerio::parse(src).unwrap();
    let powerio::PioValue::BalancedNetwork(net) = module.value() else {
        panic!("not a balanced network")
    };
    net.clone()
}

fn run(sim: &RmsSimulation, t_end: f64, dt: f64, events: Vec<RmsEvent>) -> RmsResult {
    sim.run(&RmsRequest {
        t_end,
        dt,
        output_step: dt.max(0.005),
        events,
        ..RmsRequest::default()
    })
    .unwrap()
}

/// Largest difference of two series.
fn max_diff(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0, f64::max)
}

fn fault(bus: usize, x: f64, on: f64, off: f64) -> Vec<RmsEvent> {
    vec![
        RmsEvent::BusFault {
            time: on,
            bus,
            r: 0.0,
            x,
        },
        RmsEvent::ClearFault { time: off, bus },
    ]
}

/// The initialization is a steady state: every derivative and every
/// current balance below 1e-8 at `t = 0`, on every single-machine case and
/// on WECC 240 with each load model and with the reference tool's options.
#[test]
fn the_initial_point_is_a_steady_state() {
    let Some(dir) = shipped_wecc240() else { return };
    let mut worst = 0.0_f64;
    for case in wecc240_smib_cases() {
        let (net, dyr) = smib_study(&case);
        for reactive_limits in [true, false] {
            let options = RmsOptions {
                reactive_limits,
                ..RmsOptions::default()
            };
            let sim =
                RmsSimulation::with_options(&net, &dyr, LoadModel::PowerFlow, options).unwrap();
            let r = sim.initial_residual();
            assert!(r < 1e-8, "{} (limits {reactive_limits}): {r:e}", case.id);
            worst = worst.max(r);
        }
    }
    let net = shipped_wecc240_network().unwrap();
    let dyr = powerio::parse_dyr(&std::fs::read_to_string(dir.join("case.dyr")).unwrap()).unwrap();
    for model in [
        LoadModel::PowerFlow,
        LoadModel::ConstantImpedance,
        LoadModel::ConstantCurrent,
        LoadModel::ConstantPower,
    ] {
        for reactive_limits in [true, false] {
            let options = RmsOptions {
                reactive_limits,
                ..RmsOptions::default()
            };
            let sim = RmsSimulation::with_options(&net, &dyr, model, options).unwrap();
            let r = sim.initial_residual();
            if model == LoadModel::PowerFlow {
                let r0 = sim
                    .run(&RmsRequest {
                        t_end: 0.005,
                        ..RmsRequest::default()
                    })
                    .unwrap();
                let flagged: Vec<&String> = r0
                    .notes
                    .iter()
                    .filter(|n| n.starts_with("generator bus"))
                    .collect();
                eprintln!(
                    "WECC 240, reactive limits {reactive_limits}: {} generator bus(es) noted{}",
                    flagged.len(),
                    flagged
                        .first()
                        .map_or(String::new(), |n| format!(", e.g. {n}"))
                );
            }
            assert!(
                r < 1e-8,
                "WECC 240 {model:?} (limits {reactive_limits}): {r:e}"
            );
            worst = worst.max(r);
        }
    }
    eprintln!("largest initial residual {worst:.1e}");
}

/// The per-unit bases: the single-machine case stated on a 250 MVA system
/// base instead of 100 MVA (the line reactances restated, the machine data on
/// its own `MBASE` unchanged) is the same physical system, so the machine's
/// speed, angle and terminal voltage follow the same trajectory and its
/// electrical power, on the system base, is 100/250 of it. A machine-base
/// conversion applied wrong (or twice) would change the dynamics.
#[test]
fn trajectories_do_not_depend_on_the_system_base() {
    if shipped_wecc240().is_none() {
        return;
    }
    let case = &wecc240_smib_cases()[0];
    let restated = restate_base(&case.raw, 250.0);
    let events = fault(102, case.x_line / 2.0, 1.0, 1.1);
    let a = run(
        &RmsSimulation::new(&network(&case.raw), &dyr_of(case), LoadModel::PowerFlow).unwrap(),
        2.0,
        0.0025,
        events.clone(),
    );
    let b = run(
        &RmsSimulation::new(&network(&restated), &dyr_of(case), LoadModel::PowerFlow).unwrap(),
        2.0,
        0.0025,
        events.iter().map(|e| scale_fault(e, 2.5)).collect(),
    );
    for key in [
        "omega:102",
        "delta:102",
        "vm:102",
        "va:102",
        "efd:102",
        "tm:102",
    ] {
        let d = max_diff(&a.channels[key], &b.channels[key]);
        assert!(d < 1e-8, "{key}: {d:e}");
    }
    let pe_b: Vec<f64> = b.channels["pe:102"].iter().map(|p| p * 2.5).collect();
    let d = max_diff(&a.channels["pe:102"], &pe_b);
    assert!(d < 1e-8, "pe: {d:e}");
    // On the machine base the torque balances the power at rest, less the
    // stator loss: tm (MBASE) = pe (Sbase) · Sbase / MBASE + R |I|².
    let mbase = a.generators[0].mbase;
    let (tm, pe) = (a.channels["tm:102"][0], a.channels["pe:102"][0]);
    let loss = tm - pe * a.base_mva / mbase;
    assert!(
        (-1e-12..1e-3 * tm.abs().max(1.0)).contains(&loss),
        "tm {tm} vs pe {pe} on {mbase} MVA: loss {loss}"
    );
}

fn dyr_of(case: &SmibCase) -> powerio::DynamicCase {
    powerio::parse_dyr(&case.dyr).unwrap()
}

/// `raw` (a SMIB case on 100 MVA) on the system base `sbase`: the header's
/// base and the per-unit line reactances (on the system base) restated.
fn restate_base(raw: &str, sbase: f64) -> String {
    let k = sbase / 100.0;
    raw.lines()
        .map(|line| {
            if line.starts_with("0,   100.00,") {
                line.replacen("0,   100.00,", &format!("0,   {sbase:.2},"), 1)
            } else if line.starts_with("   101,    102,") {
                let mut f: Vec<String> = line.split(',').map(str::to_owned).collect();
                let x: f64 = f[4].trim().parse().unwrap();
                f[4] = format!(" {}", sci5(x * k));
                f.join(",")
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

/// A fault's impedance restated on a system base `k` times larger.
fn scale_fault(e: &RmsEvent, k: f64) -> RmsEvent {
    match e {
        RmsEvent::BusFault { time, bus, r, x } => RmsEvent::BusFault {
            time: *time,
            bus: *bus,
            r: r * k,
            x: x * k,
        },
        other => other.clone(),
    }
}

/// Three loads at the unit's bus 102: a ZIP load (PL 40 MW, IP 30, YP 20;
/// QL 10 Mvar, IQ 5, YQ 4 capacitive), an exponential one (15 MW at
/// `γp = 1.5`, 6 Mvar at `γq = 2.5`) and a constant-power one (25 MW, 5 Mvar).
fn smib_with_mixed_loads() -> (powerio::BalancedNetwork, powerio::DynamicCase) {
    let case = &wecc240_smib_cases()[0];
    let raw = with_load(
        &case.raw,
        "102,'1 ',1,   1,   1,    40.000,    10.000,    30.000,     5.000,    20.000,     4.000,   1,1\n\
         102,'2 ',1,   1,   1,    15.000,     6.000,     0.000,     0.000,     0.000,     0.000,   1,1\n\
         102,'3 ',1,   1,   1,    25.000,     5.000,     0.000,     0.000,     0.000,     0.000,   1,1",
    );
    let mut net = network(&raw);
    assert_eq!(net.loads().len(), 3);
    let k = net
        .loads()
        .iter()
        .position(|l| l.p == 15.0)
        .expect("the second load");
    net.loads_mut()[k].voltage_model = Some(LoadVoltageModel::Exponential {
        p: 15.0,
        q: 6.0,
        v_nom: None,
        gamma_p: 1.5,
        gamma_q: 2.5,
    });
    (net, dyr_of(case))
}

/// The demand of [`smib_with_mixed_loads`] at bus voltage `v` (pu, 100 MVA),
/// part by part, with the parts of exponent below 2 drawing the constant
/// admittance of their power at `brk` below it (`None`: their law).
fn mixed_demand(v: f64, brk: Option<f64>) -> (f64, f64) {
    // (P0, Q0, γ) per part, MW / Mvar at 1 pu.
    let parts: [(f64, f64, f64); 6] = [
        (40.0 + 25.0, 10.0 + 5.0, 0.0), // constant power
        (30.0, 5.0, 1.0),               // ZIP current
        (20.0, -4.0, 2.0),              // ZIP admittance (YQ capacitive)
        (15.0, 0.0, 1.5),               // exponential P
        (0.0, 6.0, 2.5),                // exponential Q
        (0.0, 0.0, 0.0),
    ];
    let mut p = 0.0;
    let mut q = 0.0;
    for (p0, q0, g) in parts {
        let law = |v: f64| v.powf(g);
        let f = match brk {
            Some(b) if g < 2.0 && v < b => law(b) * (v / b).powi(2),
            _ => law(v),
        };
        p += p0 * f;
        q += q0 * f;
    }
    (p / 100.0, q / 100.0)
}

/// Without a disturbance the loads hold the power flow's demand and the
/// whole system stays at rest: every channel within 1e-8 of its start over
/// 2 s. At `t = 0` the loads draw the static model's demand at the solved
/// voltage, part by part.
#[test]
fn mixed_loads_start_and_stay_at_rest() {
    if shipped_wecc240().is_none() {
        return;
    }
    let (net, dyr) = smib_with_mixed_loads();
    let sim = RmsSimulation::new(&net, &dyr, LoadModel::PowerFlow).unwrap();
    assert!(
        sim.initial_residual() < 1e-8,
        "{:e}",
        sim.initial_residual()
    );
    let r = run(&sim, 2.0, 0.0025, Vec::new());
    let mut worst = (0.0_f64, String::new());
    for (key, s) in &r.channels {
        let d = s.iter().map(|x| (x - s[0]).abs()).fold(0.0, f64::max);
        if d > worst.0 {
            worst = (d, key.clone());
        }
    }
    assert!(worst.0 < 1e-8, "{} moves by {:e}", worst.1, worst.0);
    let v = r.channels["vm:102"][0];
    let (p, q) = mixed_demand(v, None);
    let (pl, ql) = (r.channels["pl:102"][0], r.channels["ql:102"][0]);
    assert!(
        (pl - p).abs() < 1e-10 && (ql - q).abs() < 1e-10,
        "{pl} {ql} vs {p} {q} at {v}"
    );
    assert!(
        r.notes.iter().any(|n| n.contains("each keeps its law")),
        "{:?}",
        r.notes
    );
}

/// Through a voltage dip each load part follows its own law: the bus's load
/// power is the sum of the parts' `P0 V^γ` at every sample (four distinct
/// exponents, so a sample set spanning several voltages pins every part).
/// Above the 0.7 pu breakpoint the conversion does not act; through a deeper
/// fault the parts of exponent below 2 draw constant admittance below it,
/// and with the exact law (PowerSimulationsDynamics.jl) they keep their law.
#[test]
fn load_parts_follow_their_law_through_a_dip() {
    if shipped_wecc240().is_none() {
        return;
    }
    let (net, dyr) = smib_with_mixed_loads();
    let x_line = wecc240_smib_cases()[0].x_line;
    let check = |options: RmsOptions, x_fault: f64, brk: Option<f64>| -> (f64, f64) {
        let sim = RmsSimulation::with_options(&net, &dyr, LoadModel::PowerFlow, options).unwrap();
        let r = run(&sim, 2.0, 0.0025, fault(102, x_fault, 1.0, 1.15));
        let vm = &r.channels["vm:102"];
        for (k, &v) in vm.iter().enumerate() {
            let (p, q) = mixed_demand(v, brk);
            let (pl, ql) = (r.channels["pl:102"][k], r.channels["ql:102"][k]);
            assert!(
                (pl - p).abs() < 1e-9 && (ql - q).abs() < 1e-9,
                "t = {}: V = {v}: pl {pl} ql {ql}, law {p} {q}",
                r.time[k]
            );
        }
        let lo = vm.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = vm.iter().copied().fold(0.0, f64::max);
        (lo, hi)
    };
    let pqbrak = RmsOptions::default();
    assert_eq!(
        pqbrak.low_voltage_load,
        LowVoltageLoad::Break { v_break: 0.7 }
    );
    // A dip that stays above the breakpoint.
    let (lo, hi) = check(pqbrak, 2.0 * x_line, Some(0.7));
    assert!(lo > 0.7 && lo < 0.95 && hi > 1.0, "dip {lo}..{hi}");
    // A deep fault, below it.
    let (lo, _) = check(pqbrak, 0.15 * x_line, Some(0.7));
    assert!(lo < 0.6 && lo > 0.25, "deep dip {lo}");
    let exact = RmsOptions {
        low_voltage_load: LowVoltageLoad::Exact,
        ..RmsOptions::default()
    };
    let (lo, _) = check(exact, 0.15 * x_line, None);
    assert!(lo < 0.6 && lo > 0.25, "deep dip {lo}");
}

/// A global load model overrides the parts' laws: the whole bus demand at
/// the power flow voltage as constant admittance.
#[test]
fn a_global_load_model_overrides_the_parts() {
    if shipped_wecc240().is_none() {
        return;
    }
    let (net, dyr) = smib_with_mixed_loads();
    let sim = RmsSimulation::new(&net, &dyr, LoadModel::ConstantImpedance).unwrap();
    let r = run(
        &sim,
        2.0,
        0.0025,
        fault(102, 2.0 * wecc240_smib_cases()[0].x_line, 1.0, 1.15),
    );
    let vm = &r.channels["vm:102"];
    let (v0, p0) = (vm[0], r.channels["pl:102"][0]);
    let (p, _) = mixed_demand(v0, None);
    assert!((p0 - p).abs() < 1e-10);
    for (k, &v) in vm.iter().enumerate() {
        let law = p0 * (v / v0).powi(2);
        assert!((r.channels["pl:102"][k] - law).abs() < 1e-9);
    }
}

/// `ESAC1A` (KC = 0.2) on a GENROU with quadratic saturation through a
/// bolted fault at its terminal: the flux falls below the saturation knee
/// and the exciter voltage collapses, and every channel stays finite.
#[test]
fn saturation_and_esac1a_stay_finite_through_a_bolted_fault() {
    if shipped_wecc240().is_none() {
        return;
    }
    let case = &wecc240_smib_cases()[0];
    let genrou = case
        .dyr
        .split('/')
        .find(|r| r.contains("GENROU"))
        .unwrap()
        .trim();
    let dyr = format!(
        "    101 'GENCLS' 1     0.0000       0.0000      /\n{genrou} /\n    102 'ESAC1A' 1 0.0 0.0 0.0 400.0 0.02 14.5 -14.5 0.8 0.03 1.0 0.2 0.38 1.0 3.14 0.03 4.18 0.1 6.03 -5.43 /\n"
    );
    let dyr = powerio::parse_dyr(&dyr).unwrap();
    let net = network(&case.raw);
    let sim = RmsSimulation::new(&net, &dyr, LoadModel::PowerFlow).unwrap();
    assert!(sim.initial_residual() < 1e-8);
    let r = run(&sim, 3.0, 0.0025, fault(102, 1e-4, 1.0, 1.15));
    assert!(r
        .channels
        .iter()
        .all(|(_, v)| v.iter().all(|x| x.is_finite())));
    let vm = &r.channels["vm:102"];
    let lo = vm.iter().copied().fold(1.0, f64::min);
    assert!(lo < 0.05, "a bolted fault: {lo}");
    let efd = &r.channels["efd:102"];
    let efd_lo = efd.iter().copied().fold(f64::INFINITY, f64::min);
    eprintln!(
        "bolted fault: V down to {lo:.4}, Efd from {:.3} down to {efd_lo:.3}",
        efd[0]
    );
    // Back near its start after the fault.
    let w = &r.channels["omega:102"];
    assert!((w[w.len() - 1] - 1.0).abs() < 1e-2);
}

/// The trapezoidal rule's order: halving the step quarters the difference
/// to the next finer run (a line trip on the GENROU-SEXS-TGOV1 unit, at
/// 4, 2 and 1 ms). The differences state the accuracy of a run: at the
/// default 2.5 ms step the rotor angle is within about the 2 → 1 ms
/// difference times (2.5/2)² of the exact solution.
#[test]
fn the_trapezoidal_rule_is_second_order() {
    if shipped_wecc240().is_none() {
        return;
    }
    let case = &wecc240_smib_cases()[0];
    let (net, dyr) = smib_study(case);
    let sim = RmsSimulation::new(&net, &dyr, LoadModel::PowerFlow).unwrap();
    let trip = vec![RmsEvent::BranchTrip {
        time: 0.2,
        from: 101,
        to: 102,
        circuit: "2".into(),
    }];
    let at = |dt: f64| {
        sim.run(&RmsRequest {
            t_end: 2.0,
            dt,
            output_step: 0.004,
            events: trip.clone(),
            ..RmsRequest::default()
        })
        .unwrap()
    };
    let runs: Vec<RmsResult> = [0.004, 0.002, 0.001].into_iter().map(at).collect();
    for r in &runs {
        assert!(
            !r.notes.iter().any(|n| n.contains("backward Euler")),
            "{:?}",
            r.notes
        );
    }
    let mut report = BTreeMap::new();
    for key in ["delta:102", "omega:102", "vm:102"] {
        let d1 = max_diff(&runs[0].channels[key], &runs[1].channels[key]);
        let d2 = max_diff(&runs[1].channels[key], &runs[2].channels[key]);
        let ratio = d1 / d2;
        report.insert(key, (d1, d2, ratio));
        assert!(
            (3.5..4.5).contains(&ratio),
            "{key}: {d1:e} / {d2:e} = {ratio}"
        );
    }
    eprintln!("step halving (4→2 ms, 2→1 ms, ratio): {report:?}");
}

/// Events are checked before the run: a time that is not finite or lies
/// outside `[0, t_end)`, an element the system does not have, a fault
/// impedance or load scale that would make a source, is an error naming the
/// event, and nothing is simulated; so is a step too small to finish.
#[test]
fn events_are_checked_before_the_run() {
    if shipped_wecc240().is_none() {
        return;
    }
    let (net, dyr) = smib_study(&wecc240_smib_cases()[0]);
    let sim = RmsSimulation::new(&net, &dyr, LoadModel::PowerFlow).unwrap();
    let fails = |events: Vec<RmsEvent>, what: &str| {
        let e = sim
            .run(&RmsRequest {
                t_end: 2.0,
                events,
                ..RmsRequest::default()
            })
            .unwrap_err();
        assert!(e.contains(what), "{e}");
    };
    let at = |time: f64| RmsEvent::ClearFault { time, bus: 102 };
    fails(vec![at(f64::NAN)], "non-finite time");
    fails(vec![at(f64::INFINITY)], "non-finite time");
    fails(vec![at(-0.1)], "outside the run");
    fails(vec![at(2.5)], "outside the run");
    fails(
        vec![RmsEvent::BusFault {
            time: 1.0,
            bus: 999,
            r: 0.0,
            x: 0.1,
        }],
        "no bus 999",
    );
    fails(
        vec![RmsEvent::BranchTrip {
            time: 1.0,
            from: 101,
            to: 102,
            circuit: "7".into(),
        }],
        "no branch 101-102 circuit 7",
    );
    fails(
        vec![RmsEvent::LoadChange {
            time: 1.0,
            bus: 102,
            scale: f64::NAN,
        }],
        "not a finite number",
    );
    fails(
        vec![RmsEvent::GeneratorTrip {
            time: 1.0,
            bus: 102,
            machine_id: "9".into(),
        }],
        "no generator 102 '9'",
    );
    // An event at the end acts on nothing the run records.
    fails(vec![at(2.0)], "outside the run");
    // A fault or a load scale that would make a source.
    let bus_fault = |r: f64, x: f64| RmsEvent::BusFault {
        time: 1.0,
        bus: 102,
        r,
        x,
    };
    fails(vec![bus_fault(-0.01, 0.0)], "r >= 0 and x >= 0");
    fails(vec![bus_fault(0.0, -0.01)], "r >= 0 and x >= 0");
    fails(
        vec![RmsEvent::LoadChange {
            time: 1.0,
            bus: 102,
            scale: -1.0,
        }],
        "finite number >= 0",
    );
    // A step too small to finish is refused before the run.
    let tiny = |dt: f64| {
        sim.run(&RmsRequest {
            t_end: 1.0,
            dt,
            output_step: 0.005,
            ..RmsRequest::default()
        })
        .unwrap_err()
    };
    assert!(tiny(1e-300).contains("dt >= "), "{}", tiny(1e-300));
    let long = sim
        .run(&RmsRequest {
            t_end: 1e5,
            dt: 0.001,
            output_step: 0.005,
            ..RmsRequest::default()
        })
        .unwrap_err();
    assert!(long.contains("exceeds the limit"), "{long}");
}

/// A run ends exactly at `t_end`, with a last shorter output interval when
/// `t_end` is not a whole number of output steps, and every accepted event
/// is applied.
#[test]
fn a_run_ends_at_t_end_and_applies_every_event() {
    if shipped_wecc240().is_none() {
        return;
    }
    let (net, dyr) = smib_study(&wecc240_smib_cases()[0]);
    let sim = RmsSimulation::new(&net, &dyr, LoadModel::PowerFlow).unwrap();
    let ends = |t_end: f64, events: Vec<RmsEvent>| {
        let r = sim
            .run(&RmsRequest {
                t_end,
                dt: 0.0025,
                output_step: 0.005,
                events,
                ..RmsRequest::default()
            })
            .unwrap();
        (*r.time.last().unwrap(), r.time.len(), r.events)
    };
    let trip = |time: f64| {
        vec![RmsEvent::GeneratorTrip {
            time,
            bus: 102,
            machine_id: "1".into(),
        }]
    };
    let (last, n, events) = ends(1.0024, trip(1.002));
    assert!((last - 1.0024).abs() < 1e-12, "{last}");
    assert_eq!(n, 202);
    assert_eq!(events.len(), 1, "{events:?}");
    let (last, n, _) = ends(1.003, Vec::new());
    assert!((last - 1.003).abs() < 1e-12 && n == 202, "{last} {n}");
    let (last, n, _) = ends(0.004, Vec::new());
    assert!((last - 0.004).abs() < 1e-12 && n == 2, "{last} {n}");
    let (last, n, _) = ends(1.0, Vec::new());
    assert!((last - 1.0).abs() < 1e-12 && n == 201, "{last} {n}");
}

/// Small-signal analysis is off unless a request asks for it.
#[test]
fn small_signal_is_off_by_default() {
    assert!(!RmsRequest::default().small_signal);
    let parsed: RmsRequest = serde_json::from_str("{}").unwrap();
    assert!(!parsed.small_signal);
    assert_eq!(parsed.load_model, None);
    if shipped_wecc240().is_none() {
        return;
    }
    let (net, dyr) = smib_study(&wecc240_smib_cases()[0]);
    let sim = RmsSimulation::new(&net, &dyr, LoadModel::PowerFlow).unwrap();
    let r = run(&sim, 0.1, 0.0025, Vec::new());
    assert!(r.eigenvalues.is_empty());
}

/// `dyr` with parameters of its `model` record set: `(index, value)`, the
/// index 1-based as in the PSS/E Model Library.
fn with_params(dyr: &str, model: &str, params: &[(usize, f64)]) -> String {
    dyr.split('/')
        .filter(|r| !r.trim().is_empty())
        .map(|r| {
            if !r.contains(&format!("'{model}'")) {
                return format!("{r}/");
            }
            let mut tokens: Vec<String> = r.split_whitespace().map(str::to_owned).collect();
            for &(i, v) in params {
                tokens[2 + i] = format!("{v}");
            }
            format!("\n {} /", tokens.join(" "))
        })
        .collect()
}

fn smib_case(id: &str) -> SmibCase {
    wecc240_smib_cases()
        .into_iter()
        .find(|c| c.id == id)
        .unwrap()
}

/// A zero time constant makes a limited lag algebraic, and it stays solvable
/// when its limit binds: SEXS with `TE = 0` (and `TB = TE = 0`), and the
/// REGCA1 converter with `Tg = 0` (and REECB1 with `Tpord = 0`), through a fault that drives the field
/// voltage and the converter currents to their limits. Before, the limited
/// row was `0 = 0` and the step matrix structurally singular.
#[test]
fn zero_time_constants_ride_through_a_limit() {
    if shipped_wecc240().is_none() {
        return;
    }
    let cases = [
        ("smib_genrou_sexs_tgov1", "SEXS", vec![(4, 0.0)]),
        ("smib_genrou_sexs_tgov1", "SEXS", vec![(2, 0.0), (4, 0.0)]),
        (
            "smib_genrou_sexs_tgov1",
            "SEXS",
            vec![(1, 1.0), (2, 0.0), (4, 0.0)],
        ),
        ("smib_regca1_reecb1_repca1", "REGCA1", vec![(2, 0.0)]),
        ("smib_regca1_reecb1_repca1", "REECB1", vec![(30, 0.0)]),
    ];
    for (id, model, params) in cases {
        let case = smib_case(id);
        let dyr = with_params(&case.dyr, model, &params);
        let dyr = powerio::parse_dyr(&dyr).unwrap();
        let net = network(&case.raw);
        let sim = RmsSimulation::new(&net, &dyr, LoadModel::ConstantImpedance).unwrap();
        assert!(sim.initial_residual() < 1e-8, "{id} {params:?}");
        let r = sim
            .run(&RmsRequest {
                t_end: 2.0,
                dt: 0.0025,
                output_step: 0.005,
                events: fault(102, 0.2 * case.x_line, 1.0, 1.1),
                ..RmsRequest::default()
            })
            .unwrap_or_else(|e| panic!("{id} {model} {params:?}: {e}"));
        assert!(r
            .channels
            .iter()
            .all(|(_, v)| v.iter().all(|x| x.is_finite())));
        if model == "SEXS" {
            // The field voltage reaches its ceiling and never passes it.
            let efd = &r.channels["efd:102"];
            let hi = efd.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            assert!((hi - 3.0).abs() < 1e-9, "{id} {params:?}: Efd up to {hi}");
        }
    }
}

/// REGCA1's ramp limits are rates in pu/s on the converter currents (PSS/E):
/// after a deep fault the active current recovers at `Rrpwr` (here 2 pu/s,
/// with REECB1's own power ramp opened up so the converter's binds), not at
/// `Rrpwr/Tg` = 100 pu/s as PowerSimulationsDynamics.jl 0.16.2 has it
/// (`RmsOptions::psid_converter_ramp`).
#[test]
fn the_converter_ramps_at_rrpwr() {
    if shipped_wecc240().is_none() {
        return;
    }
    let case = smib_case("smib_regca1_reecb1_repca1");
    let dyr = with_params(&case.dyr, "REGCA1", &[(3, 2.0)]);
    let dyr = with_params(&dyr, "REECB1", &[(25, 99.0)]);
    let dyr = powerio::parse_dyr(&dyr).unwrap();
    let net = network(&case.raw);
    let fastest = |psid: bool| {
        let options = RmsOptions {
            psid_converter_ramp: psid,
            ..RmsOptions::default()
        };
        let sim =
            RmsSimulation::with_options(&net, &dyr, LoadModel::ConstantImpedance, options).unwrap();
        let dt = 0.001;
        let r = sim
            .run(&RmsRequest {
                t_end: 1.5,
                dt,
                output_step: dt,
                events: fault(102, 0.125 * case.x_line, 1.0, 1.1),
                ..RmsRequest::default()
            })
            .unwrap();
        let ip = &r.channels["inv.Ip:102"];
        ip.windows(2)
            .map(|w| (w[1] - w[0]) / dt)
            .fold(f64::NEG_INFINITY, f64::max)
    };
    let pss = fastest(false);
    let psid = fastest(true);
    eprintln!("fastest active current rise: {pss:.3} pu/s (PSS/E), {psid:.3} pu/s (PSID)");
    assert!(pss <= 2.0 * (1.0 + 1e-6), "{pss}");
    assert!(psid > 4.0, "{psid}");
}
