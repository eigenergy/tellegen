//! Out-of-step protection (78) tested by power-swing simulation, as relay
//! test sets test it: no IEC 60255 part has type tests for out-of-step
//! tripping, so the test set injects the voltage and current of a
//! two-source equivalent at the relay and lets the sources slip against
//! each other (IEEE C37.102 for out-of-step tripping of generators; the
//! IEEE PSRC power swing report for the two-source model).
//!
//! The equivalent: a source `E1 ∠δ` behind the reactance `a X_T`, the
//! relay, and the rest of the system `(1 − a) X_T` up to a source `E2 ∠0`.
//! The relay measures `I = (E1 − E2) / (j X_T)` and
//! `V = E1 − j a X_T I = (1 − a) E1 + a E2`. With equal sources the
//! apparent impedance runs along the horizontal line `X = X_T (1/2 − a)`,
//! `R = (X_T / 2) cot(δ/2)`, and crosses the electrical centre at
//! `δ = 180°`, where the voltage at the relay is at its minimum
//! `|(1 − a) E1 − a E2|`. A slip at `s` Hz turns `δ` at `2π s` rad/s, so
//! every crossing and every voltage minimum has an analytic time.
//!
//! Two counting modes are tested: the impedance traverse between the
//! blinders `R = ±b` (crossed at `δ = 2 atan(X_T / 2b)` and `360°` less
//! that) with its minimum traverse time, and the voltage-beat mode (a beat
//! is the voltage dipping below the beat voltage while the swing passes
//! the electrical centre, where the active power at the relay,
//! `E1 E2 sin δ / X_T`, reverses; it is counted once the voltage has risen
//! for 40 ms past its minimum; the function operates `beat_delay` after the
//! voltage, past the last beat, exceeds its average over the oscillation).
//!
//! Times count from the injection step; the test set evaluates the relay
//! every 1 ms. Out-of-step tripping has no measuring time of its own: it
//! operates in the evaluation that completes the count.
#![cfg(feature = "protection")]

#[allow(dead_code)]
mod test_set {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/support/test_set.rs"
    ));
}

use std::f64::consts::PI;

use tellegen::protection::{OperationKind, OutOfStep, ProtectionFunction, ProtectionReport};
use test_set::*;

/// The swing starts at this time from the steady angle before it.
const T0: f64 = 0.1;
/// Total reactance between the sources, pu.
const X_T: f64 = 1.0;
/// Blinders of the impedance mode, pu.
const BLINDER: f64 = 0.2;
/// Reach of the impedance mode above and below the R axis, pu.
const REACH: f64 = 0.3;
/// Minimum traverse time between the blinders, s.
const MIN_TRAVERSE: f64 = 0.025;
/// The counting delay of a voltage beat after its minimum, s.
const BEAT_RISE: f64 = 0.04;

/// The two-source equivalent seen by the relay at the bus-1 end of the
/// line: `E1 ∠δ(t)` behind `a X_T`, `e2 ∠0` behind the rest.
fn two_source(a: f64, e2: f64, delta: impl Fn(f64) -> f64) -> impl Fn(f64) -> Shot {
    move |t| {
        let e1 = C64::from_polar(1.0, delta(t));
        let i = (e1 - c(e2, 0.0)) / c(0.0, X_T);
        let v = e1 - c(0.0, a * X_T) * i;
        Shot::line(v, i)
    }
}

/// The voltage magnitude at the relay at angle `δ`.
fn relay_voltage(a: f64, e2: f64, delta: f64) -> f64 {
    ((1.0 - a) * C64::from_polar(1.0, delta) + a * e2).norm()
}

/// A pole slip at `s` Hz (negative: the other way) from `δ0` at `T0`.
fn slip(d0_deg: f64, s: f64) -> impl Fn(f64) -> f64 + Copy {
    move |t| d0_deg.to_radians() + 2.0 * PI * s * (t - T0).max(0.0)
}

