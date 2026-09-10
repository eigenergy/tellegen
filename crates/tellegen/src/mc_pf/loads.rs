//! Voltage-dependent branch laws and nominal admittance compensation.

use num_complex::Complex64;
use powerio_dist::{Configuration, DistLoad, DistLoadVoltageModel};

#[derive(Clone, Debug)]
pub(crate) enum BranchVoltageModel {
    ConstantPower,
    ConstantCurrent,
    ConstantImpedance,
    Zip {
        alpha_z: Vec<f64>,
        alpha_i: Vec<f64>,
        alpha_p: Vec<f64>,
        beta_z: Vec<f64>,
        beta_i: Vec<f64>,
        beta_p: Vec<f64>,
    },
    Exponential {
        gamma_p: Vec<f64>,
        gamma_q: Vec<f64>,
    },
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
    pub y_ref: Vec<Complex64>,
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
        // this before the voltage guard so an unloaded branch is well-defined
        // even when it is disconnected from a prescribed source.
        if self.power[branch].norm() == 0.0 {
            return Ok(Complex64::default());
        }
        // Keep the magnitude calculation explicit at this numerical boundary;
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
        // Evaluate the radial laws in current form.  Dividing by |U| and
        // multiplying by the unit phasor avoids forming U*conjugate(U), which
        // underflows for a perfectly valid tiny nonzero voltage.
        let current = if options.voltage_envelope {
            self.enveloped_current(branch, u, magnitude, options)
        } else {
            self.current_at_voltage(branch, u, magnitude)
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

    fn enveloped_current(
        &self,
        branch: usize,
        u: Complex64,
        magnitude: f64,
        options: &McPfOptions,
    ) -> Complex64 {
        let v_nom = self.nominal_voltage(branch);
        let ratio = magnitude / v_nom;
        if matches!(&self.model, BranchVoltageModel::ConstantImpedance) {
            return self.y_ref[branch] * u;
        }
        if ratio <= options.v_low_pu {
            return self.y_ref[branch] * u;
        }
        if ratio <= options.v_min_pu {
            // OpenDSS transitions linearly in complex current from
            // nominal impedance at Vlow to the constant-power current at
            // Vmin. Multiplication by the voltage unit phasor retains the
            // operating branch angle.
            let low = self.y_ref[branch] * (v_nom * options.v_low_pu);
            let at_min = self.y_ref[branch] * (v_nom / options.v_min_pu);
            let fraction = (ratio - options.v_low_pu) / (options.v_min_pu - options.v_low_pu);
            return (u / magnitude) * (low + fraction * (at_min - low));
        }
        if ratio > options.v_max_pu {
            return self.y_ref[branch] / options.v_max_pu.powi(2) * u;
        }
        self.current_at_voltage(branch, u, magnitude)
    }

    fn current_at_voltage(&self, branch: usize, u: Complex64, magnitude: f64) -> Complex64 {
        let phase = u / magnitude;
        let s = self.power[branch];
        let ratio = magnitude / self.nominal_voltage(branch);
        match &self.model {
            BranchVoltageModel::ConstantPower => s.conj() * (phase / magnitude),
            BranchVoltageModel::ConstantCurrent => {
                s.conj() * (phase / self.nominal_voltage(branch))
            }
            BranchVoltageModel::ConstantImpedance => self.y_ref[branch] * u,
            BranchVoltageModel::Zip {
                alpha_z,
                alpha_i,
                alpha_p,
                beta_z,
                beta_i,
                beta_p,
            } => {
                let vnom = self.nominal_voltage(branch);
                let p = if s.re == 0.0 {
                    0.0
                } else {
                    s.re / vnom
                        * (alpha_z[branch] * ratio
                            + alpha_i[branch]
                            + if alpha_p[branch] == 0.0 {
                                0.0
                            } else {
                                alpha_p[branch] / ratio
                            })
                };
                let q = if s.im == 0.0 {
                    0.0
                } else {
                    s.im / vnom
                        * (beta_z[branch] * ratio
                            + beta_i[branch]
                            + if beta_p[branch] == 0.0 {
                                0.0
                            } else {
                                beta_p[branch] / ratio
                            })
                };
                phase * Complex64::new(p, -q)
            }
            BranchVoltageModel::Exponential { gamma_p, gamma_q } => {
                Complex64::new(
                    if s.re == 0.0 {
                        0.0
                    } else {
                        s.re / self.nominal_voltage(branch) * ratio.powf(gamma_p[branch] - 1.0)
                    },
                    if s.im == 0.0 {
                        0.0
                    } else {
                        -s.im / self.nominal_voltage(branch) * ratio.powf(gamma_q[branch] - 1.0)
                    },
                ) * phase
            }
        }
    }

    fn nominal_voltage(&self, branch: usize) -> f64 {
        self.nominal_voltage[branch]
    }

    fn zero_current_at_zero_voltage(&self, branch: usize, magnitude: f64, zero_tol: f64) -> bool {
        if magnitude > zero_tol {
            return false;
        }
        let s = self.power[branch];
        match &self.model {
            BranchVoltageModel::ConstantImpedance => true,
            BranchVoltageModel::ConstantPower | BranchVoltageModel::ConstantCurrent => {
                s.norm() == 0.0
            }
            BranchVoltageModel::Zip {
                alpha_i,
                alpha_p,
                beta_i,
                beta_p,
                ..
            } => {
                // A constant-power or linear-current contribution has no
                // unique phasor at exactly zero branch voltage. Pure Z terms
                // have a well-defined zero current.
                s.re * alpha_p[branch] == 0.0
                    && s.im * beta_p[branch] == 0.0
                    && s.re * alpha_i[branch] == 0.0
                    && s.im * beta_i[branch] == 0.0
            }
            BranchVoltageModel::Exponential { gamma_p, gamma_q } => {
                (s.re == 0.0 || gamma_p[branch] > 1.0) && (s.im == 0.0 || gamma_q[branch] > 1.0)
            }
        }
    }

    fn near_zero_law_is_safe(&self, branch: usize) -> bool {
        let s = self.power[branch];
        match &self.model {
            BranchVoltageModel::ConstantImpedance => true,
            BranchVoltageModel::ConstantCurrent => true,
            BranchVoltageModel::Zip {
                alpha_p, beta_p, ..
            } => s.re * alpha_p[branch] == 0.0 && s.im * beta_p[branch] == 0.0,
            BranchVoltageModel::Exponential { gamma_p, gamma_q } => {
                (s.re == 0.0 || gamma_p[branch] >= 1.0) && (s.im == 0.0 || gamma_q[branch] >= 1.0)
            }
            BranchVoltageModel::ConstantPower => false,
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
                    y.add(i, j, ci.conj() * self.y_ref[branch] * cj);
                }
            }
        }
    }
}

