//! Control blocks shared by the exciter, governor and stabilizer models.
//!
//! Translated from PowerSimulationsDynamics.jl `src/models/common_controls.jl`
//! (BSD-3-Clause, Copyright (c) 2020, 2023 Alliance for Sustainable Energy,
//! LLC and The Regents of the University of California; see
//! `licenses/PowerSimulationsDynamics.jl-LICENSE`). Each `*_mm` function is the
//! mass-matrix form `M ẋ = rhs` and returns `(output, rhs)`; the plain form
//! returns `(output, ẋ)`.
//!
//! @block Control blocks

const EPS: f64 = f64::EPSILON;

// @block Lags, washouts and lead-lags
// First-order blocks, each in its mass-matrix form (a zero time constant
// makes the state algebraic) and its plain form.

/// `K/(1 + sT)` as `T ẏ = K u − y`.
pub(crate) fn low_pass_mm(u: f64, y: f64, k: f64) -> (f64, f64) {
    (y, k * u - y)
}

/// `K/(1 + sT)` with `ẏ = (K u − y)/T`.
pub(crate) fn low_pass(u: f64, y: f64, k: f64, t: f64) -> (f64, f64) {
    let (y, rhs) = low_pass_mm(u, y, k);
    (y, rhs / t)
}

/// Non-windup low pass from IEEE Std 421.5 with mass `T`: the state stops at
/// a limit while its derivative pushes past it. The output is the clamped
/// state. With `T = 0` the block is algebraic, `0 = clamp(K u) − y`, which
/// keeps its row nonsingular when a limit binds (zeroing the rate would leave
/// the state with no equation).
pub(crate) fn low_pass_nonwindup_mm(
    u: f64,
    y: f64,
    k: f64,
    t: f64,
    y_min: f64,
    y_max: f64,
) -> (f64, f64) {
    if t < EPS {
        return (y.clamp(y_min, y_max), (k * u).clamp(y_min, y_max) - y);
    }
    let dydt = k * u - y;
    let y_sat = y.clamp(y_min, y_max);
    let blocked = (y >= y_max && dydt > 0.0) || (y <= y_min && dydt < 0.0);
    (y_sat, if blocked { 0.0 } else { dydt })
}

pub(crate) fn low_pass_nonwindup(
    u: f64,
    y: f64,
    k: f64,
    t: f64,
    y_min: f64,
    y_max: f64,
) -> (f64, f64) {
    let (y_sat, rhs) = low_pass_nonwindup_mm(u, y, k, t, y_min, y_max);
    (y_sat, rhs / t)
}

/// `K sT/(1 + sT)` with mass `T`: `y = x + (K/T) u`, `T ẋ = −((K/T) u + x)`.
pub(crate) fn high_pass_mm(u: f64, x: f64, k: f64, t: f64) -> (f64, f64) {
    let k_t = if t < EPS { 0.0 } else { k / t };
    (x + k_t * u, -(k_t * u + x))
}

/// `sK/(1 + sT)` with its rate: with `T = 0` the block is off (output and
/// rate zero for a state started at zero), not a pure derivative.
pub(crate) fn high_pass(u: f64, x: f64, k: f64, t: f64) -> (f64, f64) {
    let (y, rhs) = high_pass_mm(u, x, k, t);
    if t < EPS {
        return (y, 0.0);
    }
    (y, rhs / t)
}

/// `K (1 + sT1)/(1 + sT2)` with mass `T2`.
pub(crate) fn lead_lag_mm(u: f64, x: f64, k: f64, t1: f64, t2: f64) -> (f64, f64) {
    let t1_t2 = if t2 < EPS { 0.0 } else { t1 / t2 };
    (x + k * t1_t2 * u, k * (1.0 - t1_t2) * u - x)
}

pub(crate) fn lead_lag(u: f64, x: f64, k: f64, t1: f64, t2: f64) -> (f64, f64) {
    let (y, rhs) = lead_lag_mm(u, x, k, t1, t2);
    (y, rhs / t2)
}

// @block Second-order filters
// The second-order low pass and lead-lag of the stabilizers.

/// `K/(1 + sT1 + s²T2)`: states `(x, y)`, `T2 ẋ = K u − T1 x − y`, `ẏ = x`.
pub(crate) fn low_pass_2nd_mm(u: f64, x: f64, y: f64, k: f64, t1: f64) -> (f64, f64, f64) {
    (y, k * u - t1 * x - y, x)
}

/// `(1 + sT3 + s²T4)/(1 + sT1 + s²T2)`: mass `T2` on `x1`.
pub(crate) fn lead_lag_2nd_mm(
    u: f64,
    x1: f64,
    x2: f64,
    t1: f64,
    t2: f64,
    t3: f64,
    t4: f64,
) -> (f64, f64, f64) {
    let dx1 = u - t1 * x1 - x2;
    let dx2 = x1;
    let t4_t2 = if t2 < EPS { 0.0 } else { t4 / t2 };
    let y = t4_t2 * u + (t3 - t1 * t4_t2) * x1 + (1.0 - t4_t2) * x2;
    (y, dx1, dx2)
}