/// A stable swing from `δ0` out to `δ_max` and back, `f` swings a second,
/// for `cycles` swings.
fn stable_swing(d0_deg: f64, dmax_deg: f64, f: f64, cycles: f64) -> impl Fn(f64) -> f64 + Copy {
    move |t| {
        let s = (t - T0).clamp(0.0, cycles / f);
        let w = 0.5 * (1.0 - (2.0 * PI * f * s).cos());
        (d0_deg + (dmax_deg - d0_deg) * w).to_radians()
    }
}

fn out_of_step(slips: usize, beat: Option<(f64, f64)>, min_traverse: f64) -> ProtectionFunction {
    ProtectionFunction::OutOfStep(OutOfStep {
        label: None,
        blinder: BLINDER,
        forward_reach: REACH,
        reverse_reach: REACH,
        min_traverse,
        slips,
        beat_voltage: beat.map(|b| b.0),
        beat_delay: beat.map_or(0.0, |b| b.1),
    })
}

fn run(f: ProtectionFunction, t_end: f64, shot: impl Fn(f64) -> Shot) -> ProtectionReport {
    inject(
        &scheme(vec![relay("R1", line_end(1), vec![f])]),
        &line_rig(),
        t_end,
        DT,
        shot,
    )
    .0
}

/// The counts the function logs (pickups) up to its operation, in time
/// order (the measurement goes on after it).
fn counts(r: &ProtectionReport) -> Vec<f64> {
    let until = trip(r, "R1", "78").unwrap_or(f64::INFINITY);
    ops(r, "R1", OperationKind::Pickup, "78")
        .iter()
        .map(|o| o.time)
        .filter(|&t| t <= until)
        .collect()
}

/// The angle at which the impedance crosses the right blinder; it leaves
/// over the left one at `2π` less that.
fn blinder_angle() -> f64 {
    2.0 * (X_T / (2.0 * BLINDER)).atan()
}

/// The time the slip at `s` Hz from `δ0` (degrees) reaches the angle
/// `target` (radians), counting whole turns from the start.
fn at_angle(d0_deg: f64, s: f64, target: f64) -> f64 {
    T0 + (target - d0_deg.to_radians()) / (2.0 * PI * s.abs())
}

// ---------------------------------------------------------------------------
// Impedance traverse
// ---------------------------------------------------------------------------

/// Pole slips counted by the impedance traverse: for slip frequencies from
/// 0.1 to 5 Hz, relay positions with the electrical centre in front of,
/// at and behind the relay (`a` = 0.3, 0.5, 0.7), and 1 to 3 slips set, the
/// function counts each slip where the impedance leaves over the far
/// blinder, at `δ = 360° − 2 atan(X_T / 2b)` plus whole turns, and operates
/// at the set number of slips. Slips the other way (the source falling
/// behind) count alike, mirrored. Tolerance: one injection step.
#[test]
fn impedance_traverse_counts_pole_slips() {
    let leave = 2.0 * PI - blinder_angle();
    for (s, slips) in [(0.1, 1), (0.5, 2), (1.0, 3), (2.0, 2), (5.0, 3)] {
        for a in [0.3, 0.5, 0.7] {
            for dir in [1.0, -1.0] {
                let d0 = 30.0 * dir;
                let crossing = |k: usize| at_angle(30.0, s, leave + 2.0 * PI * (k as f64 - 1.0));
                let t_end = crossing(slips) + 0.2;
                let r = run(
                    out_of_step(slips, None, MIN_TRAVERSE),
                    t_end,
                    two_source(a, 1.0, slip(d0, dir * s)),
                );
                let seen = counts(&r);
                let what = format!("{s} Hz ({dir:+}), a = {a}, {slips} slips");
                assert_eq!(seen.len(), slips, "{what}: counted at {seen:?}");
                for (k, &t) in seen.iter().enumerate() {
                    assert_within(
                        &format!("{what}: slip {}", k + 1),
                        Some(t),
                        crossing(k + 1),
                        0.0,
                        DT,
                    );
                }
                assert_within(&what, trip(&r, "R1", "78"), crossing(slips), 0.0, DT);
            }
        }
    }
}

