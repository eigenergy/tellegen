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
        knots: Vec<f64>,
        values: Vec<f64>,
        epsilon: f64,
        d_knots: Vec<f64>,
        d_values: Vec<f64>,
        d_epsilon: f64,
    },
    PowerFactor {
        column: usize,
        slope_derivative: f64,
    },
}

// Softplus, sigmoid, sigmoid derivative, and softplus(z)-z*sigmoid(z).
// The entropy form avoids cancellation and overflow in saturated regions.
fn smooth(z: f64) -> [f64; 4] {
    let e = (-z.abs()).exp();
    let sigma = if z >= 0. { 1. / (1. + e) } else { e / (1. + e) };
    [
        z.max(0.) + e.ln_1p(),
        sigma,
        e / (1. + e).powi(2),
        e.ln_1p() + z.abs() * e / (1. + e),
    ]
}
impl ControlParameter {
    /// Residual parameter partial and exact mixed state–parameter partial.
    pub fn evaluate(&self, x: &[f64]) -> Result<(f64, Vec<(usize, f64)>), String> {
        let (value, gradient) = match self {
            Self::PowerFactor {
                column,
                slope_derivative,
            } => (
                -slope_derivative * x[*column],
                vec![(*column, -slope_derivative)],
            ),
            Self::Droop {
                columns,
                knots,
                values,
                epsilon,
                d_knots,
                d_values,
                d_epsilon,
            } => {
                let count = columns.len() as f64;
                let mean = columns.iter().map(|&c| x[c]).sum::<f64>() / count;
                let mut value = d_values[0];
                let mut mixed = 0.;
                for j in 0..knots.len() - 1 {
                    let width = knots[j + 1] - knots[j];
                    let slope = (values[j + 1] - values[j]) / width;
                    let ds =
                        (d_values[j + 1] - d_values[j] - slope * (d_knots[j + 1] - d_knots[j]))
                            / width;
                    let z = [(mean - knots[j]) / epsilon, (mean - knots[j + 1]) / epsilon];
                    let [a, b] = z.map(smooth);
                    let hinge = epsilon * (a[0] - b[0]);
                    let dh = d_epsilon * (a[3] - b[3]) - d_knots[j] * a[1] + d_knots[j + 1] * b[1];
                    let dz = [
                        (-d_knots[j] - z[0] * d_epsilon) / epsilon,
                        (-d_knots[j + 1] - z[1] * d_epsilon) / epsilon,
                    ];
                    value += ds * hinge + slope * dh;
                    mixed += ds * (a[1] - b[1]) + slope * (a[2] * dz[0] - b[2] * dz[1]);
                }
                (
                    -value,
                    columns.iter().map(|&c| (c, -mixed / count)).collect(),
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
    pub fn power_factor(&self, map: &mut CompilerMap, origin: usize, column: usize) {
        let Some(pf) = self.profile.power_factor.as_ref().map(|p| p.pf) else {
            return;
        };
        // At unity the canonical PF -> Q/P map has no finite two-sided derivative.
        if pf.abs() >= 1. || pf == 0. {
            return;
        }
        let derivative = 1. / (pf * pf * (1. - pf * pf).sqrt());
        map.term(
            self.parameter(None, Kind::PowerFactor),
            Some(origin),
            Rule::Control(ControlParameter::PowerFactor {
                column,
                slope_derivative: derivative,
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
                // Released preparation sets epsilon to .002 * mean(knots).
                (
                    self.parameter(Some(k), breakpoint),
                    0.002 / (n as f64 * self.bases.voltage_base_v),
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
                    knots: curve.knots.clone(),
                    values: curve.values.clone(),
                    epsilon: curve.epsilon,
                    d_knots,
                    d_values,
                    d_epsilon,
                }),
            );
        }
    }
}
