//! Canonical inverter parameter paths, including control-reference dependence.
use super::duals::{
    CompilerMap, McOpfParameter as Parameter, McOpfParameterKind as Kind,
    McOpfParameterOwner as Owner, Rule,
};
use powerio_dist::{ActivePowerReference as Reference, DistControlProfile, DistIbr, IbrTopology};
use powerio_matrix::{McOpfDevice, McOpfDroop};

#[derive(Clone, Debug)]
pub(super) struct DroopParameter {
    columns: Vec<usize>,
    knots: Vec<f64>,
    values: Vec<f64>,
    epsilon: f64,
}
impl DroopParameter {
    /// c_theta and its exact mixed derivative in the compiler's magnitude lifts.
    pub fn evaluate(&self, x: &[f64]) -> Result<(f64, Vec<(usize, f64)>), String> {
        let count = self.columns.len() as f64;
        let mean = self.columns.iter().map(|&c| x[c]).sum::<f64>() / count;
        let mut value = self.values[0];
        let mut slope = 0.;
        for j in 0..self.knots.len() - 1 {
            let d = (self.values[j + 1] - self.values[j]) / (self.knots[j + 1] - self.knots[j]);
            for (k, sign) in [(j, 1.), (j + 1, -1.)] {
                let z = (mean - self.knots[k]) / self.epsilon;
                value += sign * d * self.epsilon * (z.max(0.) + (-z.abs()).exp().ln_1p());
                let sigmoid = if z >= 0. {
                    1. / (1. + (-z).exp())
                } else {
                    z.exp() / (1. + z.exp())
                };
                slope += sign * d * sigmoid;
            }
        }
        if !value.is_finite() || !slope.is_finite() {
            return Err("nonfinite inverter control-reference derivative".into());
        }
        Ok((
            -value,
            self.columns.iter().map(|&c| (c, -slope / count)).collect(),
        ))
    }
}

