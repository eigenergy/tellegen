//! Voltage-dependent branch laws and nominal admittance compensation.

use num_complex::Complex64;
use powerio_dist::{Configuration, DistLoad, DistLoadVoltageModel};

#[derive(Clone, Debug)]
pub(crate) enum BranchVoltageModel {
    /// The typed law owns its bounds; legacy solver envelope options do not replace them.
    Bounded(DistLoadVoltageModel),
    /// An unbounded law PowerIO evaluates (`DistLoadVoltageModel::power_scale_where`):
    /// constant power, constant current, ZIP or exponential, with its nominal
    /// voltages and coefficients stated once per branch.
    Law(DistLoadVoltageModel),
    ConstantImpedance,
}

use super::{
    network::{NetworkIndex, StampedMatrix},
    McPfOptions,
};

#[derive(Clone, Debug)]
pub(crate) struct BranchLoad {
    pub name: String,
    pub bus: String,
    /// Signed incidence rows from global nodal voltages to branch voltages.
    pub incidence: Vec<Vec<(usize, Complex64)>>,
    pub power: Vec<Complex64>,
    /// Physical nominal admittance implied by the current prescribed power.
    /// This changes when an interactive session edits the load.
    pub nominal_admittance: Vec<Complex64>,
    /// Frozen compensation admittance stamped into the retained linear
    /// operator. It is deliberately independent of later load edits.
    pub reference_admittance: Vec<Complex64>,
    pub nominal_voltage: Vec<f64>,
    pub model: BranchVoltageModel,
}

impl BranchLoad {
    pub(crate) fn branch_current(
        &self,
        branch: usize,
        voltage: &[Complex64],
        options: &McPfOptions,
    ) -> Result<Complex64, String> {
        let u = self.branch_voltage(branch, voltage);
        // A zero-power branch carries no current for every supported law. Do
        // this before checking voltage so an unloaded branch is well-defined
        // even when it is disconnected from a prescribed source.
        if self.power[branch].norm() == 0.0 {
            return Ok(Complex64::default());
        }
        // Calculate the magnitude with hypot;
        // hypot preserves a nonzero magnitude for subnormal phasors.
        let magnitude = u.re.hypot(u.im);
        if magnitude == 0.0 {
            if options.voltage_envelope
                || self.zero_current_at_zero_voltage(
                    branch,
                    magnitude,
                    options.zero_voltage_tolerance,
                )
            {
                return Ok(Complex64::default());
            }
            return Err(format!(
                "load `{}` branch {branch} has near-zero voltage",
                self.name
            ));
        }
        if !options.voltage_envelope
            && magnitude <= options.zero_voltage_tolerance
            && !self.near_zero_law_is_safe(branch)
        {
            return Err(format!(
                "load `{}` branch {branch} has near-zero voltage",
                self.name
            ));
        }
        // Evaluate the radial current equations. Dividing by |U| and
        // multiplying by the unit phasor avoids forming U*conjugate(U), which
        // underflows for a perfectly valid tiny nonzero voltage.
        let current = match &self.model {
            BranchVoltageModel::Bounded(_) => self.bounded_current(branch, u, magnitude)?,
            _ if options.voltage_envelope => {
                self.enveloped_current(branch, u, magnitude, options)?
            }
            _ => self.current_at_voltage(branch, u, magnitude)?,
        };
        if !current.re.is_finite() || !current.im.is_finite() {
            return Err(format!(
                "load `{}` branch {branch} produced a non-finite current",
                self.name
            ));
        }
        let absorbed_power = u * current.conj();
        if !absorbed_power.re.is_finite() || !absorbed_power.im.is_finite() {
            return Err(format!(
                "load `{}` branch {branch} produced a non-finite absorbed power",
                self.name
            ));
        }
        Ok(current)
    }

    pub(crate) fn branch_voltage(&self, branch: usize, voltage: &[Complex64]) -> Complex64 {
        self.incidence[branch]
            .iter()
            .map(|&(i, c)| c * voltage[i])
            .sum()
    }

    /// The solver's legacy voltage envelope (`v_low_pu`, `v_min_pu`,
    /// `v_max_pu`) around an unbounded law, evaluated by PowerIO
    /// (`DistLoadVoltageModel::power_scale_in_voltage_bands`). Constant impedance
    /// is `u²` in every band.
    fn enveloped_current(
        &self,
        branch: usize,
        u: Complex64,
        magnitude: f64,
        options: &McPfOptions,
    ) -> Result<Complex64, String> {
        let BranchVoltageModel::Law(model) = &self.model else {
            return self.current_at_voltage(branch, u, magnitude);
        };
        let power = self.power[branch];
        let [p, q] = model
            .power_scale_in_voltage_bands(
                branch,
                magnitude,
                [options.v_low_pu, options.v_min_pu, options.v_max_pu],
                [power.re != 0.0, power.im != 0.0],
            )
            .map_err(|error| format!("load `{}` branch {branch}: {error}", self.name))?;
        Ok(self.current_from_scale(branch, u, magnitude, [p, q]))
    }

