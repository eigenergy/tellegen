//! Events and session operations of the RMS engine on cases built from the
//! WECC 240 case that ships with the code: faults inside lines, breakers and
//! reclosing, replay from a checkpoint after a generator trip, the integration
//! step, fault impedances, unit references, and exciter data without the
//! optional time constants.

#![cfg(feature = "sensitivity")]

use std::path::Path;

use tellegen::rms::{
    LoadModel, RecordSpec, RmsEvent, RmsOptions, RmsRequest, RmsSimulation, UnitKind,
};

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/support/smib.rs"
));

/// GENROU + SEXS + TGOV1 on two parallel lines to an infinite bus.
fn smib() -> (powerio::BalancedNetwork, powerio::DynamicCase, f64) {
    let case = &wecc240_smib_cases()[0];
    let (net, dyr) = smib_study(case);
    (net, dyr, case.x_line)
}

/// The same unit with its exciter record replaced by `exciter`.
fn smib_with_exciter(exciter: &str) -> RmsSimulation {
    let case = &wecc240_smib_cases()[0];
    let dyr: String = case
        .dyr
        .split('/')
        .filter(|r| !r.trim().is_empty())
        .map(|r| {
            if r.contains("'SEXS'") {
                format!("\n    102 {exciter} /")
            } else {
                format!("{r}/")
            }
        })
        .collect();
    let (net, _) = smib_study(case);
    let dyr = powerio::parse_dyr(&dyr).unwrap();
    RmsSimulation::new(&net, &dyr, LoadModel::ConstantImpedance).unwrap()
}

fn wecc240(options: RmsOptions) -> RmsSimulation {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/rms_examples/wecc240");
    let module = powerio::parse(powerio::Source::open(dir.join("case.raw")).unwrap()).unwrap();
    let powerio::PioValue::BalancedNetwork(net) = module.value() else {
        panic!("not a balanced network")
    };
    let dyr = powerio::parse_dyr(&std::fs::read_to_string(dir.join("case.dyr")).unwrap()).unwrap();
    RmsSimulation::with_options(net, &dyr, &[], LoadModel::ConstantImpedance, options).unwrap()
}

fn at_rest(r: &tellegen::rms::RmsResult, key: &str) -> bool {
    r.channels[key].iter().all(|w| (w - 1.0).abs() < 1e-8)
}

/// A transient fault inside one of the two lines: both breakers open, the
/// fault dies out with the line, and the line recloses onto a healthy
/// circuit; the unit comes back to its operating point. A permanent fault is
/// still there when the line recloses.
#[test]
fn line_fault_breakers_and_reclose() {
    let (net, dyr, _) = smib();
    let sim = RmsSimulation::new(&net, &dyr, LoadModel::ConstantImpedance).unwrap();
    let events = |permanent: bool| {
        vec![
            RmsEvent::LineFault {
                time: 1.0,
                from: 101,
                to: 102,
                circuit: "2".into(),
                location: 0.5,
                r: 0.0,
                x: 0.001,
                permanent,
            },
            RmsEvent::BreakerOpen {
                time: 1.08,
                from: 101,
                to: 102,
                circuit: "2".into(),
                at: 101,
            },
            RmsEvent::BreakerOpen {
                time: 1.1,
                from: 101,
                to: 102,
                circuit: "2".into(),
                at: 102,
            },
            RmsEvent::BreakerClose {
                time: 1.6,
                from: 101,
                to: 102,
                circuit: "2".into(),
                at: 101,
            },
            RmsEvent::BreakerClose {
                time: 1.6,
                from: 101,
                to: 102,
                circuit: "2".into(),
                at: 102,
            },
        ]
    };
    let request = |permanent| RmsRequest {
        t_end: 6.0,
        dt: 0.005,
        output_step: 0.01,
        load_model: LoadModel::ConstantImpedance,
        events: events(permanent),
        small_signal: false,
        record: None,
    };
    let transient = sim.run(&request(false)).unwrap();
    assert!(
        transient.events.iter().any(|e| e.contains("extinguished")),
        "{:?}",
        transient.events
    );
    let vm = &transient.channels["vm:102"];
    let k = |t: f64| (t / 0.01).round() as usize;
    // Faulted, open, then carrying its flow again after the reclose; the
    // unit back at speed and its voltage on the way back (the exciter
    // returns it slowly).
    let flow = &transient.channels["p_from:101-102#2"];
    assert!(vm[k(1.05)] < 0.8 * vm[0], "{}", vm[k(1.05)]);
    assert!(flow[k(1.3)].abs() < 1e-9);
    assert!((flow[flow.len() - 1] - flow[0]).abs() < 1e-2 * flow[0].abs());
    assert!((vm[vm.len() - 1] - vm[0]).abs() < 1e-2);
    let w = &transient.channels["omega:102"];
    assert!((w[w.len() - 1] - 1.0).abs() < 1e-3);

    let permanent = sim.run(&request(true)).unwrap();
    assert!(!permanent.events.iter().any(|e| e.contains("extinguished")));
    let vm = &permanent.channels["vm:102"];
    assert!(vm[k(1.65)] < 0.8 * vm[0], "the reclose lands on the fault");
}

