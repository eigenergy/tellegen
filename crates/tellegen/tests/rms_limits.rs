//! Limits in a run, on the single-machine infinite-bus cases:
//!
//! - held limits (`RmsOptions::freeze_limits`) against limits switching
//!   within Newton's iterations: the same steady state, close trajectories
//!   through a fault that drives the exciter to its limit, the held limits
//!   at least as close to a fine-step reference, and no more steps falling
//!   back to backward Euler;
//! - (experimental) the excitation limiters OELIT and UELPQ at an exciter's
//!   summing point: inactive at rest and until their limit is passed, then
//!   acting against it within their output range.
//!
//! The cases are built from the shipped case data; without it (the
//! published crate) every test here is skipped.

#![cfg(feature = "rms")]

use tellegen::rms::{LoadModel, RmsEvent, RmsOptions, RmsRequest, RmsResult, RmsSimulation};

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

/// Largest difference of two series.
fn max_diff(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0, f64::max)
}

fn peak(v: &[f64]) -> f64 {
    v.iter().copied().fold(f64::NEG_INFINITY, f64::max)
}

/// Steps that fell back from the trapezoidal rule to backward Euler (the
/// run's note), 0 without the note.
fn fallbacks(r: &RmsResult) -> usize {
    r.notes
        .iter()
        .find_map(|n| n.split_once(" step(s) fell back"))
        .map_or(0, |(k, _)| k.parse().unwrap())
}

/// Whether `TELLEGEN_FREEZE` overrides the option in this process.
fn freeze_overridden() -> bool {
    std::env::var_os("TELLEGEN_FREEZE").is_some()
}

/// The first single-machine case (GENROU, SEXS, TGOV1): its network, its
/// DYR with the exciter replaced by `exciter` records (none: SEXS kept),
/// and its per-circuit line reactance.
fn smib(exciter: Option<&str>) -> (powerio::BalancedNetwork, String, f64) {
    let case = &wecc240_smib_cases()[0];
    assert!(case.dyr.contains("'SEXS'"), "{}", case.dyr);
    let dyr = match exciter {
        None => case.dyr.clone(),
        Some(records) => {
            let genrou = case
                .dyr
                .split('/')
                .find(|r| r.contains("GENROU"))
                .unwrap()
                .trim();
            format!("    101 'GENCLS' 1 0.0 0.0 /\n{genrou} /\n{records}")
        }
    };
    (network(&case.raw), dyr, case.x_line)
}

/// ESAC1A with `VAMAX` = 4 (`VA` reaches it through a nearby fault).
const ESAC1A: &str = "    102 'ESAC1A' 1 0.0 0.0 0.0 400.0 0.02 4.0 -4.0 0.8 0.03 1.0 0.2 \
                      0.38 1.0 3.14 0.03 4.18 0.1 6.03 -5.43 /\n";

fn sim(net: &powerio::BalancedNetwork, dyr: &str, freeze_limits: bool) -> RmsSimulation {
    let dyr = powerio::parse_dyr(dyr).unwrap();
    RmsSimulation::with_options(
        net,
        &dyr,
        LoadModel::PowerFlow,
        RmsOptions {
            freeze_limits,
            ..RmsOptions::default()
        },
    )
    .unwrap()
}

fn run(sim: &RmsSimulation, t_end: f64, dt: f64, events: Vec<RmsEvent>) -> RmsResult {
    sim.run(&RmsRequest {
        t_end,
        dt,
        output_step: 0.005,
        events,
        ..RmsRequest::default()
    })
    .unwrap()
}

/// Held limits or not, the unit starts at the same steady state and stays
/// there: the same initial residual, and every channel of a quiet run at
/// its initial value and the same in both.
#[test]
fn held_limits_keep_the_steady_state() {
    if shipped_wecc240().is_none() {
        return;
    }
    for exciter in [None, Some(ESAC1A)] {
        let (net, dyr, _) = smib(exciter);
        let (held, free) = (sim(&net, &dyr, true), sim(&net, &dyr, false));
        assert!(held.initial_residual() < 1e-8);
        assert_eq!(held.initial_residual(), free.initial_residual());
        let a = run(&held, 0.5, 0.0025, Vec::new());
        let b = run(&free, 0.5, 0.0025, Vec::new());
        for (name, v) in &a.channels {
            let drift = v.iter().map(|x| (x - v[0]).abs()).fold(0.0, f64::max);
            assert!(drift < 1e-8, "{exciter:?} {name}: drifts by {drift}");
            let apart = max_diff(v, &b.channels[name]);
            assert!(apart < 1e-10, "{exciter:?} {name}: {apart} apart");
        }
    }
}

