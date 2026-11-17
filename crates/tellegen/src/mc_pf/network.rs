//! Terminal indexing and passive multiconductor network preparation.

use std::collections::{BTreeMap, BTreeSet};

use num_complex::Complex64;
use powerio_dist::MulticonductorNetwork;
use powerio_prob::McAcPfInstance;

use super::linear::RetainedComplexLu;
use super::loads::{prepare_load, BranchLoad};

/// The solver's terminal positions: PowerIO's flat terminal index (every
/// declared terminal, bus names folded ASCII case insensitively, display
/// spelling retained) and PowerIO's node index over it, which says which
/// terminals are ground, plus the solver's own admission rules.
#[derive(Clone, Debug)]
pub(crate) struct NetworkIndex {
    nodes: powerio_matrix::MulticonductorNodeIndex,
}

impl NetworkIndex {
    pub(crate) fn new(network: &MulticonductorNetwork) -> Result<Self, String> {
        for bus in network.buses() {
            if bus.terminals.is_empty() {
                return Err(format!("bus `{}` has no terminals", bus.id));
            }
        }
        let nodes = powerio_matrix::MulticonductorNodeIndex::build(network)
            .map_err(|error| error.to_string())?;
        for bus in network.buses() {
            for grounded in &bus.grounded {
                if nodes.terminals().resolve(&bus.id, grounded).is_none() {
                    return Err(format!(
                        "bus `{}` grounds unknown terminal `{grounded}`",
                        bus.id
                    ));
                }
            }
        }
        Ok(Self { nodes })
    }

    /// Position to `(bus, terminal)` in the network's own spelling.
    pub(crate) fn terminal_ids(&self) -> &[(String, String)] {
        self.nodes.terminals().terminals()
    }

    pub(crate) fn resolve(&self, identity: (&str, &str)) -> Result<usize, String> {
        self.nodes
            .terminals()
            .resolve(identity.0, identity.1)
            .ok_or_else(|| format!("unknown terminal identity {}:{}", identity.0, identity.1))
    }

    /// Positions of one bus's terminals in calculation order.
    pub(crate) fn bus_terminals(&self, bus: &str) -> Vec<usize> {
        self.nodes.terminals().bus_terminals(bus)
    }

    /// Whether PowerIO's node index grounds the terminal at this position:
    /// terminal `0` or a terminal its bus grounds.
    pub(crate) fn is_ground(&self, position: usize) -> bool {
        self.nodes.resolve_position(position) == Some(powerio_matrix::NodeRef::Ground)
    }
}

/// Admittance accumulator used while stamping. Frozen into [`CsrMatrix`]
/// once preparation finishes.
#[derive(Clone, Debug)]
pub(crate) struct StampedMatrix {
    pub(crate) values: BTreeMap<(usize, usize), Complex64>,
}

impl StampedMatrix {
    pub(crate) fn zeros(_n: usize) -> Self {
        Self {
            values: BTreeMap::new(),
        }
    }
    pub(crate) fn add(&mut self, row: usize, col: usize, value: Complex64) {
        if value.norm() == 0.0 {
            return;
        }
        let entry = self.values.entry((row, col)).or_default();
        *entry += value;
        if entry.norm() == 0.0 {
            self.values.remove(&(row, col));
        }
    }
}

/// Compressed rows of a stamped admittance. Each row keeps the stamped
/// matrix's ascending column order, so a row product accumulates in exactly
/// the order the former `BTreeMap` range walk did.
#[derive(Clone, Debug)]
pub(crate) struct CsrMatrix {
    row_ptr: Vec<usize>,
    cols: Vec<usize>,
    values: Vec<Complex64>,
}

impl CsrMatrix {
    pub(crate) fn from_stamped(stamped: StampedMatrix, n: usize) -> Self {
        let mut row_ptr = vec![0; n + 1];
        let mut cols = Vec::with_capacity(stamped.values.len());
        let mut values = Vec::with_capacity(stamped.values.len());
        for ((row, col), value) in stamped.values {
            row_ptr[row + 1] += 1;
            cols.push(col);
            values.push(value);
        }
        for row in 0..n {
            row_ptr[row + 1] += row_ptr[row];
        }
        Self {
            row_ptr,
            cols,
            values,
        }
    }

