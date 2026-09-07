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
    validate_mc_module_json,
};
pub use transformer::{
    build_transformer_yprim, prepare_transformer, ComplexMatrix, PreparedTransformer,
    TransformerError, TransformerPrimitive, TransformerTerminal,
};

use num_complex::Complex64;
use powerio_prob::solution::{McAcPfSolution, Residuals, Termination};
use powerio_prob::McAcPfInstance;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use network::PreparedNetwork;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
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
pub struct McSourceReaction {
    pub source: String,
    pub terminal: String,
    pub current_into_network: McComplex,
    pub power_into_network: McComplex,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
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

/// A portable, replayable distribution Study snapshot.
///
/// This deliberately sits beside the balanced KKT Study rather than pretending
/// that a multiconductor fixed-point result has balanced buses, objectives, or
/// sensitivity columns. The input and solution modules make the snapshot
/// self-contained; the rich result is the UI-facing terminal/element view.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct McStudySnapshot {
    pub schema: String,
    pub version: u32,
    pub id: String,
    pub title: String,
    pub formulation: String,
    pub input_module: String,
    pub solution_module: String,
    pub options: McPfOptions,
    pub result: McPfResult,
}

impl McStudySnapshot {
    pub const SCHEMA: &'static str = "tellegen-mc-pf-study";
    pub const VERSION: u32 = 1;

    pub fn solve(
        id: impl Into<String>,
        title: impl Into<String>,
        input_module: &str,
        options: McPfOptions,
    ) -> Result<Self, String> {
        let module = crate::ir::deserialize_module(input_module)?;
        input::validate_mc_module(&module)?;
        let instance = input::instance_from_value(module.value())?;
        let result = solve_mc_ac_pf_instance(&instance, &options)?;
        let solution = result.to_powerio_solution(&instance)?;
        let solution_module = crate::ir::serialize_module(&powerio::PioModule::new(
            powerio::PioValue::McAcPfSolution(solution),
        ))
        .map_err(|e| e.to_string())?;
        let snapshot = Self {
            schema: Self::SCHEMA.to_owned(),
            version: Self::VERSION,
            id: id.into(),
            title: title.into(),
            formulation: "mc_ac_pf".to_owned(),
            input_module: input_module.to_owned(),
            solution_module,
            options,
            result,
        };
        snapshot.validate()?;
        Ok(snapshot)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema != Self::SCHEMA || self.version != Self::VERSION {
            return Err("unsupported multiconductor Study snapshot schema".to_owned());
        }
        if self.id.trim().is_empty() || self.title.trim().is_empty() {
            return Err("multiconductor Study snapshot requires an id and title".to_owned());
        }
        if self.formulation != "mc_ac_pf" {
            return Err("multiconductor Study snapshot formulation must be mc_ac_pf".to_owned());
        }
        validate_options(&self.options)?;
        let input = crate::ir::deserialize_module(&self.input_module)?;
        input::validate_mc_module(&input)?;
        let input_instance = input::instance_from_value(input.value())?;
        let solution = crate::ir::deserialize_module(&self.solution_module)?;
        let powerio::PioValue::McAcPfSolution(solution) = solution.into_value() else {
            return Err("multiconductor Study snapshot solution must be McAcPfSolution".to_owned());
        };
        let input_instance_json = instance_module_value(&input_instance)?;
        let solution_instance_json = instance_module_value(solution.instance())?;
        if input_instance_json != solution_instance_json {
            return Err(
                "multiconductor Study snapshot input and solution instances differ".to_owned(),
            );
        }
        validate_result_against_solution(&self.result, &solution, &self.options)?;
        Ok(())
    }

    pub fn to_json(&self) -> Result<String, String> {
        self.validate()?;
        serde_json::to_string(self).map_err(|e| e.to_string())
    }

    pub fn from_json(text: &str) -> Result<Self, String> {
        let snapshot: Self = serde_json::from_str(text)
            .map_err(|e| format!("invalid multiconductor Study snapshot: {e}"))?;
        snapshot.validate()?;
        Ok(snapshot)
    }
}

fn instance_module_value(instance: &McAcPfInstance) -> Result<serde_json::Value, String> {
    let module = crate::ir::serialize_module(&powerio::PioModule::new(
        powerio::PioValue::McAcPfInstance(instance.clone()),
    ))
    .map_err(|e| e.to_string())?;
    serde_json::from_str(&module).map_err(|e| e.to_string())
}

