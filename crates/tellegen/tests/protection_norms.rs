//! Relay testing by injection, as a relay test set does it: each test drives
//! prescribed voltage and current phasors into one relay (no network solve)
//! and checks what the relay's standard specifies — start values, drop-off,
//! operate times from the standard's equations, characteristic points,
//! directional security, blocking and the sequence of breaker failure and
//! reclosing.
//!
//! | Standard or guide | Checked here |
//! |---|---|
//! | IEC 60255-151, IEEE C37.112 | inverse and definite-time operate times, start value, drop-off, IEEE reset, direction |
//! | IEC 60255-127 | under/overvoltage start values, drop-off ratio, operate time |
//! | IEC 60255-181 | frequency start values, operate time after a step, undervoltage blocking, RoCoF over its window |
//! | ENTSO-E RoCoF guidance | no RoCoF trip on a phase jump |
//! | IEC 60255-149 | thermal trip time from the cold and the hot state |
//! | IEC 60255-121 | mho characteristic points, zone times, reverse faults, close-in faults (voltage memory), load encroachment, power swing blocking |
//! | IEC 60255-187-1 | biased differential characteristic points, unrestrained stage, through-fault stability |
//! | Regulation (EU) 2016/631 (RfG) | ride-through profile: a short dip rides through, a long one trips where the voltage crosses the profile |
//! | IEEE C37.119 | breaker failure only with current still flowing |
//! | IEEE C37.104 | leader recloses, follower under the synchronism check |
//! | IEEE C37.116 | series capacitor MOV bypass above the protective level, reinsertion |
//!
//! Times are those of the relay's declared behaviour: the curve or delay
//! plus its measuring time (one cycle, 20 ms), to the injection step.
#![cfg(feature = "protection")]

use num_complex::Complex;
use tellegen::protection::{
    Action, AutoReclose, BreakerFailure, BreakerRef, Characteristic, Curve, Differential,
    Direction, Distance, End, FaultRideThrough, FrequencyStage, LoadEncroachment, Measure,
    Operation, OperationKind, Overcurrent, PowerSwingBlocking, ProtectionFunction,
    ProtectionReport, ProtectionRuntime, ProtectionScheme, Relay, RelayLocation, Rocof,
    SeriesCapacitor, Thermal, TopoBranch, TopoUnit, Topology, UnitKind, VoltageStage, Zone,
};

type C64 = Complex<f64>;

const DT: f64 = 0.001;
const MT: f64 = 0.02;
const F0: f64 = 50.0;

fn c(re: f64, im: f64) -> C64 {
    C64::new(re, im)
}

fn polar(m: f64, deg: f64) -> C64 {
    C64::from_polar(m, deg.to_radians())
}

fn balanced(v: C64) -> [C64; 3] {
    let a = polar(1.0, 120.0);
    [v, v * a * a, v * a]
}

/// Two buses (10 kV on 100 MVA: 1 Ω = 1 pu), one branch between them and one
/// inverter at bus 2.
fn rig_topology(transformer: bool, z: C64) -> Topology {
    Topology {
        sbase: 100.0,
        f0: F0,
        bus_numbers: vec![1, 2],
        base_kv: vec![10.0, 10.0],
        branches: vec![TopoBranch {
            from: 0,
            to: 1,
            from_number: 1,
            to_number: 2,
            circuit: "1".into(),
            transformer,
            rate: 1.0,
            z,
            y_fr: c(0.0, 0.0),
            y_to: c(0.0, 0.0),
            tap: c(1.0, 0.0),
        }],
        units: vec![TopoUnit {
            bus: 1,
            bus_number: 2,
            machine_id: "1".into(),
            mbase: 100.0,
            kind: UnitKind::Inverter,
            xd: None,
            xd_p: None,
            xd_pp: None,
            p0: 0.0,
        }],
    }
}

/// What the test set injects at one instant (positive sequence).
#[derive(Clone, Copy)]
struct Shot {
    v: [C64; 2],
    i_from: C64,
    i_to: C64,
    i_unit: C64,
}

impl Shot {
    fn at_rest() -> Self {
        Self {
            v: [c(1.0, 0.0), c(1.0, 0.0)],
            i_from: c(0.0, 0.0),
            i_to: c(0.0, 0.0),
            i_unit: c(0.0, 0.0),
        }
    }
}

/// The breakers as the test set models them.
#[derive(Clone, Copy)]
struct Breakers {
    end: [bool; 2],
    unit: bool,
}

struct Injection {
    t: f64,
    shot: Shot,
    breakers: Breakers,
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
        if !self.breakers.end[k] {
            return balanced(c(0.0, 0.0));
        }
        balanced(if k == 0 {
            self.shot.i_from
        } else {
            self.shot.i_to
        })
    }
    fn unit_current(&self, _u: usize) -> [C64; 3] {
        balanced(if self.breakers.unit {
            self.shot.i_unit
        } else {
            c(0.0, 0.0)
        })
    }
    fn feeder_current(&self, _b: usize) -> [C64; 3] {
        balanced(c(0.0, 0.0))
    }
    fn branch_end_voltage(&self, _e: usize, end: End) -> Option<[C64; 3]> {
        Some(balanced(self.shot.v[usize::from(end == End::To)]))
    }
    fn end_closed(&self, _e: usize, end: End) -> bool {
        self.breakers.end[usize::from(end == End::To)]
    }
    fn unit_in_service(&self, _u: usize) -> bool {
        self.breakers.unit
    }
}