/// A replay from a checkpoint taken after a generator trip reproduces the
/// run: the tripped unit stays out with the states it was tripped with.
#[test]
fn replay_after_a_generator_trip_reproduces_the_run() {
    let sim = wecc240(RmsOptions::default());
    let request = RmsRequest {
        t_end: 2.0,
        dt: 0.005,
        output_step: 0.01,
        load_model: LoadModel::ConstantImpedance,
        events: vec![RmsEvent::GeneratorTrip {
            time: 0.5,
            bus: 1431,
            machine_id: "N".into(),
        }],
        small_signal: false,
        record: Some(RecordSpec {
            channels: vec!["omega:".into()],
            checkpoint_step: 0.5,
            ..RecordSpec::default()
        }),
    };
    let full = sim
        .run(&RmsRequest {
            record: None,
            ..request.clone()
        })
        .unwrap();
    let run = sim.run(&request).unwrap();
    let cp = run
        .checkpoints
        .iter()
        .find(|c| (c.t - 1.0).abs() < 1e-9)
        .unwrap();
    let replay = sim
        .replay(
            &RmsRequest {
                record: Some(RecordSpec {
                    channels: vec!["delta:".into(), "vm:".into()],
                    ..RecordSpec::default()
                }),
                ..request.clone()
            },
            cp,
        )
        .unwrap();
    let i0 = full.time.len() - replay.time.len();
    let mut worst: f64 = 0.0;
    for (key, series) in &replay.channels {
        for (a, b) in series.iter().zip(&full.channels[key][i0..]) {
            worst = worst.max((a - b).abs());
        }
    }
    assert!(worst < 1e-10, "replay departs from the run by {worst:e}");
}

/// The integration step is never coarser than asked: 4 ms within a 5 ms
/// output step runs at 2.5 ms.
#[test]
fn the_step_is_never_coarser_than_asked() {
    let (net, dyr, _) = smib();
    let sim = RmsSimulation::new(&net, &dyr, LoadModel::ConstantImpedance).unwrap();
    let r = sim
        .run(&RmsRequest {
            t_end: 0.1,
            dt: 0.004,
            output_step: 0.005,
            small_signal: false,
            ..RmsRequest::default()
        })
        .unwrap();
    assert_eq!(r.steps, 40);
    let mut live = sim.live(0.004, 0.005);
    assert!((live.step() - 0.0025).abs() < 1e-15);
    live.set_step(0.005).unwrap();
    assert!((live.step() - 0.005).abs() < 1e-15);
}

/// A fault needs an impedance: a bolted fault through zero is refused with a
/// clear message, in a run and in a live session.
#[test]
fn a_fault_without_impedance_is_refused() {
    let (net, dyr, _) = smib();
    let sim = RmsSimulation::new(&net, &dyr, LoadModel::ConstantImpedance).unwrap();
    let fault = RmsEvent::BusFault {
        time: 0.5,
        bus: 102,
        r: 0.0,
        x: 0.0,
    };
    let err = sim
        .run(&RmsRequest {
            t_end: 1.0,
            events: vec![fault.clone()],
            small_signal: false,
            ..RmsRequest::default()
        })
        .unwrap_err();
    assert!(err.contains("nonzero impedance"), "{err}");
    let mut live = sim.live(0.005, 0.01);
    assert!(live.apply(fault).unwrap_err().contains("nonzero impedance"));
}

/// Unit references set together keep each unit's value, whatever the order:
/// a held inverter before a machine does not drop the machine's.
#[test]
fn unit_references_keep_every_value() {
    let sim = wecc240(RmsOptions {
        renewable_plants: false,
        ..RmsOptions::default()
    });
    let units = &sim.topology().units;
    let held = units
        .iter()
        .position(|u| u.kind == UnitKind::Inverter)
        .unwrap();
    let machine = units
        .iter()
        .position(|u| u.kind == UnitKind::Machine)
        .unwrap();
    let mut live = sim.live(0.005, 0.01);
    let before = live.unit_reference(machine).unwrap();
    live.set_unit_references(&[(held, 0.5), (machine, before + 0.01)])
        .unwrap();
    assert!((live.unit_reference(machine).unwrap() - (before + 0.01)).abs() < 1e-12);
    assert!((live.unit_reference(held).unwrap() - 0.5).abs() < 1e-12);
}

/// Exciter data without the optional time constants: an EXST1 without rate
/// feedback (KF = TF = 0) and an ESAC1A without a voltage transducer (TR = 0)
/// start at rest and ride through a fault.
#[test]
fn exciters_without_optional_time_constants() {
    let exst1 =
        smib_with_exciter("'EXST1' 1 0.02 0.1 -0.1 1.0 10.0 200.0 0.02 6.0 -6.0 0.05 0.0 0.0");
    let esac1a = smib_with_exciter(
        "'ESAC1A' 1 0.0 0.2 0.001 400.0 0.02 10.0 -10.0 0.8 0.03 1.0 0.2 0.38 1.0 3.14 0.03 4.18 0.1 7.32 -7.32",
    );
    for sim in [exst1, esac1a] {
        let r = sim
            .run(&RmsRequest {
                t_end: 3.0,
                events: vec![
                    RmsEvent::BusFault {
                        time: 1.0,
                        bus: 102,
                        r: 0.0,
                        x: 0.005,
                    },
                    RmsEvent::ClearFault {
                        time: 1.05,
                        bus: 102,
                    },
                ],
                small_signal: false,
                ..RmsRequest::default()
            })
            .unwrap();
        let w = &r.channels["omega:102"];
        assert!(
            w[..90].iter().all(|w| (w - 1.0).abs() < 1e-8),
            "not at rest"
        );
        assert!(r.channels.values().flatten().all(|v| v.is_finite()));
        assert!(w.iter().all(|w| (w - 1.0).abs() < 0.05));
    }
}

#[test]
fn smib_cases_start_at_rest() {
    let (net, dyr, _) = smib();
    let sim = RmsSimulation::new(&net, &dyr, LoadModel::ConstantImpedance).unwrap();
    let r = sim
        .run(&RmsRequest {
            t_end: 1.0,
            small_signal: false,
            ..RmsRequest::default()
        })
        .unwrap();
    assert!(at_rest(&r, "omega:102"));
}