pub(crate) fn prepare_load(
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
    let incidence = connection_incidence(load, index)?;
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
    let y_ref: Vec<Complex64> = power
        .iter()
        .zip(vnom.iter().copied())
        .map(|(&s, v)| s.conj() / (v * v))
        .collect();
    if y_ref
        .iter()
        .any(|value| !value.re.is_finite() || !value.im.is_finite())
    {
        return Err(format!(
            "load `{}` produced a non-finite nominal admittance",
            load.name
        ));
    }
    let model = match &load.voltage_model {
        DistLoadVoltageModel::ConstantPower { .. } => BranchVoltageModel::ConstantPower,
        DistLoadVoltageModel::ConstantCurrent { .. } => BranchVoltageModel::ConstantCurrent,
        DistLoadVoltageModel::ConstantImpedance { .. } => BranchVoltageModel::ConstantImpedance,
        DistLoadVoltageModel::Zip {
            alpha_z,
            alpha_i,
            alpha_p,
            beta_z,
            beta_i,
            beta_p,
            ..
        } => BranchVoltageModel::Zip {
            alpha_z: coefficients(&load.name, "alpha_z", alpha_z, load.p_nom.len())?,
            alpha_i: coefficients(&load.name, "alpha_i", alpha_i, load.p_nom.len())?,
            alpha_p: coefficients(&load.name, "alpha_p", alpha_p, load.p_nom.len())?,
            beta_z: coefficients(&load.name, "beta_z", beta_z, load.p_nom.len())?,
            beta_i: coefficients(&load.name, "beta_i", beta_i, load.p_nom.len())?,
            beta_p: coefficients(&load.name, "beta_p", beta_p, load.p_nom.len())?,
        },
        DistLoadVoltageModel::Exponential {
            gamma_p, gamma_q, ..
        } => BranchVoltageModel::Exponential {
            gamma_p: coefficients(&load.name, "gamma_p", gamma_p, load.p_nom.len())?,
            gamma_q: coefficients(&load.name, "gamma_q", gamma_q, load.p_nom.len())?,
        },
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
        y_ref,
        nominal_voltage: vnom,
        model,
    })
}