// @block Controllers and limits
// The non-windup PI controller, the time constant with which a limited
// state settles onto its limit, and the checks of limit pairs.

/// Time constant (s) with which a state that a limit stops (a non-windup
/// PI's integrator, the REGCA1 active current under LVPL) settles onto the
/// limit: its rate toward the limit is capped at `distance/τ`, continuous in
/// the state, so a state sliding along a limit has an implicit step.
pub(crate) const LIMIT_TAU: f64 = 1e-3;

/// Non-windup PI: `y = clamp(kp u + ki x)`; the integrator follows `ẋ = u`
/// except that it does not drive the output past a limit. Past a limit the
/// integrator stops while `u` pushes further out and integrates `u` as soon
/// as it points back inside (conditional integration, as PSS/E). Near a
/// limit its outward rate is capped at `(limit − y)/(ki τ)` with
/// `τ` = [`LIMIT_TAU`], so the rate is continuous in the state: the
/// output can slide along a limit (the proportional path pulling it inside,
/// the integrator out) and every implicit step has a solution. A rate that
/// switches between `u` and 0 at the limit has none there.
pub(crate) fn pi_nonwindup(u: f64, x: f64, kp: f64, ki: f64, y_min: f64, y_max: f64) -> (f64, f64) {
    let y = kp * u + ki * x;
    let rate = if ki > 0.0 {
        let k = 1.0 / (ki * LIMIT_TAU);
        let down = ((y_min - y) * k).min(0.0);
        let up = ((y_max - y) * k).max(0.0);
        u.clamp(down, up)
    } else if y_min < y && y < y_max {
        u
    } else {
        0.0
    };
    (y.clamp(y_min, y_max), rate)
}

/// Checks that the limits `(lo, hi)` of a `model` record are ordered
/// (`lo ≤ hi`, neither NaN): swapped limits are refused with the record's
/// parameters named, rather than reaching a clamp that cannot hold them.
pub(crate) fn ordered(
    model: &str,
    (lo_name, lo): (&str, f64),
    (hi_name, hi): (&str, f64),
) -> Result<(), String> {
    if lo <= hi {
        return Ok(());
    }
    Err(format!(
        "{model} limits are swapped: {lo_name} = {lo} is above {hi_name} = {hi}"
    ))
}

// @block Exciter curves
// The rectifier regulation of AC exciters and the saturation curves
// fitted through two points.

/// Rectifier regulation `FEX(I_N)` of IEEE Std 421.5 AC exciters.
pub(crate) fn rectifier(i: f64) -> f64 {
    if i <= 0.0 {
        1.0
    } else if i <= 0.433 {
        1.0 - 0.577 * i
    } else if i < 0.75 {
        (0.75 - i * i).sqrt()
    } else if i <= 1.0 {
        1.732 * (1.0 - i)
    } else {
        0.0
    }
}

/// Coefficients `(A, B)` of `Se(x) = B (x − A)²/x` through the two points
/// `(E1, Se1)`, `(E2, Se2)` (PowerSystems.jl `calculate_saturation_coefficients`).
/// No saturation data gives `(0, 0)`. The points may come in either order
/// (IEEE 421.5 lists `E1 > E2`, PowerSystems.jl `E1 < E2`); the curve
/// through them is the same, so they are sorted by `E` first.
pub(crate) fn quadratic_saturation(e: (f64, f64), se: (f64, f64)) -> Result<(f64, f64), String> {
    if e.0 == 0.0 || e.1 == 0.0 || se.0 == 0.0 || se.1 == 0.0 {
        return Ok((0.0, 0.0));
    }
    let (e, se) = if e.1 < e.0 {
        ((e.1, e.0), (se.1, se.0))
    } else {
        (e, se)
    };
    if e.1 <= e.0 || se.1 <= se.0 {
        return Err(format!(
            "inconsistent saturation data: E = {e:?}, Se = {se:?}: Se must increase with E \
             at two distinct points"
        ));
    }
    let a = (1.0 / (se.1 * e.1 - se.0 * e.0))
        * (e.0 * e.1 * (se.1 - se.0) - (e.0 * e.1 * se.0 * se.1 * (e.0 - e.1).powi(2)).sqrt());
    let b = se.1 * e.1 / (e.1 - a).powi(2);
    Ok((a, b))
}