fn validate_result_against_solution(
    result: &McPfResult,
    solution: &McAcPfSolution,
    options: &McPfOptions,
) -> Result<(), String> {
    if solution.termination() != &Termination::Converged {
        return Err("multiconductor Study solution is not marked converged".to_owned());
    }
    if !result.converged
        || result.iterations == 0
        || !result.voltage_change.is_finite()
        || !result.physical_kcl_residual.is_finite()
        || !result.scaled_kcl_residual.is_finite()
        || result.voltage_change < 0.0
        || result.physical_kcl_residual < 0.0
        || result.scaled_kcl_residual < 0.0
        || result.scaled_kcl_residual > 1.0 + 1e-10
        || result.voltage_change > options.tolerance * (1.0 + 1e-10)
        || result.iterations > options.max_iterations
    {
        return Err(
            "multiconductor Study snapshot does not contain a finite converged result".to_owned(),
        );
    }
    let network = solution.network();
    let expected_count: usize = network.buses().iter().map(|bus| bus.terminals.len()).sum();
    if result.terminals.len() != expected_count {
        return Err(
            "multiconductor Study result terminal count differs from its solution".to_owned(),
        );
    }
    let terminal_voltages: std::collections::BTreeMap<_, _> = result
        .terminals
        .iter()
        .map(|terminal| {
            (
                (terminal.bus.clone(), terminal.terminal.clone()),
                terminal.voltage,
            )
        })
        .collect();
    if terminal_voltages.len() != result.terminals.len() {
        return Err(
            "multiconductor Study result contains duplicate terminal identities".to_owned(),
        );
    }
    let mut index = 0usize;
    for bus in network.buses() {
        for (column, terminal) in bus.terminals.iter().enumerate() {
            let index = index + column;
            let actual = &result.terminals[index];
            if actual.bus != bus.id || actual.terminal != *terminal {
                return Err(format!(
                    "multiconductor Study result terminal identity/order differs at {}/{}",
                    bus.id, terminal
                ));
            }
            let magnitude = actual.voltage.re.hypot(actual.voltage.im);
            let angle = actual.voltage.im.atan2(actual.voltage.re);
            let expected_magnitude = solution
                .terminal_voltage_magnitude(&bus.id, terminal)
                .ok_or_else(|| format!("solution omits terminal {}/{}", bus.id, terminal))?;
            let expected_angle = solution
                .terminal_voltage_angle(&bus.id, terminal)
                .ok_or_else(|| format!("solution omits terminal {}/{}", bus.id, terminal))?;
            if !complex_finite(actual.voltage)
                || !complex_finite(actual.current_into_network)
                || !complex_finite(actual.power_into_network)
                || !close_complex(
                    actual.power_into_network,
                    actual.voltage.into_complex()
                        * actual.current_into_network.into_complex().conj(),
                )
                || !close(magnitude, expected_magnitude)
                || !close(angle, expected_angle)
            {
                return Err(format!(
                    "multiconductor Study result voltage is not linked to solution at {}/{}",
                    bus.id, terminal
                ));
            }
            let expected_current = solution.terminal_current_magnitude(&bus.id, terminal);
            let Some(expected_current) = expected_current else {
                return Err(format!(
                    "solution omits terminal current at {}/{}",
                    bus.id, terminal
                ));
            };
            if !close(
                actual
                    .current_into_network
                    .re
                    .hypot(actual.current_into_network.im),
                expected_current,
            ) {
                return Err(format!(
                    "multiconductor Study result current is not linked to solution at {}/{}",
                    bus.id, terminal
                ));
            }
            let expected_power = solution.terminal_active_power(&bus.id, terminal);
            let Some(expected_power) = expected_power else {
                return Err(format!(
                    "solution omits terminal power at {}/{}",
                    bus.id, terminal
                ));
            };
            if !close(actual.power_into_network.re, expected_power) {
                return Err(format!(
                    "multiconductor Study result power is not linked to solution at {}/{}",
                    bus.id, terminal
                ));
            }
        }
        index += bus.terminals.len();
    }
    let mut source_index = 0usize;
    for source in network.sources() {
        for terminal in &source.terminal_map {
            let reaction = result
                .source_reactions
                .get(source_index)
                .ok_or_else(|| "multiconductor Study result omits a source reaction".to_owned())?;
            if reaction.source != source.name || reaction.terminal != *terminal {
                return Err(
                    "multiconductor Study source reaction identity/order differs from solution"
                        .to_owned(),
                );
            }
            let expected_power = solution
                .source_active_injections()
                .get(source_index)
                .ok_or_else(|| "solution omits a source reaction".to_owned())?;
            let voltage = terminal_voltages
                .get(&(source.bus.clone(), terminal.clone()))
                .copied()
                .ok_or_else(|| "source reaction has no matching terminal voltage".to_owned())?;
            if !complex_finite(reaction.current_into_network)
                || !complex_finite(reaction.power_into_network)
                || !close_complex(
                    reaction.power_into_network,
                    voltage.into_complex() * reaction.current_into_network.into_complex().conj(),
                )
                || !close(reaction.power_into_network.re, *expected_power)
            {
                return Err(
                    "multiconductor Study source reaction is not linked to solution".to_owned(),
                );
            }
            source_index += 1;
        }
    }
    if result.source_reactions.len() != source_index {
        return Err("multiconductor Study result has extra source reactions".to_owned());
    }
    let expected_ports = expected_element_port_keys(solution.instance())?;
    let mut actual_ports = std::collections::BTreeSet::new();
    for port in &result.element_ports {
        let key = (
            port.kind.clone(),
            port.element.clone(),
            port.branch,
            port.bus.clone(),
            port.terminal.clone(),
        );
        if !actual_ports.insert(key) {
            return Err("multiconductor Study result contains duplicate element ports".to_owned());
        }
        let voltage = terminal_voltages
            .get(&(port.bus.clone(), port.terminal.clone()))
            .copied()
            .ok_or_else(|| "element port has no matching terminal voltage".to_owned())?;
        if !complex_finite(port.current_into_element)
            || !complex_finite(port.power_into_element)
            || !close_complex(
                port.power_into_element,
                voltage.into_complex() * port.current_into_element.into_complex().conj(),
            )
        {
            return Err(
                "multiconductor Study result contains a non-finite element port".to_owned(),
            );
        }
    }
    if actual_ports != expected_ports {
        return Err(
            "multiconductor Study result element port identities do not match its input".to_owned(),
        );
    }
    Ok(())
}

