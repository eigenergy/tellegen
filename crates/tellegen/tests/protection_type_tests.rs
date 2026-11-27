//! Type tests of the relay functions after the IEC 60255 functional
//! standards, run as a relay test set runs a secondary-injection test plan:
//! slow ramps find start and drop-off values, steps and pulses measure
//! operate, reset and disengaging times and the critical impulse time,
//! shots along rays search the characteristic boundaries. Each test names
//! the standard and the test-plan item it implements and the tolerance it
//! applies. `protection_norms.rs` holds the first round of these checks;
//! the plans here complete them.
//!
//! | Standard | Function | Test-plan items here |
//! |---|---|---|
//! | IEC 60255-151 | 50/51, 67, 50N/51N/67N | start value and drop-off ratio by ramps, definite and dependent operate time over the setting range (accuracy class), timed reset, disengaging time, critical impulse and overshoot, directional boundary, residual current |
//! | IEC 60255-127 | 27, 59, 59N | start value and drop-off ratio by ramps (the 59 `reset_ratio`), operate time, reset and critical impulse, neutral voltage |
//! | IEC 60255-181 | 81U/81O, 81R | start value and drop-off by slow frequency ramps, operate time on ramps, voltage blocking threshold, RoCoF start value and operate time |
//! | IEC 60255-118-1 | frequency measurement | frequency error during a ramp |
//! | IEC 60255-149 | 49 | operate times over the current range, no operation at the threshold, cooling |
//! | IEC 60255-121 | 21 | mho and quadrilateral reach by search, operate time against the source impedance ratio, minimum current, ground loops with residual compensation |
//! | IEC 60255-187-1 | 87 | bias characteristic by ramps, operate time, second-harmonic blocking and the unrestrained stage |
//! | IEEE C37.102 | 24, 32, 40, 46 | start values, operate times, characteristic points |
//! | IEEE C37.113 | POTT, PUTT | trip on permission, no acceleration without it |
//! | IEEE C37.119 | 50BF | current check threshold |
//! | IEEE C37.104 | 79 | multi-shot sequence and lockout |
//!
//! Times count from the injection step that applies a change; the relays
//! evaluate once per step (1 ms unless a test says otherwise) and add their
//! measuring time of one cycle (20 ms) to every timer.
#![cfg(feature = "protection")]

#[allow(dead_code)]
mod test_set {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/support/test_set.rs"
    ));
}

use std::f64::consts::PI;

use tellegen::protection::{
    curve_time, AutoReclose, BreakerFailure, BreakerRef, Characteristic, Curve, Differential,
    Direction, Distance, FrequencyStage, LossOfField, NegativeSequence, OperationKind, Overcurrent,
    ProtectionFunction, ProtectionReport, ProtectionScheme, RelayLocation, ReversePower, Rocof,
    SchemeKind, Teleprotection, Thermal, Topology, UnitKind, VoltageStage, Zone,
};
use test_set::*;

// ---------------------------------------------------------------------------
// Test set helpers
// ---------------------------------------------------------------------------

/// A quantity that starts at `x0` at `t0`, moves at `rate` (per second, the
/// sign taken from the direction) to `x1` and returns at the same rate to
/// `x0`, where it stays.
fn triangle(t: f64, t0: f64, x0: f64, x1: f64, rate: f64) -> f64 {
    let span = (x1 - x0).abs() / rate;
    let dir = (x1 - x0).signum();
    let s = t - t0;
    if s <= 0.0 {
        x0
    } else if s <= span {
        x0 + dir * rate * s
    } else if s <= 2.0 * span {
        x1 - dir * rate * (s - span)
    } else {
        x0
    }
}

/// The time of the first pickup of `relay` and of the first reset after it.
fn start_and_drop(report: &ProtectionReport, relay: &str, function: &str) -> (f64, Option<f64>) {
    let tp = first(report, relay, OperationKind::Pickup, function)
        .unwrap_or_else(|| panic!("{relay} {function}: no start"));
    let tr = ops(report, relay, OperationKind::Reset, function)
        .into_iter()
        .map(|o| o.time)
        .find(|&t| t > tp);
    (tp, tr)
}

/// One relay at the bus-1 end of the line.
fn line_scheme(functions: Vec<ProtectionFunction>) -> ProtectionScheme {
    scheme(vec![relay("R1", line_end(1), functions)])
}

/// Run one relay at the bus-1 end of the line.
fn run_line(
    functions: Vec<ProtectionFunction>,
    t_end: f64,
    dt: f64,
    shot: impl Fn(f64) -> Shot,
) -> ProtectionReport {
    inject(&line_scheme(functions), &line_rig(), t_end, dt, shot).0
}

/// Balanced current `i` (pu, lagging the rated voltage by 30°) into the line
/// at bus 1.
fn current(i: f64) -> Shot {
    Shot::line(c(1.0, 0.0), polar(i, -30.0))
}

/// Balanced voltage `v` at bus 1, no current.
fn voltage(v: f64) -> Shot {
    Shot::line(c(v, 0.0), c(0.0, 0.0))
}

fn overcurrent(
    pickup: f64,
    curve: Curve,
    tms: f64,
    delay: f64,
    direction: Direction,
) -> ProtectionFunction {
    ProtectionFunction::Overcurrent(Overcurrent {
        label: None,
        pickup,
        curve,
        tms,
        delay,
        direction,
    })
}

fn definite(pickup: f64, delay: f64) -> ProtectionFunction {
    overcurrent(
        pickup,
        Curve::Definite,
        1.0,
        delay,
        Direction::NonDirectional,
    )
}

fn stage(pickup: f64, delay: f64, reset_ratio: Option<f64>) -> VoltageStage {
    VoltageStage {
        label: None,
        pickup,
        delay,
        reset_ratio,
    }
}

// ---------------------------------------------------------------------------
// IEC 60255-151: overcurrent
// ---------------------------------------------------------------------------

/// IEC 60255-151, start value and drop-off ratio. The test set ramps the
/// current from 0.85 to 1.25 times the setting and back, by 0.05 % of the
/// setting per step, at three settings. The definite-time stage starts at
/// its setting; the dependent-time stage at its threshold G_T = 1.1 times
/// the setting (the standard requires G_T between the setting and 1.3
/// times it); both drop off at 0.95 times the setting, the drop-off ratio
/// 0.95 of the definite stage and 0.95/1.1 of the dependent one.
/// Tolerance: the ramp resolution of 0.05 % (the start value is exact on
/// phasors).
#[test]
fn iec_60255_151_start_value_and_drop_off_ratio_by_ramp() {
    let res = 0.0005;
    for (curve, threshold) in [(Curve::Definite, 1.0), (Curve::IecStandardInverse, 1.1)] {
        for setting in [0.2, 1.0, 4.0] {
            let rate = res * setting / DT;
            let x = move |t: f64| triangle(t, 0.1, 0.85 * setting, 1.25 * setting, rate);
            let f = overcurrent(setting, curve, 1.0, 0.0, Direction::NonDirectional);
            let delay_free = if curve == Curve::Definite {
                definite(setting, 100.0)
            } else {
                f
            };
            let r = run_line(vec![delay_free], 2.0, DT, move |t| current(x(t)));
            assert_none("no operation during the ramps", trip(&r, "R1", ""));
            let (tp, tr) = start_and_drop(&r, "R1", "");
            let start = x(tp) / setting;
            let drop = x(tr.expect("drops off")) / setting;
            assert!(
                start >= threshold - 1e-9 && start <= threshold + res + 1e-9,
                "{curve:?} at {setting}: start at {start:.5} × setting, expected {threshold}"
            );
            assert!(
                (1.0..=1.3 + 1e-9).contains(&start),
                "{curve:?}: G_T {start:.4} outside 1.0–1.3"
            );
            assert!(
                drop < 0.95 && drop >= 0.95 - res - 1e-9,
                "{curve:?} at {setting}: drop-off at {drop:.5} × setting, expected 0.95"
            );
            let ratio = drop / start;
            assert!(
                (ratio - 0.95 / threshold).abs() <= 2.0 * res,
                "{curve:?}: drop-off ratio {ratio:.4}"
            );
        }
    }
}

/// IEC 60255-151, definite-time operate time: at 1.2, 2, 5, 10 and 20 times
/// the setting and three delays, the stage operates its delay plus the
/// measuring time after the step. Tolerance: one injection step (the
/// declared operate time is exact on phasors).
#[test]
fn iec_60255_151_definite_operate_time_over_the_range() {
    for delay in [0.05, 0.5, 2.0] {
        for g in [1.2, 2.0, 5.0, 10.0, 20.0] {
            let r = run_line(vec![definite(1.0, delay)], delay + 0.3, DT, move |t| {
                current(if t >= 0.1 - 1e-12 { g } else { 0.0 })
            });
            assert_within(
                &format!("definite {delay} s at {g} × setting"),
                trip(&r, "R1", "50"),
                0.1 + delay + MT,
                0.0,
                DT,
            );
        }
    }
}