/// The upper slip frequency of the impedance traverse: a slip counts only
/// when the impedance spends the minimum traverse time between the
/// blinders, `(360° − 2 δ_b) / (360° s) ≥ 25 ms`, which ends the relay's
/// range at `s_max = (360° − 2 δ_b) / (360° · 25 ms)` (9.7 Hz here). Slips
/// 15 % below it operate; 15 % above it no slip is counted. Tolerance: one
/// injection step on the traverse time (4 % of 25 ms).
#[test]
fn impedance_traverse_slip_frequency_limit() {
    let span = 2.0 * PI - 2.0 * blinder_angle();
    let s_max = span / (2.0 * PI * MIN_TRAVERSE);
    assert!((9.0..10.5).contains(&s_max), "{s_max}");
    for (m, operates) in [(0.85, true), (1.15, false)] {
        let s = m * s_max;
        let r = run(
            out_of_step(2, None, MIN_TRAVERSE),
            T0 + 3.0 / s + 0.1,
            two_source(0.5, 1.0, slip(30.0, s)),
        );
        let what = format!("{s:.2} Hz, traverse {:.1} ms", span / (2.0 * PI * s) * 1e3);
        if operates {
            assert_eq!(counts(&r).len(), 2, "{what}");
            assert!(trip(&r, "R1", "78").is_some(), "{what}: no operation");
        } else {
            assert!(counts(&r).is_empty(), "{what}: {:?}", counts(&r));
            assert_none(&what, trip(&r, "R1", "78"));
        }
    }
}

/// Stable swings and swings outside the reach. A swing that enters between
/// the blinders and turns back (out to 150° and to 220°, short of the far
/// blinder at 223.6°) is no slip, however often it repeats; a pole slip
/// whose impedance passes above or below the reach (electrical centre far
/// in front of or behind the relay, `a` = 0.05 and 0.95) is not counted.
#[test]
fn impedance_traverse_ignores_stable_swings_and_remote_centres() {
    for dmax in [150.0, 220.0] {
        let r = run(
            out_of_step(1, None, MIN_TRAVERSE),
            5.0,
            two_source(0.5, 1.0, stable_swing(30.0, dmax, 1.0, 4.0)),
        );
        assert!(counts(&r).is_empty(), "swing to {dmax}°: {:?}", counts(&r));
        assert_none(&format!("swing to {dmax}°"), trip(&r, "R1", "78"));
    }
    for a in [0.05, 0.95] {
        let r = run(
            out_of_step(1, None, MIN_TRAVERSE),
            4.0,
            two_source(a, 1.0, slip(30.0, 1.0)),
        );
        assert_none(
            &format!("slip with the centre at a = {a}"),
            trip(&r, "R1", "78"),
        );
    }
}

// ---------------------------------------------------------------------------
// Voltage beats
// ---------------------------------------------------------------------------

/// The analytic operate time of the voltage-beat mode on the voltage
/// `v(t)`: the voltage first falls below `vb` at `t_e`; after the last beat
/// is counted at `counted`, the first instant the voltage exceeds its
/// average over `[t_e, t]`; plus the delay. The average is integrated by
/// the trapezoidal rule in steps of 10 µs.
fn beat_operate_time(v: impl Fn(f64) -> f64, vb: f64, counted: f64, delay: f64) -> f64 {
    let h = 1e-5;
    let mut t = T0;
    while v(t) >= vb {
        t += h;
        assert!(t < 100.0, "the voltage never falls below {vb}");
    }
    let t_e = t;
    let mut area = 0.0;
    let mut last = v(t);
    loop {
        t += h;
        let now = v(t);
        area += 0.5 * (last + now) * h;
        last = now;
        if t >= counted && now > area / (t - t_e) {
            return t + delay;
        }
        assert!(t < 200.0, "the voltage never exceeds its average");
    }
}