type ElementPortKey = (String, String, usize, String, String);

fn expected_element_port_keys(
    instance: &McAcPfInstance,
) -> Result<std::collections::BTreeSet<ElementPortKey>, String> {
    use powerio_dist::Configuration;
    let network = instance.network();
    let mut expected = std::collections::BTreeSet::new();
    let mut push = |kind: &str, element: &str, branch: usize, bus: &str, terminal: &str| {
        expected.insert((
            kind.to_owned(),
            element.to_owned(),
            branch,
            bus.to_owned(),
            terminal.to_owned(),
        ));
    };
    for load_instance in instance.loads() {
        let load = network
            .loads()
            .iter()
            .find(|load| load.name == load_instance.load)
            .ok_or_else(|| {
                format!(
                    "instance load `{}` is absent from its network",
                    load_instance.load
                )
            })?;
        let terminals = &load_instance.terminals;
        let neutral = if load.configuration == Configuration::Wye
            && terminals.len() == load_instance.p_w.len() + 1
        {
            load.extras
                .get("neutral_terminal")
                .and_then(|value| value.as_str())
                .or_else(|| terminals.last().map(String::as_str))
        } else {
            None
        };
        let branches: Vec<Vec<&str>> = match load.configuration {
            Configuration::Wye => terminals
                .iter()
                .filter(|t| Some(t.as_str()) != neutral)
                .map(|phase| {
                    let mut row = vec![phase.as_str()];
                    if let Some(n) = neutral {
                        row.push(n);
                    }
                    row
                })
                .collect(),
            Configuration::Delta if terminals.len() == 2 * load_instance.p_w.len() => terminals
                .chunks_exact(2)
                .map(|pair| vec![pair[0].as_str(), pair[1].as_str()])
                .collect(),
            Configuration::Delta
                if terminals.len() == load_instance.p_w.len() && !terminals.is_empty() =>
            {
                (0..load_instance.p_w.len())
                    .map(|i| {
                        vec![
                            terminals[i].as_str(),
                            terminals[(i + 1) % terminals.len()].as_str(),
                        ]
                    })
                    .collect()
            }
            Configuration::Delta => {
                return Err(format!(
                    "load `{}` terminal/power dimensions are inconsistent",
                    load.name
                ));
            }
            Configuration::SinglePhase if terminals.len() == 2 && load_instance.p_w.len() == 1 => {
                vec![vec![terminals[0].as_str(), terminals[1].as_str()]]
            }
            Configuration::SinglePhase => {
                return Err(format!(
                    "load `{}` requires two terminals and one power",
                    load.name
                ));
            }
            _ => {
                return Err(format!(
                    "load `{}` uses unsupported connection configuration",
                    load.name
                ))
            }
        };
        if branches.len() != load_instance.p_w.len() {
            return Err(format!(
                "load `{}` branch count does not match its powers",
                load.name
            ));
        }
        for (branch, row) in branches.iter().enumerate() {
            for terminal in row {
                push("load", &load.name, branch, &load.bus, terminal);
            }
        }
    }
    for line in network.lines() {
        for terminal in &line.terminal_map_from {
            push("line", &line.name, 0, &line.bus_from, terminal);
        }
        for terminal in &line.terminal_map_to {
            push("line", &line.name, 0, &line.bus_to, terminal);
        }
    }
    for shunt in network.shunts() {
        for terminal in &shunt.terminal_map {
            push("shunt", &shunt.name, 0, &shunt.bus, terminal);
        }
    }
    for capacitor in network.capacitors() {
        for terminal in &capacitor.terminal_map {
            push("capacitor", &capacitor.name, 0, &capacitor.bus, terminal);
        }
    }
    for transformer in network.transformers() {
        let primitive = prepare_transformer(transformer).map_err(|error| error.to_string())?;
        for terminal in primitive.terminals {
            push(
                "transformer",
                &primitive.name,
                0,
                &terminal.bus,
                &terminal.terminal,
            );
        }
    }
    Ok(expected)
}