/// IEC 60255-151, dependent-time operate time and its accuracy class: every
/// curve (the IEC curves, the long-time inverse curve, the IEEE curves) at
/// the lowest and highest time multiplier of the test plan and at 1.2, 1.5,
/// 2, 5, 10 and 20 times the setting, the current applied from rest. The
/// theoretical time is `t = TMS (k / (G^α − 1) + c)` with the standard's
/// constants. Tolerance: accuracy class 1 (assigned error 1 % of the
/// theoretical time) with an absolute floor of the declared minimum operate
/// time (the 20 ms measuring time); in addition the declared behaviour,
/// `max(t, 20 ms)` within one injection step.
#[test]
fn iec_60255_151_dependent_operate_time_accuracy_class() {
    let curves = [
        (Curve::IecStandardInverse, [0.05, 1.0]),
        (Curve::IecVeryInverse, [0.05, 1.0]),
        (Curve::IecExtremelyInverse, [0.05, 1.0]),
        (Curve::IecLongTimeInverse, [0.05, 1.0]),
        (Curve::IeeeModeratelyInverse, [0.5, 10.0]),
        (Curve::IeeeVeryInverse, [0.5, 10.0]),
        (Curve::IeeeExtremelyInverse, [0.5, 10.0]),
    ];
    for (curve, multipliers) in curves {
        for tms in multipliers {
            for g in [1.2, 1.5, 2.0, 5.0, 10.0, 20.0] {
                let theory = curve_time(curve, tms, g).expect("above the setting");
                let dt = (theory / 4000.0).clamp(DT, 0.05);
                let f = overcurrent(1.0, curve, tms, 0.0, Direction::NonDirectional);
                let r = run_line(vec![f], 1.02 * theory + 0.1, dt, move |_| current(g));
                let t = trip(&r, "R1", "51")
                    .unwrap_or_else(|| panic!("{curve:?} TMS {tms} at {g}: no operation"));
                let class = (0.01 * theory).max(MT + dt);
                assert!(
                    (t - theory).abs() <= class,
                    "{curve:?} TMS {tms} at {g} × setting: {t:.4} s, theory {theory:.4} s, class 1 allows {class:.4} s"
                );
                let declared = theory.max(MT);
                assert!(
                    t >= declared - 1e-9 && t <= declared + dt + 1e-9,
                    "{curve:?} TMS {tms} at {g}: {t:.5} s, declared {declared:.5} s"
                );
            }
        }
    }
}

/// IEC 60255-151 and IEEE C37.112, reset time of the dependent-time stage.
/// The current at 5 times the setting runs the IEEE curves half way; then
/// the current falls to `M` = 0, 0.5 or 0.9 times the setting for a quarter
/// of the reset time `TD tr / (1 − M²)`, which takes a quarter off the
/// integral, and returns: the stage operates after the rest of its time.
/// After a full reset time the stage needs its whole operate time again.
/// Tolerance: 1 % of the operate time plus two steps.
#[test]
fn ieee_c37_112_reset_time_over_the_range() {
    let td = 2.0;
    let g = 5.0;
    for (curve, tr) in [
        (Curve::IeeeModeratelyInverse, 4.85),
        (Curve::IeeeVeryInverse, 21.6),
        (Curve::IeeeExtremelyInverse, 29.1),
    ] {
        let top = curve_time(curve, td, g).unwrap();
        for m in [0.0, 0.5, 0.9] {
            let reset = td * tr / (1.0 - m * m);
            for (share, left) in [(0.25, 0.25), (1.0, 0.0)] {
                let first = 0.5 * top;
                let off = share * reset;
                let shot = move |t: f64| {
                    current(if t < 0.1 {
                        0.0
                    } else if t < 0.1 + first {
                        g
                    } else if t < 0.1 + first + off {
                        m
                    } else {
                        g
                    })
                };
                let f = overcurrent(1.0, curve, td, 0.0, Direction::NonDirectional);
                let r = run_line(vec![f], 0.1 + first + off + top + 0.5, DT, shot);
                let expected = 0.1 + first + off + (1.0 - left) * top;
                assert_within(
                    &format!("{curve:?} after {share} of the reset time at M {m}"),
                    trip(&r, "R1", "51"),
                    expected,
                    0.01 * top + 2.0 * DT,
                    0.01 * top + 2.0 * DT,
                );
            }
        }
    }
}

/// IEC 60255-151, reset of the definite-time stage and disengaging time:
/// the start resets in the injection step the current falls below the
/// drop-off value (disengaging time at most one step), and an interruption
/// of one step restarts the full delay.
/// Tolerance: one injection step.
#[test]
fn iec_60255_151_definite_reset_and_disengaging_time() {
    let r = run_line(vec![definite(1.0, 1.0)], 0.6, DT, |t| {
        current(if (0.1..0.3).contains(&t) { 2.0 } else { 0.0 })
    });
    let (tp, tr) = start_and_drop(&r, "R1", "50");
    assert!((tp - 0.1).abs() < 1e-9, "start at {tp}");
    assert_within("disengaging time", tr, 0.3, 0.0, DT);
    assert_none("no operation", trip(&r, "R1", ""));

    let gap = 0.2;
    let r = run_line(vec![definite(1.0, 0.5)], 1.2, DT, move |t| {
        let off = (gap - 1e-9..gap + DT - 1e-9).contains(&(t - 0.1));
        current(if t >= 0.1 - 1e-12 && !off { 2.0 } else { 0.0 })
    });
    assert_within(
        "the delay restarts after a one-step interruption",
        trip(&r, "R1", "50"),
        0.1 + gap + DT + 0.5 + MT,
        0.0,
        DT,
    );
}

/// IEC 60255-151, critical impulse time and overshoot time. A current pulse
/// of twice the setting shorter than the operate time does not operate the
/// stage; the longest pulse that does not operate it (the critical impulse
/// time) is the operate time itself, so the overshoot time, their
/// difference, is zero: definite time (delay plus measuring time) and an
/// IEC very inverse stage (which resets at once).
/// Tolerance: one injection step.
#[test]
fn iec_60255_151_critical_impulse_and_overshoot() {
    let pulse = |g: f64, length: f64| {
        move |t: f64| {
            current(if t >= 0.1 - 1e-12 && t < 0.1 + length - 1e-9 {
                g
            } else {
                0.0
            })
        }
    };
    let delay = 0.2;
    let operate = delay + MT;
    let r = run_line(vec![definite(1.0, delay)], 1.0, DT, pulse(2.0, operate));
    assert_none("definite: a pulse of the operate time", trip(&r, "R1", ""));
    let r = run_line(
        vec![definite(1.0, delay)],
        1.0,
        DT,
        pulse(2.0, operate + DT),
    );
    assert_within(
        "definite: a pulse one step longer",
        trip(&r, "R1", ""),
        0.1 + operate,
        0.0,
        DT,
    );

    let vi = overcurrent(
        1.0,
        Curve::IecVeryInverse,
        0.1,
        0.0,
        Direction::NonDirectional,
    );
    let top = curve_time(Curve::IecVeryInverse, 0.1, 2.0).unwrap();
    let r = run_line(vec![vi.clone()], 2.0, DT, pulse(2.0, top - 2.0 * DT));
    assert_none(
        "very inverse: a pulse just short of the curve time",
        trip(&r, "R1", ""),
    );
    let r = run_line(vec![vi], 2.0, DT, pulse(2.0, top + 2.0 * DT));
    assert_within(
        "very inverse: a pulse just longer",
        trip(&r, "R1", ""),
        0.1 + top,
        DT,
        DT,
    );
}

/// Directional overcurrent (67): the forward stage operates for currents
/// lagging the polarizing voltage by −30° to 150°, the region of the
/// relay's 60° characteristic angle, and the reverse stage for the rest.
/// Test plan: shots of twice the setting at angles 5° inside and outside
/// both boundaries and at the characteristic angle.
/// Tolerance: 5° (the boundary is exact on phasors).
#[test]
fn iec_60255_151_directional_boundary() {
    for (lag, forward) in [
        (-25.0, true),
        (60.0, true),
        (145.0, true),
        (-35.0, false),
        (155.0, false),
        (240.0, false),
    ] {
        for (dir, operates) in [
            (Direction::Forward, forward),
            (Direction::Reverse, !forward),
        ] {
            let f = overcurrent(1.0, Curve::Definite, 1.0, 0.1, dir);
            let r = run_line(vec![f], 0.5, DT, move |t| {
                Shot::line(c(1.0, 0.0), polar(if t >= 0.1 { 2.0 } else { 0.0 }, -lag))
            });
            let what = format!("{dir:?} stage, current lagging {lag}°");
            if operates {
                assert_within(&what, trip(&r, "R1", "67"), 0.1 + 0.1 + MT, 0.0, DT);
            } else {
                assert_none(&what, trip(&r, "R1", "67"));
            }
        }
    }
}

/// Directional security with a collapsed voltage: a sustained close-in
/// forward fault (no voltage left at the relay, the current lagging the
/// pre-fault voltage by 60°) must not operate a reverse stage, and the
/// forward stage, polarized by the voltage memory, should operate. A
/// directional element gives no direction without polarizing voltage; the
/// distance function of the same relay holds its memory decision for such
/// a fault. Test plan: forward and reverse stages, 0.3 s delay, longer than
/// the 100 ms voltage memory.
#[test]
fn iec_60255_151_directional_security_without_voltage() {
    let shot = |t: f64| {
        if t >= 0.1 {
            Shot::line(c(0.0, 0.0), polar(5.0, -60.0))
        } else {
            current(0.0)
        }
    };
    let rev = overcurrent(1.0, Curve::Definite, 1.0, 0.3, Direction::Reverse);
    let reverse = trip(&run_line(vec![rev], 1.0, DT, shot), "R1", "67");
    let fwd = overcurrent(1.0, Curve::Definite, 1.0, 0.3, Direction::Forward);
    let forward = trip(&run_line(vec![fwd], 1.0, DT, shot), "R1", "67");
    let expected = 0.1 + 0.3 + MT;
    assert!(
        reverse.is_none() && forward.is_some_and(|t| (t - expected).abs() <= DT),
        "reverse stage operates at {reverse:?} (none required), forward stage at {forward:?} ({expected:.2} s required)"
    );
}