/// A fault next to the unit drives SEXS's field voltage to `EMAX`
/// (ESAC1A's `VA` to `VAMAX`). With held limits the run agrees with
/// limits switching within Newton's iterations to a small tolerance, is at
/// least as close to a 0.1 ms reference, falls back to backward Euler no
/// more often, and keeps SEXS's field voltage at or below `EMAX`. The two
/// runs are not the same to the bit: the option reaches the engine.
#[test]
fn held_limits_agree_through_a_fault() {
    if shipped_wecc240().is_none() {
        return;
    }
    for exciter in [None, Some(ESAC1A)] {
        let (net, dyr, x_line) = smib(exciter);
        let events = fault(102, 0.2 * x_line, 0.1, 0.2);
        let fine = run(&sim(&net, &dyr, false), 1.5, 1e-4, events.clone());
        let held = run(&sim(&net, &dyr, true), 1.5, 0.0025, events.clone());
        let free = run(&sim(&net, &dyr, false), 1.5, 0.0025, events);
        let name = if exciter.is_none() { "SEXS" } else { "ESAC1A" };
        for (ch, tol) in [
            ("delta:102", 1e-3),
            ("omega:102", 1e-4),
            ("vm:102", 1e-3),
            ("efd:102", 5e-2),
        ] {
            let d = max_diff(&held.channels[ch], &free.channels[ch]);
            assert!(d < tol, "{name} {ch}: held and free {d} apart");
        }
        for ch in ["delta:102", "efd:102"] {
            let e_held = max_diff(&held.channels[ch], &fine.channels[ch]);
            let e_free = max_diff(&free.channels[ch], &fine.channels[ch]);
            eprintln!("{name} {ch}: error held {e_held:.3e}, free {e_free:.3e}");
            assert!(
                e_held <= e_free,
                "{name} {ch}: held {e_held}, free {e_free}"
            );
        }
        assert!(
            fallbacks(&held) <= fallbacks(&free),
            "{name}: {} fallbacks held, {} free",
            fallbacks(&held),
            fallbacks(&free)
        );
        if exciter.is_none() {
            let hi = peak(&held.channels["efd:102"]);
            assert!(hi <= 3.0 && hi > 3.0 - 1e-9, "Efd up to {hi}");
        }
        if !freeze_overridden() {
            assert!(
                max_diff(&held.channels["efd:102"], &free.channels["efd:102"]) > 0.0,
                "{name}: held and free limits ran the same"
            );
        }
    }
}

/// ESST1A with `VAMAX` = 5, which `VA` reaches through the fault.
#[cfg(feature = "rms-experimental")]
const ESST1A_VAMAX: &str = "    102 'ESST1A' 1 1 1 0.02 99.0 -99.0 1.0 10.0 0.0 0.0 200.0 0.05 \
                            5.0 -5.0 6.0 -5.0 0.04 0.0 1.0 0.0 0.0 /\n";

/// The documented accuracy of held limits on a block graph's limited lag
/// whose output is the state itself (ESST1A's `VA`, a non-windup lag as
/// PowerSimulationsDynamics.jl has it): at least as close to a 0.1 ms
/// reference as limits switching within Newton's iterations.
#[cfg(feature = "rms-experimental")]
#[test]
fn held_limits_on_a_graph_lag_without_output_clamp_are_as_accurate() {
    if shipped_wecc240().is_none() {
        return;
    }
    let (net, dyr, x_line) = smib(Some(ESST1A_VAMAX));
    let events = fault(102, 0.2 * x_line, 0.1, 0.2);
    let fine = run(&sim(&net, &dyr, false), 1.5, 1e-4, events.clone());
    let held = run(&sim(&net, &dyr, true), 1.5, 0.0025, events.clone());
    let free = run(&sim(&net, &dyr, false), 1.5, 0.0025, events);
    let va = "sig:exciter.Va:102";
    eprintln!(
        "VA up to {:.4} held, {:.4} free, {:.4} at 0.1 ms",
        peak(&held.channels[va]),
        peak(&free.channels[va]),
        peak(&fine.channels[va])
    );
    for ch in ["delta:102", "efd:102"] {
        let e_held = max_diff(&held.channels[ch], &fine.channels[ch]);
        let e_free = max_diff(&free.channels[ch], &fine.channels[ch]);
        eprintln!("ESST1A {ch}: error held {e_held:.3e}, free {e_free:.3e}");
        assert!(
            e_held <= e_free,
            "{ch}: error held {e_held:.3e}, free {e_free:.3e}"
        );
    }
}

