//! Multiconductor constant-power AC power flow.
//!
//! The prepared snapshot contains one global complex sparse LU.  Each
//! fixed-point iteration changes only the compensated load-current RHS, so the
//! factor is retained through initialization and convergence.

mod input;
mod linear;
mod loads;
mod network;
mod transformer;

pub use input::{
    parse_bmopf_instance, solve_bmopf_json, solve_mc_module_json, validate_bmopf_json,
};
pub use transformer::{
    build_transformer_yprim, prepare_transformer, ComplexMatrix, PreparedTransformer,
    TransformerError, TransformerPrimitive, TransformerTerminal,
};

use num_complex::Complex64;
use powerio_prob::solution::{McAcPfSolution, Residuals, Termination};
use powerio_prob::McAcPfInstance;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

use network::PreparedNetwork;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default)]
pub struct McPfOptions {
    pub tolerance: f64,
    pub max_iterations: usize,
    pub damping: f64,
    pub zero_voltage_tolerance: f64,
    pub absolute_kcl_tolerance: f64,
    pub relative_kcl_tolerance: f64,
}

impl Default for McPfOptions {
    fn default() -> Self {
        Self {
            tolerance: 1e-8,
            max_iterations: 100,
            damping: 1.0,
            zero_voltage_tolerance: 1e-9,
            // Sparse LU residual floors accumulate across large feeder
            // matrices.  This is an absolute physical-current floor; the
            // relative term still scales acceptance with incident currents.
            absolute_kcl_tolerance: 1e-6,
            relative_kcl_tolerance: 1e-8,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct McComplex {
    pub re: f64,
    pub im: f64,
}

impl From<Complex64> for McComplex {
    fn from(value: Complex64) -> Self {
        Self {
            re: value.re,
            im: value.im,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct McTerminalResult {
    pub bus: String,
    pub terminal: String,
    pub voltage: McComplex,
    /// Current injected into the network at this terminal, amperes.
    pub current_into_network: McComplex,
    /// Terminal complex power using `V * conj(I_into_network)`, VA.
    pub power_into_network: McComplex,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct McSourceReaction {
    pub source: String,
    pub terminal: String,
    pub current_into_network: McComplex,
    pub power_into_network: McComplex,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct McElementPort {
    pub element: String,
    pub kind: String,
    pub branch: usize,
    pub bus: String,
    pub terminal: String,
    pub current_into_element: McComplex,
    pub power_into_element: McComplex,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct McPfResult {
    pub converged: bool,
    pub iterations: usize,
    pub factorization_count: usize,
    pub matrix_dimension: usize,
    pub matrix_nonzeros: usize,
    pub voltage_change: f64,
    pub physical_kcl_residual: f64,
    pub scaled_kcl_residual: f64,
    pub terminals: Vec<McTerminalResult>,
    pub element_ports: Vec<McElementPort>,
    pub source_reactions: Vec<McSourceReaction>,
}

impl McPfResult {
    /// Convert the terminal-aware engine result to PowerIO's portable typed
    /// solution.  Complex currents remain available on this result because
    /// the v0.11 portable solution stores current and active-power magnitudes.
    pub fn to_powerio_solution(&self, instance: &McAcPfInstance) -> Result<McAcPfSolution, String> {
        if !self.converged {
            return Err("cannot export an unconverged AC power flow as a solution".to_owned());
        }
        let mut by_terminal = BTreeMap::new();
        for terminal in &self.terminals {
            let key = (terminal.bus.as_str(), terminal.terminal.as_str());
            if by_terminal.insert(key, terminal).is_some() {
                return Err(format!("duplicate result terminal {}:{}", key.0, key.1));
            }
            for value in [
                terminal.voltage,
                terminal.current_into_network,
                terminal.power_into_network,
            ] {
                if !value.re.is_finite() || !value.im.is_finite() {
                    return Err(format!("non-finite result at terminal {}:{}", key.0, key.1));
                }
            }
        }
        let mut ordered = Vec::with_capacity(self.terminals.len());
        for bus in instance.network().buses() {
            for terminal in &bus.terminals {
                ordered.push(
                    by_terminal
                        .remove(&(bus.id.as_str(), terminal.as_str()))
                        .ok_or_else(|| format!("missing result terminal {}:{terminal}", bus.id))?,
                );
            }
        }
        if !by_terminal.is_empty() {
            return Err("result contains terminals absent from the calculation".to_owned());
        }
        let mut by_source = BTreeMap::new();
        for source in &self.source_reactions {
            let key = (source.source.as_str(), source.terminal.as_str());
            if by_source
                .insert(key, source.power_into_network.re)
                .is_some()
            {
                return Err(format!("duplicate source result {}:{}", key.0, key.1));
            }
            if !source.power_into_network.re.is_finite() {
                return Err(format!("non-finite source power {}:{}", key.0, key.1));
            }
        }
        let mut source_active = Vec::with_capacity(self.source_reactions.len());
        for source in instance.network().sources() {
            for terminal in &source.terminal_map {
                source_active.push(
                    by_source
                        .remove(&(source.name.as_str(), terminal.as_str()))
                        .ok_or_else(|| {
                            format!("missing source result {}:{terminal}", source.name)
                        })?,
                );
            }
        }
        if !by_source.is_empty() {
            return Err("result contains sources absent from the calculation".to_owned());
        }
        let magnitudes = ordered
            .iter()
            .map(|t| t.voltage.re.hypot(t.voltage.im))
            .collect();
        let angles = ordered
            .iter()
            .map(|t| t.voltage.im.atan2(t.voltage.re))
            .collect();
        let solution = McAcPfSolution::new(
            Arc::new(instance.clone()),
            Termination::Converged,
            magnitudes,
            angles,
            source_active,
        )
        .map_err(|e| e.to_string())?;
        let currents = ordered
            .iter()
            .map(|t| t.current_into_network.re.hypot(t.current_into_network.im))
            .collect();
        let powers = ordered.iter().map(|t| t.power_into_network.re).collect();
        let solution = solution
            .with_terminal_currents(currents)
            .map_err(|e| e.to_string())?
            .with_terminal_powers(powers)
            .map_err(|e| e.to_string())?
            .with_residuals(Residuals::default())
            .with_producer("tellegen.mc-pf");
        Ok(solution)
    }
}

/// Solve one typed PowerIO multiconductor AC PF instance.
pub fn solve_mc_ac_pf_instance(
    instance: &McAcPfInstance,
    options: &McPfOptions,
) -> Result<McPfResult, String> {
    validate_options(options)?;
    let prepared = PreparedNetwork::prepare(instance)?;
    let mut voltage = prepared.fixed_voltage_vector();
    let base_rhs = prepared.source_rhs();
    let mut unknown_pos = vec![usize::MAX; prepared.index.terminal_ids.len()];
    for (u, &i) in prepared.unknown.iter().enumerate() {
        unknown_pos[i] = u;
    }
    if let Some(factor) = &prepared.factor {
        let initial = factor.solve(&base_rhs)?;
        for (u, &i) in prepared.unknown.iter().enumerate() {
            voltage[i] = initial[u];
        }
    }
    let mut final_change = f64::INFINITY;
    let mut iterations = 0;
    for k in 0..options.max_iterations {
        iterations = k + 1;
        let load_current = total_load_current(&prepared, &voltage, options.zero_voltage_tolerance)?;
        let compensated = prepared
            .unknown
            .iter()
            .map(|&i| {
                let yv = prepared.yref.row_mul(i, &voltage);
                base_rhs[unknown_pos[i]] + yv - load_current[i]
            })
            .collect::<Vec<_>>();
        let solved = prepared
            .factor
            .as_ref()
            .map(|factor| factor.solve(&compensated))
            .transpose()?
            .unwrap_or_default();
        final_change = 0.0;
        for (u, &i) in prepared.unknown.iter().enumerate() {
            let next = voltage[i] + options.damping * (solved[u] - voltage[i]);
            final_change = final_change.max((next - voltage[i]).norm());
            voltage[i] = next;
        }
        if final_change <= options.tolerance {
            let candidate_load =
                total_load_current(&prepared, &voltage, options.zero_voltage_tolerance)?;
            let (_, candidate_scaled) = kcl_metrics(&prepared, &voltage, &candidate_load, options);
            if candidate_scaled <= 1.0 {
                break;
            }
        }
    }
    let load_current = total_load_current(&prepared, &voltage, options.zero_voltage_tolerance)?;
    let (residual, scaled_residual) = kcl_metrics(&prepared, &voltage, &load_current, options);
    if !final_change.is_finite() || !residual.is_finite() || !scaled_residual.is_finite() {
        return Err("fixed-point iteration produced a non-finite residual".to_owned());
    }
    if final_change > options.tolerance || scaled_residual > 1.0 {
        return Err(format!("multiconductor PF did not converge after {iterations} iterations (voltage change {final_change:.3e}, KCL residual {residual:.3e})"));
    }
    let mut terminals = Vec::with_capacity(prepared.index.terminal_ids.len());
    for (i, (bus, terminal)) in prepared.index.terminal_ids.iter().enumerate() {
        let current = passive_current(&prepared, &voltage, i);
        terminals.push(McTerminalResult {
            bus: bus.clone(),
            terminal: terminal.clone(),
            voltage: voltage[i].into(),
            power_into_network: (voltage[i] * current.conj()).into(),
            current_into_network: current.into(),
        });
    }
    let mut element_ports = Vec::new();
    for load in &prepared.loads {
        for (branch, incidence) in load.incidence.iter().enumerate() {
            let current = load.branch_current(branch, &voltage, options.zero_voltage_tolerance)?;
            for &(i, c) in incidence {
                let terminal_current = c.conj() * current;
                let (bus, terminal) = &prepared.index.terminal_ids[i];
                element_ports.push(McElementPort {
                    element: load.name.clone(),
                    kind: "load".to_owned(),
                    branch,
                    bus: bus.clone(),
                    terminal: terminal.clone(),
                    current_into_element: terminal_current.into(),
                    power_into_element: (voltage[i] * terminal_current.conj()).into(),
                });
            }
        }
    }
    for element in &prepared.elements {
        let port_voltages: Vec<_> = element.terminals.iter().map(|&i| voltage[i]).collect();
        for (row, &i) in element.terminals.iter().enumerate() {
            let current: Complex64 = element.yprim[row]
                .iter()
                .zip(&port_voltages)
                .map(|(&y, &v)| y * v)
                .sum();
            let (bus, terminal) = &prepared.index.terminal_ids[i];
            element_ports.push(McElementPort {
                element: element.name.clone(),
                kind: element.kind.clone(),
                branch: 0,
                bus: bus.clone(),
                terminal: terminal.clone(),
                current_into_element: current.into(),
                power_into_element: (voltage[i] * current.conj()).into(),
            });
        }
    }
    let mut source_reactions = Vec::new();
    for source_row in instance.network().sources() {
        let source = instance
            .sources()
            .iter()
            .find(|s| s.source == source_row.name)
            .ok_or_else(|| {
                format!(
                    "instance source `{}` is absent from its network",
                    source_row.name
                )
            })?;
        for name in &source.terminals {
            let i = prepared.index.resolve((&source_row.bus, name))?;
            let current = passive_current(&prepared, &voltage, i) + load_current[i];
            source_reactions.push(McSourceReaction {
                source: source.source.clone(),
                terminal: name.clone(),
                current_into_network: current.into(),
                power_into_network: (voltage[i] * current.conj()).into(),
            });
        }
    }
    if terminals.iter().any(|t| {
        !complex_finite(t.voltage)
            || !complex_finite(t.current_into_network)
            || !complex_finite(t.power_into_network)
    }) || element_ports
        .iter()
        .any(|p| !complex_finite(p.current_into_element) || !complex_finite(p.power_into_element))
        || source_reactions.iter().any(|s| {
            !complex_finite(s.current_into_network) || !complex_finite(s.power_into_network)
        })
    {
        return Err(
            "fixed-point PF produced a non-finite voltage, current, or power output".to_owned(),
        );
    }
    Ok(McPfResult {
        converged: true,
        iterations,
        factorization_count: prepared
            .factor
            .as_ref()
            .map_or(0, |factor| factor.factorization_count()),
        matrix_dimension: prepared.factor.as_ref().map_or(0, |factor| factor.dim()),
        matrix_nonzeros: prepared
            .factor
            .as_ref()
            .map_or(0, |factor| factor.nonzeros()),
        voltage_change: final_change,
        physical_kcl_residual: residual,
        scaled_kcl_residual: scaled_residual,
        terminals,
        element_ports,
        source_reactions,
    })
}

fn complex_finite(value: McComplex) -> bool {
    value.re.is_finite() && value.im.is_finite()
}

fn validate_options(options: &McPfOptions) -> Result<(), String> {
    if !options.tolerance.is_finite()
        || options.tolerance <= 0.0
        || options.max_iterations == 0
        || !options.damping.is_finite()
        || options.damping <= 0.0
        || options.damping > 1.0
        || !options.zero_voltage_tolerance.is_finite()
        || options.zero_voltage_tolerance <= 0.0
        || !options.absolute_kcl_tolerance.is_finite()
        || options.absolute_kcl_tolerance <= 0.0
        || !options.relative_kcl_tolerance.is_finite()
        || options.relative_kcl_tolerance < 0.0
    {
        return Err("invalid multiconductor PF options".to_owned());
    }
    Ok(())
}

fn passive_current(network: &PreparedNetwork, voltage: &[Complex64], row: usize) -> Complex64 {
    network.passive.row_mul(row, voltage)
}

fn kcl_metrics(
    network: &PreparedNetwork,
    voltage: &[Complex64],
    load: &[Complex64],
    options: &McPfOptions,
) -> (f64, f64) {
    let mut incident = vec![0.0; voltage.len()];
    for element in &network.elements {
        let port_voltages: Vec<_> = element.terminals.iter().map(|&i| voltage[i]).collect();
        for (row, &terminal) in element.terminals.iter().enumerate() {
            let current: Complex64 = element.yprim[row]
                .iter()
                .zip(&port_voltages)
                .map(|(&y, &v)| y * v)
                .sum();
            incident[terminal] += current.norm();
        }
    }
    for load in &network.loads {
        for branch in 0..load.power.len() {
            let current = match load.branch_current(branch, voltage, options.zero_voltage_tolerance)
            {
                Ok(current) => current.norm(),
                Err(_) => f64::INFINITY,
            };
            for &(terminal, _) in &load.incidence[branch] {
                incident[terminal] += current;
            }
        }
    }
    let mut raw: f64 = 0.0;
    let mut scaled: f64 = 0.0;
    for &i in &network.unknown {
        let residual = (passive_current(network, voltage, i) + load[i]).norm();
        let scale = incident[i].max(load[i].norm());
        raw = raw.max(residual);
        scaled = scaled.max(
            residual / (options.absolute_kcl_tolerance + options.relative_kcl_tolerance * scale),
        );
    }
    (raw, scaled)
}

fn total_load_current(
    network: &PreparedNetwork,
    voltage: &[Complex64],
    zero_tol: f64,
) -> Result<Vec<Complex64>, String> {
    let mut current = vec![Complex64::new(0.0, 0.0); voltage.len()];
    for load in &network.loads {
        load.add_current(voltage, zero_tol, &mut current)?;
    }
    Ok(current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use powerio_dist::{
        Configuration, DistBus, DistLine, DistLineCode, DistLoad, DistLoadVoltageModel,
        MulticonductorNetwork, VoltageSource,
    };
    use powerio_prob::McAcPfInstance;

    fn one_phase(p: f64) -> McAcPfInstance {
        let mut net = MulticonductorNetwork::new();
        net.buses_mut()
            .push(DistBus::new("source", vec!["1".into()]));
        net.buses_mut().push(DistBus::new("load", vec!["1".into()]));
        net.line_codes_mut()
            .push(DistLineCode::new("z", vec![vec![1.0]], vec![vec![0.0]]));
        net.lines_mut().push(DistLine::new(
            "l",
            "source",
            "load",
            vec!["1".into()],
            vec!["1".into()],
            "z",
            1.0,
        ));
        net.sources_mut().push(VoltageSource::new(
            "vs",
            "source",
            vec!["1".into()],
            vec![10.0],
            vec![0.0],
        ));
        let mut load = DistLoad::new(
            "pl",
            "load",
            vec!["1".into()],
            Configuration::Wye,
            vec![p],
            vec![0.0],
        );
        load.voltage_model = DistLoadVoltageModel::ConstantPower { v_nom: vec![10.0] };
        net.loads_mut().push(load);
        McAcPfInstance::from_network(net).expect("instance")
    }

    #[test]
    fn resistive_feeder_selects_high_voltage_root_and_reuses_one_factor() {
        let result = solve_mc_ac_pf_instance(
            &one_phase(1.0),
            &McPfOptions {
                tolerance: 1e-9,
                ..Default::default()
            },
        )
        .expect("solve");
        assert!(result.converged);
        assert_eq!(result.factorization_count, 1);
        let v = result
            .terminals
            .iter()
            .find(|x| x.bus == "load")
            .unwrap()
            .voltage
            .re;
        let expected = (10.0_f64 + (100.0_f64 - 4.0).sqrt()) / 2.0;
        assert!((v - expected).abs() < 1e-6, "{v} vs {expected}");
    }

    #[test]
    fn portable_solution_matches_terminal_identities_after_reordering() {
        let instance = one_phase(1.0);
        let mut result = solve_mc_ac_pf_instance(&instance, &McPfOptions::default()).unwrap();
        result.terminals.reverse();
        let solution = result.to_powerio_solution(&instance).unwrap();
        assert_eq!(
            solution.terminal_voltage_magnitude("source", "1"),
            Some(10.0)
        );
        let expected = (10.0_f64 + 96.0_f64.sqrt()) / 2.0;
        assert!(
            (solution.terminal_voltage_magnitude("load", "1").unwrap() - expected).abs() < 1e-6
        );
        result.terminals[0].bus = "absent".to_owned();
        assert!(result
            .to_powerio_solution(&instance)
            .unwrap_err()
            .contains("missing result terminal"));
    }

    #[test]
    fn portable_solution_rejects_duplicate_nonfinite_and_unconverged_results() {
        let instance = one_phase(1.0);
        let result = solve_mc_ac_pf_instance(&instance, &McPfOptions::default()).unwrap();
        let mut duplicate = result.clone();
        duplicate.terminals.push(result.terminals[0].clone());
        assert!(duplicate
            .to_powerio_solution(&instance)
            .unwrap_err()
            .contains("duplicate result terminal"));
        let mut nonfinite = result.clone();
        nonfinite.terminals[0].voltage.re = f64::NAN;
        assert!(nonfinite
            .to_powerio_solution(&instance)
            .unwrap_err()
            .contains("non-finite"));
        let mut unconverged = result;
        unconverged.converged = false;
        assert!(unconverged
            .to_powerio_solution(&instance)
            .unwrap_err()
            .contains("unconverged"));
    }

    #[test]
    fn zero_voltage_branch_is_a_controlled_error() {
        let mut instance = one_phase(1.0);
        // The source is the only fixed terminal; force a zero source voltage.
        let mut net = instance.network().clone();
        net.sources_mut()[0].v_magnitude[0] = 0.0;
        instance = McAcPfInstance::from_network(net).expect("instance");
        let error = solve_mc_ac_pf_instance(&instance, &McPfOptions::default()).unwrap_err();
        assert!(!error.is_empty());
    }

    #[test]
    fn constant_power_current_conjugates_the_voltage_quotient() {
        let angle = std::f64::consts::FRAC_PI_4;
        let voltage = Complex64::from_polar(2.0, angle);
        let load = crate::mc_pf::loads::BranchLoad {
            name: "rotated".into(),
            incidence: vec![vec![(0, Complex64::new(1.0, 0.0))]],
            power: vec![Complex64::new(3.0, 2.0)],
            y_ref: vec![Complex64::new(0.0, 0.0)],
        };
        let mut actual_values = vec![Complex64::default()];
        load.add_current(&[voltage], 1e-12, &mut actual_values)
            .unwrap();
        let actual = actual_values[0];
        let expected = (Complex64::new(3.0, 2.0) / voltage).conj();
        assert!((actual - expected).norm() < 1e-12);
    }

    #[test]
    fn unbalanced_four_wire_wye_retains_neutral_terminal() {
        let mut net = MulticonductorNetwork::new();
        net.buses_mut().push(DistBus::new(
            "source",
            (1..=4).map(|x| x.to_string()).collect(),
        ));
        net.buses_mut().push(DistBus::new(
            "load",
            (1..=4).map(|x| x.to_string()).collect(),
        ));
        net.buses_mut()[0].grounded.push("4".into());
        let mut r = vec![vec![0.0; 4]; 4];
        let mut x = r.clone();
        for i in 0..4 {
            r[i][i] = 0.2;
            x[i][i] = 0.05;
        }
        net.line_codes_mut().push(DistLineCode::new("z4", r, x));
        net.lines_mut().push(DistLine::new(
            "l",
            "source",
            "load",
            (1..=4).map(|x| x.to_string()).collect(),
            (1..=4).map(|x| x.to_string()).collect(),
            "z4",
            1.0,
        ));
        net.sources_mut().push(VoltageSource::new(
            "vs",
            "source",
            (1..=4).map(|x| x.to_string()).collect(),
            vec![10.0, 10.0, 10.0, 0.0],
            vec![0.0, -2.0, 2.0, 0.0],
        ));
        let mut load = DistLoad::new(
            "pl",
            "load",
            (1..=4).map(|x| x.to_string()).collect(),
            Configuration::Wye,
            vec![0.7, 1.1, 1.4],
            vec![0.2, 0.1, 0.3],
        );
        load.voltage_model = DistLoadVoltageModel::ConstantPower {
            v_nom: vec![10.0; 3],
        };
        net.loads_mut().push(load);
        let result = solve_mc_ac_pf_instance(
            &McAcPfInstance::from_network(net).unwrap(),
            &McPfOptions::default(),
        )
        .unwrap();
        assert_eq!(
            result.terminals.iter().filter(|t| t.bus == "load").count(),
            4
        );
        let neutral = result
            .terminals
            .iter()
            .find(|t| t.bus == "load" && t.terminal == "4")
            .unwrap();
        assert!(neutral.voltage.re.abs() > 1e-8 || neutral.voltage.im.abs() > 1e-8);
    }

    #[test]
    fn three_wire_delta_uses_cyclic_branch_incidence() {
        let mut net = MulticonductorNetwork::new();
        net.buses_mut().push(DistBus::new(
            "source",
            vec!["1".into(), "2".into(), "3".into()],
        ));
        net.buses_mut().push(DistBus::new(
            "load",
            vec!["1".into(), "2".into(), "3".into()],
        ));
        let mut r = vec![vec![0.0; 3]; 3];
        let mut x = r.clone();
        for i in 0..3 {
            r[i][i] = 0.15;
            x[i][i] = 0.04;
        }
        net.line_codes_mut().push(DistLineCode::new("z3", r, x));
        net.lines_mut().push(DistLine::new(
            "l",
            "source",
            "load",
            vec!["1".into(), "2".into(), "3".into()],
            vec!["1".into(), "2".into(), "3".into()],
            "z3",
            1.0,
        ));
        net.sources_mut().push(VoltageSource::new(
            "vs",
            "source",
            vec!["1".into(), "2".into(), "3".into()],
            vec![10.0; 3],
            vec![0.0, -2.0943951023931953, 2.0943951023931953],
        ));
        let mut load = DistLoad::new(
            "dl",
            "load",
            vec!["1".into(), "2".into(), "3".into()],
            Configuration::Delta,
            vec![0.4, 0.5, 0.3],
            vec![0.1, 0.2, 0.15],
        );
        load.voltage_model = DistLoadVoltageModel::ConstantPower {
            v_nom: vec![10.0; 3],
        };
        net.loads_mut().push(load);
        let result = solve_mc_ac_pf_instance(
            &McAcPfInstance::from_network(net).unwrap(),
            &McPfOptions::default(),
        )
        .unwrap();
        assert!(result.converged);
        assert_eq!(
            result
                .element_ports
                .iter()
                .filter(|p| p.element == "dl")
                .count(),
            6
        );
    }

    #[test]
    fn all_fixed_zero_unknown_snapshot_reports_no_factorization() {
        let mut net = MulticonductorNetwork::new();
        net.buses_mut()
            .push(DistBus::new("source", vec!["1".into()]));
        net.sources_mut().push(VoltageSource::new(
            "vs",
            "source",
            vec!["1".into()],
            vec![10.0],
            vec![0.0],
        ));
        let instance = McAcPfInstance::from_network(net).unwrap();
        let result = solve_mc_ac_pf_instance(&instance, &McPfOptions::default()).unwrap();
        assert!(result.converged);
        assert_eq!(result.factorization_count, 0);
        assert_eq!(result.matrix_dimension, 0);
        assert_eq!(result.iterations, 1);
    }

    #[test]
    fn source_reaction_includes_a_load_on_the_fixed_source_bus() {
        let mut net = MulticonductorNetwork::new();
        net.buses_mut()
            .push(DistBus::new("source", vec!["1".into()]));
        net.sources_mut().push(VoltageSource::new(
            "vs",
            "source",
            vec!["1".into()],
            vec![10.0],
            vec![0.0],
        ));
        let mut load = DistLoad::new(
            "local",
            "source",
            vec!["1".into()],
            Configuration::Wye,
            vec![1.0],
            vec![0.0],
        );
        load.voltage_model = DistLoadVoltageModel::ConstantPower { v_nom: vec![10.0] };
        net.loads_mut().push(load);
        let instance = McAcPfInstance::from_network(net).unwrap();
        let result = solve_mc_ac_pf_instance(&instance, &McPfOptions::default()).unwrap();
        assert!((result.source_reactions[0].power_into_network.re - 1.0).abs() < 1e-10);
    }

    #[test]
    fn zero_power_at_zero_voltage_is_finite() {
        let mut net = MulticonductorNetwork::new();
        net.buses_mut()
            .push(DistBus::new("source", vec!["1".into()]));
        net.sources_mut().push(VoltageSource::new(
            "vs",
            "source",
            vec!["1".into()],
            vec![0.0],
            vec![0.0],
        ));
        let mut idle = DistLoad::new(
            "idle",
            "source",
            vec!["1".into()],
            Configuration::Wye,
            vec![0.0],
            vec![0.0],
        );
        idle.voltage_model = DistLoadVoltageModel::ConstantPower { v_nom: vec![1.0] };
        net.loads_mut().push(idle);
        let result = solve_mc_ac_pf_instance(
            &McAcPfInstance::from_network(net).unwrap(),
            &McPfOptions::default(),
        )
        .unwrap();
        assert!(result.terminals[0].voltage.re.is_finite());
        assert_eq!(result.source_reactions[0].power_into_network.re, 0.0);
    }

    #[test]
    fn duplicate_ideal_source_constraints_are_rejected() {
        let mut net = MulticonductorNetwork::new();
        net.buses_mut()
            .push(DistBus::new("source", vec!["1".into()]));
        net.sources_mut().push(VoltageSource::new(
            "vs1",
            "source",
            vec!["1".into()],
            vec![10.0],
            vec![0.0],
        ));
        net.sources_mut().push(VoltageSource::new(
            "vs2",
            "source",
            vec!["1".into()],
            vec![10.0],
            vec![0.0],
        ));
        let instance = McAcPfInstance::from_network(net).unwrap();
        let error = solve_mc_ac_pf_instance(&instance, &McPfOptions::default()).unwrap_err();
        assert!(error.contains("multiple ideal sources"), "{error}");
    }

    #[test]
    fn omitted_load_nominal_voltage_is_rejected_explicitly() {
        let mut network = one_phase(1.0).network().clone();
        network.loads_mut()[0].voltage_model =
            DistLoadVoltageModel::ConstantPower { v_nom: Vec::new() };
        let instance = McAcPfInstance::from_network(network).unwrap();
        let error = solve_mc_ac_pf_instance(&instance, &McPfOptions::default()).unwrap_err();
        assert!(
            error.contains("omits explicit nominal branch voltage"),
            "{error}"
        );
    }
}