/// The time of the `k`-th voltage minimum of a slip at `s` Hz from 30°
/// (`δ = 180°` plus whole turns), and when the beat is counted.
fn beat_counted(s: f64, k: usize) -> f64 {
    at_angle(30.0, s, PI * (2.0 * k as f64 - 1.0)) + BEAT_RISE
}

/// Voltage beats counted per pole slip: slip frequencies from 0.25 to 8 Hz,
/// relay positions with the voltage minimum at 0 pu (the electrical centre
/// at the relay, `a` = 0.5), 0.3 pu (`a` = 0.35) and, with a weaker remote
/// source (`E2` = 0.9), 0.05 pu; beat voltage 0.6 pu, 1 to 3 slips and beat
/// delays of 0, 0.1 and 0.3 s. Each beat is counted 40 ms after the
/// voltage minimum at `δ = 180°`; the function operates its delay after the
/// voltage, past the last beat, exceeds its average since the first dip
/// below the beat voltage (computed from the two-source voltage). The
/// minimum traverse time does not act in this mode.
/// Tolerance: one injection step before and two after on a count (the
/// minimum falls between two steps), three either way on the operate time
/// (the relay integrates the average per step).
#[test]
fn voltage_beats_count_pole_slips() {
    let vb = 0.6;
    for (s, slips, delay) in [
        (0.25, 1, 0.1),
        (0.5, 2, 0.0),
        (1.0, 2, 0.1),
        (1.0, 3, 0.3),
        (2.0, 3, 0.1),
        (4.0, 2, 0.3),
        (8.0, 3, 0.0),
    ] {
        for (a, e2) in [(0.5, 1.0), (0.35, 1.0), (0.5, 0.9)] {
            let delta = slip(30.0, s);
            let expected = beat_operate_time(
                |t| relay_voltage(a, e2, delta(t)),
                vb,
                beat_counted(s, slips),
                delay,
            );
            let what = format!("{s} Hz, a = {a}, E2 = {e2}, {slips} beats, delay {delay}");
            let mut first_trip = None;
            for min_traverse in [MIN_TRAVERSE, 10.0] {
                let r = run(
                    out_of_step(slips, Some((vb, delay)), min_traverse),
                    expected + 0.1,
                    two_source(a, e2, delta),
                );
                let seen = counts(&r);
                // The slip goes on during the delay, and so does the count.
                assert!(seen.len() >= slips, "{what}: beats at {seen:?}");
                for (k, &t) in seen.iter().take(slips).enumerate() {
                    assert_within(
                        &format!("{what}: beat {}", k + 1),
                        Some(t),
                        beat_counted(s, k + 1),
                        DT,
                        2.0 * DT,
                    );
                }
                let t = trip(&r, "R1", "78");
                assert_within(&what, t, expected, 3.0 * DT, 3.0 * DT);
                if let Some(t0) = first_trip {
                    assert_eq!(t, Some(t0), "{what}: the minimum traverse time acts");
                }
                first_trip = t;
            }
        }
    }
}

