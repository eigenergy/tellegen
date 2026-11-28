//! The live session's derived channels (`coi:`, `f:`, `dcoi:`) against the
//! screening of a batch run of the same events, on two equal machines that a
//! line trip separates into two islands.
//!
//! The case is the SMIB case of the PowerSimulationsDynamics.jl benchmarks
//! with both ends made classical machines (GENCLS) of the same inertia
//! (H = 4 s, D = 40, MBASE = 873 MVA, X'd = 0.2 pu); the damping lets each
//! island settle at its own speed after the split. Neither is a voltage source, so
//! the simulation frame turns with the machine at the slack bus 101: a
//! moving reference that the bus frequencies must add back.

#![cfg(feature = "rms-experimental")]

use std::collections::BTreeMap;

use tellegen::rms::{
    LoadModel, RecordSpec, RmsEvent, RmsRequest, RmsSimulation, ScreeningCriteria,
};

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/support/smib.rs"
));

fn two_machines() -> RmsSimulation {
    let case = &wecc240_smib_cases()[0];
    let raw = case.raw.replacen(
        ",    100.000, 0.00000E+0, 1.00000E-5,",
        ",    873.000, 0.00000E+0, 2.00000E-1,",
        1,
    );
    assert_ne!(raw, case.raw, "the template's bus 101 generator changed");
    let src = powerio::Source::from_memory("case.raw", raw.into_bytes()).unwrap();
    let module = powerio::parse(src).unwrap();
    let powerio::PioValue::BalancedNetwork(net) = module.value() else {
        panic!("not a balanced network")
    };
    let dyr = powerio::parse_dyr("101 'GENCLS' 1 4.0 40.0 /\n102 'GENCLS' 1 4.0 40.0 /\n").unwrap();
    RmsSimulation::new(net, &dyr, LoadModel::ConstantImpedance).unwrap()
}

const FAULT: f64 = 0.2;
const CLEAR: f64 = 0.3;
const SPLIT: f64 = 0.6;
const END: f64 = 3.0;

fn events() -> Vec<RmsEvent> {
    let mut e = vec![
        RmsEvent::BusFault {
            time: FAULT,
            bus: 102,
            r: 0.0,
            x: 0.05,
        },
        RmsEvent::ClearFault {
            time: CLEAR,
            bus: 102,
        },
    ];
    for circuit in ["1", "2"] {
        e.push(RmsEvent::BranchTrip {
            time: SPLIT,
            from: 101,
            to: 102,
            circuit: circuit.into(),
        });
    }
    e
}

/// The live session over the same events, advanced in uneven slices (as a
/// paced view takes them), its chunks joined.
fn live_run(sim: &RmsSimulation) -> (Vec<f64>, BTreeMap<String, Vec<f64>>) {
    let mut live = sim.live(0.0025, 0.01);
    let mut time = Vec::new();
    let mut channels: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    let mut take = |live: &mut tellegen::rms::RmsLive, until: f64| {
        while live.time() < until - 1e-9 {
            let c = live.advance((until - live.time()).min(0.037)).unwrap();
            time.extend(&c.time);
            for (k, v) in c.channels {
                channels.entry(k).or_default().extend(v);
            }
        }
    };
    // The sample at t = 0 comes with the first chunk.
    for e in events() {
        take(&mut live, e.time());
        live.apply(e).unwrap();
    }
    take(&mut live, END);
    (time, channels)
}

fn batch(sim: &RmsSimulation) -> (RmsRequest, tellegen::rms::RmsResult) {
    let request = RmsRequest {
        t_end: END,
        dt: 0.0025,
        output_step: 0.01,
        events: events(),
        small_signal: false,
        record: Some(RecordSpec::display()),
        ..RmsRequest::default()
    };
    let result = sim.run(&request).unwrap();
    (request, result)
}

fn worst(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b)
        .map(|(x, y)| {
            if x.is_nan() && y.is_nan() {
                0.0
            } else {
                (x - y).abs()
            }
        })
        .fold(0.0, f64::max)
}