pub(super) struct InverterParameters<'a> {
    pub inv: &'a DistIbr,
    profile: Option<&'a DistControlProfile>,
    count: usize,
    sb: f64,
}
impl<'a> InverterParameters<'a> {
    pub fn profile(&self) -> Option<&'a DistControlProfile> {
        self.profile
    }
    pub fn new(
        inv: &'a DistIbr,
        net: &'a powerio::MulticonductorNetwork,
        count: usize,
        sb: f64,
    ) -> Self {
        Self {
            inv,
            profile: inv
                .control_profile
                .as_ref()
                .and_then(|name| net.control_profiles().iter().find(|p| p.name == *name)),
            count,
            sb,
        }
    }
    fn parameter(&self, axis: usize, kind: Kind) -> Parameter {
        Parameter::new(Owner::Inverter, &self.inv.name, Some(axis), kind)
    }
    fn availability(&self, axis: usize) -> bool {
        let Some(p) = &self.inv.p_max else {
            return false;
        };
        if p.len() != self.count || p[axis] <= 0. || p.iter().any(|v| !v.is_finite() || *v < 0.) {
            return false;
        }
        // Only an availability-referenced controller observes p_avail. Its
        // current reference must agree with the retained-session edit path.
        let observes_total = self
            .profile
            .and_then(|p| p.volt_watt.as_ref())
            .is_some_and(|v| v.p_ref == Some(Reference::PAvailable));
        let total: f64 = p.iter().sum();
        if !total.is_finite() {
            return false;
        }
        !observes_total
            || self
                .inv
                .p_avail
                .is_some_and(|a| (a - total).abs() <= 1e-12 * (1. + total.abs()))
    }
    pub fn initialize(&self, map: &mut CompilerMap) {
        for k in 0..self.count {
            let p = self.parameter(k, Kind::Availability);
            map.ensure(p.clone());
            if self.availability(k) {
                map.term(p, None, Rule::Zero);
            }
        }
    }
    pub fn bound(
        &self,
        map: &mut CompilerMap,
        axis: usize,
        kind: Kind,
        origin: usize,
        lower: bool,
    ) {
        let rule = Rule::Bound {
            scale: self.sb.recip(),
            lower,
        };
        map.term(self.parameter(axis, kind), Some(origin), rule.clone());
        if kind == Kind::ActivePowerUpper && self.availability(axis) {
            map.term(self.parameter(axis, Kind::Availability), Some(origin), rule);
        }
    }
    pub fn apparent(&self, map: &mut CompilerMap, axis: usize, origin: usize, cap: f64) {
        map.term(
            self.parameter(axis, Kind::ApparentPowerRating),
            Some(origin),
            Rule::Norm {
                physical: cap * self.sb,
            },
        );
    }
    pub fn current(
        &self,
        map: &mut CompilerMap,
        dev: &McOpfDevice,
        axis: Option<usize>,
        origin: usize,
        cap_a: f64,
    ) {
        let Some(ratings) = &self.inv.i_max else {
            return;
        };
        let at = |k: usize| ratings[k];
        let position = |terminal| {
            dev.terminals
                .iter()
                .position(|&t| t == terminal)
                .expect("prepared inverter terminal")
        };
        let indices = if self.inv.topology == IbrTopology::ThreeLeg {
            vec![axis.expect("delta coil")]
        } else if let Some(k) = axis {
            let coil = &dev.coils[k];
            let mut ids = vec![position(coil.positive)];
            if dev.coils.len() == 1 {
                if let Some(t) = coil.negative {
                    ids.push(position(t));
                }
            }
            ids
        } else {
            vec![position(dev.coils[0].negative.expect("neutral"))]
        };
        for &k in &indices {
            if !at(k).is_finite() {
                continue;
            }
            let p = self.parameter(k, Kind::CurrentRating);
            map.ensure(p.clone());
            if indices.len() == 2 && at(indices[0]) == at(indices[1]) {
                // min(I_phase,I_return) has no independent two-sided partial
                // at a tie.
                continue;
            }
            let rule = if indices.iter().any(|&j| at(j) < at(k)) {
                Rule::Zero
            } else {
                Rule::Norm { physical: cap_a }
            };
            map.term(p, Some(origin), rule);
        }
    }
    pub fn control(
        &self,
        map: &mut CompilerMap,
        axis: usize,
        family: &str,
        origin: usize,
        curve: &McOpfDroop,
        columns: Vec<usize>,
    ) {
        let Some(profile) = self.profile else {
            return;
        };
        let (kind, values, total) = if family == "volt_var" {
            let v = profile.volt_var.as_ref().expect("prepared volt-var");
            (
                Kind::ApparentPowerRating,
                vec![v.q_limits[1], 0., 0., v.q_limits[0]],
                false,
            )
        } else {
            let v = profile.volt_watt.as_ref().expect("prepared volt-watt");
            let (kind, total) = match v.p_ref.unwrap_or(Reference::SMax) {
                Reference::SMax => (Kind::ApparentPowerRating, false),
                Reference::PMax => (Kind::ActivePowerUpper, false),
                Reference::PAvailable => (Kind::Availability, true),
                _ => return,
            };
            (kind, vec![v.p_limits[1], v.p_limits[0]], total)
        };
        let factor = 1. / (self.sb * if total { self.count as f64 } else { 1. });
        if kind == Kind::ApparentPowerRating && self.inv.s_max[axis] == 0. {
            map.ensure(self.parameter(axis, kind));
            return;
        }
        let rule = Rule::Inverter(DroopParameter {
            columns,
            knots: curve.knots.clone(),
            values: values.iter().map(|v| v * factor).collect(),
            epsilon: curve.epsilon,
        });
        if total {
            for k in 0..self.count {
                if self.availability(k) {
                    map.term(self.parameter(k, kind), Some(origin), rule.clone());
                }
            }
        } else {
            map.term(self.parameter(axis, kind), Some(origin), rule.clone());
            if kind == Kind::ActivePowerUpper && self.availability(axis) {
                map.term(self.parameter(axis, Kind::Availability), Some(origin), rule);
            }
        }
    }
}
