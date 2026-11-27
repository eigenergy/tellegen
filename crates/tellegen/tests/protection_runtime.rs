//! Relay functions and the relay runtime driven by injection (prescribed
//! phasors, no network solve), as in `protection_norms.rs`, and on a run:
//!
//! - an overvoltage stage drops off at its reset ratio times its pickup
//!   (0.98 by default);
//! - the out-of-step function in its voltage-beat mode counts dips of the
//!   positive-sequence voltage below its beat voltage and operates its
//!   delay after the voltage then exceeds its average over the oscillation;
//! - relays evaluated on parallel threads and inline give the same
//!   operations, by injection and in a run with relays beside the step.
#![cfg(feature = "protection")]

use num_complex::Complex;
use tellegen::protection::setting::{auto_scheme, SettingOptions};
use tellegen::protection::{
    Action, Curve, Direction, End, Measure, OperationKind, OutOfStep, Overcurrent,
    ProtectionFunction, ProtectionReport, ProtectionRuntime, ProtectionScheme, Relay,
    RelayLocation, TopoBranch, TopoUnit, Topology, UnitKind, VoltageStage,
};
use tellegen::rms::{LoadModel, RmsEvent, RmsOptions, RmsRequest, RmsSimulation};

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/support/smib.rs"
));
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/support/wecc240.rs"
));

type C64 = Complex<f64>;

const DT: f64 = 0.001;
const MT: f64 = 0.02;

fn c(re: f64, im: f64) -> C64 {
    C64::new(re, im)
}

fn balanced(v: C64) -> [C64; 3] {
    let a = C64::from_polar(1.0, 120f64.to_radians());
    [v, v * a * a, v * a]
}

/// Two buses (10 kV on 100 MVA), one line between them and a synchronous
/// unit at bus 2.
fn rig() -> Topology {
    Topology {
        sbase: 100.0,
        f0: 50.0,
        bus_numbers: vec![1, 2],
        base_kv: vec![10.0, 10.0],
        branches: vec![TopoBranch {
            from: 0,
            to: 1,
            from_number: 1,
            to_number: 2,
            circuit: "1".into(),
            transformer: false,
            rate: 1.0,
            z: c(0.0, 1.0),
            y_fr: c(0.0, 0.0),
            y_to: c(0.0, 0.0),
            tap: c(1.0, 0.0),
        }],
        units: vec![TopoUnit {
            bus: 1,
            bus_number: 2,
            machine_id: "1".into(),
            mbase: 100.0,
            kind: UnitKind::Machine,
            xd: None,
            xd_p: None,
            xd_pp: None,
            p0: 0.0,
        }],
    }
}

/// What the test set injects at one instant: the positive-sequence bus
/// voltages and one current for every branch end and the unit.
#[derive(Clone, Copy)]
struct Shot {
    v: [C64; 2],
    i: C64,
}

struct Injection {
    t: f64,
    shot: Shot,
    closed: [bool; 2],
    unit: bool,
}

impl Measure for Injection {
    fn time(&self) -> f64 {
        self.t
    }
    fn frame_speed(&self) -> f64 {
        1.0
    }
    fn bus_voltage(&self, b: usize) -> [C64; 3] {
        balanced(self.shot.v[b])
    }
    fn branch_current(&self, _e: usize, end: End) -> [C64; 3] {
        let k = usize::from(end == End::To);
        balanced(if self.closed[k] {
            self.shot.i
        } else {
            c(0.0, 0.0)
        })
    }
    fn unit_current(&self, _u: usize) -> [C64; 3] {
        balanced(if self.unit { self.shot.i } else { c(0.0, 0.0) })
    }
    fn feeder_current(&self, _b: usize) -> [C64; 3] {
        balanced(c(0.0, 0.0))
    }
    fn branch_end_voltage(&self, _e: usize, end: End) -> Option<[C64; 3]> {
        Some(balanced(self.shot.v[usize::from(end == End::To)]))
    }
    fn end_closed(&self, _e: usize, end: End) -> bool {
        self.closed[usize::from(end == End::To)]
    }
    fn unit_in_service(&self, _u: usize) -> bool {
        self.unit
    }
}