/// Coefficients `(A, B)` of `Se(x) = B x^A` through `(1.0, Se1)`, `(1.2, Se2)`.
pub(crate) fn exponential_saturation(se: (f64, f64)) -> Result<(f64, f64), String> {
    if se.0 == 0.0 || se.1 == 0.0 {
        return Ok((0.0, 0.0));
    }
    if se.1 <= se.0 {
        return Err(format!(
            "inconsistent saturation data: S(1.2) {} <= S(1.0) {}",
            se.1, se.0
        ));
    }
    let b = se.0;
    let a = (se.1 / b).ln() / 1.2_f64.ln();
    Ok((a, b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quadratic_saturation_passes_through_both_points() {
        let (a, b) = quadratic_saturation((1.0, 1.2), (0.1, 0.8)).unwrap();
        let se = |x: f64| b * (x - a).powi(2) / x;
        assert!((se(1.0) - 0.1).abs() < 1e-12, "{}", se(1.0));
        assert!((se(1.2) - 0.8).abs() < 1e-12, "{}", se(1.2));
    }

    /// IEEE 421.5 lists the ESAC1A points high first (VE1 = 4.18, VE2 = 3.14);
    /// either order is the same curve.
    #[test]
    #[allow(clippy::approx_constant)] // 3.14 is the IEEE sample's VE2, not π.
    fn quadratic_saturation_does_not_depend_on_point_order() {
        let ieee = quadratic_saturation((4.18, 3.14), (0.1, 0.03)).unwrap();
        let ascending = quadratic_saturation((3.14, 4.18), (0.03, 0.1)).unwrap();
        assert_eq!(ieee, ascending);
        let (a, b) = ieee;
        let se = |x: f64| b * (x - a).powi(2) / x;
        assert!((se(4.18) - 0.1).abs() < 1e-12);
        assert!((se(3.14) - 0.03).abs() < 1e-12);
        // Se falling as E rises is still refused, in either order.
        assert!(quadratic_saturation((4.18, 3.14), (0.03, 0.1)).is_err());
        assert!(quadratic_saturation((3.14, 4.18), (0.1, 0.03)).is_err());
        assert!(quadratic_saturation((3.14, 3.14), (0.03, 0.1)).is_err());
    }

    #[test]
    fn exponential_saturation_passes_through_both_points() {
        let (a, b) = exponential_saturation((0.0392, 0.2672)).unwrap();
        assert!((b - 0.0392).abs() < 1e-15);
        assert!((b * 1.2_f64.powf(a) - 0.2672).abs() < 1e-12);
    }

    #[test]
    fn pi_integrator_does_not_drive_past_a_limit() {
        let (kp, ki) = (2.0, 0.5);
        // Inside, far from the limits: a plain PI.
        assert_eq!(pi_nonwindup(0.1, 0.0, kp, ki, -1.0, 1.0), (0.2, 0.1));
        // Past the upper limit and pushed further out: the integrator stops.
        assert_eq!(pi_nonwindup(1.0, 0.0, kp, ki, -1.0, 1.0), (1.0, 0.0));
        // Past it, the input pointing back inside: it integrates at once.
        let (y, rate) = pi_nonwindup(-0.1, 4.0, kp, ki, -1.0, 1.0);
        assert_eq!((y, rate), (1.0, -0.1));
        // Below the lower limit, pushed further down: stopped; pushed up:
        // integrating.
        assert_eq!(pi_nonwindup(-1.0, 0.0, kp, ki, -1.0, 1.0).1, 0.0);
        assert_eq!(pi_nonwindup(0.1, -4.0, kp, ki, -1.0, 1.0).1, 0.1);
        // The rate is continuous across a limit (Lipschitz in `x` with
        // constant `1/τ`).
        let at = |x: f64| pi_nonwindup(-0.5, x, kp, ki, 0.0, 1.0).1;
        let x_lim = 2.0; // y = −1 + 0.5 x = 0
        for d in [1e-6, 1e-9, 1e-12] {
            assert!((at(x_lim + d) - at(x_lim - d)).abs() <= 2.0 * d / LIMIT_TAU * (1.0 + 1e-6));
        }
        // Without integral gain the old rule: stopped outside.
        assert_eq!(pi_nonwindup(1.0, 0.0, kp, 0.0, -1.0, 1.0), (1.0, 0.0));
    }

    #[test]
    fn nonwindup_holds_at_limit() {
        assert_eq!(
            low_pass_nonwindup_mm(10.0, 5.0, 1.0, 0.1, -5.0, 5.0),
            (5.0, 0.0)
        );
        assert_eq!(
            low_pass_nonwindup_mm(0.0, 5.0, 1.0, 0.1, -5.0, 5.0),
            (5.0, -5.0)
        );
        // Algebraic (`T = 0`): the state is the clamped input, with a
        // residual that still depends on the state at a binding limit.
        assert_eq!(
            low_pass_nonwindup_mm(10.0, 5.0, 1.0, 0.0, -5.0, 5.0),
            (5.0, 0.0)
        );
        assert_eq!(
            low_pass_nonwindup_mm(10.0, 4.0, 1.0, 0.0, -5.0, 5.0),
            (4.0, 1.0)
        );
    }
}