/// The slip frequency range of the voltage-beat mode. A beat is counted
/// 40 ms after its minimum, and the next one only after the voltage has
/// risen above 1.1 times the beat voltage. With the electrical centre at the
/// relay (`|V| = |cos(δ/2)|`) and a beat voltage of 0.6 pu the count at
/// `40 ms` past the minimum still finds the voltage above 0.66 pu up to
/// `s = (2π − 2 asin 0.66) / (2π · 40 ms)` = 19.3 Hz: at 15 Hz every slip
/// is counted; at 30 Hz, with a slip period shorter than the counting delay,
/// the beats run together and fewer are counted than slips. At 0.1 Hz
/// (slow slips) every slip is counted.
#[test]
fn voltage_beats_at_the_slip_frequency_limits() {
    let vb: f64 = 0.6;
    let limit = (2.0 * PI - 2.0 * (1.1 * vb).asin()) / (2.0 * PI * BEAT_RISE);
    assert!((limit - 19.3).abs() < 0.05, "{limit}");
    for (s, slips) in [(0.1, 2), (15.0, 6)] {
        let r = run(
            out_of_step(slips + 1, Some((vb, 0.0)), MIN_TRAVERSE),
            beat_counted(s, slips) + 0.5 / s,
            two_source(0.5, 1.0, slip(30.0, s)),
        );
        let seen = counts(&r);
        assert_eq!(seen.len(), slips, "{s} Hz: beats at {seen:?}");
        for (k, &t) in seen.iter().enumerate() {
            assert_within(
                &format!("{s} Hz: beat {}", k + 1),
                Some(t),
                beat_counted(s, k + 1),
                DT,
                2.0 * DT,
            );
        }
        assert_none(&format!("{s} Hz, one slip short"), trip(&r, "R1", "78"));
    }
    let s = 30.0;
    let slips = 12;
    let r = run(
        out_of_step(slips, Some((vb, 0.0)), MIN_TRAVERSE),
        beat_counted(s, slips) + 0.5 / s,
        two_source(0.5, 1.0, slip(30.0, s)),
    );
    assert!(
        counts(&r).len() < slips,
        "30 Hz: {} beats counted for {slips} slips",
        counts(&r).len()
    );
}

/// Swings the voltage-beat mode must not operate for: a pole slip whose
/// voltage minimum at the relay stays above the beat voltage (the electrical
/// centre far from the relay: `a` = 0.15, minimum 0.7 pu against 0.6 pu),
/// and a stable swing out to 120° and back with the centre at the relay
/// (minimum `cos 60°` = 0.5 pu against a beat voltage of 0.4 pu), repeated
/// four times.
#[test]
fn voltage_beats_ignore_swings_above_the_beat_voltage() {
    let r = run(
        out_of_step(1, Some((0.6, 0.0)), MIN_TRAVERSE),
        5.0,
        two_source(0.15, 1.0, slip(30.0, 1.0)),
    );
    assert!(counts(&r).is_empty(), "{:?}", counts(&r));
    assert_none("slip with its minimum at 0.7 pu", trip(&r, "R1", "78"));
    let r = run(
        out_of_step(1, Some((0.4, 0.0)), MIN_TRAVERSE),
        5.0,
        two_source(0.5, 1.0, stable_swing(30.0, 120.0, 1.0, 4.0)),
    );
    assert!(counts(&r).is_empty(), "{:?}", counts(&r));
    assert_none("stable swing to 120°", trip(&r, "R1", "78"));
}

/// A stable swing whose voltage dips below the beat voltage: out to 150°
/// and back four times with the electrical centre at the relay (minimum
/// `cos 75°` = 0.26 pu against a beat voltage of 0.4 pu), two slips set.
/// The angle never reaches 180°, so the active power at the relay keeps its
/// sign. Out-of-step tripping is for pole slips only (IEEE C37.102); the
/// impedance traverse of the same relay does not count this swing (see
/// `impedance_traverse_ignores_stable_swings_and_remote_centres`).
#[test]
fn voltage_beats_ignore_a_stable_swing_below_the_beat_voltage() {
    let r = run(
        out_of_step(2, Some((0.4, 0.0)), MIN_TRAVERSE),
        5.0,
        two_source(0.5, 1.0, stable_swing(30.0, 150.0, 1.0, 4.0)),
    );
    let t = trip(&r, "R1", "78");
    assert!(
        t.is_none(),
        "stable swing to 150°: beats at {:?}, operated at {t:?}",
        counts(&r)
    );
}