    fn row(&self, row: usize) -> impl Iterator<Item = (usize, Complex64)> + '_ {
        let range = self.row_ptr[row]..self.row_ptr[row + 1];
        self.cols[range.clone()]
            .iter()
            .copied()
            .zip(self.values[range].iter().copied())
    }

    pub(crate) fn row_mul(&self, row: usize, vector: &[Complex64]) -> Complex64 {
        self.row(row).map(|(c, value)| value * vector[c]).sum()
    }

    pub(crate) fn row_fixed_mul(&self, row: usize, fixed: &[Option<Complex64>]) -> Complex64 {
        self.row(row)
            .filter_map(|(c, value)| fixed[c].map(|v| value * v))
            .sum()
    }
}

/// A PowerIO failure as its message alone, without the diagnostic code
/// prefix its display adds.
pub(crate) fn powerio_message(error: &powerio::Error) -> String {
    error
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.severity() == powerio_dist::DiagnosticSeverity::Error)
        .map_or_else(
            || error.to_string(),
            |diagnostic| diagnostic.message().to_owned(),
        )
}

/// [`powerio_message`] for a matrix builder failure that carries one.
pub(crate) fn matrix_message(error: powerio_matrix::Error) -> String {
    match &error {
        powerio_matrix::Error::Commit(inner) => powerio_message(inner),
        _ => error.to_string(),
    }
}

/// PowerIO's assembled passive admittance `G + jB` at the terminal positions
/// of its nodes. Ground has no row or column in PowerIO's grounded system,
/// and no closed switch reaches this snapshot, so every node is exactly one
/// terminal position.
fn passive_at_positions(
    admittance: &powerio_matrix::MulticonductorAdmittance,
    position_of_node: &[usize],
) -> StampedMatrix {
    let mut passive = StampedMatrix::zeros(position_of_node.len());
    for (&value, (row, col)) in admittance.conductance().iter() {
        passive.add(
            position_of_node[row],
            position_of_node[col],
            Complex64::new(value, 0.0),
        );
    }
    for (&value, (row, col)) in admittance.susceptance().iter() {
        passive.add(
            position_of_node[row],
            position_of_node[col],
            Complex64::new(0.0, value),
        );
    }
    passive
}

/// The terminal voltages PowerIO's assembled system prescribes: each source
/// row and each solidly grounded transformer neutral row fixes one node. A
/// row over several nodes (an ideal transformer, a floating source
/// reference) is an exact constraint this fixed-point snapshot does not
/// solve.
fn prescribed_voltages(
    admittance: &powerio_matrix::MulticonductorAdmittance,
    position_of_node: &[usize],
) -> Result<Vec<(usize, Complex64)>, String> {
    let augmented = admittance.augmented();
    let rows = augmented.labels.len();
    let mut entries = vec![Vec::<(usize, Complex64)>::new(); rows];
    for (&value, (row, col)) in augmented.constraint_re.iter() {
        if row < rows {
            entries[row].push((col, Complex64::new(value, 0.0)));
        }
    }
    for (&value, (row, col)) in augmented.constraint_im.iter() {
        if row < rows {
            entries[row].push((col, Complex64::new(0.0, value)));
        }
    }
    entries
        .iter()
        .enumerate()
        .map(|(row, entries)| match entries.as_slice() {
            [(node, coefficient)] => Ok((
                position_of_node[*node],
                Complex64::new(augmented.rhs_re[row], augmented.rhs_im[row]) / coefficient,
            )),
            _ => Err(format!(
                "`{}` constrains several terminals; the fixed-point snapshot prescribes terminal voltages only",
                augmented.labels[row]
            )),
        })
        .collect()
}

#[derive(Debug)]
pub(crate) struct PreparedNetwork {
    pub(crate) index: NetworkIndex,
    /// PowerIO's assembled passive admittance at terminal positions; ground
    /// positions have no row ([`Self::passive_current`]).
    pub(crate) passive: CsrMatrix,
    /// The element ports at each grounded terminal position: `(element,
    /// row)` into [`Self::elements`].
    pub(crate) ground_ports: BTreeMap<usize, Vec<(usize, usize)>>,
    pub(crate) yref: CsrMatrix,
    pub(crate) fixed: Vec<Option<Complex64>>,
    pub(crate) unknown: Vec<usize>,
    /// Position of each terminal in `unknown`, or `usize::MAX` when fixed.
    pub(crate) unknown_pos: Vec<usize>,
    /// The fixed-terminal elimination RHS. The compensation reference is
    /// frozen, so this is constant for the life of the factorization.
    pub(crate) source_rhs: Vec<Complex64>,
    /// Source reaction terminals in network source and terminal order.
    pub(crate) source_terminals: Vec<SourceTerminal>,
    pub(crate) loads: Vec<BranchLoad>,
    pub(crate) elements: Vec<PreparedElement>,
    pub(crate) factor: Option<RetainedComplexLu>,
}