/// ESST1A without its own `VA` limits binding (`VAMAX` 99).
#[cfg(feature = "rms-experimental")]
const ESST1A: &str = "    102 'ESST1A' 1 1 1 0.02 99.0 -99.0 1.0 10.0 0.0 0.0 200.0 0.05 \
                      99.0 -99.0 6.0 -5.0 0.04 0.0 1.0 0.0 0.0 /\n";

/// A voltage reference step of `dv` at 0.2 s (from the unit's reference at
/// rest), every step recorded.
#[cfg(feature = "rms-experimental")]
fn v_ref_step(sim: &RmsSimulation, dv: f64, t_end: f64) -> RmsResult {
    let v_ref0 = run(sim, 0.005, 0.0025, Vec::new()).channels["sig:exciter.v_ref:102"][0];
    sim.run(&RmsRequest {
        t_end,
        dt: 0.0025,
        output_step: 0.0025,
        events: vec![RmsEvent::SetReference {
            time: 0.2,
            bus: 102,
            machine_id: "1".into(),
            signal: tellegen::rms::InputSignal::VRef,
            value: v_ref0 + dv,
        }],
        ..RmsRequest::default()
    })
    .unwrap()
}

/// OELIT on ESST1A (instantaneous limit 1.1 × IFDRAT = 2.2 pu field
/// current, above the unit's 1.85 at rest): VOEL is zero at rest and until
/// the field current first exceeds the limit after a voltage reference
/// step, negative whenever it exceeds it, within `[VOELMIN, 0]`, and the
/// field current's peak is lower than without the limiter.
#[cfg(feature = "rms-experimental")]
#[test]
fn oelit_acts_on_the_field_current_in_a_run() {
    if shipped_wecc240().is_none() {
        return;
    }
    let oel = "    102 'OELIT' 1 2.0 1.0 1.0 1.1 33.75 1200.0 0.05 0.5 -0.5 /\n";
    let (net, with, _) = smib(Some(&format!("{ESST1A}{oel}")));
    let (_, without, _) = smib(Some(ESST1A));
    let limited = sim(&net, &with, true);
    assert!(limited.initial_residual() < 1e-8);
    let r = v_ref_step(&limited, 0.1, 3.0);
    let bare = v_ref_step(&sim(&net, &without, true), 0.1, 3.0);
    let ifd = &r.channels["sig:exciter.ifd:102"];
    let voel = &r.channels["sig:exciter.VOEL:102"];
    let limit = 2.2;
    assert!(ifd[0] < limit);
    let first = ifd
        .iter()
        .position(|&i| i > limit)
        .expect("the step passes the limit");
    assert!(voel[..first].iter().all(|&v| v == 0.0), "inactive before");
    for (k, (&i, &v)) in ifd.iter().zip(voel).enumerate() {
        assert!((-0.5..=0.0).contains(&v), "VOEL {v} at {} s", r.time[k]);
        if i > limit + 1e-9 {
            assert!(
                v < 0.0,
                "Ifd {i} over its limit at {} s, VOEL {v}",
                r.time[k]
            );
        }
    }
    let (hi, hi_bare) = (peak(ifd), peak(&bare.channels["sig:exciter.ifd:102"]));
    assert!(
        hi < hi_bare - 0.05,
        "Ifd peak {hi} with OELIT, {hi_bare} without"
    );
}

