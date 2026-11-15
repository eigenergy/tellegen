//! Nominal P/Q dependence retained independently of preparation's zero-term
//! elimination. Coefficients and nominal voltage are held fixed.
use super::model::Pair;
use powerio::electrical::{calc_log_voltage_power_term, calc_voltage_power_term};
use powerio_dist::DistLoad;
use powerio_matrix::McOpfLoadLaw;
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

/// Unit-normalized canonical terms of coil `coil`, including terms whose
/// nominal P/Q is zero: PowerIO's term form of the load law
/// (`DistLoadVoltageModel::calc_power_terms`), each fraction divided by the
/// power base. Every coefficient is required, one entry per coil, as the
/// preparation reads it.
pub(super) fn terms(load: &DistLoad, coil: usize, sb: f64) -> Result<[Vec<[f64; 2]>; 2], String> {
    let terms = load
        .voltage_model
        .calc_power_terms(coil)
        .map_err(|error| format!("{}: {error}", load.name))?
        .ok_or_else(|| format!("{}: unsupported nominal load parameter law", load.name))?;
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
            // The compiled residual's own coordinate: the model states each
            // term as `w exp(g ell)` over the log-voltage lift `ell`, which
            // is `w (|U|/V_nom)^g` on the positive voltage domain the lift
            // defines; PowerIO's log voltage term gives that expression and
            // its exact partial.
            let mut value = 0.;
            let mut derivative = 0.;
            for &[w, g] in &self.terms {
                let (term, slope) = calc_log_voltage_power_term(w, g, x[ell]);
                value -= term;
                derivative -= slope;
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
                        // PowerIO's one term evaluation; `radial` is the
                        // slope times the ratio, `|U| d/d|U|`.
                        let (term, slope) = calc_voltage_power_term(w, g, ratio);
                        value -= term;
                        radial -= slope * ratio;
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
                if magnitude == 0. {
                    // At zero voltage only a constant impedance law has a
                    // current: PowerIO's one admittance `Σ w / V_nom²` of an
                    // all exponent two law. Added to `h = +0.0` so an empty
                    // term list keeps a positive zero (an empty f64 sum is
                    // -0.0).
                    h += McOpfLoadLaw::new(self.nominal, [self.terms.clone(), Vec::new()])
                        .calc_impedance_admittance()
                        .ok_or_else(invalid)?
                        .re;
                } else {
                    for &[w, g] in &self.terms {
                        // The admittance form `S / |U|^2` of the same term.
                        let (term, slope) = calc_voltage_power_term(w, g - 2., ratio);
                        h += term / self.nominal.powi(2);
                        radial += slope * ratio / self.nominal.powi(2);
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