#[test]
fn live_derived_channels_match_the_batch_screening() {
    let sim = two_machines();
    let (request, result) = batch(&sim);
    let criteria = ScreeningCriteria::default();
    let derived = sim.derived_series(&request, &result, &criteria).unwrap();
    let (time, live) = live_run(&sim);
    assert_eq!(time.len(), result.time.len());
    assert!(worst(&time, &result.time) < 1e-12);
    // The states: the live session takes its events at its own time, not
    // at a step the run planned for them, so it lands within the solver's
    // tolerance of the run, not on it (2e-6 rad here).
    for key in ["delta:101", "delta:102", "omega:101", "omega:102", "va:102"] {
        let w = worst(&live[key], &result.channels[key]);
        assert!(w < 1e-5, "{key}: live departs from the run by {w:e}");
    }
    // What the live session derives is what the screening derives from the
    // same samples, to rounding.
    let same_samples = tellegen::rms::RmsResult {
        time: time.clone(),
        channels: live.clone(),
        ..result.clone()
    };
    let from_live = sim
        .derived_series(&request, &same_samples, &criteria)
        .unwrap();
    for key in ["coi:f", "coi:omega", "coi:delta", "f:101", "f:102"] {
        let w = worst(&live[key], &from_live[key]);
        assert!(w < 1e-12, "{key}: live departs from the screening by {w:e}");
        // And from the run's own screening as far as the states differ.
        let w = worst(&live[key], &derived[key]);
        assert!(
            w < 1e-5,
            "{key}: live departs from the run's screening by {w:e}"
        );
    }
    // The largest separation within an island, as the screening reports it.
    let s_pre = time.iter().position(|&t| t > FAULT + 1e-9).unwrap() - 1;
    let live_sep = live["coi:sep"][s_pre..]
        .iter()
        .fold(0.0_f64, |m, v| m.max(*v))
        .to_degrees();
    let screened = sim.screen(&request, &same_samples, &criteria).unwrap();
    assert!(
        (live_sep - screened.angle.max_separation_deg).abs() < 1e-7,
        "live {live_sep}° vs screening {}°",
        screened.angle.max_separation_deg
    );
}

#[test]
fn equal_machines_split_the_angle_until_they_are_islands() {
    let sim = two_machines();
    let (time, live) = live_run(&sim);
    let (d1, d2) = (&live["delta:101"], &live["delta:102"]);
    let (c1, c2) = (&live["dcoi:101"], &live["dcoi:102"]);
    for (s, &t) in time.iter().enumerate() {
        let gap = d2[s] - d1[s];
        if t <= SPLIT + 1e-9 {
            // One island: equal inertia puts the COI halfway, each machine
            // half the separation from it.
            assert_eq!(live["coi:islands"][s], 1.0, "t = {t}");
            assert!((c2[s] - gap / 2.0).abs() < 1e-12, "t = {t}");
            assert!((c1[s] + gap / 2.0).abs() < 1e-12, "t = {t}");
            assert!((live["coi:sep"][s] - gap.abs()).abs() < 1e-12, "t = {t}");
        } else {
            // Two islands of one machine each: each is its own COI, and
            // machines in different islands are not a separation.
            assert_eq!(live["coi:islands"][s], 2.0, "t = {t}");
            assert!(c1[s].abs() < 1e-12 && c2[s].abs() < 1e-12, "t = {t}");
            assert!(live["coi:sep"][s].abs() < 1e-12, "t = {t}");
        }
    }
    // The angles do part: the islands run at different speeds.
    let last = time.len() - 1;
    assert!((d2[last] - d1[last]).abs() > 0.1);
}

#[test]
fn bus_frequencies_follow_their_island_in_a_moving_frame() {
    let sim = two_machines();
    let (time, live) = live_run(&sim);
    let f0 = 60.0;
    let last = time.len() - 1;
    let (w1, w2) = (live["omega:101"][last], live["omega:102"][last]);
    // The frame turns with machine 101, and the islands drift apart.
    assert!((w1 - 1.0).abs() > 1e-3, "the frame does not move: {w1}");
    assert!(
        (w2 - w1).abs() > 1e-3,
        "the islands run together: {w1} {w2}"
    );
    // Each bus runs at its own island's machine, not at the frame's speed:
    // bus 102's angle turns against the frame, and its frequency adds it.
    for (bus, w) in [(101, w1), (102, w2)] {
        let f = live[&format!("f:{bus}")][last];
        assert!(
            (f - f0 * w).abs() < 2e-3,
            "bus {bus}: {f} Hz, its machine {} Hz",
            f0 * w
        );
    }
}