/// Run the relays of `scheme` on `topo` for `t_end` seconds with the
/// injection `shot(t)`; breakers open and close as the relays command.
fn inject(
    scheme: &ProtectionScheme,
    topo: &Topology,
    t_end: f64,
    dt: f64,
    shot: impl Fn(f64) -> Shot,
) -> (ProtectionReport, Vec<(f64, Action)>) {
    let mut rt = ProtectionRuntime::bind(scheme, topo, true).expect("relays bind");
    let mut breakers = Breakers {
        end: [true, true],
        unit: true,
    };
    let mut actions = Vec::new();
    let steps = (t_end / dt).round() as usize;
    for k in 0..=steps {
        let t = k as f64 * dt;
        let s = shot(t);
        let before = Injection {
            t,
            shot: s,
            breakers,
        };
        let mut due = rt.take_due(t, &before);
        due.extend(rt.bank_actions(&before));
        for a in &due {
            match *a {
                Action::OpenEnd(_, end) => breakers.end[usize::from(end == End::To)] = false,
                Action::CloseEnd(_, end) => breakers.end[usize::from(end == End::To)] = true,
                Action::TripUnit(_) => breakers.unit = false,
                Action::Bypass(_) | Action::Reinsert(_) => {}
            }
            actions.push((t, a.clone()));
        }
        rt.applied();
        rt.evaluate(&Injection {
            t,
            shot: s,
            breakers,
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

fn line_relay(at: usize, functions: Vec<ProtectionFunction>) -> Relay {
    Relay {
        id: format!("R{at}"),
        location: RelayLocation::Line {
            from: 1,
            to: 2,
            circuit: "1".into(),
            at,
        },
        functions,
        breaker_time: 0.05,
        breaker_failure: None,
        autoreclose: None,
        enabled: true,
        i_nom: Some(1.0),
    }
}

/// The first trip of `relay` by a function starting with `function`.
fn trip_time(report: &ProtectionReport, relay: &str, function: &str) -> Option<f64> {
    report
        .operations
        .iter()
        .find(|o| {
            o.kind == OperationKind::Trip && o.relay == relay && o.function.starts_with(function)
        })
        .map(|o| o.time)
}

fn ops_of(report: &ProtectionReport, kind: OperationKind) -> Vec<&Operation> {
    report
        .operations
        .iter()
        .filter(|o| o.kind == kind)
        .collect()
}

/// `t` lies in `[expected, expected + slack]` (the injection step decides
/// the instant after the condition is met).
fn assert_time(what: &str, t: Option<f64>, expected: f64, slack: f64) {
    assert_window(what, t, expected, 0.0, slack);
}

/// Write one measurement of the test running now to
/// `$PROTECTION_REPORT_DIR/<test>.jsonl` (when set): the test protocol.
fn record(kind: &str, mut value: serde_json::Value) {
    let Some(dir) = std::env::var_os("PROTECTION_REPORT_DIR") else {
        return;
    };
    let test = std::thread::current()
        .name()
        .unwrap_or("test")
        .replace("::", "_");
    value["test"] = serde_json::json!(test);
    value["kind"] = serde_json::json!(kind);
    let path = std::path::Path::new(&dir).join(format!("{test}.jsonl"));
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .expect("report file");
    std::io::Write::write_all(&mut f, format!("{value}\n").as_bytes()).expect("report write");
}

/// Nothing of the relay operates (recorded for the protocol).
fn assert_no_trip(what: &str, t: Option<f64>) {
    record(
        "no_trip",
        serde_json::json!({ "what": what, "measured": t, "pass": t.is_none() }),
    );
    assert!(
        t.is_none(),
        "{what}: operated at {:.4} s",
        t.unwrap_or_default()
    );
}

/// As [`assert_time`], also up to `early` before: a dependent-time integral
/// sums `dt / t(G)` from the first step that sees the current, so it may
/// complete one step before the exact time.
fn assert_window(what: &str, t: Option<f64>, expected: f64, early: f64, slack: f64) {
    let pass = t.is_some_and(|t| t >= expected - early - 1e-9 && t <= expected + slack + 1e-9);
    record(
        "time",
        serde_json::json!({ "what": what, "expected": expected, "measured": t, "early": early, "late": slack, "pass": pass }),
    );
    let t = t.unwrap_or_else(|| panic!("{what}: no trip, expected at {expected:.4} s"));
    assert!(
        t >= expected - early - 1e-9 && t <= expected + slack + 1e-9,
        "{what}: trip at {t:.4} s, expected {expected:.4} s (+{slack:.4})"
    );
}

// ---------------------------------------------------------------------------
// IEC 60255-151 and IEEE C37.112: overcurrent
// ---------------------------------------------------------------------------

fn overcurrent(curve: Curve, tms: f64, delay: f64, direction: Direction) -> ProtectionFunction {
    ProtectionFunction::Overcurrent(Overcurrent {
        label: None,
        pickup: 1.0,
        curve,
        tms,
        delay,
        direction,
    })
}

/// A current of `g` times the setting from `t0` on, lagging the voltage by
/// `lag` degrees.
fn current_step(g: f64, t0: f64, lag: f64) -> impl Fn(f64) -> Shot {
    move |t| {
        let mut s = Shot::at_rest();
        if t >= t0 - 1e-12 {
            s.i_from = polar(g, -lag);
        }
        s
    }
}

/// The dependent-time operate time at every curve, two time multipliers and
/// five multiples of the setting: the curve's equation (flat above 20 times
/// the setting), never below the measuring time.
#[test]
fn iec_60255_151_inverse_operate_times() {
    let curves = [
        Curve::IecStandardInverse,
        Curve::IecVeryInverse,
        Curve::IecExtremelyInverse,
        Curve::IeeeModeratelyInverse,
        Curve::IeeeVeryInverse,
        Curve::IeeeExtremelyInverse,
    ];
    for curve in curves {
        for tms in [0.1, 0.5] {
            for g in [2.0, 5.0, 10.0, 20.0, 40.0] {
                let s = scheme(vec![line_relay(
                    1,
                    vec![overcurrent(curve, tms, 0.0, Direction::NonDirectional)],
                )]);
                let t_curve = tellegen::protection::curve_time(curve, tms, g).unwrap();
                let (report, _) = inject(
                    &s,
                    &rig_topology(false, c(0.0, 1.0)),
                    0.1 + t_curve + 0.2,
                    DT,
                    current_step(g, 0.1, 30.0),
                );
                record(
                    "curve_point",
                    serde_json::json!({ "curve": format!("{curve:?}"), "tms": tms, "g": g, "expected": t_curve.max(MT), "measured": trip_time(&report, "R1", "").map(|t| t - 0.1) }),
                );
                assert_window(
                    &format!("{curve:?} TMS {tms} at {g} × I>"),
                    trip_time(&report, "R1", "51")
                        .or(trip_time(&report, "R1", "50"))
                        .or(trip_time(&report, "R1", "")),
                    0.1 + t_curve.max(MT),
                    DT,
                    2.0 * DT,
                );
            }
        }
    }
}

/// The definite-time stage operates after its delay plus the measuring time.
#[test]
fn iec_60255_151_definite_operate_time() {
    for delay in [0.0, 0.3, 1.0] {
        let s = scheme(vec![line_relay(
            1,
            vec![overcurrent(
                Curve::Definite,
                1.0,
                delay,
                Direction::NonDirectional,
            )],
        )]);
        let (report, _) = inject(
            &s,
            &rig_topology(false, c(0.0, 1.0)),
            0.1 + delay + 0.2,
            DT,
            current_step(2.0, 0.1, 30.0),
        );
        assert_time(
            &format!("definite {delay} s"),
            trip_time(&report, "R1", ""),
            0.1 + delay + MT,
            2.0 * DT,
        );
    }
}

/// Start values: the definite-time stage starts at its setting, the
/// dependent-time stage at the threshold G_T = 1.1 times it (within the
/// standard's 1.0–1.3); just below, nothing operates.
#[test]
fn iec_60255_151_start_values() {
    let topo = rig_topology(false, c(0.0, 1.0));
    let def = scheme(vec![line_relay(
        1,
        vec![overcurrent(
            Curve::Definite,
            1.0,
            0.1,
            Direction::NonDirectional,
        )],
    )]);
    let (r, _) = inject(&def, &topo, 1.0, DT, current_step(0.98, 0.1, 30.0));
    assert_no_trip(
        "definite: 0.98 × I> must not operate",
        trip_time(&r, "R1", ""),
    );
    let (r, _) = inject(&def, &topo, 1.0, DT, current_step(1.02, 0.1, 30.0));
    assert_time(
        "definite at 1.02 × I>",
        trip_time(&r, "R1", ""),
        0.1 + 0.1 + MT,
        2.0 * DT,
    );

    let inv = scheme(vec![line_relay(
        1,
        vec![overcurrent(
            Curve::IecStandardInverse,
            0.05,
            0.0,
            Direction::NonDirectional,
        )],
    )]);
    let (r, _) = inject(&inv, &topo, 40.0, 0.01, current_step(1.05, 0.1, 30.0));
    assert_no_trip("inverse: 1.05 × I> is below G_T", trip_time(&r, "R1", ""));
    let t_curve = tellegen::protection::curve_time(Curve::IecStandardInverse, 0.05, 1.15).unwrap();
    let (r, _) = inject(
        &inv,
        &topo,
        0.1 + t_curve + 1.0,
        0.01,
        current_step(1.15, 0.1, 30.0),
    );
    assert_window(
        "inverse at 1.15 × I>",
        trip_time(&r, "R1", ""),
        0.1 + t_curve,
        0.01,
        0.02,
    );
}

/// Drop-off: once started, the stage holds down to 0.95 times the setting
/// and drops below it.
#[test]
fn iec_60255_151_drop_off() {
    let topo = rig_topology(false, c(0.0, 1.0));
    let def = scheme(vec![line_relay(
        1,
        vec![overcurrent(
            Curve::Definite,
            1.0,
            0.5,
            Direction::NonDirectional,
        )],
    )]);
    let step_down = |g2: f64| {
        move |t: f64| {
            let mut s = Shot::at_rest();
            if t >= 0.1 {
                s.i_from = polar(if t < 0.3 { 1.2 } else { g2 }, -30.0);
            }
            s
        }
    };
    let (r, _) = inject(&def, &topo, 1.0, DT, step_down(0.97));
    assert_time(
        "held at 0.97 × I>",
        trip_time(&r, "R1", ""),
        0.1 + 0.5 + MT,
        2.0 * DT,
    );
    let (r, _) = inject(&def, &topo, 1.0, DT, step_down(0.93));
    assert_no_trip("dropped off at 0.93 × I>", trip_time(&r, "R1", ""));
}

/// IEEE C37.112 reset: below the setting the integral decays at
/// `1 / (TD tr / (1 − M²))`; a re-applied current then trips after the rest
/// of its operate time. The IEC curves reset at once.
#[test]
fn ieee_c37_112_reset() {
    let topo = rig_topology(false, c(0.0, 1.0));
    let td = 1.0;
    let g = 5.0;
    let t5 = tellegen::protection::curve_time(Curve::IeeeModeratelyInverse, td, g).unwrap();
    let off = 1.0;
    let first = 0.5 * t5;
    let shot = move |t: f64| {
        let mut s = Shot::at_rest();
        let on = (0.1..0.1 + first).contains(&t) || t >= 0.1 + first + off;
        if on {
            s.i_from = polar(g, -30.0);
        }
        s
    };
    let s = scheme(vec![line_relay(
        1,
        vec![overcurrent(
            Curve::IeeeModeratelyInverse,
            td,
            0.0,
            Direction::NonDirectional,
        )],
    )]);
    let (r, _) = inject(&s, &topo, 0.1 + first + off + t5 + 0.5, DT, shot);
    let left = 0.5 - off / (td * 4.85);
    let expected = 0.1 + first + off + (1.0 - left) * t5;
    assert_window(
        "IEEE MI after a partial reset",
        trip_time(&r, "R1", ""),
        expected,
        DT,
        0.01,
    );

    let s = scheme(vec![line_relay(
        1,
        vec![overcurrent(
            Curve::IecStandardInverse,
            td,
            0.0,
            Direction::NonDirectional,
        )],
    )]);
    let t_si = tellegen::protection::curve_time(Curve::IecStandardInverse, td, g).unwrap();
    let first = 0.5 * t_si;
    let shot = move |t: f64| {
        let mut s = Shot::at_rest();
        if (0.1..0.1 + first).contains(&t) || t >= 0.1 + first + 0.1 {
            s.i_from = polar(g, -30.0);
        }
        s
    };
    let (r, _) = inject(&s, &topo, 0.1 + first + 0.1 + t_si + 0.5, DT, shot);
    assert_window(
        "IEC SI resets at once",
        trip_time(&r, "R1", ""),
        0.1 + first + 0.1 + t_si,
        DT,
        0.01,
    );
}

/// A forward-directional stage operates for a current lagging the voltage
/// (a forward fault) and stays for one leading it by the opposite angle.
#[test]
fn iec_60255_151_direction() {
    let topo = rig_topology(false, c(0.0, 1.0));
    let s = scheme(vec![line_relay(
        1,
        vec![overcurrent(Curve::Definite, 1.0, 0.1, Direction::Forward)],
    )]);
    let (r, _) = inject(&s, &topo, 0.5, DT, current_step(3.0, 0.1, 60.0));
    assert_time(
        "forward fault",
        trip_time(&r, "R1", ""),
        0.1 + 0.1 + MT,
        2.0 * DT,
    );
    let (r, _) = inject(&s, &topo, 0.5, DT, current_step(3.0, 0.1, 60.0 + 180.0));
    assert_no_trip("reverse fault must not operate", trip_time(&r, "R1", ""));
}

// ---------------------------------------------------------------------------
// IEC 60255-127: voltage
// ---------------------------------------------------------------------------

fn voltage_step(v1: f64, t0: f64) -> impl Fn(f64) -> Shot {
    move |t| {
        let mut s = Shot::at_rest();
        if t >= t0 {
            s.v[0] = c(v1, 0.0);
        }
        s
    }
}

/// Start values on either side of the setting, operate time, and the
/// drop-off ratio (1.02 for undervoltage, 0.98 for overvoltage).
#[test]
fn iec_60255_127_voltage_stages() {
    let topo = rig_topology(false, c(0.0, 1.0));
    let under = scheme(vec![line_relay(
        1,
        vec![ProtectionFunction::Undervoltage(VoltageStage {
            label: None,
            pickup: 0.8,
            delay: 0.5,
        })],
    )]);
    let (r, _) = inject(&under, &topo, 1.0, DT, voltage_step(0.79, 0.1));
    assert_time(
        "27 at 0.79 pu",
        trip_time(&r, "R1", "27"),
        0.1 + 0.5 + MT,
        2.0 * DT,
    );
    let (r, _) = inject(&under, &topo, 1.0, DT, voltage_step(0.81, 0.1));
    assert_no_trip("27 must not start at 0.81 pu", trip_time(&r, "R1", "27"));
    let back = |v2: f64| {
        move |t: f64| {
            let mut s = Shot::at_rest();
            if t >= 0.1 {
                s.v[0] = c(if t < 0.3 { 0.79 } else { v2 }, 0.0);
            }
            s
        }
    };
    let (r, _) = inject(&under, &topo, 1.0, DT, back(0.81));
    assert_time(
        "27 held below its drop-off",
        trip_time(&r, "R1", "27"),
        0.1 + 0.5 + MT,
        2.0 * DT,
    );
    let (r, _) = inject(&under, &topo, 1.0, DT, back(0.83));
    assert_no_trip(
        "27 drops off above 1.02 × its setting",
        trip_time(&r, "R1", "27"),
    );

    let over = scheme(vec![line_relay(
        1,
        vec![ProtectionFunction::Overvoltage(VoltageStage {
            label: None,
            pickup: 1.1,
            delay: 0.1,
        })],
    )]);
    let (r, _) = inject(&over, &topo, 0.5, DT, voltage_step(1.11, 0.1));
    assert_time(
        "59 at 1.11 pu",
        trip_time(&r, "R1", "59"),
        0.1 + 0.1 + MT,
        2.0 * DT,
    );
    let (r, _) = inject(&over, &topo, 0.5, DT, voltage_step(1.09, 0.1));
    assert_no_trip("59 must not start at 1.09 pu", trip_time(&r, "R1", "59"));
}

// ---------------------------------------------------------------------------
// IEC 60255-181: frequency
// ---------------------------------------------------------------------------

/// A voltage of magnitude `vm` at frequency `f(t)` (its angle integrated
/// in the 50 Hz frame).
fn frequency_profile(vm: f64, f: impl Fn(f64) -> f64 + Copy) -> impl Fn(f64) -> Shot {
    move |t: f64| {
        // Midpoint rule over 1 ms slices: the angle of the integrated frequency.
        let n = (t / DT).round() as usize;
        let theta: f64 = (0..n)
            .map(|k| 2.0 * std::f64::consts::PI * (f((k as f64 + 0.5) * DT) - F0) * DT)
            .sum();
        let mut s = Shot::at_rest();
        s.v[0] = C64::from_polar(vm, theta);
        s
    }
}

/// The time the relay's frequency measurement (a first-order filter of
/// 40 ms on the angle rate) needs to cross `pickup` after a step from 50 Hz
/// to `f1`, at the injection step.
fn filter_crossing(f1: f64, pickup: f64) -> f64 {
    let a = DT / 0.04;
    let mut f = F0;
    let mut k = 0usize;
    while (f - pickup).signum() == (F0 - pickup).signum() {
        f += a * (f1 - f);
        k += 1;
    }
    k as f64 * DT
}

/// Under- and overfrequency: a step beyond the setting operates after the
/// measurement crosses it plus the delay; a step short of it does not; an
/// undervoltage blocks the stage.
#[test]
fn iec_60255_181_frequency_stages() {
    let topo = rig_topology(false, c(0.0, 1.0));
    let stage = |under: bool| {
        let st = FrequencyStage {
            label: None,
            pickup: if under { 47.5 } else { 51.5 },
            delay: 0.1,
            min_voltage: 0.2,
        };
        scheme(vec![line_relay(
            1,
            vec![if under {
                ProtectionFunction::Underfrequency(st)
            } else {
                ProtectionFunction::Overfrequency(st)
            }],
        )])
    };
    let step = |f1: f64| move |t: f64| if t < 0.1 { F0 } else { f1 };
    let (r, _) = inject(
        &stage(true),
        &topo,
        1.0,
        DT,
        frequency_profile(1.0, step(47.0)),
    );
    assert_time(
        "81U after a step to 47 Hz",
        trip_time(&r, "R1", "81U"),
        0.1 + filter_crossing(47.0, 47.5) + 0.1 + MT,
        4.0 * DT,
    );
    let (r, _) = inject(
        &stage(true),
        &topo,
        1.0,
        DT,
        frequency_profile(1.0, step(47.6)),
    );
    assert_no_trip(
        "81U must not operate at 47.6 Hz",
        trip_time(&r, "R1", "81U"),
    );
    let (r, _) = inject(
        &stage(true),
        &topo,
        1.0,
        DT,
        frequency_profile(0.1, step(47.0)),
    );
    assert_no_trip(
        "81U is blocked below its minimum voltage",
        trip_time(&r, "R1", "81U"),
    );
    let (r, _) = inject(
        &stage(false),
        &topo,
        1.0,
        DT,
        frequency_profile(1.0, step(52.0)),
    );
    assert_time(
        "81O after a step to 52 Hz",
        trip_time(&r, "R1", "81O"),
        0.1 + filter_crossing(52.0, 51.5) + 0.1 + MT,
        4.0 * DT,
    );
}

fn rocof_relay() -> ProtectionScheme {
    scheme(vec![line_relay(
        1,
        vec![ProtectionFunction::Rocof(Rocof {
            label: None,
            pickup: 2.0,
            window: 0.5,
            delay: 0.0,
            min_voltage: 0.8,
        })],
    )])
}

/// RoCoF over its 500 ms window: a ramp of 2.5 Hz/s operates once the
/// window holds enough of it; 1.5 Hz/s never does.
#[test]
fn iec_60255_181_rocof_ramps() {
    let topo = rig_topology(false, c(0.0, 1.0));
    let ramp = |rate: f64| move |t: f64| if t < 1.0 { F0 } else { F0 - rate * (t - 1.0) };
    let (r, _) = inject(
        &rocof_relay(),
        &topo,
        2.5,
        DT,
        frequency_profile(1.0, ramp(2.5)),
    );
    let t = trip_time(&r, "R1", "81R").expect("2.5 Hz/s operates");
    assert!(
        t > 1.1 && t < 1.6,
        "operates within the window after the ramp starts: {t:.3} s"
    );
    let (r, _) = inject(
        &rocof_relay(),
        &topo,
        2.5,
        DT,
        frequency_profile(1.0, ramp(1.5)),
    );
    assert_no_trip("1.5 Hz/s must not operate", trip_time(&r, "R1", "81R"));
}

/// ENTSO-E RoCoF guidance: a phase jump is no change of frequency. A 20°
/// jump at healthy voltage, and a 40° jump on recovery from a dip, do not
/// operate the RoCoF stage.
#[test]
fn entsoe_rocof_phase_jump_immunity() {
    let topo = rig_topology(false, c(0.0, 1.0));
    let jump = |t: f64| {
        let mut s = Shot::at_rest();
        s.v[0] = polar(1.0, if t >= 1.0 { 20.0 } else { 0.0 });
        s
    };
    let (r, _) = inject(&rocof_relay(), &topo, 2.5, DT, jump);
    assert_no_trip("20° jump at healthy voltage", trip_time(&r, "R1", "81R"));
    let dip = |t: f64| {
        let mut s = Shot::at_rest();
        s.v[0] = if (1.0..1.15).contains(&t) {
            polar(0.3, 10.0)
        } else if t >= 1.15 {
            polar(1.0, 40.0)
        } else {
            c(1.0, 0.0)
        };
        s
    };
    let (r, _) = inject(&rocof_relay(), &topo, 3.0, DT, dip);
    assert_no_trip(
        "40° jump on recovery from a dip",
        trip_time(&r, "R1", "81R"),
    );
}

// ---------------------------------------------------------------------------
// IEC 60255-149: thermal
// ---------------------------------------------------------------------------

/// Trip time from the cold state, `τ ln(I² / (I² − (k I_B)²))`, and from
/// the hot state of a steady pre-load `I_P`,
/// `τ ln((I² − I_P²) / (I² − (k I_B)²))`.
#[test]
fn iec_60255_149_thermal_trip_times() {
    let topo = rig_topology(false, c(0.0, 1.0));
    let tau = 600.0;
    let k = 1.05;
    let s = scheme(vec![line_relay(
        1,
        vec![ProtectionFunction::Thermal(Thermal {
            label: None,
            k,
            tau,
        })],
    )]);
    let dt = 0.05;
    for (pre, mult) in [(0.0, 2.0), (0.0, 1.5), (0.9, 2.0), (0.9, 1.2)] {
        let i = mult * k;
        let ip = pre * k;
        let shot = move |t: f64| {
            let mut s = Shot::at_rest();
            s.i_from = polar(if t < 1.0 { ip } else { i }, -30.0);
            s
        };
        let expected = tau * ((i * i - ip * ip) / (i * i - k * k)).ln();
        let (r, _) = inject(&s, &topo, 1.0 + 1.2 * expected, dt, shot);
        let t = trip_time(&r, "R1", "49").expect("49 trips") - 1.0;
        record(
            "thermal",
            serde_json::json!({ "pre": pre, "mult": mult, "tau": tau, "expected": expected, "measured": t, "pass": (t - expected).abs() <= 0.005 * expected + dt }),
        );
        assert!(
            (t - expected).abs() <= 0.005 * expected + dt,
            "49 from {pre} × kI_B at {mult} × kI_B: {t:.1} s, expected {expected:.1} s"
        );
    }
}

// ---------------------------------------------------------------------------
// IEC 60255-121: distance
// ---------------------------------------------------------------------------

const LINE_ANGLE: f64 = 75.0;

fn distance_relay(load: Option<LoadEncroachment>, psb: Option<PowerSwingBlocking>) -> Relay {
    let zone = |name: &str, reach: f64, delay: f64, direction: Direction| Zone {
        name: name.into(),
        reach,
        angle: None,
        resistive_reach: None,
        delay,
        direction,
        autoreclose: true,
    };
    let mut r = line_relay(
        1,
        vec![ProtectionFunction::Distance(Distance {
            characteristic: Characteristic::Mho,
            zones: vec![
                zone("Z1", 10.0, 0.0, Direction::Forward),
                zone("Z2", 15.0, 0.4, Direction::Forward),
                zone("Z3", 25.0, 1.0, Direction::Forward),
            ],
            line_angle: LINE_ANGLE,
            k0: [0.0, 0.0],
            min_current: 0.2,
            power_swing_blocking: psb,
            teleprotection: None,
            load_encroachment: load,
        })],
    );
    r.i_nom = Some(0.1);
    r
}

/// A fault from `t0` on seen as `z` (Ω = pu here): the voltage keeps the
/// pre-fault angle (`|V|` = 0.5 pu) and the current follows from `z`, so
/// the voltage memory and the present voltage agree (the static
/// characteristic).
fn impedance_step(z: C64, t0: f64) -> impl Fn(f64) -> Shot {
    move |t| {
        let mut s = Shot::at_rest();
        s.i_from = polar(0.01, -20.0);
        if t >= t0 {
            s.v[0] = c(0.5, 0.0);
            s.i_from = c(0.5, 0.0) / z;
        }
        s
    }
}

/// Mho characteristic points: at 95 % of the reach in each direction the
/// zone operates in its time, at 105 % it does not.
#[test]
fn iec_60255_121_mho_characteristic_points() {
    let topo = rig_topology(false, polar(8.0, LINE_ANGLE));
    let s = scheme(vec![distance_relay(None, None)]);
    for dir in [-30.0, -15.0, 0.0, 15.0, 30.0, 45.0] {
        let angle = LINE_ANGLE + dir;
        let boundary = 10.0 * (dir.to_radians()).cos();
        let (r, _) = inject(
            &s,
            &topo,
            0.3,
            DT,
            impedance_step(polar(0.95 * boundary, angle), 0.1),
        );
        record(
            "mho_point",
            serde_json::json!({ "angle": angle, "z": 0.95 * boundary, "reach": 10.0, "line_angle": LINE_ANGLE, "trip": trip_time(&r, "R1", "21 Z1").is_some(), "expect_trip": true }),
        );
        assert_time(
            &format!("Z1 at 95 % along {angle}°"),
            trip_time(&r, "R1", "21 Z1"),
            0.1 + MT,
            2.0 * DT,
        );
        let (r, _) = inject(
            &s,
            &topo,
            0.3,
            DT,
            impedance_step(polar(1.05 * boundary, angle), 0.1),
        );
        record(
            "mho_point",
            serde_json::json!({ "angle": angle, "z": 1.05 * boundary, "reach": 10.0, "line_angle": LINE_ANGLE, "trip": trip_time(&r, "R1", "21 Z1").is_some(), "expect_trip": false }),
        );
        assert_no_trip(
            &format!("Z1 must not operate at 105 % along {angle}°"),
            trip_time(&r, "R1", "21 Z1"),
        );
    }
}

/// Zone times: a fault in Z2 but not Z1 trips in Z2 time, one in Z3 only
/// in Z3 time.
#[test]
fn iec_60255_121_zone_times() {
    let topo = rig_topology(false, polar(8.0, LINE_ANGLE));
    let s = scheme(vec![distance_relay(None, None)]);
    let (r, _) = inject(
        &s,
        &topo,
        1.0,
        DT,
        impedance_step(polar(12.0, LINE_ANGLE), 0.1),
    );
    assert_time("Z2", trip_time(&r, "R1", "21"), 0.1 + 0.4 + MT, 2.0 * DT);
    assert!(trip_time(&r, "R1", "21 Z2").is_some());
    let (r, _) = inject(
        &s,
        &topo,
        1.5,
        DT,
        impedance_step(polar(20.0, LINE_ANGLE), 0.1),
    );
    assert_time("Z3", trip_time(&r, "R1", "21"), 0.1 + 1.0 + MT, 2.0 * DT);
}

/// Directional security: a reverse fault, even close in, operates no
/// forward zone.
#[test]
fn iec_60255_121_reverse_faults() {
    let topo = rig_topology(false, polar(8.0, LINE_ANGLE));
    let s = scheme(vec![distance_relay(None, None)]);
    for m in [0.5, 2.0, 8.0] {
        let (r, _) = inject(
            &s,
            &topo,
            1.5,
            DT,
            impedance_step(polar(m, LINE_ANGLE + 180.0), 0.1),
        );
        record(
            "mho_point",
            serde_json::json!({ "angle": LINE_ANGLE + 180.0, "z": m, "reach": 10.0, "line_angle": LINE_ANGLE, "trip": trip_time(&r, "R1", "21").is_some(), "expect_trip": false }),
        );
        assert_no_trip(
            &format!("reverse fault at {m} Ω"),
            trip_time(&r, "R1", "21"),
        );
    }
}

/// Close-in faults (the voltage at the relay collapses): the voltage memory
/// gives the direction, so a forward one operates Z1 and a reverse one
/// nothing.
#[test]
fn iec_60255_121_close_in_faults() {
    let topo = rig_topology(false, polar(8.0, LINE_ANGLE));
    let s = scheme(vec![distance_relay(None, None)]);
    let close_in = |reverse: bool| {
        move |t: f64| {
            let mut s = Shot::at_rest();
            s.i_from = polar(0.01, -20.0);
            if t >= 0.1 {
                s.v[0] = c(0.0, 0.0);
                s.i_from = polar(5.0, -LINE_ANGLE + if reverse { 180.0 } else { 0.0 });
            }
            s
        }
    };
    let (r, _) = inject(&s, &topo, 0.6, DT, close_in(false));
    assert_time(
        "forward close-in fault",
        trip_time(&r, "R1", "21 Z1"),
        0.1 + MT,
        2.0 * DT,
    );
    let (r, _) = inject(&s, &topo, 1.5, DT, close_in(true));
    assert_no_trip("reverse close-in fault", trip_time(&r, "R1", "21"));
}

/// Load encroachment: a load impedance inside the mho but in the load
/// region does not operate; a fault of the same magnitude along the line
/// does.
#[test]
fn iec_60255_121_load_encroachment() {
    let topo = rig_topology(false, polar(8.0, LINE_ANGLE));
    let s = scheme(vec![distance_relay(
        Some(LoadEncroachment {
            r: 3.0,
            angle: 35.0,
        }),
        None,
    )]);
    let (r, _) = inject(&s, &topo, 1.5, DT, impedance_step(polar(4.0, 25.0), 0.1));
    assert_no_trip("load at 4 Ω, 25°", trip_time(&r, "R1", "21"));
    let (r, _) = inject(&s, &topo, 0.5, DT, impedance_step(polar(4.0, 70.0), 0.1));
    assert_time(
        "fault at 4 Ω, 70°",
        trip_time(&r, "R1", "21 Z1"),
        0.1 + MT,
        2.0 * DT,
    );
}

/// Power swing blocking: an impedance travelling slowly into the zones is
/// a swing and operates nothing while it passes; a jump into Z1 is a fault
/// and operates.
#[test]
fn iec_60255_121_power_swing_blocking() {
    let topo = rig_topology(false, polar(8.0, LINE_ANGLE));
    let psb = PowerSwingBlocking {
        outer: 1.3,
        delta_t: 0.03,
        max_block: 2.0,
    };
    let s = scheme(vec![distance_relay(None, Some(psb))]);
    // R from +60 Ω to −60 Ω in 1.5 s at X = 5 Ω: through every zone.
    let swing = |t: f64| {
        let mut s = Shot::at_rest();
        let r = 60.0 - 80.0 * (t - 0.1).max(0.0);
        let z = c(r, 5.0);
        s.v[0] = c(0.8, 0.0);
        s.i_from = c(0.8, 0.0) / z;
        s
    };
    let (r, _) = inject(&s, &topo, 1.6, DT, swing);
    assert_no_trip("a swing through the zones", trip_time(&r, "R1", "21"));
    assert!(
        !ops_of(&r, OperationKind::Blocked).is_empty(),
        "the swing is blocked"
    );
    let (r, _) = inject(
        &s,
        &topo,
        0.5,
        DT,
        impedance_step(polar(5.0, LINE_ANGLE), 0.1),
    );
    assert_time(
        "a fault jumps into Z1",
        trip_time(&r, "R1", "21 Z1"),
        0.1 + MT,
        2.0 * DT,
    );
}

// ---------------------------------------------------------------------------
// IEC 60255-187-1: biased differential
// ---------------------------------------------------------------------------

fn differential_scheme() -> ProtectionScheme {
    scheme(vec![Relay {
        id: "T".into(),
        location: RelayLocation::Transformer {
            from: 1,
            to: 2,
            circuit: "1".into(),
        },
        functions: vec![ProtectionFunction::Differential(Differential {
            label: Some("87T".into()),
            pickup: 0.3,
            slope1: 0.3,
            slope2: 0.7,
            knee: 2.0,
            unrestrained: 10.0,
            harmonic2: None,
            channel_time: 0.0,
            delay: 0.0,
        })],
        breaker_time: 0.05,
        breaker_failure: None,
        autoreclose: None,
        enabled: true,
        i_nom: Some(1.0),
    }])
}

/// Currents `a` into the from end and `b` out of the to end (in phase):
/// `I_d = a − b`, `I_r = (a + b) / 2`.
fn differential_shot(a: f64, b: f64) -> impl Fn(f64) -> Shot {
    move |t| {
        let mut s = Shot::at_rest();
        if t >= 0.1 {
            s.i_from = polar(a, -30.0);
            s.i_to = polar(-b, -30.0);
        }
        s
    }
}

/// Points just above and below the biased characteristic on both slopes,
/// the unrestrained stage, and a heavy through fault.
#[test]
fn iec_60255_187_1_bias_characteristic() {
    let topo = rig_topology(true, c(0.0, 0.1));
    let s = differential_scheme();
    // (I_r, I_d) and whether it operates; restrained = 0.3 I_r up to the
    // knee at 2, then 0.6 + 0.7 (I_r − 2).
    let points = [
        (1.0, 0.33, true),
        (1.0, 0.27, false),
        (4.0, 2.2, true),
        (4.0, 1.8, false),
        (0.5, 0.25, false),
        (20.0, 0.0, false),
        (12.0, 10.5, true),
    ];
    for (ir, id, operates) in points {
        let a = ir + id / 2.0;
        let b = ir - id / 2.0;
        let (r, _) = inject(&s, &topo, 0.3, DT, differential_shot(a, b));
        let t = trip_time(&r, "T", "87T");
        record(
            "bias_point",
            serde_json::json!({ "ir": ir, "id": id, "expect_trip": operates, "trip": t.is_some(), "pickup": 0.3, "slope1": 0.3, "slope2": 0.7, "knee": 2.0, "unrestrained": 10.0 }),
        );
        if operates {
            assert_time(&format!("I_r {ir}, I_d {id}"), t, 0.1 + MT, 2.0 * DT);
        } else {
            assert_no_trip(&format!("I_r {ir}, I_d {id} must not operate"), t);
        }
    }
}

// ---------------------------------------------------------------------------
// Regulation (EU) 2016/631 (RfG): ride-through profile
// ---------------------------------------------------------------------------

fn frt_scheme() -> ProtectionScheme {
    scheme(vec![Relay {
        id: "I".into(),
        location: RelayLocation::Inverter {
            bus: 2,
            machine_id: "1".into(),
        },
        functions: vec![ProtectionFunction::FaultRideThrough(FaultRideThrough {
            label: None,
            points: vec![[0.0, 0.0], [0.15, 0.0], [1.5, 0.85]],
            start: 0.85,
            reset: 0.5,
        })],
        breaker_time: 0.05,
        breaker_failure: None,
        autoreclose: None,
        enabled: true,
        i_nom: Some(1.0),
    }])
}

fn dip(v: f64, from: f64, duration: f64) -> impl Fn(f64) -> Shot {
    move |t| {
        let mut s = Shot::at_rest();
        s.i_unit = c(0.5, 0.0);
        if (from..from + duration).contains(&t) {
            s.v[1] = c(v, 0.0);
        }
        s
    }
}

/// A dip to 0 pu cleared within the profile's 150 ms rides through; one
/// that lasts trips once the voltage stays below the profile; a dip to
/// 0.5 pu trips where the rising profile passes 0.5 pu. A unit that sits
/// below the start value without a dip is no ride-through.
#[test]
fn rfg_ride_through_profile() {
    let topo = rig_topology(false, c(0.0, 1.0));
    let s = frt_scheme();
    let (r, _) = inject(&s, &topo, 2.0, DT, dip(0.0, 0.5, 0.14));
    record(
        "frt_dip",
        serde_json::json!({ "v": 0.0, "from": 0.5, "duration": 0.14, "trip": trip_time(&r, "I", "FRT"), "points": [[0.0, 0.0], [0.15, 0.0], [1.5, 0.85]] }),
    );
    assert_no_trip("0 pu for 140 ms rides through", trip_time(&r, "I", "FRT"));
    let (r, _) = inject(&s, &topo, 2.0, DT, dip(0.0, 0.5, 0.4));
    record(
        "frt_dip",
        serde_json::json!({ "v": 0.0, "from": 0.5, "duration": 0.4, "trip": trip_time(&r, "I", "FRT"), "points": [[0.0, 0.0], [0.15, 0.0], [1.5, 0.85]] }),
    );
    assert_time(
        "0 pu for 400 ms",
        trip_time(&r, "I", "FRT"),
        0.5 + 0.15 + MT,
        3.0 * DT,
    );
    let (r, _) = inject(&s, &topo, 3.0, DT, dip(0.5, 0.5, 2.0));
    record(
        "frt_dip",
        serde_json::json!({ "v": 0.5, "from": 0.5, "duration": 2.0, "trip": trip_time(&r, "I", "FRT"), "points": [[0.0, 0.0], [0.15, 0.0], [1.5, 0.85]] }),
    );
    let cross = 0.15 + 0.5 / 0.85 * 1.35;
    assert_time(
        "0.5 pu held",
        trip_time(&r, "I", "FRT"),
        0.5 + cross + MT,
        3.0 * DT,
    );
    let low = |_t: f64| {
        let mut s = Shot::at_rest();
        s.v[1] = c(0.83, 0.0);
        s.i_unit = c(0.5, 0.0);
        s
    };
    let (r, _) = inject(&s, &topo, 3.0, DT, low);
    assert_no_trip("steady 0.83 pu is no fault", trip_time(&r, "I", "FRT"));
}

// ---------------------------------------------------------------------------
// IEEE C37.119: breaker failure
// ---------------------------------------------------------------------------

/// A stuck breaker with current still flowing brings breaker failure after
/// its delay; a breaker that opens (the current stops) does not.
#[test]
fn ieee_c37_119_breaker_failure() {
    let topo = rig_topology(false, c(0.0, 1.0));
    let mut relay = line_relay(
        1,
        vec![overcurrent(
            Curve::Definite,
            1.0,
            0.0,
            Direction::NonDirectional,
        )],
    );
    relay.breaker_failure = Some(BreakerFailure {
        delay: 0.15,
        current: 0.1,
    });
    let mut stuck = scheme(vec![relay.clone()]);
    stuck.stuck_breakers.push(BreakerRef {
        from: 1,
        to: 2,
        circuit: "1".into(),
        at: 1,
        machine_id: None,
    });
    let (r, _) = inject(&stuck, &topo, 0.6, DT, current_step(3.0, 0.1, 30.0));
    let trip = trip_time(&r, "R1", "").expect("trips");
    let bf = ops_of(&r, OperationKind::BreakerFailure);
    record(
        "time",
        serde_json::json!({ "what": "50BF with the breaker stuck", "expected": trip + 0.15, "measured": bf.first().map(|o| o.time), "early": 2.0 * DT, "late": 2.0 * DT, "pass": bf.len() == 1 && (bf[0].time - (trip + 0.15)).abs() <= 2.0 * DT }),
    );
    assert_eq!(bf.len(), 1, "breaker failure once");
    assert!(
        (bf[0].time - (trip + 0.15)).abs() <= 2.0 * DT,
        "after its delay: {:.3}",
        bf[0].time
    );
    let (r, _) = inject(
        &scheme(vec![relay]),
        &topo,
        0.6,
        DT,
        current_step(3.0, 0.1, 30.0),
    );
    let bf = ops_of(&r, OperationKind::BreakerFailure);
    assert_no_trip("50BF when the breaker opened", bf.first().map(|o| o.time));
}

// ---------------------------------------------------------------------------
// IEEE C37.104: auto-reclose with synchronism check
// ---------------------------------------------------------------------------

/// Both line ends trip a transient fault in Z1; after the dead time the
/// leader (from end) recloses onto the dead line and the follower closes
/// when the angle across is within its limit, and holds when not.
#[test]
fn ieee_c37_104_leader_follower_reclosing() {
    let topo = rig_topology(false, polar(8.0, LINE_ANGLE));
    let ar = AutoReclose {
        dead_time: 0.5,
        reclaim: 5.0,
        shots: 1,
        sync_angle: 30.0,
    };
    let relays = || {
        [1usize, 2].map(|at| {
            let mut r = distance_relay(None, None);
            r.id = format!("R{at}");
            r.location = RelayLocation::Line {
                from: 1,
                to: 2,
                circuit: "1".into(),
                at,
            };
            r.autoreclose = Some(ar.clone());
            r
        })
    };
    let run = |angle: f64| {
        let shot = move |t: f64| {
            let mut s = Shot::at_rest();
            s.v[1] = polar(1.0, angle);
            if (0.1..0.18).contains(&t) {
                // The fault, seen from both ends at 4 Ω along the line.
                s.v = [c(0.5, 0.0), c(0.5, 0.0)];
                s.i_from = c(0.5, 0.0) / polar(4.0, LINE_ANGLE);
                s.i_to = c(0.5, 0.0) / polar(4.0, LINE_ANGLE);
            }
            s
        };
        inject(&scheme(relays().to_vec()), &topo, 2.5, DT, shot)
    };
    let (r, actions) = run(20.0);
    let closes: Vec<_> = actions
        .iter()
        .filter(|(_, a)| matches!(a, Action::CloseEnd(..)))
        .collect();
    for (k, who) in ["leader", "follower"].iter().enumerate() {
        record(
            "time",
            serde_json::json!({ "what": format!("{who} recloses at 20°"), "expected": closes.first().map(|c| c.0 + 0.1 * k as f64), "measured": closes.get(k).map(|c| c.0), "early": 2.0 * DT, "late": 2.0 * DT, "pass": closes.len() == 2 }),
        );
    }
    assert_eq!(
        closes.len(),
        2,
        "leader and follower close at 20°: {actions:?}"
    );
    assert!(
        closes[1].0 > closes[0].0 + 0.09,
        "the follower 0.1 s after the leader"
    );
    assert!(ops_of(&r, OperationKind::SyncHeld).is_empty());
    let (r, actions) = run(40.0);
    let closes: Vec<_> = actions
        .iter()
        .filter(|(_, a)| matches!(a, Action::CloseEnd(..)))
        .collect();
    record(
        "event",
        serde_json::json!({ "what": "follower held by the synchronism check at 40°", "pass": closes.len() == 1 && ops_of(&r, OperationKind::SyncHeld).len() == 1 }),
    );
    assert_eq!(closes.len(), 1, "only the leader closes at 40°");
    assert_eq!(
        ops_of(&r, OperationKind::SyncHeld).len(),
        1,
        "the follower holds"
    );
    assert!(ops_of(&r, OperationKind::Lockout).is_empty(), "no lockout");
}

// ---------------------------------------------------------------------------
// IEEE C37.116: series capacitor bank protection
// ---------------------------------------------------------------------------

/// The MOV bypasses the bank while its current exceeds the protective level
/// (2 times the rating), not below it, and the bank returns once the
/// current has stayed below its rating for 50 ms.
#[test]
fn ieee_c37_116_series_capacitor_bypass() {
    let topo = rig_topology(false, c(0.0, -0.5));
    let mut s = scheme(Vec::new());
    s.series_capacitors.push(SeriesCapacitor {
        from: 1,
        to: 2,
        circuit: "1".into(),
        rated: 1.0,
        protective_level: 2.0,
        reinsert: 0.05,
    });
    let current = |i: f64, until: f64| {
        move |t: f64| {
            let mut s = Shot::at_rest();
            s.i_from = polar(if (0.1..until).contains(&t) { i } else { 0.5 }, -10.0);
            s
        }
    };
    let (_, actions) = inject(&s, &topo, 0.5, DT, current(1.9, 0.3));
    record(
        "event",
        serde_json::json!({ "what": "no bypass at 1.9 × rated", "pass": actions.is_empty() }),
    );
    assert!(
        actions.is_empty(),
        "1.9 × rated stays below the protective level"
    );
    let (_, actions) = inject(&s, &topo, 0.6, DT, current(2.5, 0.3));
    let bypass: Vec<_> = actions
        .iter()
        .filter(|(_, a)| matches!(a, Action::Bypass(_)))
        .collect();
    let back: Vec<_> = actions
        .iter()
        .filter(|(_, a)| matches!(a, Action::Reinsert(_)))
        .collect();
    record(
        "time",
        serde_json::json!({ "what": "MOV bypass at 2.5 × rated", "expected": 0.1, "measured": bypass.first().map(|b| b.0), "early": 0.0, "late": 0.0, "pass": bypass.len() == 1 }),
    );
    record(
        "time",
        serde_json::json!({ "what": "bank reinserted 50 ms below rating", "expected": 0.35, "measured": back.first().map(|b| b.0), "early": 2.0 * DT, "late": 2.0 * DT, "pass": back.len() == 1 }),
    );
    assert_eq!(bypass.len(), 1, "{actions:?}");
    assert!((bypass[0].0 - 0.1).abs() < 1e-9, "bypassed at once");
    assert_eq!(back.len(), 1, "{actions:?}");
    assert!(
        (back[0].0 - (0.3 + 0.05)).abs() <= 2.0 * DT,
        "back 50 ms after: {:.3}",
        back[0].0
    );
}