/// UELPQ on ESST1A (a flat limit at Q = −0.1 pu of MBASE, the unit at
/// +0.23 at rest): VUEL is zero at rest and until the absorbed reactive
/// power first passes the limit after a voltage reference step down,
/// positive whenever it is past it, within `[0, VUELMAX]`, and the unit
/// absorbs less at its deepest than without the limiter.
#[cfg(feature = "rms-experimental")]
#[test]
fn uelpq_acts_on_the_absorbed_reactive_power_in_a_run() {
    if shipped_wecc240().is_none() {
        return;
    }
    let uel = "    102 'UELPQ' 1 -0.1 0.0 0.0 0.05 0.5 0.5 /\n";
    let (net, with, _) = smib(Some(&format!("{ESST1A}{uel}")));
    let (_, without, _) = smib(Some(ESST1A));
    let limited = sim(&net, &with, true);
    assert!(limited.initial_residual() < 1e-8);
    let r = v_ref_step(&limited, -0.1, 3.0);
    let bare = v_ref_step(&sim(&net, &without, true), -0.1, 3.0);
    let q = &r.channels["sig:exciter.uel_qin:102"];
    let vuel = &r.channels["sig:exciter.VUEL:102"];
    let limit = -0.1;
    assert!(q[0] > limit);
    let first = q
        .iter()
        .position(|&v| v < limit)
        .expect("the step passes the limit");
    assert!(vuel[..first].iter().all(|&v| v == 0.0), "inactive before");
    for (k, (&qk, &v)) in q.iter().zip(vuel).enumerate() {
        assert!((0.0..=0.5).contains(&v), "VUEL {v} at {} s", r.time[k]);
        if qk < limit - 1e-9 {
            assert!(
                v > 0.0,
                "Q {qk} past its limit at {} s, VUEL {v}",
                r.time[k]
            );
        }
    }
    let low = |v: &[f64]| v.iter().copied().fold(f64::INFINITY, f64::min);
    let (lo, lo_bare) = (low(&r.channels["qe:102"]), low(&bare.channels["qe:102"]));
    assert!(
        lo > lo_bare + 0.1,
        "Q down to {lo} with UELPQ, {lo_bare} without"
    );
}

/// OELIT and UELPQ on ESST1A, each already past its limit at the power flow
/// (OELIT with IFDRAT 1.5: continuous limit 1.5 pu against the unit's 1.85
/// at rest, heated far beyond C; UELPQ with a flat limit at Q = +0.4 pu
/// against the unit's +0.23): the unit starts at rest with the limiter held
/// at its output limit (VOEL at VOELMIN, VUEL at VUELMAX), its integrator
/// at the same bound, the voltage reference taking up that output, and a
/// quiet run stays there.
#[cfg(feature = "rms-experimental")]
#[test]
fn limiters_past_their_limit_at_the_start_start_at_rest() {
    if shipped_wecc240().is_none() {
        return;
    }
    let _one = one_at_a_time();
    let oel = "    102 'OELIT' 1 1.5 1.0 1.0 1.1 33.75 1200.0 0.05 0.5 -0.5 /\n";
    let uel = "    102 'UELPQ' 1 0.4 0.0 0.0 0.05 0.5 0.5 /\n";
    let (net, bare, _) = smib(Some(ESST1A));
    let v_ref_bare = sim(&net, &bare, true).generators()[0].v_ref;
    for (records, signal, held) in [(oel, "VOEL", -0.5), (uel, "VUEL", 0.5)] {
        let (_, dyr, _) = smib(Some(&format!("{ESST1A}{records}")));
        let s = sim(&net, &dyr, true);
        assert!(
            s.initial_residual() < 1e-8,
            "{signal}: f(x0) = {:e}",
            s.initial_residual()
        );
        let unit = &s.generators()[0];
        let r = run(&s, 1.0, 0.0025, Vec::new());
        let v = &r.channels[&format!("sig:exciter.{signal}:102")];
        assert!(
            v.iter().all(|&x| (x - held).abs() < 1e-9),
            "{signal} from {} to {}",
            v[0],
            v.last().unwrap()
        );
        let pre = &signal[1..].to_ascii_lowercase();
        let i = unit
            .states
            .iter()
            .position(|n| n == &format!("{pre}_x"))
            .unwrap();
        let x = unit.initial_states[i];
        assert!((x - held.abs()).abs() < 1e-9, "{signal}: integrator at {x}");
        // VOEL and VUEL enter the voltage error with the stabilizer signal.
        let d = unit.v_ref - v_ref_bare;
        assert!(
            (d + held).abs() < 1e-6,
            "{signal}: V_ref {} against {v_ref_bare} without the limiter",
            unit.v_ref
        );
        for (key, series) in &r.channels {
            let moved = series
                .iter()
                .fold(0.0_f64, |m, x| m.max((x - series[0]).abs()));
            assert!(moved < 1e-8, "{signal}: {key} moves by {moved:e}");
        }
    }
}