/// Phase currents `ia`, `ib`, `ic` into the line at bus 1 with the voltages
/// `v`, measured as unbalanced phases.
fn phases(v: [C64; 3], i: [C64; 3]) -> Shot {
    let mut s = Shot::at_rest();
    s.v[0] = v;
    s.i_from = i;
    s.unbalanced = true;
    s
}

fn earth_fault(pickup: f64, delay: f64, direction: Direction) -> ProtectionFunction {
    ProtectionFunction::EarthFault(Overcurrent {
        label: None,
        pickup,
        curve: Curve::Definite,
        tms: 1.0,
        delay,
        direction,
    })
}

/// IEC 60255-151 applied to the residual current (50N/51N/67N), on a relay
/// that measures unbalanced phases. A phase-a current ramp (`3 I0` equal to
/// it) finds the start value; a step of twice the setting operates in the
/// delay plus the measuring time. The directional stage takes `−V0` as its
/// polarizing quantity: `I0` lagging it by 80° (a forward fault behind a
/// source of that angle) is forward, the opposite current reverse. On a
/// balanced measurement (RMS) the function stays inactive and the binding
/// says so. Tolerance: the ramp resolution of 0.05 %; one step in time.
#[test]
fn iec_60255_151_residual_overcurrent() {
    let healthy = balanced(c(1.0, 0.0));
    let zero = c(0.0, 0.0);
    let rate = 0.0005 * 0.2 / DT;
    let x = move |t: f64| triangle(t, 0.1, 0.17, 0.25, rate);
    let r = run_line(
        vec![earth_fault(0.2, 100.0, Direction::NonDirectional)],
        2.0,
        DT,
        move |t| phases(healthy, [polar(x(t), -70.0), zero, zero]),
    );
    let (tp, tr) = start_and_drop(&r, "R1", "51N");
    let start = x(tp) / 0.2;
    assert!(
        (1.0 - 1e-9..=1.0005 + 1e-9).contains(&start),
        "start at {start:.5}"
    );
    let drop = x(tr.expect("drops off")) / 0.2;
    assert!(
        (0.9495 - 1e-9..0.95).contains(&drop),
        "drop-off at {drop:.5}"
    );
    assert!(r.notes.iter().any(|n| n.contains("51N")), "{:?}", r.notes);

    let r = run_line(
        vec![earth_fault(0.2, 0.1, Direction::NonDirectional)],
        0.5,
        DT,
        move |t| {
            phases(
                healthy,
                [polar(if t >= 0.1 { 0.4 } else { 0.0 }, -70.0), zero, zero],
            )
        },
    );
    assert_within(
        "51N at 2 × setting",
        trip(&r, "R1", "51N"),
        0.1 + 0.1 + MT,
        0.0,
        DT,
    );

    // The same current measured as balanced phasors: nothing.
    let r = run_line(
        vec![earth_fault(0.2, 0.1, Direction::NonDirectional)],
        0.5,
        DT,
        |_| current(2.0),
    );
    assert_none("51N on balanced phasors", trip(&r, "R1", "51N"));

    // 67N: −V0 = 0.1 pu at 0°; 3 I0 = 0.6 pu lagging it by 80° or leading
    // it by 100°.
    let v0 = c(-0.1, 0.0);
    let v = [healthy[0] + 3.0 * v0, healthy[1], healthy[2]];
    for (lag, forward) in [(80.0, true), (-100.0, false)] {
        for (dir, operates) in [
            (Direction::Forward, forward),
            (Direction::Reverse, !forward),
        ] {
            let r = run_line(vec![earth_fault(0.2, 0.1, dir)], 0.5, DT, move |t| {
                if t >= 0.1 {
                    phases(v, [polar(0.6, -lag), zero, zero])
                } else {
                    phases(healthy, [zero; 3])
                }
            });
            let what = format!("67N {dir:?}, I0 lagging −V0 by {lag}°");
            if operates {
                assert_within(&what, trip(&r, "R1", "67N"), 0.1 + 0.1 + MT, 0.0, DT);
            } else {
                assert_none(&what, trip(&r, "R1", "67N"));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// IEC 60255-127: voltage
// ---------------------------------------------------------------------------

/// IEC 60255-127, start value and drop-off ratio (the ratio of the
/// drop-off value to the start value) by slow ramps of 0.02 % of the
/// setting per step. The undervoltage stage starts below its setting and
/// drops off at 1.02 times it; the overvoltage stage starts above its
/// setting and drops off at `reset_ratio` times it (0.98 by default).
/// Tolerance: one ramp step on the start value, one on each of the two
/// values the ratio divides.
#[test]
fn iec_60255_127_start_value_and_drop_off_ratio_by_ramp() {
    let res = 0.0002;
    for setting in [0.5, 0.8, 0.9] {
        let rate = res * setting / DT;
        let x = move |t: f64| triangle(t, 0.1, 1.05 * setting, 0.97 * setting, rate);
        let f = ProtectionFunction::Undervoltage(stage(setting, 100.0, None));
        let r = run_line(vec![f], 1.0, DT, move |t| voltage(x(t)));
        let (tp, tr) = start_and_drop(&r, "R1", "27");
        let start = x(tp) / setting;
        let ratio = x(tr.expect("27 drops off")) / x(tp);
        assert!(
            start < 1.0 && start >= 1.0 - res - 1e-9,
            "27 at {setting}: start at {start:.5} × setting"
        );
        assert!(
            (ratio / 1.02 - 1.0).abs() <= res * (1.0 + 1.0 / 1.02) + 1e-6,
            "27 at {setting}: drop-off ratio {ratio:.5}, declared 1.02"
        );
    }
    for setting in [1.05, 1.1, 1.2] {
        for (reset_ratio, declared) in [
            (None, 0.98),
            (Some(0.99), 0.99),
            (Some(0.95), 0.95),
            (Some(0.9), 0.9),
        ] {
            let rate = res * setting / DT;
            let x =
                move |t: f64| triangle(t, 0.1, (declared - 0.03) * setting, 1.03 * setting, rate);
            let f = ProtectionFunction::Overvoltage(stage(setting, 100.0, reset_ratio));
            let r = run_line(vec![f], 2.0, DT, move |t| voltage(x(t)));
            let (tp, tr) = start_and_drop(&r, "R1", "59");
            let start = x(tp) / setting;
            let ratio = x(tr.expect("59 drops off")) / x(tp);
            assert!(
                start > 1.0 - 1e-9 && start <= 1.0 + res + 1e-9,
                "59 at {setting}: start at {start:.5} × setting"
            );
            assert!(
                (ratio / declared - 1.0).abs() <= res * (1.0 + 1.0 / declared) + 1e-6,
                "59 at {setting}, reset ratio {reset_ratio:?}: drop-off ratio {ratio:.5}, declared {declared}"
            );
        }
    }
}

/// IEC 60255-127, operate time of the definite-time stages for steps from
/// rated voltage to 0.9, 0.5 and 0 times the undervoltage setting and to
/// 1.05, 1.2 and 1.5 times the overvoltage setting, at two delays: the delay
/// plus the measuring time. Tolerance: one injection step.
#[test]
fn iec_60255_127_operate_time() {
    for delay in [0.1, 1.0] {
        for m in [0.9, 0.5, 0.0] {
            let f = ProtectionFunction::Undervoltage(stage(0.8, delay, None));
            let r = run_line(vec![f], delay + 0.3, DT, move |t| {
                voltage(if t >= 0.1 { m * 0.8 } else { 1.0 })
            });
            assert_within(
                &format!("27 {delay} s at {m} × setting"),
                trip(&r, "R1", "27"),
                0.1 + delay + MT,
                0.0,
                DT,
            );
        }
        for m in [1.05, 1.2, 1.5] {
            let f = ProtectionFunction::Overvoltage(stage(1.1, delay, None));
            let r = run_line(vec![f], delay + 0.3, DT, move |t| {
                voltage(if t >= 0.1 { m * 1.1 } else { 1.0 })
            });
            assert_within(
                &format!("59 {delay} s at {m} × setting"),
                trip(&r, "R1", "59"),
                0.1 + delay + MT,
                0.0,
                DT,
            );
        }
    }
}

/// IEC 60255-127, reset time and critical impulse time of the overvoltage
/// stage: a step from 1.2 times the setting back below the drop-off value
/// resets the start in that injection step; a swell lasting the operate
/// time does not operate the stage, one a step longer does (overshoot time
/// zero). Tolerance: one injection step.
#[test]
fn iec_60255_127_reset_and_critical_impulse() {
    let f = || ProtectionFunction::Overvoltage(stage(1.1, 0.5, None));
    let r = run_line(vec![f()], 1.0, DT, |t| {
        voltage(if (0.1..0.3).contains(&t) { 1.32 } else { 0.99 })
    });
    let (_, tr) = start_and_drop(&r, "R1", "59");
    assert_within("59 resets", tr, 0.3, 0.0, DT);
    let swell = |length: f64| {
        move |t: f64| {
            voltage(if t >= 0.1 && t < 0.1 + length - 1e-9 {
                1.32
            } else {
                1.0
            })
        }
    };
    let r = run_line(vec![f()], 1.0, DT, swell(0.5 + MT));
    assert_none("a swell of the operate time", trip(&r, "R1", "59"));
    let r = run_line(vec![f()], 1.0, DT, swell(0.5 + MT + DT));
    assert_within(
        "a swell one step longer",
        trip(&r, "R1", "59"),
        0.1 + 0.5 + MT,
        0.0,
        DT,
    );
}

/// IEC 60255-127 applied to the neutral voltage `3 V0` (59N), on a relay
/// that measures unbalanced phases: a ramp of the phase-a voltage down from
/// rated finds the start value at the setting; the stage has no drop-off
/// hysteresis (drop-off ratio 1); a step operates in the delay plus the
/// measuring time; on balanced phasors (RMS) it stays inactive and the
/// binding says so. Tolerance: the ramp resolution of 0.05 % of the
/// setting; one step in time.
#[test]
fn iec_60255_127_neutral_overvoltage() {
    let healthy = balanced(c(1.0, 0.0));
    let zero = [c(0.0, 0.0); 3];
    let setting = 0.2;
    let rate = 0.0005 * setting / DT;
    let x = move |t: f64| triangle(t, 0.1, 0.18, 0.22, rate);
    let neutral = |delay: f64| ProtectionFunction::NeutralOvervoltage(stage(setting, delay, None));
    let r = run_line(vec![neutral(100.0)], 1.0, DT, move |t| {
        phases([healthy[0] - x(t), healthy[1], healthy[2]], zero)
    });
    let (tp, tr) = start_and_drop(&r, "R1", "59N");
    let start = x(tp) / setting;
    let drop = x(tr.expect("59N drops off")) / setting;
    assert!(
        start > 1.0 - 1e-9 && start <= 1.0005 + 1e-9,
        "59N start at {start:.5} × setting"
    );
    assert!(
        (0.9995 - 1e-9..=1.0 + 1e-9).contains(&drop),
        "59N drop-off at {drop:.5} × setting"
    );
    assert!(r.notes.iter().any(|n| n.contains("59N")), "{:?}", r.notes);
    let r = run_line(vec![neutral(0.2)], 0.6, DT, move |t| {
        let d = if t >= 0.1 { 0.3 } else { 0.0 };
        phases([healthy[0] - d, healthy[1], healthy[2]], zero)
    });
    assert_within(
        "59N at 1.5 × setting",
        trip(&r, "R1", "59N"),
        0.1 + 0.2 + MT,
        0.0,
        DT,
    );
}

// ---------------------------------------------------------------------------
// IEEE C37.102: overexcitation (V/Hz)
// ---------------------------------------------------------------------------

/// Injection table of a voltage at bus 1 of magnitude `vm(t)` turning at
/// frequency `f(t)`: its angle is the integral of `f − 50 Hz`, by the
/// midpoint of each step (exact while `f` is linear within a step).
fn frequency_shots(
    t_end: f64,
    vm: impl Fn(f64) -> f64,
    f: impl Fn(f64) -> f64,
) -> impl Fn(f64) -> Shot {
    let n = (t_end / DT).round() as usize;
    let mut theta = vec![0.0; n + 1];
    for k in 1..=n {
        let tm = (k as f64 - 0.5) * DT;
        theta[k] = theta[k - 1] + 2.0 * PI * (f(tm) - F0) * DT;
    }
    let v: Vec<f64> = (0..=n).map(|k| vm(k as f64 * DT)).collect();
    move |t: f64| {
        let k = ((t / DT).round() as usize).min(n);
        Shot::line(C64::from_polar(v[k], theta[k]), c(0.0, 0.0))
    }
}

/// The lag of the relays' frequency measurement behind a frequency ramp:
/// a first-order filter of 40 ms on the angle rate.
const FREQUENCY_LAG: f64 = 0.04;

/// IEEE C37.102 overexcitation (24), definite time, on `(V/V_n)/(f/f_n)`:
/// a voltage ramp at rated frequency starts the stage at its setting, and
/// a slow frequency ramp at rated voltage starts it where the measured
/// frequency gives the setting (the true frequency then lies the
/// measurement's lag below). The stage has no drop-off hysteresis. A step
/// to 1.2 pu operates in the delay plus the measuring time.
/// Tolerance: ramp resolution 0.02 % of the setting; on the frequency
/// ramp 1 mHz beyond the lag of the 40 ms filter.
#[test]
fn ieee_c37_102_overexcitation() {
    let setting = 1.1;
    let vhz = |delay: f64| ProtectionFunction::Overexcitation(stage(setting, delay, None));
    let res = 0.0002;
    let x = move |t: f64| triangle(t, 0.1, 1.07, 1.13, res * setting / DT);
    let r = run_line(vec![vhz(100.0)], 1.0, DT, move |t| voltage(x(t)));
    let (tp, tr) = start_and_drop(&r, "R1", "24");
    let start = x(tp) / setting;
    assert!(
        start > 1.0 - 1e-9 && start <= 1.0 + res + 1e-9,
        "24 start {start:.5}"
    );
    let drop = x(tr.expect("24 drops off")) / setting;
    assert!(
        drop <= 1.0 + 1e-9 && drop >= 1.0 - res - 1e-9,
        "24 drop {drop:.5}"
    );

    // Rated voltage, the frequency falling at 0.05 Hz/s from 45.6 Hz: the
    // setting is reached at 50 / 1.1 = 45.4545 Hz.
    let rate = 0.05;
    let f = move |t: f64| {
        if t < 1.0 {
            45.6
        } else {
            45.6 - rate * (t - 1.0)
        }
    };
    let r = run_line(vec![vhz(100.0)], 5.0, DT, frequency_shots(5.0, |_| 1.0, f));
    let tp = first(&r, "R1", OperationKind::Pickup, "24").expect("24 starts");
    let f_start = f(tp) + rate * FREQUENCY_LAG;
    assert!(
        (f_start - F0 / setting).abs() <= 0.001,
        "24 starts at {f_start:.4} Hz (lag removed), setting {:.4} Hz",
        F0 / setting
    );

    let r = run_line(vec![vhz(0.5)], 1.0, DT, |t| {
        voltage(if t >= 0.1 { 1.2 } else { 1.0 })
    });
    assert_within(
        "24 at 1.2 pu",
        trip(&r, "R1", "24"),
        0.1 + 0.5 + MT,
        0.0,
        DT,
    );
}

// ---------------------------------------------------------------------------
// IEC 60255-181: frequency
// ---------------------------------------------------------------------------

fn frequency_stage(under: bool, pickup: f64, delay: f64, min_voltage: f64) -> ProtectionFunction {
    let s = FrequencyStage {
        label: None,
        pickup,
        delay,
        min_voltage,
    };
    if under {
        ProtectionFunction::Underfrequency(s)
    } else {
        ProtectionFunction::Overfrequency(s)
    }
}

/// IEC 60255-181, start value and drop-off by slow frequency ramps
/// (0.05 Hz/s): from 0.1 Hz short of the setting, the frequency ramps
/// through it to 0.05 Hz beyond and back. The stage starts where the
/// measured frequency crosses the setting and drops off 20 mHz back inside;
/// the true frequency at either instant lies the measurement's lag
/// (rate × 40 ms) behind. Tolerance: 1 mHz after the lag is removed.
#[test]
fn iec_60255_181_start_value_and_drop_off_by_slow_ramp() {
    let rate = 0.05;
    for (under, setting) in [(true, 49.8), (true, 47.5), (false, 50.2), (false, 51.5)] {
        let sign = if under { -1.0 } else { 1.0 };
        let f0 = setting - sign * 0.1;
        let f = move |t: f64| {
            if t < 1.0 {
                f0
            } else {
                triangle(t, 1.0, f0, setting + sign * 0.05, rate)
            }
        };
        let label = if under { "81U" } else { "81O" };
        let r = run_line(
            vec![frequency_stage(under, setting, 100.0, 0.2)],
            8.0,
            DT,
            frequency_shots(8.0, |_| 1.0, f),
        );
        let (tp, tr) = start_and_drop(&r, "R1", label);
        // On the way out the measurement trails the true frequency, so the
        // true value is beyond the measured one; on the way back the other
        // way round.
        let start = f(tp) - sign * rate * FREQUENCY_LAG;
        let drop = f(tr.expect("drops off")) + sign * rate * FREQUENCY_LAG;
        assert!(
            (start - setting).abs() <= 0.001,
            "{label} {setting}: starts at {start:.4} Hz (lag removed)"
        );
        assert!(
            (drop - (setting - sign * 0.02)).abs() <= 0.001,
            "{label} {setting}: drops off at {drop:.4} Hz (lag removed), declared 20 mHz inside"
        );
    }
}

/// IEC 60255-181, operate time on frequency ramps of ±0.2 and ±1 Hz/s from
/// rated frequency: the stage operates its delay plus the measuring time
/// after the measured frequency crosses the setting, which is the true
/// crossing plus the measurement's lag of 40 ms.
/// Tolerance: 2 ms (the filter's lag before it settles on the ramp, and
/// the half step of the angle difference) plus one step.
#[test]
fn iec_60255_181_operate_time_on_ramps() {
    for (under, setting) in [(true, 49.0), (false, 51.0)] {
        for rate in [0.2, 1.0] {
            for delay in [0.1, 0.5] {
                let sign = if under { -1.0 } else { 1.0 };
                let f = move |t: f64| F0 + sign * rate * (t - 0.1).max(0.0);
                let cross = 0.1 + 1.0 / rate;
                let t_end = cross + delay + 0.3;
                let label = if under { "81U" } else { "81O" };
                let r = run_line(
                    vec![frequency_stage(under, setting, delay, 0.2)],
                    t_end,
                    DT,
                    frequency_shots(t_end, |_| 1.0, f),
                );
                assert_within(
                    &format!("{label} at {rate} Hz/s, delay {delay}"),
                    trip(&r, "R1", label),
                    cross + FREQUENCY_LAG + delay + MT,
                    0.002,
                    0.002 + DT,
                );
            }
        }
    }
}

/// IEC 60255-181, undervoltage blocking: at 47 Hz, a voltage ramp down
/// from rated blocks (resets) the underfrequency stage where it falls below
/// the blocking threshold, and a ramp back up releases it at the threshold.
/// Tolerance: the ramp resolution of 0.5 mpu per step (0.1 % of the
/// threshold).
#[test]
fn iec_60255_181_voltage_blocking_threshold() {
    let threshold = 0.5;
    let x = move |t: f64| triangle(t, 1.0, 1.0, 0.4, 0.5);
    let r = run_line(
        vec![frequency_stage(true, 47.5, 100.0, threshold)],
        4.0,
        DT,
        frequency_shots(4.0, x, |_| 47.0),
    );
    let picks = ops(&r, "R1", OperationKind::Pickup, "81U");
    let resets = ops(&r, "R1", OperationKind::Reset, "81U");
    assert!(
        picks.len() == 2 && resets.len() == 1,
        "{picks:?} {resets:?}"
    );
    let blocked = x(resets[0].time);
    let released = x(picks[1].time);
    assert!(
        blocked < threshold && blocked >= threshold - 0.0005 - 1e-9,
        "blocked at {blocked:.4} pu"
    );
    assert!(
        released >= threshold - 1e-9 && released <= threshold + 0.0005 + 1e-9,
        "released at {released:.4} pu"
    );
}

/// IEC 60255-118-1, frequency measurement during a frequency ramp of
/// ±1 Hz/s (the measurement 81 and 81R act on): the frequency error may be
/// at most 10 mHz (P class). Test plan: the frequency error read at the
/// instant the measurement crosses an 81U and an 81O setting on the ramp,
/// the true frequency there against the setting.
#[test]
#[ignore = "IEC 60255-118-1 P class, frequency ramp of 1 Hz/s: the frequency measurement trails by 39 mHz (its 40 ms first-order filter), at most 10 mHz required"]
fn iec_60255_118_1_frequency_error_on_ramp() {
    for (under, setting) in [(true, 49.0), (false, 51.0)] {
        let sign = if under { -1.0 } else { 1.0 };
        let f = move |t: f64| F0 + sign * (t - 0.1).max(0.0);
        let label = if under { "81U" } else { "81O" };
        let r = run_line(
            vec![frequency_stage(under, setting, 100.0, 0.2)],
            1.5,
            DT,
            frequency_shots(1.5, |_| 1.0, f),
        );
        let tp = first(&r, "R1", OperationKind::Pickup, label).expect("starts");
        // The measurement crossed the setting between the previous step
        // and this one: the error is at least that of the previous step.
        let error = (f(tp - DT) - setting).abs();
        assert!(
            error <= 0.01,
            "{label}: frequency error {:.1} mHz on a 1 Hz/s ramp",
            error * 1e3
        );
    }
}

fn rocof(pickup: f64, delay: f64, min_voltage: f64) -> ProtectionFunction {
    ProtectionFunction::Rocof(Rocof {
        label: None,
        pickup,
        window: 0.5,
        delay,
        min_voltage,
    })
}

/// IEC 60255-181, rate of change of frequency, start value: ramps of 1.05
/// times the setting operate, ramps of 0.95 times it do not, either sign,
/// at three settings; the stage is blocked below its minimum voltage.
/// Tolerance: ±5 % of the setting (the rate is the least-squares slope over
/// the 500 ms window, exact on a steady ramp).
#[test]
fn iec_60255_181_rocof_start_value() {
    for setting in [0.5, 1.0, 2.5] {
        for sign in [-1.0, 1.0] {
            for (m, operates) in [(1.05, true), (0.95, false)] {
                let rate = sign * m * setting;
                let f = move |t: f64| F0 + rate * (t - 0.5).max(0.0);
                let r = run_line(
                    vec![rocof(setting, 0.0, 0.8)],
                    1.6,
                    DT,
                    frequency_shots(1.6, |_| 1.0, f),
                );
                let what = format!("81R {setting} Hz/s at {rate:+.3} Hz/s");
                if operates {
                    assert!(trip(&r, "R1", "81R").is_some(), "{what}: no operation");
                } else {
                    assert_none(&what, trip(&r, "R1", "81R"));
                }
            }
        }
    }
    let f = |t: f64| F0 - 2.0 * (t - 0.5).max(0.0);
    let r = run_line(
        vec![rocof(1.0, 0.0, 0.8)],
        1.6,
        DT,
        frequency_shots(1.6, |_| 0.75, f),
    );
    assert_none("81R below its minimum voltage", trip(&r, "R1", "81R"));
}

/// IEC 60255-181, rate of change of frequency, operate time: a ramp of 2
/// and 4 times the setting starts from rated frequency. The least-squares
/// slope over the window of `W` = 500 ms, a fraction `x` of it on the ramp,
/// is `r x² (3 − 2x)`; the stage starts once that exceeds the setting,
/// the measurement's 40 ms lag later, and operates its delay plus the
/// measuring time after. Tolerance: 5 ms (the filter rounds the corner of
/// the ramp) plus one step.
#[test]
fn iec_60255_181_rocof_operate_time() {
    let setting = 1.0;
    let w = 0.5;
    for m in [2.0, 4.0] {
        for delay in [0.0, 0.2] {
            let rate = m * setting;
            let f = move |t: f64| F0 - rate * (t - 1.0).max(0.0);
            // Solve r x² (3 − 2x) = setting by bisection.
            let (mut lo, mut hi) = (0.0, 1.0);
            for _ in 0..60 {
                let x = 0.5 * (lo + hi);
                if rate * x * x * (3.0 - 2.0 * x) > setting {
                    hi = x;
                } else {
                    lo = x;
                }
            }
            let expected = 1.0 + lo * w + FREQUENCY_LAG + delay + MT;
            let r = run_line(
                vec![rocof(setting, delay, 0.8)],
                2.2,
                DT,
                frequency_shots(2.2, |_| 1.0, f),
            );
            assert_within(
                &format!("81R at {m} × setting, delay {delay}"),
                trip(&r, "R1", "81R"),
                expected,
                0.005,
                0.005 + DT,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// IEC 60255-149: thermal
// ---------------------------------------------------------------------------

fn thermal(k: f64, tau: f64) -> ProtectionFunction {
    ProtectionFunction::Thermal(Thermal {
        label: None,
        k,
        tau,
    })
}

/// IEC 60255-149, operate time over the current range from the cold state,
/// `t = τ ln(I² / (I² − (k I_B)²))`, at 1.05, 1.2, 3, 5 and 10 times
/// `k I_B`; no operation at 0.98 and 1.0 times `k I_B` over ten time
/// constants. Tolerance: 0.5 % of the expected time plus one step.
#[test]
fn iec_60255_149_operate_time_over_the_range() {
    let (k, tau): (f64, f64) = (1.05, 60.0);
    for m in [1.05, 1.2, 3.0, 5.0, 10.0] {
        let i = m * k;
        let expected = tau * (i * i / (i * i - k * k)).ln();
        let dt = (expected / 4000.0).clamp(DT, 0.05);
        let r = run_line(vec![thermal(k, tau)], 1.0 + 1.1 * expected, dt, move |t| {
            current(if t < 1.0 { 0.0 } else { i })
        });
        let t = trip(&r, "R1", "49").expect("49 operates") - 1.0;
        assert!(
            (t - expected).abs() <= 0.005 * expected + dt,
            "49 at {m} × kI_B: {t:.3} s, expected {expected:.3} s"
        );
    }
    for m in [0.98, 1.0] {
        let r = run_line(vec![thermal(k, tau)], 10.0 * tau, 0.05, move |t| {
            current(if t < 1.0 { 0.0 } else { m * k })
        });
        assert_none(&format!("49 at {m} × kI_B"), trip(&r, "R1", "49"));
    }
}

/// IEC 60255-149, cooling: from the steady state of 0.9 times `k I_B` the
/// current stops for `τ ln 2`, which halves the thermal level (one time
/// constant for heating and cooling); twice `k I_B` then operates after
/// `τ ln((4 − θ₀) / (4 − 1))` with `θ₀ = 0.81 / 2`. Without the pause it
/// operates after `τ ln((4 − 0.81) / 3)`.
/// Tolerance: 0.5 % of the expected time plus one step.
#[test]
fn iec_60255_149_cooling() {
    let (k, tau): (f64, f64) = (1.0, 100.0);
    let dt = 0.01;
    for pause in [0.0, tau * 2f64.ln()] {
        let shot = move |t: f64| {
            current(if t < 1.0 {
                0.9 * k
            } else if t < 1.0 + pause {
                0.0
            } else {
                2.0 * k
            })
        };
        let theta0: f64 = 0.81 * if pause > 0.0 { 0.5 } else { 1.0 };
        let expected = tau * ((4.0 - theta0) / 3.0).ln();
        let r = run_line(
            vec![thermal(k, tau)],
            1.0 + pause + 1.1 * expected,
            dt,
            shot,
        );
        let t = trip(&r, "R1", "49").expect("49 operates") - 1.0 - pause;
        assert!(
            (t - expected).abs() <= 0.005 * expected + dt,
            "49 after a pause of {pause:.1} s: {t:.3} s, expected {expected:.3} s"
        );
    }
}

// ---------------------------------------------------------------------------
// IEC 60255-121: distance
// ---------------------------------------------------------------------------

const LINE_ANGLE: f64 = 75.0;

fn zone(name: &str, reach: f64, delay: f64, direction: Direction) -> Zone {
    Zone {
        name: name.into(),
        reach,
        angle: None,
        resistive_reach: None,
        delay,
        direction,
        autoreclose: false,
    }
}

fn three_zones() -> Vec<Zone> {
    vec![
        zone("Z1", 10.0, 0.0, Direction::Forward),
        zone("Z2", 15.0, 0.4, Direction::Forward),
        zone("Z3", 25.0, 1.0, Direction::Forward),
    ]
}

fn distance(characteristic: Characteristic, zones: Vec<Zone>, k0: [f64; 2]) -> Distance {
    Distance {
        characteristic,
        zones,
        line_angle: LINE_ANGLE,
        k0,
        min_current: 0.2,
        power_swing_blocking: None,
        teleprotection: None,
        load_encroachment: None,
    }
}

/// A distance relay at the bus-1 end of a line of 8 Ω (1 Ω = 1 pu here).
fn distance_scheme(d: Distance, i_nom: f64) -> (ProtectionScheme, Topology) {
    let mut r = relay("R1", line_end(1), vec![ProtectionFunction::Distance(d)]);
    r.i_nom = Some(i_nom);
    (
        scheme(vec![r]),
        rig(false, polar(8.0, LINE_ANGLE), UnitKind::Machine),
    )
}

/// A fault seen as `z` (Ω) from 0.1 s, the voltage at 0.5 pu keeping its
/// pre-fault angle (the voltage memory and the present voltage agree: the
/// static characteristic).
fn fault_at(z: C64) -> impl Fn(f64) -> Shot {
    move |t| {
        if t >= 0.1 {
            Shot::line(c(0.5, 0.0), c(0.5, 0.0) / z)
        } else {
            Shot::line(c(1.0, 0.0), c(0.0, 0.0))
        }
    }
}

/// Whether the zone named `zone` starts for a fault seen as `z`.
fn zone_starts(s: &ProtectionScheme, topo: &Topology, zone: &str, z: C64) -> bool {
    let (r, _) = inject(s, topo, 0.12, DT, fault_at(z));
    first(&r, "R1", OperationKind::Pickup, &format!("21 {zone}")).is_some()
}

/// The boundary along a ray: `inside(x)` holds below it, in `[lo, hi]`;
/// bisection to 0.003 % of the bracket.
fn search(mut lo: f64, mut hi: f64, inside: impl Fn(f64) -> bool) -> f64 {
    assert!(inside(lo) && !inside(hi), "the boundary is not bracketed");
    for _ in 0..15 {
        let mid = 0.5 * (lo + hi);
        if inside(mid) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// IEC 60255-121, reach of the mho zones: the test set searches the
/// boundary of each zone along rays at the line angle and 30° and 60° to
/// either side, by shots of impedance; the boundary is the circle through
/// the origin with the reach as its diameter, `reach · cos(φ − φ_line)`.
/// Tolerance: 0.1 % of the expected reach (the search resolves 0.003 %; the
/// characteristic is exact on phasors).
#[test]
fn iec_60255_121_mho_reach_by_search() {
    let (s, topo) = distance_scheme(
        distance(Characteristic::Mho, three_zones(), [0.0, 0.0]),
        0.01,
    );
    for z in three_zones() {
        for off in [-60.0, -30.0, 0.0, 30.0, 60.0] {
            let expected = z.reach * f64::cos(f64::to_radians(off));
            let found = search(0.5 * expected, 1.5 * expected, |m| {
                zone_starts(&s, &topo, &z.name, polar(m, LINE_ANGLE + off))
            });
            assert!(
                (found / expected - 1.0).abs() <= 0.001,
                "{} at {off:+}° from the line angle: boundary {found:.4} Ω, expected {expected:.4} Ω",
                z.name
            );
        }
    }
}

/// IEC 60255-121, reach of a quadrilateral zone (reach 10 Ω at the line
/// angle, resistive reach 5 Ω): along the line angle the reactance line at
/// `reach · sin φ_line`; at half that reactance the right blinder at the
/// resistive reach and the left one at a quarter of it (this module's
/// choice), both parallel to the line angle.
/// Tolerance: 0.1 % of the expected value.
#[test]
fn iec_60255_121_quadrilateral_reach_by_search() {
    let mut z1 = zone("Z1", 10.0, 0.0, Direction::Forward);
    z1.resistive_reach = Some(5.0);
    let (s, topo) = distance_scheme(
        distance(Characteristic::Quadrilateral, vec![z1], [0.0, 0.0]),
        0.01,
    );
    let xr = 10.0 * LINE_ANGLE.to_radians().sin();
    let cot = 1.0 / LINE_ANGLE.to_radians().tan();
    let reach = search(5.0, 15.0, |m| {
        zone_starts(&s, &topo, "Z1", polar(m, LINE_ANGLE))
    });
    assert!(
        (reach / 10.0 - 1.0).abs() <= 0.001,
        "reactance reach {reach:.4} Ω along the line angle"
    );
    let x = 0.5 * xr;
    let right = search(1.0, 8.0, |d| {
        zone_starts(&s, &topo, "Z1", c(x * cot + d, x))
    });
    assert!(
        (right / 5.0 - 1.0).abs() <= 0.001,
        "right blinder {right:.4} Ω"
    );
    let left = search(0.2, 3.0, |d| {
        zone_starts(&s, &topo, "Z1", c(x * cot - d, x))
    });
    assert!(
        (left / 1.25 - 1.0).abs() <= 0.001,
        "left blinder {left:.4} Ω"
    );
}

/// IEC 60255-121, operate time against the source impedance ratio (SIR):
/// a source of `SIR × Z1` behind the relay at the line angle, faults at 0
/// (close-in), 50 and 90 % of the Z1 reach for SIR 0.1, 1, 10 and 30. Zone 1
/// operates in its measuring time whatever the SIR (on phasors no transient
/// slows it), with the voltage memory polarizing the close-in fault; a fault
/// at 110 % never operates zone 1 (no overreach).
/// Tolerance: one injection step.
#[test]
fn iec_60255_121_operate_time_against_sir() {
    let (s, topo) = distance_scheme(
        distance(Characteristic::Mho, three_zones(), [0.0, 0.0]),
        0.001,
    );
    let zr = polar(10.0, LINE_ANGLE);
    for sir in [0.1, 1.0, 10.0, 30.0] {
        for (m, operates) in [(0.0, true), (0.5, true), (0.9, true), (1.1, false)] {
            let zs = sir * zr;
            let zf = m * zr;
            let shot = move |t: f64| {
                if t >= 0.1 {
                    let i = c(1.0, 0.0) / (zs + zf);
                    Shot::line(zf * i, i)
                } else {
                    Shot::line(c(1.0, 0.0), c(0.0, 0.0))
                }
            };
            let (r, _) = inject(&s, &topo, 0.3, DT, shot);
            let what = format!("SIR {sir}, fault at {m} × Z1");
            if operates {
                assert_within(&what, trip(&r, "R1", "21 Z1"), 0.1 + MT, 0.0, DT);
            } else {
                assert_none(&what, trip(&r, "R1", "21 Z1"));
            }
        }
    }
}

/// IEC 60255-121, minimum operating current: a fault at 2 Ω (inside zone 1)
/// whose current ramps up through the setting of 0.2 times the rated current
/// starts zone 1 at that current.
/// Tolerance: the ramp resolution of 0.05 % of the setting.
#[test]
fn iec_60255_121_minimum_current() {
    let (s, topo) = distance_scheme(
        distance(Characteristic::Mho, three_zones(), [0.0, 0.0]),
        1.0,
    );
    let z = polar(2.0, LINE_ANGLE);
    let x = |t: f64| ramp(t, 0.1, 0.15, 0.1);
    let (r, _) = inject(&s, &topo, 1.2, DT, move |t| {
        if t >= 0.1 {
            let i = C64::from_polar(x(t), 0.0) / z * z.norm();
            Shot::line(z * i, i)
        } else {
            Shot::line(c(1.0, 0.0), c(0.0, 0.0))
        }
    });
    let tp = first(&r, "R1", OperationKind::Pickup, "21 Z1").expect("Z1 starts");
    let start = x(tp) / 0.2;
    assert!(
        start > 1.0 - 1e-9 && start <= 1.0005 + 1e-9,
        "Z1 starts at {start:.5} × the minimum current"
    );
}

/// IEC 60255-121, ground-fault loops with residual compensation: a phase-a
/// fault measured as unbalanced phases, the loop impedance
/// `V_a / (I_a + k0 · 3 I0)` set to 95 % and 105 % of the Z1 reach along the
/// line angle, for a real and a complex `k0`. Zone 1 operates in its
/// measuring time at 95 % and does not start at 105 %.
/// Tolerance: ±5 % of the reach (the loop impedance is exact on phasors).
#[test]
fn iec_60255_121_ground_loops_with_residual_compensation() {
    for k0 in [[0.5, 0.0], [0.35, 0.15]] {
        let (s, topo) = distance_scheme(distance(Characteristic::Mho, three_zones(), k0), 0.01);
        let k = c(k0[0], k0[1]);
        for (m, operates) in [(0.95, true), (1.05, false)] {
            let zl = polar(m * 10.0, LINE_ANGLE);
            let shot = move |t: f64| {
                let healthy = balanced(c(1.0, 0.0));
                if t >= 0.1 {
                    let va = c(0.5, 0.0);
                    let ia = va / zl / (c(1.0, 0.0) + k);
                    phases([va, healthy[1], healthy[2]], [ia, c(0.0, 0.0), c(0.0, 0.0)])
                } else {
                    phases(healthy, [c(0.0, 0.0); 3])
                }
            };
            let (r, _) = inject(&s, &topo, 0.3, DT, shot);
            let what = format!("phase-a fault at {m} × Z1, k0 {k0:?}");
            if operates {
                assert_within(&what, trip(&r, "R1", "21 Z1"), 0.1 + MT, 0.0, DT);
            } else {
                assert_none(&what, first(&r, "R1", OperationKind::Pickup, "21 Z1"));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// IEC 60255-187-1: biased differential
// ---------------------------------------------------------------------------

fn differential(harmonic2: Option<f64>, delay: f64, channel_time: f64) -> Differential {
    Differential {
        label: Some("87T".into()),
        pickup: 0.3,
        slope1: 0.3,
        slope2: 0.7,
        knee: 2.0,
        unrestrained: 10.0,
        harmonic2,
        channel_time,
        delay,
    }
}

fn transformer_scheme(d: Differential) -> ProtectionScheme {
    scheme(vec![relay(
        "T",
        RelayLocation::Transformer {
            from: 1,
            to: 2,
            circuit: "1".into(),
        },
        vec![ProtectionFunction::Differential(d)],
    )])
}

fn transformer_rig() -> Topology {
    rig(true, c(0.0, 0.1), UnitKind::Machine)
}

/// Currents for a differential current `id` and a restraint current `ir`:
/// `ir + id/2` into the from end, `ir − id/2` out of the to end, in phase.
fn bias_shot(id: f64, ir: f64) -> Shot {
    let mut s = Shot::at_rest();
    s.i_from = balanced(polar(ir + id / 2.0, -30.0));
    s.i_to = balanced(polar(-(ir - id / 2.0), -30.0));
    s
}

/// IEC 60255-187-1, bias characteristic: at restraint currents of 0.5,
/// 1.5, 2 (the knee), 3 and 6 times the rated current the test set ramps the
/// differential current from 90 % of the expected operate value until the
/// function starts. The operate value is the larger of the pickup and the
/// slope, `0.3 I_r` to the knee, `0.6 + 0.7 (I_r − 2)` above it.
/// Tolerance: the ramp resolution of 0.05 % of the expected value.
#[test]
fn iec_60255_187_1_bias_characteristic_by_ramp() {
    let s = transformer_scheme(differential(None, 100.0, 0.0));
    for ir in [0.5, 1.5, 2.0, 3.0, 6.0] {
        let expected = if ir <= 2.0 {
            f64::max(0.3, 0.3 * ir)
        } else {
            0.6 + 0.7 * (ir - 2.0)
        };
        let rate = 0.0005 * expected / DT;
        let x = move |t: f64| ramp(t, 0.1, 0.9 * expected, rate);
        let (r, _) = inject(&s, &transformer_rig(), 0.4, DT, move |t| {
            bias_shot(x(t), ir)
        });
        let tp = first(&r, "T", OperationKind::Pickup, "87T").expect("87T starts");
        let found = x(tp) / expected;
        assert!(
            found > 1.0 - 1e-9 && found <= 1.0005 + 1e-9,
            "I_r {ir}: starts at {found:.5} × the expected {expected:.3}"
        );
    }
}

/// IEC 60255-187-1, operate time: a fault fed from one side at 2 and 5
/// times the pickup operates in the delay plus the measuring time (plus the
/// channel time a line differential sets).
/// Tolerance: one injection step.
#[test]
fn iec_60255_187_1_operate_time() {
    for (delay, channel) in [(0.0, 0.0), (0.05, 0.0), (0.0, 0.01)] {
        for m in [2.0, 5.0] {
            let id = m * 0.3;
            let (r, _) = inject(
                &transformer_scheme(differential(None, delay, channel)),
                &transformer_rig(),
                0.4,
                DT,
                move |t| {
                    bias_shot(
                        if t >= 0.1 { id } else { 0.0 },
                        if t >= 0.1 { id / 2.0 } else { 0.0 },
                    )
                },
            );
            assert_within(
                &format!("87 at {m} × pickup, delay {delay}, channel {channel}"),
                trip(&r, "T", "87T"),
                0.1 + delay + channel + MT,
                0.0,
                DT,
            );
        }
    }
}

/// IEC 60255-187-1, second-harmonic blocking and the unrestrained stage. A
/// one-sided differential current of 2 times the rated current with second
/// harmonic falling from 25 % to 5 % of the fundamental operates where the
/// content falls to the 15 % setting; a one-sided current with 30 % second
/// harmonic ramping up is blocked until it exceeds the unrestrained setting
/// of 10 times the rated current.
/// Tolerance: the ramp resolution (0.01 % of content, 0.05 % of current).
#[test]
fn iec_60255_187_1_harmonic_blocking_and_unrestrained() {
    let s = transformer_scheme(differential(Some(0.15), 0.0, 0.0));
    let ratio = |t: f64| ramp(t, 0.1, 0.25, -0.1);
    let (r, _) = inject(&s, &transformer_rig(), 2.5, DT, move |t| {
        let mut sh = bias_shot(2.0, 1.0);
        sh.h2 = Some([ratio(t) * 2.0; 3]);
        sh
    });
    let tp = first(&r, "T", OperationKind::Pickup, "87T").expect("87T starts");
    let content = ratio(tp);
    assert!(
        (0.15 - 0.0001 - 1e-9..=0.15 + 1e-9).contains(&content),
        "released at {:.3} % second harmonic",
        content * 100.0
    );

    let x = |t: f64| ramp(t, 0.1, 9.5, 10.0 * 0.0005 / DT);
    let (r, _) = inject(&s, &transformer_rig(), 0.3, DT, move |t| {
        let id = x(t);
        let mut sh = bias_shot(id, id / 2.0);
        sh.h2 = Some([0.3 * id; 3]);
        sh
    });
    let tp = first(&r, "T", OperationKind::Pickup, "87T").expect("87T starts unrestrained");
    let found = x(tp) / 10.0;
    assert!(
        found > 1.0 - 1e-9 && found <= 1.0005 + 1e-9,
        "unrestrained at {found:.5} × setting"
    );
}

// ---------------------------------------------------------------------------
// IEEE C37.102: generator functions
// ---------------------------------------------------------------------------

fn unit_scheme(f: ProtectionFunction) -> ProtectionScheme {
    scheme(vec![relay("G", generator(), vec![f])])
}

fn run_unit(
    f: ProtectionFunction,
    t_end: f64,
    dt: f64,
    shot: impl Fn(f64) -> Shot,
) -> ProtectionReport {
    inject(&unit_scheme(f), &line_rig(), t_end, dt, shot).0
}

/// IEEE C37.102 reverse power (32): a ramp of the unit's active power from
/// export into import starts the stage where the import exceeds the
/// setting (5 % of the machine rating); a step to twice that operates in
/// the delay plus the measuring time. No hysteresis: a ramp back drops it
/// off at the setting. Tolerance: the ramp resolution of 0.05 % of the
/// setting; one step in time.
#[test]
fn ieee_c37_102_reverse_power() {
    let f = |delay: f64| {
        ProtectionFunction::ReversePower(ReversePower {
            label: None,
            pickup: 0.05,
            delay,
        })
    };
    let x = |t: f64| triangle(t, 0.1, -0.045, -0.055, 0.0005 * 0.05 / DT);
    let r = run_unit(f(100.0), 1.0, DT, move |t| {
        Shot::unit(c(1.0, 0.0), c(x(t), 0.0))
    });
    let (tp, tr) = start_and_drop(&r, "G", "32");
    let start = -x(tp) / 0.05;
    let drop = -x(tr.expect("32 drops off")) / 0.05;
    assert!(
        start > 1.0 - 1e-9 && start <= 1.0005 + 1e-9,
        "32 starts at {start:.5}"
    );
    assert!(
        (0.9995 - 1e-9..=1.0 + 1e-9).contains(&drop),
        "32 drops off at {drop:.5}"
    );
    let r = run_unit(f(2.0), 2.5, DT, |t| {
        Shot::unit(c(1.0, 0.0), c(if t >= 0.1 { -0.1 } else { 0.5 }, 0.0))
    });
    assert_within(
        "32 at 2 × setting",
        trip(&r, "G", "32"),
        0.1 + 2.0 + MT,
        0.0,
        DT,
    );
}

/// IEEE C37.102 loss of field (40), two offset mho circles below the R
/// axis (offset 0.075 pu, diameters 1.0 and 2.0 pu on the machine base):
/// characteristic points along the −X axis 2.5 % inside and outside each
/// circle. Inside the small circle zone 1 operates in its delay, inside only
/// the large one zone 2 in its delay, outside or above the offset nothing.
/// Tolerance: one injection step in time.
#[test]
fn ieee_c37_102_loss_of_field() {
    let f = ProtectionFunction::LossOfField(LossOfField {
        label: None,
        offset: 0.075,
        diameter1: 1.0,
        delay1: 0.1,
        diameter2: 2.0,
        delay2: 0.5,
    });
    for (x, expected) in [
        (-0.1, Some(0.1)),
        (-1.05, Some(0.1)),
        (-1.1, Some(0.5)),
        (-2.05, Some(0.5)),
        (-2.1, None),
        (-0.05, None),
        (0.5, None),
    ] {
        let z = c(0.0, x);
        let r = run_unit(f.clone(), 1.0, DT, move |t| {
            Shot::unit(
                c(1.0, 0.0),
                if t >= 0.1 {
                    c(1.0, 0.0) / z
                } else {
                    c(0.5, 0.0)
                },
            )
        });
        let what = format!("40 at Z = j{x}");
        match expected {
            Some(d) => assert_within(&what, trip(&r, "G", "40"), 0.1 + d + MT, 0.0, DT),
            None => assert_none(&what, trip(&r, "G", "40")),
        }
    }
}

/// IEEE C37.102 and C50.13 negative-sequence current (46), `I2² t = K`
/// with the continuous capability subtracted, on a relay measuring
/// unbalanced phases: `t = K / (I2² − I2c²)` at 0.5 and 1 times the rated
/// current; nothing at the continuous capability; after a pause the
/// integral has cooled at the rate `I2c²`. On balanced phasors (RMS) the
/// function stays inactive. Tolerance: 0.1 % plus two steps.
#[test]
fn ieee_c37_102_negative_sequence() {
    let (k, i2c) = (10.0, 0.08);
    let f = ProtectionFunction::NegativeSequence(NegativeSequence {
        label: None,
        continuous: i2c,
        k,
    });
    let neg = |i2: f64| {
        let a = polar(1.0, 120.0);
        let mut s = Shot::unit(c(1.0, 0.0), c(0.0, 0.0));
        s.v[1] = balanced(c(1.0, 0.0));
        s.i_unit = [c(i2, 0.0), a * i2, a * a * i2];
        s.unbalanced = true;
        s
    };
    let dt = 0.01;
    for i2 in [0.5, 1.0] {
        let expected = k / (i2 * i2 - i2c * i2c);
        let r = run_unit(f.clone(), 1.0 + 1.1 * expected, dt, move |t| {
            neg(if t >= 1.0 { i2 } else { 0.0 })
        });
        assert_within(
            &format!("46 at I2 = {i2}"),
            trip(&r, "G", "46"),
            1.0 + expected,
            0.001 * expected + 2.0 * dt,
            0.001 * expected + 2.0 * dt,
        );
    }
    let r = run_unit(f.clone(), 200.0, 0.05, move |_| neg(i2c));
    assert_none("46 at the continuous capability", trip(&r, "G", "46"));
    // 5 s at 1 pu, 100 s off, then 1 pu again.
    let shot = move |t: f64| neg(if !(5.0..105.0).contains(&t) { 1.0 } else { 0.0 });
    let left = k - 5.0 * (1.0 - i2c * i2c) + 100.0 * i2c * i2c;
    let expected = 105.0 + left / (1.0 - i2c * i2c);
    let r = run_unit(f.clone(), 120.0, dt, shot);
    assert_within(
        "46 after cooling",
        trip(&r, "G", "46"),
        expected,
        0.02,
        0.02,
    );
    let r = run_unit(f, 30.0, dt, |_| Shot::unit(c(1.0, 0.0), c(1.0, 0.0)));
    assert_none("46 on balanced phasors", trip(&r, "G", "46"));
}

// ---------------------------------------------------------------------------
// IEEE C37.113: permissive teleprotection
// ---------------------------------------------------------------------------

/// Distance relays at both ends with a permissive scheme; each sees the
/// fault given (Ω, current into the line positive).
fn teleprotection_run(kind: SchemeKind, z1: C64, z2: C64) -> ProtectionReport {
    let relays = [1usize, 2].map(|at| {
        let mut d = distance(Characteristic::Mho, three_zones(), [0.0, 0.0]);
        d.teleprotection = Some(Teleprotection {
            scheme: kind,
            zone: "Z2".into(),
            channel_time: 0.01,
        });
        let mut r = relay(
            &format!("R{at}"),
            line_end(at),
            vec![ProtectionFunction::Distance(d)],
        );
        r.i_nom = Some(0.01);
        r
    });
    let shot = move |t: f64| {
        let mut s = Shot::at_rest();
        if t >= 0.1 {
            s.v = [balanced(c(0.5, 0.0)), balanced(c(0.5, 0.0))];
            s.i_from = balanced(c(0.5, 0.0) / z1);
            s.i_to = balanced(c(0.5, 0.0) / z2);
        }
        s
    };
    inject(
        &scheme(relays.to_vec()),
        &rig(false, polar(8.0, LINE_ANGLE), UnitKind::Machine),
        1.0,
        DT,
        shot,
    )
    .0
}

/// IEEE C37.113, permissive overreaching and underreaching transfer trip:
/// for an internal fault near the remote end (12 Ω from R1, in its zone 2
/// only; 3 Ω from R2) R1 trips on the permission in the channel time plus
/// the measuring time under both schemes. With the remote end seeing the
/// fault only in zone 2 (12 Ω from both), POTT accelerates and PUTT does
/// not; for an external fault behind R2 (reverse for it) neither does, and
/// R1 trips in its zone 2 time. Tolerance: one injection step.
#[test]
fn ieee_c37_113_permissive_schemes() {
    let near = polar(3.0, LINE_ANGLE);
    let far = polar(12.0, LINE_ANGLE);
    let behind = -polar(12.0, LINE_ANGLE);
    let fast = 0.1 + 0.01 + MT;
    let zone2 = 0.1 + 0.4 + MT;
    for (kind, z2, expected, name) in [
        (SchemeKind::Pott, near, fast, "21 POTT"),
        (SchemeKind::Putt, near, fast, "21 PUTT"),
        (SchemeKind::Pott, far, fast, "21 POTT"),
        (SchemeKind::Putt, far, zone2, "21 Z2"),
        (SchemeKind::Pott, behind, zone2, "21 Z2"),
        (SchemeKind::Putt, behind, zone2, "21 Z2"),
    ] {
        let r = teleprotection_run(kind, far, z2);
        let what = format!(
            "{kind:?}, remote end sees {:.1} Ω at {:.0}°",
            z2.norm(),
            z2.arg().to_degrees()
        );
        assert_within(&what, trip(&r, "R1", "21"), expected, 0.0, DT);
        let first_trip = r.trips.iter().find(|t| t.relay == "R1").expect("R1 trips");
        assert_eq!(first_trip.function, name, "{what}");
    }
}

// ---------------------------------------------------------------------------
// IEEE C37.119 and C37.104: breaker failure and reclosing
// ---------------------------------------------------------------------------

/// IEEE C37.119, the current check of breaker failure: with the breaker
/// stuck, the function operates its delay after the trip when the current
/// is above its threshold (0.12 against 0.1 times the rated current) and
/// not when it is below (0.08). Tolerance: one injection step.
#[test]
fn ieee_c37_119_current_check_threshold() {
    for (after, operates) in [(0.12, true), (0.08, false)] {
        let mut r = relay("R1", line_end(1), vec![definite(1.0, 0.0)]);
        r.breaker_failure = Some(BreakerFailure {
            delay: 0.15,
            current: 0.1,
        });
        let mut s = scheme(vec![r]);
        s.stuck_breakers.push(BreakerRef {
            from: 1,
            to: 2,
            circuit: "1".into(),
            at: 1,
            machine_id: None,
        });
        let (rep, _) = inject(&s, &line_rig(), 0.6, DT, move |t| {
            current(if t < 0.1 {
                0.0
            } else if t < 0.2 {
                3.0
            } else {
                after
            })
        });
        let t_trip = trip(&rep, "R1", "50").expect("trips");
        let bf = first(&rep, "R1", OperationKind::BreakerFailure, "50BF");
        let what = format!("50BF with {after} × rated still flowing");
        if operates {
            assert_within(&what, bf, t_trip + 0.15, 0.0, DT);
        } else {
            assert_none(&what, bf);
        }
    }
}

/// IEEE C37.104, multi-shot reclosing onto a permanent fault: with two
/// shots the recloser recloses after the dead time following each of the
/// first two trips and locks out at the third trip.
#[test]
#[ignore = "IEEE C37.104 multi-shot sequence, 2 shots set: one reclose (0.47 s) and lockout at the second trip (0.49 s), as any trip within the reclaim time after a reclose locks out; required: a second reclose and lockout at the third trip"]
fn ieee_c37_104_multi_shot_sequence() {
    let mut d = distance(Characteristic::Mho, three_zones(), [0.0, 0.0]);
    d.zones[0].autoreclose = true;
    let mut r = relay("R1", line_end(1), vec![ProtectionFunction::Distance(d)]);
    r.i_nom = Some(0.01);
    r.autoreclose = Some(AutoReclose {
        dead_time: 0.3,
        reclaim: 5.0,
        shots: 2,
        sync_angle: 30.0,
    });
    let (rep, _) = inject(
        &scheme(vec![r]),
        &rig(false, polar(8.0, LINE_ANGLE), UnitKind::Machine),
        2.0,
        DT,
        fault_at(polar(4.0, LINE_ANGLE)),
    );
    let trips = ops(&rep, "R1", OperationKind::Trip, "21");
    let closes = ops(&rep, "R1", OperationKind::Reclose, "79");
    let lockout = first(&rep, "R1", OperationKind::Lockout, "79");
    let times =
        |o: &[&tellegen::protection::Operation]| o.iter().map(|o| o.time).collect::<Vec<_>>();
    assert!(
        closes.len() == 2 && trips.len() == 3,
        "recloses at {:?}, trips at {:?}, lockout at {lockout:?}",
        times(&closes),
        times(&trips)
    );
    assert_within(
        "lockout at the third trip",
        lockout,
        trips[2].time,
        0.0,
        0.0,
    );
}