    fn current_at_voltage(
        &self,
        branch: usize,
        u: Complex64,
        magnitude: f64,
    ) -> Result<Complex64, String> {
        match &self.model {
            BranchVoltageModel::Bounded(_) => unreachable!("bounded laws use bounded_current"),
            BranchVoltageModel::ConstantImpedance => Ok(self.nominal_admittance[branch] * u),
            BranchVoltageModel::Law(model) => self.law_current(model, branch, u, magnitude),
        }
    }

    /// The branch current `conj(S·scale / U)` of a law PowerIO evaluates,
    /// formed as the unit phasor times `(P·p, -Q·q) / |U|`, so no product
    /// `U·conj(U)` can underflow. A component the load does not draw is not
    /// evaluated.
    fn law_current(
        &self,
        model: &DistLoadVoltageModel,
        branch: usize,
        u: Complex64,
        magnitude: f64,
    ) -> Result<Complex64, String> {
        let power = self.power[branch];
        let scale = model
            .power_scale_where(branch, magnitude, [power.re != 0.0, power.im != 0.0])
            .map_err(|error| format!("load `{}` branch {branch}: {error}", self.name))?;
        Ok(self.current_from_scale(branch, u, magnitude, scale))
    }

    /// `conj(S·scale / U)`, as the unit phasor times `(P·p, -Q·q) / |U|`.
    fn current_from_scale(
        &self,
        branch: usize,
        u: Complex64,
        magnitude: f64,
        [p, q]: [f64; 2],
    ) -> Complex64 {
        let power = self.power[branch];
        (u / magnitude) * Complex64::new(power.re * p / magnitude, -power.im * q / magnitude)
    }

    /// A bounded law owns its envelope; PowerIO evaluates it whole.
    fn bounded_current(
        &self,
        branch: usize,
        u: Complex64,
        magnitude: f64,
    ) -> Result<Complex64, String> {
        let BranchVoltageModel::Bounded(model) = &self.model else {
            unreachable!()
        };
        self.law_current(model, branch, u, magnitude)
    }

    fn zero_current_at_zero_voltage(&self, branch: usize, magnitude: f64, zero_tol: f64) -> bool {
        if magnitude > zero_tol {
            return false;
        }
        // A term `c (|U|/V_nom)^γ` draws a current magnitude in `|U|^(γ-1)`:
        // only `γ > 1` vanishes at zero voltage, so a constant power or
        // current contribution has no unique phasor there.
        self.law_exponents_exceed(branch, |exponent| exponent > 1.0)
    }

    fn near_zero_law_is_safe(&self, branch: usize) -> bool {
        // The current stays bounded near zero voltage for `γ >= 1`.
        self.law_exponents_exceed(branch, |exponent| exponent >= 1.0)
    }

    /// Whether every voltage power law term this branch draws (a nonzero
    /// fraction of a nonzero nominal component, PowerIO's term form
    /// `DistLoadVoltageModel::calc_power_terms`) has an exponent `admits`.
    /// Constant impedance and the bounded laws, whose low voltage band is
    /// constant impedance, always do.
    fn law_exponents_exceed(&self, branch: usize, admits: impl Fn(f64) -> bool) -> bool {
        let BranchVoltageModel::Law(model) = &self.model else {
            return true;
        };
        let s = self.power[branch];
        match model.calc_power_terms(branch) {
            Ok(Some(terms)) => [s.re, s.im]
                .iter()
                .zip(&terms)
                .filter(|(component, _)| **component != 0.0)
                .all(|(_, terms)| {
                    terms
                        .iter()
                        .filter(|[fraction, _]| *fraction != 0.0)
                        .all(|&[_, exponent]| admits(exponent))
                }),
            _ => false,
        }
    }