fn close(left: f64, right: f64) -> bool {
    left.is_finite()
        && right.is_finite()
        && (left - right).abs() <= 1e-8 * (1.0 + left.abs().max(right.abs()))
}

fn close_complex(left: McComplex, right: Complex64) -> bool {
    close(left.re, right.re) && close(left.im, right.im)
}

trait McComplexValue {
    fn into_complex(self) -> Complex64;
}

impl McComplexValue for McComplex {
    fn into_complex(self) -> Complex64 {
        Complex64::new(self.re, self.im)
    }
}

/// Solve a typed MC Study and return its self-contained snapshot JSON.
pub fn solve_mc_study_json(
    module_json: &str,
    id: &str,
    title: &str,
    options: &McPfOptions,
) -> Result<String, String> {
    McStudySnapshot::solve(id, title, module_json, *options)?.to_json()
}

/// Validate and canonicalize a saved MC Study snapshot without re-solving it.
pub fn replay_mc_study_json(snapshot_json: &str) -> Result<String, String> {
    McStudySnapshot::from_json(snapshot_json)?.to_json()
}

impl McPfResult {
    /// Convert the terminal-aware engine result to PowerIO's portable typed
    /// solution.  Complex currents remain available on this result because
    /// the v0.11 portable solution stores current and active-power magnitudes.
    pub fn to_powerio_solution(&self, instance: &McAcPfInstance) -> Result<McAcPfSolution, String> {
        let magnitudes = self
            .terminals
            .iter()
            .map(|t| (t.voltage.re * t.voltage.re + t.voltage.im * t.voltage.im).sqrt())
            .collect();
        let angles = self
            .terminals
            .iter()
            .map(|t| t.voltage.im.atan2(t.voltage.re))
            .collect();
        let source_active = self
            .source_reactions
            .iter()
            .map(|s| s.power_into_network.re)
            .collect();
        let solution = McAcPfSolution::new(
            Arc::new(instance.clone()),
            Termination::Converged,
            magnitudes,
            angles,
            source_active,
        )
        .map_err(|e| e.to_string())?;
        let currents = self
            .terminals
            .iter()
            .map(|t| {
                (t.current_into_network.re * t.current_into_network.re
                    + t.current_into_network.im * t.current_into_network.im)
                    .sqrt()
            })
            .collect();
        let powers = self
            .terminals
            .iter()
            .map(|t| t.power_into_network.re)
            .collect();
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
            nominal_voltage: vec![1.0],
            model: crate::mc_pf::loads::BranchVoltageModel::ConstantPower,
        };
        let mut actual_values = vec![Complex64::default()];
        load.add_current(&[voltage], 1e-12, &mut actual_values)
            .unwrap();
        let actual = actual_values[0];
        let expected = (Complex64::new(3.0, 2.0) / voltage).conj();
        assert!((actual - expected).norm() < 1e-12);
    }

    fn direct_branch_load(
        model: crate::mc_pf::loads::BranchVoltageModel,
    ) -> crate::mc_pf::loads::BranchLoad {
        crate::mc_pf::loads::BranchLoad {
            name: "law".into(),
            incidence: vec![vec![(0, Complex64::new(1.0, 0.0))]],
            power: vec![Complex64::new(10.0, 5.0)],
            y_ref: vec![Complex64::new(0.1, -0.05)],
            nominal_voltage: vec![10.0],
            model,
        }
    }

    #[test]
    fn voltage_dependent_branch_laws_follow_powerio_equations() {
        let voltage = [Complex64::new(8.0, 0.0)];
        let expected = [
            (
                crate::mc_pf::loads::BranchVoltageModel::ConstantCurrent,
                Complex64::new(1.0, -0.5),
            ),
            (
                crate::mc_pf::loads::BranchVoltageModel::ConstantImpedance,
                Complex64::new(0.8, -0.4),
            ),
            (
                crate::mc_pf::loads::BranchVoltageModel::Zip {
                    alpha_z: vec![0.2],
                    alpha_i: vec![0.3],
                    alpha_p: vec![0.5],
                    beta_z: vec![0.1],
                    beta_i: vec![0.4],
                    beta_p: vec![0.5],
                },
                Complex64::new(8.68 / 8.0, -4.42 / 8.0),
            ),
            (
                crate::mc_pf::loads::BranchVoltageModel::Exponential {
                    gamma_p: vec![0.7],
                    gamma_q: vec![2.3],
                },
                Complex64::new(
                    10.0 * 0.8_f64.powf(0.7) / 8.0,
                    -5.0 * 0.8_f64.powf(2.3) / 8.0,
                ),
            ),
        ];
        for (model, expected) in expected {
            let load = direct_branch_load(model);
            let actual = load.branch_current(0, &voltage, 1e-12).unwrap();
            assert!(
                (actual - expected).norm() < 1e-12,
                "{actual:?} vs {expected:?}"
            );
        }
    }

    #[test]
    fn voltage_dependent_laws_have_controlled_zero_voltage_limits() {
        let zero = [Complex64::default()];
        let mut load =
            direct_branch_load(crate::mc_pf::loads::BranchVoltageModel::ConstantImpedance);
        assert_eq!(
            load.branch_current(0, &zero, 1e-12).unwrap(),
            Complex64::default()
        );

        load.model = crate::mc_pf::loads::BranchVoltageModel::ConstantCurrent;
        assert!(load.branch_current(0, &zero, 1e-12).is_err());

        load.model = crate::mc_pf::loads::BranchVoltageModel::Zip {
            alpha_z: vec![0.0],
            alpha_i: vec![1.0],
            alpha_p: vec![0.0],
            beta_z: vec![0.0],
            beta_i: vec![1.0],
            beta_p: vec![0.0],
        };
        assert!(load.branch_current(0, &zero, 1e-12).is_err());

        load.model = crate::mc_pf::loads::BranchVoltageModel::Exponential {
            gamma_p: vec![2.0],
            gamma_q: vec![2.0],
        };
        assert_eq!(
            load.branch_current(0, &zero, 1e-12).unwrap(),
            Complex64::default()
        );

        load.model = crate::mc_pf::loads::BranchVoltageModel::Exponential {
            gamma_p: vec![0.7],
            gamma_q: vec![2.0],
        };
        assert!(load.branch_current(0, &zero, 1e-12).is_err());

        // A very small nonzero voltage remains evaluable for a bounded
        // current law; it is not rounded to zero by the guard.
        load.model = crate::mc_pf::loads::BranchVoltageModel::ConstantCurrent;
        let tiny = [Complex64::new(1e-200, 1e-200)];
        let ci_current = load.branch_current(0, &tiny, 1e-12).unwrap();
        assert!(ci_current.norm() > 0.0);
        load.model = crate::mc_pf::loads::BranchVoltageModel::Exponential {
            gamma_p: vec![1.0],
            gamma_q: vec![1.0],
        };
        let exp_current = load.branch_current(0, &tiny, 1e-12).unwrap();
        assert!((exp_current - ci_current).norm() < 1e-12);

        // An inactive P or Q component must not evaluate its unused exponent:
        // 0 * infinity would otherwise turn a finite current into NaN.
        load.power = vec![Complex64::new(10.0, 0.0)];
        load.model = crate::mc_pf::loads::BranchVoltageModel::Exponential {
            gamma_p: vec![2.0],
            gamma_q: vec![-1_000_000.0],
        };
        let current = load
            .branch_current(0, &[Complex64::new(8.0, 0.0)], 1e-12)
            .unwrap();
        assert!(current.re.is_finite() && current.im.is_finite());

        let huge = crate::mc_pf::loads::BranchLoad {
            name: "huge".into(),
            incidence: vec![vec![(0, Complex64::new(1.0, 0.0))]],
            power: vec![Complex64::new(1e308, 0.0)],
            y_ref: vec![Complex64::new(1e306, 0.0)],
            nominal_voltage: vec![10.0],
            model: crate::mc_pf::loads::BranchVoltageModel::ConstantImpedance,
        };
        assert!(huge
            .branch_current(0, &[Complex64::new(20.0, 0.0)], 1e-12)
            .is_err());
    }

    #[test]
    fn voltage_dependent_models_reuse_one_factorization() {
        let models = [
            DistLoadVoltageModel::ConstantCurrent { v_nom: vec![10.0] },
            DistLoadVoltageModel::ConstantImpedance { v_nom: vec![10.0] },
            DistLoadVoltageModel::Zip {
                v_nom: vec![10.0],
                alpha_z: vec![0.2],
                alpha_i: vec![0.3],
                alpha_p: vec![0.5],
                beta_z: vec![0.1],
                beta_i: vec![0.4],
                beta_p: vec![0.5],
            },
            DistLoadVoltageModel::Exponential {
                v_nom: vec![10.0],
                gamma_p: vec![0.7],
                gamma_q: vec![2.3],
            },
        ];
        for model in models {
            let mut network = one_phase(1.0).network().clone();
            network.loads_mut()[0].voltage_model = model;
            let instance = McAcPfInstance::from_network(network).unwrap();
            let result = solve_mc_ac_pf_instance(&instance, &McPfOptions::default()).unwrap();
            assert!(result.converged);
            assert_eq!(result.factorization_count, 1);
        }
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

    fn one_phase_module(power: f64) -> String {
        let instance = one_phase(power);
        crate::ir::serialize_module(&powerio::PioModule::new(powerio::PioValue::McAcPfInstance(
            instance,
        )))
        .expect("MC instance module")
    }

    #[test]
    fn study_snapshot_is_self_contained_and_replayable() {
        let snapshot = McStudySnapshot::solve(
            "distribution-study",
            "Distribution PF",
            &one_phase_module(1.0),
            McPfOptions::default(),
        )
        .expect("snapshot solve");
        let text = snapshot.to_json().expect("snapshot JSON");
        let replayed = McStudySnapshot::from_json(&text).expect("snapshot replay");
        assert_eq!(replayed.schema, McStudySnapshot::SCHEMA);
        assert_eq!(replayed.formulation, "mc_ac_pf");
        assert_eq!(replayed.result.factorization_count, 1);
        assert!(replayed.solution_module.contains("McAcPfSolution"));
    }

    #[test]
    fn study_snapshot_rejects_tampered_result_and_instance() {
        let snapshot = McStudySnapshot::solve(
            "distribution-study",
            "Distribution PF",
            &one_phase_module(1.0),
            McPfOptions::default(),
        )
        .expect("snapshot solve");
        let mut value: serde_json::Value =
            serde_json::from_str(&snapshot.to_json().unwrap()).unwrap();
        value["result"]["terminals"][0]["voltage"]["re"] = serde_json::json!(999.0);
        assert!(McStudySnapshot::from_json(&value.to_string()).is_err());

        value = serde_json::from_str(&snapshot.to_json().unwrap()).unwrap();
        value["input_module"] = serde_json::Value::String(one_phase_module(2.0));
        assert!(McStudySnapshot::from_json(&value.to_string()).is_err());
    }
}