#[derive(Clone, Debug)]
pub(crate) struct SourceTerminal {
    pub(crate) source: String,
    pub(crate) terminal: String,
    pub(crate) index: usize,
}

/// Wall time spent assembling and factoring a prepared network, in
/// milliseconds of the profile clock.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct PrepareTiming {
    pub(crate) assemble_ms: f64,
    pub(crate) factor_ms: f64,
}

#[derive(Clone, Debug)]
pub(crate) struct PreparedElement {
    pub(crate) name: String,
    pub(crate) kind: String,
    pub(crate) terminals: Vec<usize>,
    pub(crate) yprim: Vec<Vec<Complex64>>,
}

impl PreparedNetwork {
    pub(crate) fn prepare(instance: &McAcPfInstance) -> Result<Self, String> {
        Self::prepare_timed(instance).map(|(prepared, _)| prepared)
    }

    pub(crate) fn prepare_timed(
        instance: &McAcPfInstance,
    ) -> Result<(Self, PrepareTiming), String> {
        let started = super::profile_now();
        let network = instance.network();
        if !instance.isolated_terminals().is_empty() {
            return Err(
                "isolated terminals are not supported by the fixed-point PF snapshot".to_owned(),
            );
        }
        if !instance.control_modes().is_empty() {
            return Err("active regulator/capacitor controls are unsupported; solve a fixed operating snapshot".to_owned());
        }
        super::input::validate_mc_network(network)?;
        let index = NetworkIndex::new(network)?;
        let n = index.terminal_ids().len();
        // Name lookups keep the first matching row, exactly as the linear
        // searches they replace did, without their quadratic cost.
        let mut source_rows = BTreeMap::<&str, &powerio_dist::VoltageSource>::new();
        for source in network.sources() {
            source_rows.entry(source.name.as_str()).or_insert(source);
        }
        let mut load_rows = BTreeMap::<&str, &powerio_dist::DistLoad>::new();
        for load in network.loads() {
            load_rows.entry(load.name.as_str()).or_insert(load);
        }
        let mut yref = StampedMatrix::zeros(n);
        let mut elements = Vec::new();
        let mut fixed = vec![None; n];
        // PowerIO's node index decides which terminals are ground.
        for (position, value) in fixed.iter_mut().enumerate() {
            if index.is_ground(position) {
                *value = Some(Complex64::new(0.0, 0.0));
            }
        }

        // The instance's ideal sources. PowerIO's assembled system states
        // their prescribed voltages (below); the admission rules here are the
        // solver's own: finite voltages, no finite source impedance, and one
        // source per terminal, since a shared terminal's reaction current has
        // no allocation between its sources.
        let mut prescribed_source_terminals = BTreeSet::new();
        for source in instance.sources() {
            if source.terminals.len() != source.v_magnitude.len()
                || source.terminals.len() != source.v_angle.len()
            {
                return Err(format!(
                    "source `{}` terminal and voltage dimensions are inconsistent",
                    source.source
                ));
            }
            let source_row = source_rows
                .get(source.source.as_str())
                .copied()
                .ok_or_else(|| {
                    format!(
                        "instance source `{}` is absent from its network",
                        source.source
                    )
                })?;
            if source
                .v_magnitude
                .iter()
                .zip(&source.v_angle)
                .any(|(&mag, &angle)| !mag.is_finite() || !angle.is_finite() || mag < 0.0)
            {
                return Err(format!(
                    "source `{}` has a non-finite or negative prescribed voltage",
                    source.source
                ));
            }
            // The v0.11 typed source is an ideal prescribed source.  A
            // finite source impedance retained in raw extras must not be
            // silently discarded by this ideal-terminal solver.
            for key in [
                "r1", "x1", "r0", "x0", "r2", "x2", "isc3", "isc1", "mvasc", "mvasc3", "mvasc1",
                "z_source", "z1", "z0", "puz1", "puz0", "bus2",
            ] {
                if source_row.extras.contains_key(key) {
                    return Err(format!("source `{}` carries finite impedance metadata `{key}`; normalize it to a Norton source before MC PF", source.source));
                }
            }
            for terminal in &source.terminals {
                let i = index.resolve((&source_row.bus, terminal))?;
                if !prescribed_source_terminals.insert(i) {
                    return Err(format!(
                        "multiple ideal sources prescribe terminal `{}`:{}; reaction-current allocation is indeterminate",
                        source_row.bus, terminal
                    ));
                }
            }
        }

        for switch in network.switches() {
            if !switch.open {
                return Err(format!(
                    "closed switch `{}` is unsupported by the fixed linear snapshot",
                    switch.name
                ));
            }
        }
        // PowerIO's element list: lines, shunts, capacitor banks and leakage
        // transformers with their primitives. An element it cannot stamp, or
        // an ideal transformer (exact constraints this snapshot does not
        // solve), would leave the solved network short, so the snapshot
        // refuses it.
        let listed = powerio_matrix::calc_passive_primitives(network).map_err(matrix_message)?;
        if let Some(finding) = listed.diagnostics.first() {
            return Err(finding.message().to_string());
        }
        if let Some(ideal) = listed.ideal_transformers.first() {
            // The primitive's own refusal names why.
            powerio_matrix::calc_transformer_primitive_admittance(network, ideal)
                .map_err(|error| powerio_message(&error))?;
        }
        // PowerIO assembles the nodal system once from that list: the
        // passive admittance, a row per prescribed source terminal and a row
        // per solidly grounded transformer neutral that no source fixes.
        let admittance = powerio_matrix::assemble_multiconductor_admittance(network, &listed)
            .map_err(matrix_message)?;
        let mut position_of_node = vec![usize::MAX; admittance.index().len()];
        for position in 0..n {
            if let Some(powerio_matrix::NodeRef::Node(node)) =
                admittance.index().resolve_position(position)
            {
                position_of_node[node] = position;
            }
        }
        let passive = passive_at_positions(&admittance, &position_of_node);
        for (position, value) in prescribed_voltages(&admittance, &position_of_node)? {
            fixed[position] = Some(value);
        }
        // Each element keeps its own primitive for its port currents.
        let mut ground_ports = BTreeMap::<usize, Vec<(usize, usize)>>::new();
        for element in listed.elements {
            let terminals = element
                .terminals
                .iter()
                .map(|&identity| index.resolve(identity))
                .collect::<Result<Vec<_>, _>>()?;
            for (row, &position) in terminals.iter().enumerate() {
                if index.is_ground(position) {
                    ground_ports
                        .entry(position)
                        .or_default()
                        .push((elements.len(), row));
                }
            }
            elements.push(PreparedElement {
                name: element.name.to_owned(),
                kind: element.family.noun().to_owned(),
                terminals,
                yprim: element.primitive,
            });
        }
        // PowerIO infers the branch voltage base of each constant power load
        // whose source stated none, from its zone's nameplate or source anchor.
        let inferred_nominal_voltage = instance
            .calc_inferred_load_nominal_voltages()
            .map_err(|e| e.to_string())?;
        let loads = instance
            .loads()
            .iter()
            .map(|l| {
                let source = load_rows.get(l.load.as_str()).copied().ok_or_else(|| {
                    format!("instance load `{}` is absent from its network", l.load)
                })?;
                let mut effective = source.clone();
                // The instance is the calculation boundary.  Its prescribed
                // operating powers may be edited without mutating the network.
                effective.p_nom = l.p_w.clone();
                effective.q_nom = l.q_var.clone();
                effective.terminal_map = l.terminals.clone();
                effective.voltage_model = l.voltage_model.clone();
                prepare_load(
                    network,
                    &effective,
                    &index,
                    inferred_nominal_voltage
                        .get(&effective.name)
                        .map(Vec::as_slice),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        for load in &loads {
            load.stamp_yref(&mut yref);
        }

        let unknown: Vec<_> = (0..n).filter(|&i| fixed[i].is_none()).collect();
        let mut position = vec![usize::MAX; n];
        for (u, &i) in unknown.iter().enumerate() {
            position[i] = u;
        }
        let mut triplets_by_global = passive.values.clone();
        for (&key, &value) in &yref.values {
            *triplets_by_global.entry(key).or_default() += value;
        }
        let triplets = triplets_by_global
            .into_iter()
            .filter_map(|((i, j), value)| {
                let r = position[i];
                let c = position[j];
                (r != usize::MAX && c != usize::MAX && value.norm() > 0.0).then_some((r, c, value))
            })
            .collect::<Vec<_>>();
        let mut instance_sources = BTreeMap::<&str, &powerio_prob::PrescribedSourceVoltage>::new();
        for source in instance.sources() {
            instance_sources
                .entry(source.source.as_str())
                .or_insert(source);
        }
        let mut source_terminals = Vec::new();
        for source_row in network.sources() {
            let source = instance_sources
                .get(source_row.name.as_str())
                .ok_or_else(|| {
                    format!(
                        "instance source `{}` is absent from its network",
                        source_row.name
                    )
                })?;
            for terminal in &source.terminals {
                source_terminals.push(SourceTerminal {
                    source: source.source.clone(),
                    terminal: terminal.clone(),
                    index: index.resolve((&source_row.bus, terminal))?,
                });
            }
        }
        let assembled = super::profile_now();
        let factor = if unknown.is_empty() {
            None
        } else {
            Some(RetainedComplexLu::factor(unknown.len(), &triplets)?)
        };
        let factored = super::profile_now();
        let mut prepared = Self {
            index,
            passive: CsrMatrix::from_stamped(passive, n),
            ground_ports,
            yref: CsrMatrix::from_stamped(yref, n),
            fixed,
            unknown,
            unknown_pos: position,
            source_rhs: Vec::new(),
            source_terminals,
            loads,
            elements,
            factor,
        };
        prepared.source_rhs = prepared.eliminate_fixed_terminals();
        let timing = PrepareTiming {
            assemble_ms: super::elapsed_ms(started, assembled),
            factor_ms: super::elapsed_ms(assembled, factored),
        };
        Ok((prepared, timing))
    }

    /// The passive current into the network at terminal position `row`: its
    /// row of the assembled admittance times the voltages, or, at a grounded
    /// terminal (which has no row), the sum of its element port currents.
    pub(crate) fn passive_current(&self, voltage: &[Complex64], row: usize) -> Complex64 {
        match self.ground_ports.get(&row) {
            Some(ports) => ports
                .iter()
                .map(|&(element, port)| {
                    let element = &self.elements[element];
                    element.yprim[port]
                        .iter()
                        .zip(&element.terminals)
                        .map(|(&y, &terminal)| y * voltage[terminal])
                        .sum::<Complex64>()
                })
                .sum(),
            None if self.index.is_ground(row) => Complex64::default(),
            None => self.passive.row_mul(row, voltage),
        }
    }

    pub(crate) fn fixed_voltage_vector(&self) -> Vec<Complex64> {
        self.fixed.iter().map(|x| x.unwrap_or_default()).collect()
    }
    fn eliminate_fixed_terminals(&self) -> Vec<Complex64> {
        // The retained matrix is Ypassive + Yref and compensation is evaluated
        // on the full voltage vector.  Eliminate fixed terminals with the full
        // Yuf block so the fixed Yref coupling cancels exactly in the RHS.
        self.unknown
            .iter()
            .map(|&i| {
                -(self.passive.row_fixed_mul(i, &self.fixed)
                    + self.yref.row_fixed_mul(i, &self.fixed))
            })
            .collect()
    }
}

#[cfg(test)]
mod identity_tests {
    use super::*;

    #[test]
    fn bus_lookup_folds_case_but_preserves_terminal_identity_and_display() {
        let mut network = MulticonductorNetwork::default();
        let mut bus = powerio_dist::DistBus::default();
        bus.id = "SourceBus".into();
        bus.terminals = vec!["a".into(), "A".into()];
        network.buses_mut().push(bus);
        let index = NetworkIndex::new(&network).unwrap();
        assert_eq!(index.resolve(("sourcebus", "a")).unwrap(), 0);
        assert_eq!(index.resolve(("SOURCEBUS", "A")).unwrap(), 1);
        assert_eq!(index.bus_terminals("sOuRcEbUs"), vec![0, 1]);
        assert_eq!(index.terminal_ids()[0].0, "SourceBus");
        let mut duplicate = powerio_dist::DistBus::default();
        duplicate.id = "sourcebus".into();
        duplicate.terminals = vec!["other".into()];
        network.buses_mut().push(duplicate);
        assert!(NetworkIndex::new(&network)
            .unwrap_err()
            .contains("duplicated case-insensitively"));
    }
}

#[cfg(test)]
#[path = "linear_comparison.rs"]
mod linear_comparison;