    pub(crate) fn add_current(
        &self,
        voltage: &[Complex64],
        options: &McPfOptions,
        result: &mut [Complex64],
    ) -> Result<(), String> {
        if result.len() != voltage.len() {
            return Err("load current scratch vector has the wrong dimension".to_owned());
        }
        for (row, incidence) in self.incidence.iter().enumerate() {
            let i_branch = self.branch_current(row, voltage, options)?;
            // I_load = conjugate(S / U), with positive S consumed by the
            // branch.  Conjugating only S is wrong for nonzero voltage angle.
            for &(i, c) in incidence {
                // C is real for supported connections; using conjugate here
                // also keeps the formula correct if a future primitive uses a
                // phase-shifted branch map.
                result[i] += c.conj() * i_branch;
            }
        }
        Ok(())
    }

    pub(crate) fn stamp_yref(&self, y: &mut StampedMatrix) {
        for (branch, incidence) in self.incidence.iter().enumerate() {
            for &(i, ci) in incidence {
                for &(j, cj) in incidence {
                    y.add(i, j, ci.conj() * self.reference_admittance[branch] * cj);
                }
            }
        }
    }

    pub(crate) fn replace_power(&mut self, power: Vec<Complex64>) -> Result<(), String> {
        if power.len() != self.power.len() {
            return Err(format!(
                "load `{}` received {} branch powers; expected {}",
                self.name,
                power.len(),
                self.power.len()
            ));
        }
        if power
            .iter()
            .any(|value| !value.re.is_finite() || !value.im.is_finite())
        {
            return Err(format!(
                "load `{}` received a non-finite prescribed power",
                self.name
            ));
        }
        let nominal_admittance = power
            .iter()
            .zip(&self.nominal_voltage)
            .map(|(&s, &v)| powerio_dist::calc_load_nominal_admittance(s, v))
            .collect::<Vec<_>>();
        if nominal_admittance
            .iter()
            .any(|value| !value.re.is_finite() || !value.im.is_finite())
        {
            return Err(format!(
                "load `{}` produced a non-finite nominal admittance",
                self.name
            ));
        }
        self.power = power;
        self.nominal_admittance = nominal_admittance;
        Ok(())
    }
}

pub(crate) fn prepare_load(
    network: &powerio_dist::MulticonductorNetwork,
    load: &DistLoad,
    index: &NetworkIndex,
    inferred_nominal_voltage: Option<&[f64]>,
) -> Result<BranchLoad, String> {
    if load.p_nom.len() != load.q_nom.len() {
        return Err(format!(
            "load `{}` has mismatched p_nom/q_nom lengths",
            load.name
        ));
    }
    if load.p_nom.is_empty() {
        return Err(format!("load `{}` has no branch powers", load.name));
    }
    if load
        .p_nom
        .iter()
        .chain(load.q_nom.iter())
        .any(|value| !value.is_finite())
    {
        return Err(format!(
            "load `{}` has a non-finite prescribed power",
            load.name
        ));
    }
    let incidence = connection_incidence(network, load, index)?;
    if incidence.len() != load.p_nom.len() {
        return Err(format!(
            "load `{}` branch map has {} rows for {} powers",
            load.name,
            incidence.len(),
            load.p_nom.len()
        ));
    }
    let vnom_raw = load.voltage_model.v_nom();
    if vnom_raw.is_empty()
        && !matches!(
            load.voltage_model,
            DistLoadVoltageModel::ConstantPower { .. }
        )
    {
        return Err(format!(
            "load `{}` omits nominal branch voltage required by its voltage-dependent model",
            load.name
        ));
    }
    let vnom = if vnom_raw.is_empty() {
        coefficients(
            &load.name,
            "inferred v_nom",
            inferred_nominal_voltage.ok_or_else(|| {
                format!(
                    "load `{}` omits nominal branch voltage and its bus voltage base could not be inferred",
                    load.name
                )
            })?,
            load.p_nom.len(),
        )?
    } else {
        coefficients(&load.name, "v_nom", vnom_raw, load.p_nom.len())?
    };
    if vnom.iter().any(|v| *v <= 0.0) {
        return Err(format!(
            "load `{}` has invalid nominal branch voltages",
            load.name
        ));
    }
    let power: Vec<_> = load
        .p_nom
        .iter()
        .zip(&load.q_nom)
        .map(|(&p, &q)| Complex64::new(p, q))
        .collect();
    let nominal_admittance: Vec<Complex64> = power
        .iter()
        .zip(vnom.iter().copied())
        .map(|(&s, v)| powerio_dist::calc_load_nominal_admittance(s, v))
        .collect();
    if nominal_admittance
        .iter()
        .any(|value| !value.re.is_finite() || !value.im.is_finite())
    {
        return Err(format!(
            "load `{}` produced a non-finite nominal admittance",
            load.name
        ));
    }
    let model = match &load.voltage_model {
        DistLoadVoltageModel::ConstantImpedance { .. } => BranchVoltageModel::ConstantImpedance,
        DistLoadVoltageModel::BoundedZip { .. }
        | DistLoadVoltageModel::BoundedExponential { .. }
        | DistLoadVoltageModel::BoundedConstantPower { .. } => {
            BranchVoltageModel::Bounded(prepare_law(load, &vnom)?)
        }
        DistLoadVoltageModel::ConstantPower { .. }
        | DistLoadVoltageModel::ConstantCurrent { .. }
        | DistLoadVoltageModel::Zip { .. }
        | DistLoadVoltageModel::Exponential { .. } => {
            BranchVoltageModel::Law(prepare_law(load, &vnom)?)
        }
        _ => {
            return Err(format!(
                "load `{}` uses an unsupported voltage model",
                load.name
            ))
        }
    };
    Ok(BranchLoad {
        name: load.name.clone(),
        bus: load.bus.clone(),
        incidence,
        power,
        reference_admittance: nominal_admittance.clone(),
        nominal_admittance,
        nominal_voltage: vnom,
        model,
    })
}