/// Run the relays of `scheme` for `t_end` s with the injection `shot(t)`,
/// on parallel threads or inline; breakers open and close as commanded.
fn inject(
    scheme: &ProtectionScheme,
    threads: bool,
    t_end: f64,
    shot: impl Fn(f64) -> Shot,
) -> (ProtectionReport, Vec<(f64, Action)>) {
    let mut rt = ProtectionRuntime::bind(scheme, &rig(), true).expect("relays bind");
    rt.set_threads(threads);
    let (mut closed, mut unit) = ([true, true], true);
    let mut actions = Vec::new();
    for k in 0..=(t_end / DT).round() as usize {
        let t = k as f64 * DT;
        let s = shot(t);
        let m = Injection {
            t,
            shot: s,
            closed,
            unit,
        };
        for a in rt.take_due(t, &m) {
            match a {
                Action::OpenEnd(_, end) => closed[usize::from(end == End::To)] = false,
                Action::CloseEnd(_, end) => closed[usize::from(end == End::To)] = true,
                Action::TripUnit(_) => unit = false,
                Action::Bypass(_) | Action::Reinsert(_) => {}
            }
            actions.push((t, a));
        }
        rt.applied();
        rt.evaluate(&Injection {
            t,
            shot: s,
            closed,
            unit,
        });
    }
    (rt.into_report(), actions)
}

fn scheme(relays: Vec<Relay>) -> ProtectionScheme {
    ProtectionScheme {
        relays,
        stuck_breakers: Vec::new(),
        measuring_time: MT,
        series_capacitors: Vec::new(),
    }
}

fn relay(id: &str, location: RelayLocation, functions: Vec<ProtectionFunction>) -> Relay {
    Relay {
        id: id.into(),
        location,
        functions,
        breaker_time: 0.05,
        breaker_failure: None,
        autoreclose: None,
        enabled: true,
        i_nom: Some(1.0),
    }
}

fn line_end(at: usize) -> RelayLocation {
    RelayLocation::Line {
        from: 1,
        to: 2,
        circuit: "1".into(),
        at,
    }
}

fn unit() -> RelayLocation {
    RelayLocation::Generator {
        bus: 2,
        machine_id: "1".into(),
    }
}

/// The operations of `kind` by a function starting with `function`, as
/// `(time, detail)`.
fn ops(report: &ProtectionReport, kind: OperationKind, function: &str) -> Vec<(f64, String)> {
    report
        .operations
        .iter()
        .filter(|o| o.kind == kind && o.function.starts_with(function))
        .map(|o| (o.time, o.detail.clone()))
        .collect()
}

fn first_trip(report: &ProtectionReport, function: &str) -> Option<f64> {
    ops(report, OperationKind::Trip, function)
        .first()
        .map(|o| o.0)
}

// ---------------------------------------------------------------------------
// Overvoltage reset ratio
// ---------------------------------------------------------------------------

/// 1.12 pu at bus 1 from 0.1 s to 0.2 s, `after` from then on.
fn swell(after: f64) -> impl Fn(f64) -> Shot {
    move |t| {
        let v = if t < 0.1 {
            1.0
        } else if t < 0.2 {
            1.12
        } else {
            after
        };
        Shot {
            v: [c(v, 0.0), c(1.0, 0.0)],
            i: c(0.0, 0.0),
        }
    }
}

