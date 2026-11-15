//! Shared canonical control-profile derivatives in existing compiler coordinates.
use super::duals::{
    CompilerMap, McOpfParameter as Parameter, McOpfParameterKind as Kind,
    McOpfParameterOwner as Owner, Rule,
};
use powerio_dist::{ActivePowerReference, DistControlProfile, DistIbr};
use powerio_matrix::{McAcOpfAssemblyOptions, McOpfDroop};

#[derive(Clone, Debug)]
pub(super) enum ControlParameter {
    Droop {
        columns: Vec<usize>,
        curve: McOpfDroop,
        d_knots: Vec<f64>,
        d_values: Vec<f64>,
        d_epsilon: f64,
    },
    PowerFactor {
        columns: [usize; 2],
        active_coefficient_derivative: f64,
    },
}

impl ControlParameter {
    /// Residual parameter partial and exact mixed state–parameter partial.
    pub fn evaluate(&self, x: &[f64]) -> Result<(f64, Vec<(usize, f64)>), String> {
        let (value, gradient) = match self {
            // The row is `a(pf) * P - pf * Q` (OpenDSS and BMOPF sign: pf > 0
            // puts Q on the same side as P), so d/dpf is `a'(pf) * P - Q`.
            Self::PowerFactor {
                columns: [p, q],
                active_coefficient_derivative: da,
            } => (da * x[*p] - x[*q], vec![(*p, *da), (*q, -1.)]),
            // PowerIO's droop parameter derivative at the mean monitored
            // magnitude: its value, and its voltage slope spread over the
            // averaged magnitude columns.
            Self::Droop {
                columns,
                curve,
                d_knots,
                d_values,
                d_epsilon,
            } => {
                let count = columns.len() as f64;
                let mean = columns.iter().map(|&c| x[c]).sum::<f64>() / count;
                let derivative =
                    curve.calc_parameter_derivative(mean, d_knots, d_values, *d_epsilon);
                (
                    -derivative.value,
                    columns
                        .iter()
                        .map(|&c| (c, -derivative.slope / count))
                        .collect(),
                )
            }
        };
        if !value.is_finite() || gradient.iter().any(|(_, v)| !v.is_finite()) {
            return Err("nonfinite control-profile parameter derivative".into());
        }
        Ok((value, gradient))
    }
}

pub(super) struct ControlParameters<'a> {
    inv: &'a DistIbr,
    profile: &'a DistControlProfile,
    count: usize,
    bases: McAcOpfAssemblyOptions,
}
impl<'a> ControlParameters<'a> {
    pub fn new(
        inv: &'a DistIbr,
        profile: &'a DistControlProfile,
        count: usize,
        bases: McAcOpfAssemblyOptions,
    ) -> Self {
        Self {
            inv,
            profile,
            count,
            bases,
        }
    }
    fn parameter(&self, axis: Option<usize>, kind: Kind) -> Parameter {
        Parameter::new(Owner::ControlProfile, &self.profile.name, axis, kind)
    }
    pub fn initialize(&self, map: &mut CompilerMap) {
        if self.profile.power_factor.is_some() {
            map.ensure(self.parameter(None, Kind::PowerFactor));
        }
        for (present, knots, breakpoint, fraction) in [
            (
                self.profile.volt_var.is_some(),
                4,
                Kind::VoltVarBreakpoint,
                Kind::VoltVarFraction,
            ),
            (
                self.profile.volt_watt.is_some(),
                2,
                Kind::VoltWattBreakpoint,
                Kind::VoltWattFraction,
            ),
        ] {
            if present {
                for k in 0..knots {
                    map.ensure(self.parameter(Some(k), breakpoint));
                }
                for k in 0..2 {
                    map.ensure(self.parameter(Some(k), fraction));
                }
            }
        }
    }
    pub fn power_factor(&self, map: &mut CompilerMap, origin: usize, columns: [usize; 2]) {
        let Some(pf) = self.profile.power_factor.as_ref().map(|p| p.pf) else {
            return;
        };
        // At unity the canonical PF -> Q/P map has no finite two-sided derivative.
        if pf.abs() >= 1. || pf == 0. {
            return;
        }
        let [derivative, _] = powerio::electrical::calc_power_factor_coefficients_derivative(pf);
        map.term(
            self.parameter(None, Kind::PowerFactor),
            Some(origin),
            Rule::Control(ControlParameter::PowerFactor {
                columns,
                active_coefficient_derivative: derivative,
            }),
        );
    }
    pub fn droop(
        &self,
        map: &mut CompilerMap,
        axis: usize,
        family: &str,
        origin: usize,
        curve: &McOpfDroop,
        columns: Vec<usize>,
    ) {
        let (breakpoint, fraction, base) = if family == "volt_var" {
            (
                Kind::VoltVarBreakpoint,
                Kind::VoltVarFraction,
                self.inv.s_max[axis] / self.bases.power_base_va,
            )
        } else {
            let reference = self
                .profile
                .volt_watt
                .as_ref()
                .expect("prepared volt-watt")
                .p_ref
                .unwrap_or(ActivePowerReference::SMax);
            let base = match reference {
                ActivePowerReference::SMax => self.inv.s_max[axis],
                ActivePowerReference::PMax => {
                    self.inv.p_max.as_ref().expect("prepared PMax reference")[axis]
                }
                ActivePowerReference::PAvailable => {
                    self.inv.p_avail.expect("prepared availability reference") / self.count as f64
                }
                _ => return,
            } / self.bases.power_base_va;
            (Kind::VoltWattBreakpoint, Kind::VoltWattFraction, base)
        };
        let n = curve.knots.len();
        for k in 0..n + 2 {
            let mut d_knots = vec![0.; n];
            let mut d_values = vec![0.; n];
            let (parameter, d_epsilon) = if k < n {
                d_knots[k] = self.bases.voltage_base_v.recip();
                // The preparation sets epsilon to a share of the mean knot.
                (
                    self.parameter(Some(k), breakpoint),
                    powerio_matrix::MC_OPF_DROOP_SMOOTHING / (n as f64 * self.bases.voltage_base_v),
                )
            } else {
                let index = k - n;
                d_values[if index == 0 { n - 1 } else { 0 }] = base;
                (self.parameter(Some(index), fraction), 0.)
            };
            map.term(
                parameter,
                Some(origin),
                Rule::Control(ControlParameter::Droop {
                    columns: columns.clone(),
                    curve: curve.clone(),
                    d_knots,
                    d_values,
                    d_epsilon,
                }),
            );
        }
    }
}