/// The load's law with its branch nominal voltages, every coefficient checked
/// to state one finite entry per branch (PowerIO's per channel law) before
/// PowerIO's per-branch evaluator reads it. Validation is independent of the
/// operating voltage.
fn prepare_law(load: &DistLoad, vnom: &[f64]) -> Result<DistLoadVoltageModel, String> {
    let mut model = load.voltage_model.clone();
    match &mut model {
        DistLoadVoltageModel::ConstantPower { v_nom }
        | DistLoadVoltageModel::ConstantCurrent { v_nom } => *v_nom = vnom.to_vec(),
        DistLoadVoltageModel::Zip {
            v_nom,
            alpha_z,
            alpha_i,
            alpha_p,
            beta_z,
            beta_i,
            beta_p,
        } => {
            *v_nom = vnom.to_vec();
            for (name, values) in [
                ("alpha_z", alpha_z),
                ("alpha_i", alpha_i),
                ("alpha_p", alpha_p),
                ("beta_z", beta_z),
                ("beta_i", beta_i),
                ("beta_p", beta_p),
            ] {
                *values = coefficients(&load.name, name, values, vnom.len())?;
            }
        }
        DistLoadVoltageModel::Exponential {
            v_nom,
            gamma_p,
            gamma_q,
        } => {
            *v_nom = vnom.to_vec();
            *gamma_p = coefficients(&load.name, "gamma_p", gamma_p, vnom.len())?;
            *gamma_q = coefficients(&load.name, "gamma_q", gamma_q, vnom.len())?;
        }
        DistLoadVoltageModel::BoundedZip {
            v_nom,
            alpha_z,
            alpha_i,
            alpha_p,
            beta_z,
            beta_i,
            beta_p,
            v_low_pu,
            v_min_pu,
            v_max_pu,
            cutoff_pu,
        } => {
            *v_nom = vnom.to_vec();
            for (name, values) in [
                ("alpha_z", alpha_z),
                ("alpha_i", alpha_i),
                ("alpha_p", alpha_p),
                ("beta_z", beta_z),
                ("beta_i", beta_i),
                ("beta_p", beta_p),
            ] {
                *values = coefficients(&load.name, name, values, vnom.len())?;
            }
            validate_bounds(&load.name, *v_low_pu, *v_min_pu, *v_max_pu)?;
            if !cutoff_pu.is_finite() || *cutoff_pu < 0.0 {
                return Err(format!("load `{}` has invalid cutoff", load.name));
            }
        }
        DistLoadVoltageModel::BoundedExponential {
            v_nom,
            gamma_p,
            gamma_q,
            v_low_pu,
            v_min_pu,
            v_max_pu,
        } => {
            *v_nom = vnom.to_vec();
            *gamma_p = coefficients(&load.name, "gamma_p", gamma_p, vnom.len())?;
            *gamma_q = coefficients(&load.name, "gamma_q", gamma_q, vnom.len())?;
            validate_bounds(&load.name, *v_low_pu, *v_min_pu, *v_max_pu)?;
        }
        DistLoadVoltageModel::BoundedConstantPower { v_nom, .. } => *v_nom = vnom.to_vec(),
        _ => unreachable!(),
    }
    for (branch, &voltage) in vnom.iter().enumerate() {
        model
            .power_scale(branch, voltage)
            .map_err(|e| format!("load `{}`: {e}", load.name))?;
    }
    Ok(model)
}

