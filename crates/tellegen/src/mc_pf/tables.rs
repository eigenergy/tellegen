//! The complete multiconductor result as columns.
//!
//! [`McPfResult`] holds one object and several strings per terminal and
//! equipment port. A whole-city operating point is then hundreds of
//! megabytes once a host parses it, although the values are a few arrays of
//! numbers. [`McPfResultTables`] carries the same rows as columns, with every
//! name stored once.

use std::collections::HashMap;

use num_complex::Complex64;

use super::network::PreparedNetwork;
use super::{
    load_port_values, passive_current, passive_port_values, FixedPoint, McComplex, McPfOptions,
    McPfResult, NON_FINITE_OUTPUT,
};

/// The terminals, equipment ports, and source reactions of an
/// [`McPfResult`] as columns, row for row in the same order. Names are
/// indices into `strings`, each name stored once; complex values are
/// interleaved `[re, im]`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct McPfResultTables {
    pub iterations: usize,
    /// The names the index columns refer to.
    pub strings: Vec<String>,
    pub terminal_bus: Vec<u32>,
    pub terminal_name: Vec<u32>,
    /// Volts.
    pub terminal_voltage: Vec<f64>,
    /// Current injected into the network, amperes.
    pub terminal_current: Vec<f64>,
    /// `V * conj(I_into_network)`, VA.
    pub terminal_power: Vec<f64>,
    pub port_element: Vec<u32>,
    pub port_kind: Vec<u32>,
    pub port_branch: Vec<u32>,
    pub port_bus: Vec<u32>,
    pub port_terminal: Vec<u32>,
    /// Current into the element, amperes.
    pub port_current: Vec<f64>,
    /// Power into the element, VA.
    pub port_power: Vec<f64>,
    pub source_name: Vec<u32>,
    pub source_terminal: Vec<u32>,
    /// Current injected into the network, amperes.
    pub source_current: Vec<f64>,
    /// Power injected into the network, VA.
    pub source_power: Vec<f64>,
}

impl McPfResultTables {
    /// The columns of a complete result, interning names in row order.
    pub fn from_result(result: &McPfResult) -> Self {
        let mut names = Names::default();
        let mut tables = Self {
            iterations: result.iterations,
            ..Self::default()
        };
        for row in &result.terminals {
            tables.terminal_bus.push(names.index(&row.bus));
            tables.terminal_name.push(names.index(&row.terminal));
            push(&mut tables.terminal_voltage, row.voltage);
            push(&mut tables.terminal_current, row.current_into_network);
            push(&mut tables.terminal_power, row.power_into_network);
        }
        for port in &result.element_ports {
            tables.port_element.push(names.index(&port.element));
            tables.port_kind.push(names.index(&port.kind));
            tables.port_branch.push(port.branch as u32);
            tables.port_bus.push(names.index(&port.bus));
            tables.port_terminal.push(names.index(&port.terminal));
            push(&mut tables.port_current, port.current_into_element);
            push(&mut tables.port_power, port.power_into_element);
        }
        for source in &result.source_reactions {
            tables.source_name.push(names.index(&source.source));
            tables.source_terminal.push(names.index(&source.terminal));
            push(&mut tables.source_current, source.current_into_network);
            push(&mut tables.source_power, source.power_into_network);
        }
        tables.strings = names.strings;
        tables
    }

    pub fn terminal_count(&self) -> usize {
        self.terminal_bus.len()
    }

    pub fn port_count(&self) -> usize {
        self.port_element.len()
    }

    pub fn source_count(&self) -> usize {
        self.source_name.len()
    }
}

/// Columns of the result `build_result` would return, with the same
/// arithmetic, row order, and refusal of non-finite output, but without
/// building that result first.
pub(super) fn build_tables(
    prepared: &PreparedNetwork,
    options: &McPfOptions,
    state: &FixedPoint,
) -> Result<McPfResultTables, String> {
    let voltage = &state.voltage;
    let mut names = Names::default();
    let mut finite = true;
    let mut tables = McPfResultTables {
        iterations: state.iterations,
        ..McPfResultTables::default()
    };
    for (i, (bus, terminal)) in prepared.index.terminal_ids.iter().enumerate() {
        let current = passive_current(prepared, voltage, i);
        let power = voltage[i] * current.conj();
        finite &= is_finite(voltage[i]) && is_finite(current) && is_finite(power);
        tables.terminal_bus.push(names.index(bus));
        tables.terminal_name.push(names.index(terminal));
        push(&mut tables.terminal_voltage, voltage[i]);
        push(&mut tables.terminal_current, current);
        push(&mut tables.terminal_power, power);
    }
    for load in &prepared.loads {
        for (branch, incidence) in load.incidence.iter().enumerate() {
            let branch_current = load.branch_current(branch, voltage, options)?;
            for &(i, c) in incidence {
                let (current, power) = load_port_values(voltage, i, c, branch_current);
                let (bus, terminal) = &prepared.index.terminal_ids[i];
                finite &= is_finite(current) && is_finite(power);
                tables.port_element.push(names.index(&load.name));
                tables.port_kind.push(names.index("load"));
                tables.port_branch.push(branch as u32);
                tables.port_bus.push(names.index(bus));
                tables.port_terminal.push(names.index(terminal));
                push(&mut tables.port_current, current);
                push(&mut tables.port_power, power);
            }
        }
    }
    for element in &prepared.elements {
        for row in 0..element.terminals.len() {
            let (i, current, power) = passive_port_values(voltage, element, row);
            let (bus, terminal) = &prepared.index.terminal_ids[i];
            finite &= is_finite(current) && is_finite(power);
            tables.port_element.push(names.index(&element.name));
            tables.port_kind.push(names.index(&element.kind));
            tables.port_branch.push(0);
            tables.port_bus.push(names.index(bus));
            tables.port_terminal.push(names.index(terminal));
            push(&mut tables.port_current, current);
            push(&mut tables.port_power, power);
        }
    }
    for source in &prepared.source_terminals {
        let i = source.index;
        let current = passive_current(prepared, voltage, i) + state.load_current[i];
        let power = voltage[i] * current.conj();
        finite &= is_finite(current) && is_finite(power);
        tables.source_name.push(names.index(&source.source));
        tables.source_terminal.push(names.index(&source.terminal));
        push(&mut tables.source_current, current);
        push(&mut tables.source_power, power);
    }
    if !finite {
        return Err(NON_FINITE_OUTPUT.to_owned());
    }
    tables.strings = names.strings;
    Ok(tables)
}

/// Each distinct name once, indexed in first-seen order.
#[derive(Default)]
struct Names<'a> {
    index: HashMap<&'a str, u32>,
    strings: Vec<String>,
}

impl<'a> Names<'a> {
    fn index(&mut self, name: &'a str) -> u32 {
        let strings = &mut self.strings;
        *self.index.entry(name).or_insert_with(|| {
            strings.push(name.to_owned());
            (strings.len() - 1) as u32
        })
    }
}

fn push(column: &mut Vec<f64>, value: impl Into<McComplex>) {
    let value = value.into();
    column.push(value.re);
    column.push(value.im);
}

fn is_finite(value: Complex64) -> bool {
    value.re.is_finite() && value.im.is_finite()
}
