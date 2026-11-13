//! Control blocks shared by the exciter, governor and stabilizer models.
//!
//! Translated from PowerSimulationsDynamics.jl `src/models/common_controls.jl`
//! (BSD-3-Clause, Copyright (c) 2020, 2023 Alliance for Sustainable Energy,
//! LLC and The Regents of the University of California; see
//! `licenses/PowerSimulationsDynamics.jl-LICENSE`). Each `*_mm` function is the
//! mass-matrix form `M ẋ = rhs` and returns `(output, rhs)`; the plain form
//! returns `(output, ẋ)`.

const EPS: f64 = f64::EPSILON;

/// `K/(1 + sT)` as `T ẏ = K u − y`.
pub(crate) fn low_pass_mm(u: f64, y: f64, k: f64) -> (f64, f64) {
    (y, k * u - y)
}

/// `K/(1 + sT)` with `ẏ = (K u − y)/T`.
pub(crate) fn low_pass(u: f64, y: f64, k: f64, t: f64) -> (f64, f64) {
    let (y, rhs) = low_pass_mm(u, y, k);
    (y, rhs / t)
}

/// Non-windup low pass from IEEE Std 421.5: the state stops at a limit while
/// its derivative pushes past it. The output is the clamped state.
pub(crate) fn low_pass_nonwindup_mm(u: f64, y: f64, k: f64, y_min: f64, y_max: f64) -> (f64, f64) {
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
    let (y_sat, rhs) = low_pass_nonwindup_mm(u, y, k, y_min, y_max);
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

/// Non-windup PI: `y = clamp(kp u + ki x)`, `ẋ = u` only while unclamped.
pub(crate) fn pi_nonwindup(u: f64, x: f64, kp: f64, ki: f64, y_min: f64, y_max: f64) -> (f64, f64) {
    let y = kp * u + ki * x;
    let inside = y_min < y && y < y_max;
    (y.clamp(y_min, y_max), if inside { u } else { 0.0 })
}

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
/// No saturation data gives `(0, 0)`.
pub(crate) fn quadratic_saturation(e: (f64, f64), se: (f64, f64)) -> Result<(f64, f64), String> {
    if e.0 == 0.0 || e.1 == 0.0 || se.0 == 0.0 || se.1 == 0.0 {
        return Ok((0.0, 0.0));
    }
    if e.1 <= e.0 || se.1 <= se.0 {
        return Err(format!(
            "inconsistent saturation data: E = {e:?}, Se = {se:?} must both increase"
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

    #[test]
    fn exponential_saturation_passes_through_both_points() {
        let (a, b) = exponential_saturation((0.0392, 0.2672)).unwrap();
        assert!((b - 0.0392).abs() < 1e-15);
        assert!((b * 1.2_f64.powf(a) - 0.2672).abs() < 1e-12);
    }

    #[test]
    fn nonwindup_holds_at_limit() {
        assert_eq!(low_pass_nonwindup_mm(10.0, 5.0, 1.0, -5.0, 5.0), (5.0, 0.0));
        assert_eq!(low_pass_nonwindup_mm(0.0, 5.0, 1.0, -5.0, 5.0), (5.0, -5.0));
    }
}
