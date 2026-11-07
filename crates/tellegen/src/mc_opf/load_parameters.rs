//! Nominal P/Q dependence retained independently of preparation's zero-term
//! elimination. Coefficients and nominal voltage are held fixed.
use super::model::Pair;
use powerio_dist::{DistLoad, DistLoadVoltageModel as Law};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub(super) enum Coordinate {
    Power { log_voltage: Option<usize> },
    Current { reactive: bool, part: usize },
}

#[derive(Clone, Debug)]
pub(super) struct LoadParameter {
    /// [weight / power_base_va, voltage exponent], per W or var.
    terms: Vec<[f64; 2]>,
    nominal: f64,
    voltage: Pair,
    coordinate: Coordinate,
}

/// Unit-normalized canonical terms, including terms whose nominal P/Q is zero.
/// Defaults and scalar/per-coil broadcasting match released PowerIO preparation.
pub(super) fn terms(
    load: &DistLoad,
    coil: usize,
    count: usize,
    sb: f64,
) -> Result<[Vec<[f64; 2]>; 2], String> {
    let at = |v: &[f64], default: f64| -> Result<f64, String> {
        let value = match v.len() {
            0 => default,
            1 => v[0],
            n if n == count => v[coil],
            _ => return Err(format!("{}: invalid load parameter axis", load.name)),
        };
        if value.is_finite() {
            Ok(value)
        } else {
            Err(format!("{}: nonfinite load parameter", load.name))
        }
    };
    let terms = match &load.voltage_model {
        Law::ConstantPower { .. } => [vec![[1., 0.]], vec![[1., 0.]]],
        Law::ConstantCurrent { .. } => [vec![[1., 1.]], vec![[1., 1.]]],
        Law::ConstantImpedance { .. } => [vec![[1., 2.]], vec![[1., 2.]]],
        Law::Exponential {
            gamma_p, gamma_q, ..
        } => [vec![[1., at(gamma_p, 0.)?]], vec![[1., at(gamma_q, 0.)?]]],
        Law::Zip {
            alpha_z,
            alpha_i,
            alpha_p,
            beta_z,
            beta_i,
            beta_p,
            ..
        } => {
            let a_empty = alpha_z.is_empty() && alpha_i.is_empty() && alpha_p.is_empty();
            let b_empty = beta_z.is_empty() && beta_i.is_empty() && beta_p.is_empty();
            [
                vec![
                    [at(alpha_z, 0.)?, 2.],
                    [at(alpha_i, 0.)?, 1.],
                    [at(alpha_p, f64::from(a_empty))?, 0.],
                ],
                vec![
                    [at(beta_z, 0.)?, 2.],
                    [at(beta_i, 0.)?, 1.],
                    [at(beta_p, f64::from(b_empty))?, 0.],
                ],
            ]
        }
        _ => {
            return Err(format!(
                "{}: unsupported nominal load parameter law",
                load.name
            ))
        }
    };
    Ok(terms.map(|terms| {
        terms
            .into_iter()
            .filter(|t| t[0] != 0.)
            .map(|[w, g]| [w / sb, g])
            .collect()
    }))
}

impl LoadParameter {
    pub fn new(terms: Vec<[f64; 2]>, nominal: f64, voltage: Pair, coordinate: Coordinate) -> Self {
        Self {
            terms,
            nominal,
            voltage,
            coordinate,
        }
    }

    /// Partial derivative of a compiled constraint residual with respect to
    /// nominal P or Q, and its exact mixed derivative with the primal columns.
    pub fn evaluate(&self, x: &[f64]) -> Result<(f64, Vec<(usize, f64)>), String> {
        let invalid = || {
            "load parameter derivative outside its finite differentiable voltage domain".to_string()
        };
        if let Coordinate::Power {
            log_voltage: Some(ell),
        } = self.coordinate
        {
            let mut value = 0.;
            let mut derivative = 0.;
            for &[w, g] in &self.terms {
                let term = w * (g * x[ell]).exp();
                value -= term;
                derivative -= g * term;
            }
            return if value.is_finite() && derivative.is_finite() {
                Ok((value, vec![(ell, derivative)]))
            } else {
                Err(invalid())
            };
        }
        let u = self.voltage.each_ref().map(|v| v.value(x));
        let magnitude = u[0].hypot(u[1]);
        let ratio = magnitude / self.nominal;
        let mut gradient = BTreeMap::<usize, f64>::new();
        let (value, du) = match self.coordinate {
            Coordinate::Power { .. } => {
                let mut value = 0.;
                let mut radial = 0.;
                for &[w, g] in &self.terms {
                    if magnitude == 0. {
                        if g == 0. {
                            value -= w;
                        } else {
                            return Err(invalid());
                        }
                    } else {
                        let term = w * ratio.powf(g);
                        value -= term;
                        radial -= g * term;
                    }
                }
                let du = if magnitude == 0. {
                    [0.; 2]
                } else {
                    u.map(|v| radial * (v / magnitude) / magnitude)
                };
                (value, du)
            }
            Coordinate::Current { reactive, part } => {
                // I = conj(S)*U/|U|^2. The current residual is I minus this
                // constitutive law. Reactive injection rotates by -j.
                let index = if reactive { 1 - part } else { part };
                let sign = if reactive && part == 1 { 1. } else { -1. };
                let mut h = 0.;
                let mut radial = 0.;
                for &[w, g] in &self.terms {
                    if magnitude == 0. {
                        if g == 2. {
                            h += w / self.nominal.powi(2);
                        } else {
                            return Err(invalid());
                        }
                    } else {
                        let term = w * ratio.powf(g - 2.) / self.nominal.powi(2);
                        h += term;
                        radial += (g - 2.) * term;
                    }
                }
                let mut du = if magnitude == 0. {
                    [0.; 2]
                } else {
                    u.map(|v| sign * u[index] * radial * (v / magnitude) / magnitude)
                };
                du[index] += sign * h;
                (sign * u[index] * h, du)
            }
        };
        for (v, d) in self.voltage.iter().zip(du) {
            for (&column, &coefficient) in &v.terms {
                *gradient.entry(column).or_default() += d * coefficient;
            }
        }
        if !value.is_finite() || du.iter().chain(gradient.values()).any(|v| !v.is_finite()) {
            return Err(invalid());
        }
        Ok((value, gradient.into_iter().collect()))
    }
}
