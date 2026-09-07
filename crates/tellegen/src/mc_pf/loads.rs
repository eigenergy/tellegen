//! Constant-power branch laws and nominal admittance compensation.

use num_complex::Complex64;
use powerio_dist::{Configuration, DistLoad, DistLoadVoltageModel};

use super::network::{NetworkIndex, StampedMatrix};

#[derive(Clone, Debug)]
pub(crate) struct BranchLoad {
    pub name: String,
    /// Signed incidence rows from global nodal voltages to branch voltages.
    pub incidence: Vec<Vec<(usize, Complex64)>>,
    pub power: Vec<Complex64>,
    pub y_ref: Vec<Complex64>,
}

impl BranchLoad {
    pub(crate) fn branch_current(
        &self,
        branch: usize,
        voltage: &[Complex64],
        zero_tol: f64,
    ) -> Result<Complex64, String> {
        let s = self.power[branch];
        if s.norm() == 0.0 {
            return Ok(Complex64::default());
        }
        let u: Complex64 = self.incidence[branch]
            .iter()
            .map(|&(i, c)| c * voltage[i])
            .sum();
        if u.norm() <= zero_tol {
            return Err(format!(
                "load `{}` branch {branch} has near-zero voltage",
                self.name
            ));
        }
        Ok((s / u).conj())
    }

    pub(crate) fn add_current(
        &self,
        voltage: &[Complex64],
        zero_tol: f64,
        result: &mut [Complex64],
    ) -> Result<(), String> {
        if result.len() != voltage.len() {
            return Err("load current scratch vector has the wrong dimension".to_owned());
        }
        for (row, incidence) in self.incidence.iter().enumerate() {
            let i_branch = self.branch_current(row, voltage, zero_tol)?;
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

pub(crate) fn prepare_load(load: &DistLoad, index: &NetworkIndex) -> Result<BranchLoad, String> {
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
    if !matches!(
        load.voltage_model,
        DistLoadVoltageModel::ConstantPower { .. }
    ) {
        return Err(format!(
            "load `{}` uses {:?}; only constant-power loads are supported",
            load.name, load.voltage_model
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
    let vnom = load.voltage_model.v_nom().to_vec();
    if vnom.is_empty() {
        return Err(format!(
            "load `{}` omits explicit nominal branch voltage; cross-voltage inference is unsupported",
            load.name
        ));
    }
    if vnom.len() != load.p_nom.len() || vnom.iter().any(|v| !v.is_finite() || *v <= 0.0) {
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
        .zip(vnom)
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
    Ok(BranchLoad {
        name: load.name.clone(),
        incidence,
        power,
        y_ref,
    })
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