fn coefficients(
    name: &str,
    field: &str,
    values: &[f64],
    branches: usize,
) -> Result<Vec<f64>, String> {
    if values.len() != 1 && values.len() != branches {
        return Err(format!(
            "load `{name}` {field} has length {}; expected 1 or {branches}",
            values.len()
        ));
    }
    if values.iter().any(|value| !value.is_finite()) {
        return Err(format!("load `{name}` {field} contains a non-finite value"));
    }
    if values.len() == 1 {
        Ok(vec![values[0]; branches])
    } else {
        Ok(values.to_vec())
    }
}

fn connection_incidence(
    load: &DistLoad,
    index: &NetworkIndex,
) -> Result<Vec<Vec<(usize, Complex64)>>, String> {
    let terminal = |name: &str| index.resolve((&load.bus, name));
    match load.configuration {
        Configuration::Wye => {
            let neutral = if load.terminal_map.len() == load.p_nom.len() + 1 {
                let explicit = load.extras.get("neutral_terminal").and_then(|v| v.as_str());
                Some(explicit.unwrap_or_else(|| load.terminal_map.last().unwrap().as_str()))
            } else if load.terminal_map.len() == load.p_nom.len() {
                None
            } else {
                return Err(format!(
                    "wye load `{}` terminal/power dimensions are inconsistent",
                    load.name
                ));
            };
            let phases: Vec<_> = load
                .terminal_map
                .iter()
                .filter(|phase| Some(phase.as_str()) != neutral)
                .collect();
            if phases.len() != load.p_nom.len() {
                return Err(format!(
                    "wye load `{}` has an explicit neutral that does not leave one branch per power",
                    load.name
                ));
            }
            phases
                .into_iter()
                .map(|phase| {
                    let i = terminal(phase)?;
                    let mut row = vec![(i, Complex64::new(1.0, 0.0))];
                    if let Some(n) = neutral {
                        let j = terminal(n)?;
                        row.push((j, Complex64::new(-1.0, 0.0)));
                    }
                    Ok(row)
                })
                .collect()
        }
        Configuration::Delta => {
            if load.terminal_map.len() == 2 * load.p_nom.len() {
                load.terminal_map
                    .chunks_exact(2)
                    .map(|pair| {
                        Ok(vec![
                            (terminal(&pair[0])?, Complex64::new(1.0, 0.0)),
                            (terminal(&pair[1])?, Complex64::new(-1.0, 0.0)),
                        ])
                    })
                    .collect()
            } else if load.terminal_map.len() == load.p_nom.len() {
                let nodes: Vec<_> = load
                    .terminal_map
                    .iter()
                    .map(|x| terminal(x))
                    .collect::<Result<_, _>>()?;
                Ok((0..load.p_nom.len())
                    .map(|k| {
                        vec![
                            (nodes[k], Complex64::new(1.0, 0.0)),
                            (nodes[(k + 1) % nodes.len()], Complex64::new(-1.0, 0.0)),
                        ]
                    })
                    .collect())
            } else {
                Err(format!(
                    "delta load `{}` terminal/power dimensions are inconsistent",
                    load.name
                ))
            }
        }
        Configuration::SinglePhase => {
            if load.terminal_map.len() != 2 || load.p_nom.len() != 1 {
                return Err(format!(
                    "single-phase load `{}` requires two terminals and one power",
                    load.name
                ));
            }
            Ok(vec![vec![
                (terminal(&load.terminal_map[0])?, Complex64::new(1.0, 0.0)),
                (terminal(&load.terminal_map[1])?, Complex64::new(-1.0, 0.0)),
            ]])
        }
        _ => Err(format!(
            "load `{}` uses an unsupported connection configuration",
            load.name
        )),
    }
}