fn validate_bounds(name: &str, low: f64, min: f64, max: f64) -> Result<(), String> {
    // A positive low threshold guarantees a finite zero-voltage continuation.
    // Zero/reversed thresholds are left outside this bounded snapshot profile.
    if !low.is_finite()
        || !min.is_finite()
        || !max.is_finite()
        || low <= 0.0
        || low >= min
        || min > max
    {
        return Err(format!(
            "load `{name}` requires finite bounds 0 < vlow < vmin <= vmax"
        ));
    }
    Ok(())
}

fn coefficients(
    name: &str,
    field: &str,
    values: &[f64],
    branches: usize,
) -> Result<Vec<f64>, String> {
    if values.len() != branches {
        return Err(format!(
            "load `{name}` {field} has length {}; expected one entry per branch ({branches})",
            values.len()
        ));
    }
    if values.iter().any(|value| !value.is_finite()) {
        return Err(format!("load `{name}` {field} contains a non-finite value"));
    }
    Ok(values.to_vec())
}

/// Signed rows from the load's terminal voltages to its branch voltages, one
/// per stated power, over PowerIO's connection channels
/// (`powerio_dist::calc_connection_channels`).
fn connection_incidence(
    network: &powerio_dist::MulticonductorNetwork,
    load: &DistLoad,
    index: &NetworkIndex,
) -> Result<Vec<Vec<(usize, Complex64)>>, String> {
    let channels = load_channels(network, load, &load.terminal_map, load.p_nom.len())?;
    let nodes: Vec<usize> = load
        .terminal_map
        .iter()
        .map(|name| index.resolve((&load.bus, name)))
        .collect::<Result<_, _>>()?;
    Ok(channels
        .iter()
        .map(|channel| {
            let mut row = vec![(nodes[channel.from], Complex64::new(1.0, 0.0))];
            if let Some(to) = channel.to {
                row.push((nodes[to], Complex64::new(-1.0, 0.0)));
            }
            row
        })
        .collect())
}

/// The load's connection channels over `terminals`, one per power: PowerIO's
/// one connection rule (`powerio_dist::calc_element_connection`), whose WYE
/// neutral is the mapped terminal the bus conventions do not call a phase,
/// else the last. The power flow keeps its single phase rule, one power
/// across two terminals: PowerIO also reads a single phase load with one
/// power per terminal as grounded wye legs, which this solver has never
/// accepted for a load stated single phase.
pub(crate) fn load_channels(
    network: &powerio_dist::MulticonductorNetwork,
    load: &DistLoad,
    terminals: &[String],
    powers: usize,
) -> Result<Vec<powerio_dist::ConnectionChannel>, String> {
    if load.configuration == Configuration::SinglePhase && (terminals.len() != 2 || powers != 1) {
        return Err(format!(
            "single-phase load `{}` requires two terminals and one power",
            load.name
        ));
    }
    powerio_dist::calc_element_connection(
        network,
        &load.bus,
        terminals,
        load.configuration,
        Some(powers),
    )
    .map_err(|error| format!("load `{}`: {error}", load.name))
}

#[cfg(test)]
mod bounded_tests {
    use super::*;
    #[test]
    fn bounded_constant_power_preserves_its_limit_and_low_voltage_continuation() {
        let load = BranchLoad {
            name: "bounded".into(),
            bus: "bus".into(),
            incidence: vec![vec![(0, Complex64::new(1.0, 0.0))]],
            power: vec![Complex64::new(2.0, 1.0)],
            nominal_admittance: vec![Complex64::new(2.0, -1.0)],
            reference_admittance: vec![Complex64::new(2.0, -1.0)],
            nominal_voltage: vec![1.0],
            model: BranchVoltageModel::Bounded(DistLoadVoltageModel::BoundedConstantPower {
                v_nom: vec![1.0],
                v_min_pu: 0.8,
                v_max_pu: 1.1,
                current_limit_pu: Some(0.5),
            }),
        };
        for envelope in [false, true] {
            let options = McPfOptions {
                voltage_envelope: envelope,
                ..Default::default()
            };
            for (voltage, expected) in [
                (0.0_f64, 0.0_f64),
                (0.1, 0.1 / 0.64),
                (1.0, 0.5),
                (2.0, 0.5),
            ] {
                let current = load
                    .branch_current(0, &[Complex64::new(voltage, 0.0)], &options)
                    .unwrap();
                let scale = expected.max(1e-300);
                assert!((current.re - 2.0 * expected).abs() / scale < 1e-12);
                assert!((current.im + expected).abs() / scale < 1e-12);
            }
        }
    }
}