/// An overvoltage stage (pickup 1.1 pu, 0.3 s) picked up by a swell stays
/// picked up while the voltage stays above `reset_ratio × pickup` and
/// trips after its delay; just below that it drops off (a reset when the
/// voltage falls) and does not trip. Without a reset ratio it is 0.98.
#[test]
fn overvoltage_drops_off_at_its_reset_ratio() {
    let stage = |reset_ratio: Option<f64>| {
        scheme(vec![relay(
            "R1",
            line_end(1),
            vec![ProtectionFunction::Overvoltage(VoltageStage {
                label: None,
                pickup: 1.1,
                delay: 0.3,
                reset_ratio,
            })],
        )])
    };
    for (ratio, effective) in [(None, 0.98), (Some(0.95), 0.95), (Some(0.99), 0.99)] {
        let drop_off = effective * 1.1;
        let (r, _) = inject(&stage(ratio), false, 0.8, swell(drop_off + 0.002));
        let t = first_trip(&r, "59").unwrap_or_else(|| panic!("{ratio:?}: held, no trip"));
        assert!(
            (t - (0.1 + 0.3 + MT)).abs() <= 2.0 * DT,
            "{ratio:?}: trip at {t}"
        );
        assert!(ops(&r, OperationKind::Reset, "59").is_empty(), "{ratio:?}");

        let (r, _) = inject(&stage(ratio), false, 0.8, swell(drop_off - 0.002));
        assert_eq!(first_trip(&r, "59"), None, "{ratio:?}: dropped off");
        let resets = ops(&r, OperationKind::Reset, "59");
        assert!(
            resets.len() == 1 && (resets[0].0 - 0.2).abs() < 1e-9,
            "{ratio:?}: resets {resets:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Out of step: voltage beats
// ---------------------------------------------------------------------------

/// The out-of-step function in its voltage-beat mode, threshold 0.6 pu.
fn beats(slips: usize, beat_delay: f64) -> ProtectionScheme {
    scheme(vec![relay(
        "G",
        unit(),
        vec![ProtectionFunction::OutOfStep(OutOfStep {
            label: None,
            blinder: 0.5,
            forward_reach: 2.0,
            reverse_reach: 1.0,
            min_traverse: 0.025,
            slips,
            beat_voltage: Some(0.6),
            beat_delay,
        })],
    )])
}

/// The unit's voltage beating at `f` Hz from 0.1 s: `mid + amp cos(2π f
/// (t − 0.1))`, its minima at 0.1 + (k + ½)/f s, for `cycles` cycles; 1 pu
/// before and `mid + amp` after.
fn beating(mid: f64, amp: f64, f: f64, cycles: f64) -> impl Fn(f64) -> Shot + Copy {
    move |t| {
        let s = t - 0.1;
        let v = if s < 0.0 {
            1.0
        } else if s < cycles / f {
            mid + amp * (2.0 * std::f64::consts::PI * f * s).cos()
        } else {
            mid + amp
        };
        Shot {
            v: [c(1.0, 0.0), c(v, 0.0)],
            i: c(0.5, 0.0),
        }
    }
}

/// When the function operates on `shot`: after the `slips`-th beat (the
/// minimum below 0.6 pu, then 40 ms of rising voltage), the first instant
/// the voltage exceeds its average since the first dip, plus the delay.
fn expected_beat_trip(shot: impl Fn(f64) -> Shot, slips: usize, delay: f64) -> f64 {
    let (vb, mut counted) = (0.6, 0);
    let (mut entered, mut dip, mut min) = (false, false, (f64::INFINITY, 0.0));
    let (mut area, mut span) = (0.0, 0.0);
    let mut ready = None;
    for k in 0..20_000 {
        let t = k as f64 * DT;
        let vm = shot(t).v[1].norm();
        if entered || counted > 0 {
            area += vm * DT;
            span += DT;
        }
        if vm < vb {
            if !entered {
                (entered, dip, min) = (true, true, (vm, t));
            }
            if vm < min.0 {
                min = (vm, t);
            }
        }
        if entered && dip && t - min.1 >= 0.04 - 1e-9 && vm > min.0 {
            dip = false;
            counted += 1;
        }
        if vm > 1.1 * vb && !dip {
            entered = false;
        }
        if counted >= slips && ready.is_none() && span > 0.0 && vm > area / span {
            ready = Some(t);
        }
        if let Some(r) = ready {
            if t - r >= delay - 1e-9 {
                return t;
            }
        }
    }
    panic!("no trip expected")
}

/// Voltage beats at 2 Hz between 0.3 and 1.0 pu: each beat is counted 40
/// ms after its minimum (a pickup naming it); with two slips set the
/// function operates its delay after the voltage, past the second beat,
/// exceeds its average over the oscillation; three slips operate a cycle
/// later, a longer delay later by the difference. One dip, dips that stay
/// above the beat voltage, and too few beats do not operate.
#[test]
fn out_of_step_counts_voltage_beats() {
    let f = 2.0;
    let shot = beating(0.65, 0.35, f, 4.0);
    let minimum = |k: usize| 0.1 + (k as f64 + 0.5) / f;

    let (r, _) = inject(&beats(2, 0.1), false, 2.5, shot);
    let picks = ops(&r, OperationKind::Pickup, "78");
    assert!(picks.len() >= 2, "{picks:?}");
    for (k, (t, detail)) in picks.iter().take(2).enumerate() {
        assert!(
            (t - (minimum(k) + 0.04)).abs() <= 2.0 * DT,
            "beat {} at {t}",
            k + 1
        );
        assert!(
            detail.starts_with(&format!("voltage beat {}", k + 1)),
            "{detail}"
        );
    }
    let t2 = first_trip(&r, "78").expect("two beats operate");
    let expected = expected_beat_trip(shot, 2, 0.1);
    assert!(
        (t2 - expected).abs() <= 2.0 * DT,
        "trip at {t2}, expected {expected}"
    );
    assert!(t2 > minimum(1) + 0.04 + 0.1 && t2 < minimum(2), "{t2}");

    let (r, _) = inject(&beats(3, 0.1), false, 2.5, shot);
    let t3 = first_trip(&r, "78").expect("three beats operate");
    let expected = expected_beat_trip(shot, 3, 0.1);
    assert!(
        (t3 - expected).abs() <= 2.0 * DT,
        "trip at {t3}, expected {expected}"
    );
    assert!(t3 > minimum(2) + 0.04, "{t3}");

    let (r, _) = inject(&beats(2, 0.25), false, 2.5, shot);
    let later = first_trip(&r, "78").expect("operates");
    assert!(
        (later - t2 - 0.15).abs() <= 2.0 * DT,
        "delay 0.25 s: {later}, 0.1 s: {t2}"
    );

    // One beat, then steady: no second beat.
    let (r, _) = inject(&beats(2, 0.1), false, 2.5, beating(0.65, 0.35, f, 1.0));
    assert_eq!(ops(&r, OperationKind::Pickup, "78").len(), 1);
    assert_eq!(first_trip(&r, "78"), None);
    // Dips down to 0.62 pu only: no beat.
    let (r, _) = inject(&beats(1, 0.0), false, 2.5, beating(0.81, 0.19, f, 4.0));
    assert!(ops(&r, OperationKind::Pickup, "78").is_empty());
    assert_eq!(first_trip(&r, "78"), None);
    // Four beats, five set.
    let (r, _) = inject(&beats(5, 0.1), false, 2.5, shot);
    assert_eq!(ops(&r, OperationKind::Pickup, "78").len(), 4);
    assert_eq!(first_trip(&r, "78"), None);
}

// ---------------------------------------------------------------------------
// Relays on threads and inline
// ---------------------------------------------------------------------------

/// Many relays (several chunks of the relays' pool), each at one line end
/// with an overcurrent and an undervoltage stage of its own setting.
fn many_relays(n: usize) -> ProtectionScheme {
    scheme(
        (0..n)
            .map(|k| {
                let x = k as f64 / n as f64;
                relay(
                    &format!("R{k}"),
                    line_end(1 + k % 2),
                    vec![
                        ProtectionFunction::Overcurrent(Overcurrent {
                            label: None,
                            pickup: 1.0 + 2.0 * x,
                            curve: if k % 3 == 0 {
                                Curve::IecStandardInverse
                            } else {
                                Curve::Definite
                            },
                            tms: 0.1,
                            delay: 0.05 * (k % 7) as f64,
                            direction: Direction::NonDirectional,
                        }),
                        ProtectionFunction::Undervoltage(VoltageStage {
                            label: None,
                            pickup: 0.5 + 0.4 * x,
                            delay: 0.1 + 0.02 * (k % 5) as f64,
                            reset_ratio: None,
                        }),
                    ],
                )
            })
            .collect(),
    )
}

/// A fault: the voltage dips to 0.7 pu and 2.5 pu current flows from 0.1 s
/// to 0.35 s (whichever breakers are still closed).
fn fault_shot(t: f64) -> Shot {
    let on = (0.1..0.35).contains(&t);
    Shot {
        v: [c(if on { 0.7 } else { 1.0 }, 0.0); 2],
        i: if on { c(2.5, -1.0) } else { c(0.0, 0.0) },
    }
}

/// The relays evaluated in chunks on parallel threads and all inline give
/// the same operations, trips and breaker actions, in the same order.
#[test]
fn relays_on_threads_and_inline_operate_alike() {
    let s = many_relays(300);
    let (threaded, a1) = inject(&s, true, 0.8, fault_shot);
    let (inline, a2) = inject(&s, false, 0.8, fault_shot);
    assert!(
        threaded.trips.len() > 10,
        "{} relays trip",
        threaded.trips.len()
    );
    assert!(threaded.operations.len() > 300);
    assert_eq!(threaded, inline);
    assert_eq!(a1, a2);
}

/// A run with relays set by rule through a fault at the unit: with threads
/// (the relays beside the integration step) and without, the same
/// operations, events and channels to the bit.
#[test]
fn a_run_with_relays_beside_the_step_is_the_same_inline() {
    if shipped_wecc240().is_none() {
        return;
    }
    let case = &wecc240_smib_cases()[0];
    let (net, dyr) = smib_study(case);
    let request = RmsRequest {
        t_end: 1.0,
        events: vec![
            RmsEvent::BusFault {
                time: 0.1,
                bus: 102,
                r: 0.0,
                x: 0.05 * case.x_line,
            },
            RmsEvent::ClearFault {
                time: 0.3,
                bus: 102,
            },
        ],
        ..RmsRequest::default()
    };
    let mut runs = Vec::new();
    for threads in [true, false] {
        let sim = RmsSimulation::with_options(
            &net,
            &dyr,
            LoadModel::ConstantImpedance,
            RmsOptions {
                threads,
                ..RmsOptions::default()
            },
        )
        .unwrap();
        let set = auto_scheme(sim.topology(), &SettingOptions::default());
        assert!(!set.scheme.relays.is_empty());
        runs.push(sim.run_with_protection(&request, &set.scheme).unwrap());
    }
    let ((r1, p1), (r2, p2)) = (&runs[0], &runs[1]);
    assert!(
        p1.operations
            .iter()
            .any(|o| o.kind == OperationKind::Pickup),
        "the fault picks up a relay: {:?}",
        p1.operations
    );
    assert_eq!(p1, p2);
    assert_eq!(r1.events, r2.events);
    // Bit for bit (the impedance channels are NaN while no current flows).
    let bits = |v: &[f64]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
    for (name, v) in &r1.channels {
        assert_eq!(bits(v), bits(&r2.channels[name]), "{name}");
    }
}
