//! Terminal indexing and passive multiconductor network preparation.

use std::collections::{BTreeMap, BTreeSet};

use num_complex::Complex64;
use powerio_dist::{DistLineCode, MulticonductorNetwork};
use powerio_prob::McAcPfInstance;

use super::linear::RetainedComplexLu;
use super::loads::{prepare_load, BranchLoad};
use super::transformer::prepare_transformer;

#[derive(Clone, Debug)]
pub(crate) struct NetworkIndex {
    pub terminal_ids: Vec<(String, String)>,
    /// Terminal positions by ASCII-folded bus name and exact terminal name.
    /// PowerIO bus identities are case insensitive; display spelling is retained.
    positions: BTreeMap<String, BTreeMap<String, usize>>,
}

impl NetworkIndex {
    pub(crate) fn new(network: &MulticonductorNetwork) -> Result<Self, String> {
        let mut terminal_ids = Vec::new();
        let mut positions = BTreeMap::<String, BTreeMap<String, usize>>::new();
        for bus in network.buses() {
            if bus.terminals.is_empty() {
                return Err(format!("bus `{}` has no terminals", bus.id));
            }
            let bus_key = bus.id.to_ascii_lowercase();
            if positions.contains_key(&bus_key) {
                return Err(format!("duplicate bus identity {}", bus.id));
            }
            let terminals = positions.entry(bus_key).or_default();
            for terminal in &bus.terminals {
                if terminals
                    .insert(terminal.clone(), terminal_ids.len())
                    .is_some()
                {
                    return Err(format!("duplicate terminal identity {}:{terminal}", bus.id));
                }
                terminal_ids.push((bus.id.clone(), terminal.clone()));
            }
            for grounded in &bus.grounded {
                if !terminals.contains_key(grounded) {
                    return Err(format!(
                        "bus `{}` grounds unknown terminal `{grounded}`",
                        bus.id
                    ));
                }
            }
        }
        Ok(Self {
            terminal_ids,
            positions,
        })
    }

    pub(crate) fn resolve(&self, identity: (&str, &str)) -> Result<usize, String> {
        self.positions
            .get(identity.0)
            .or_else(|| self.positions.get(&identity.0.to_ascii_lowercase()))
            .and_then(|terminals| terminals.get(identity.1))
            .copied()
            .ok_or_else(|| format!("unknown terminal identity {}:{}", identity.0, identity.1))
    }

    /// Positions of one bus's terminals in calculation order.
    pub(crate) fn bus_terminals(&self, bus: &str) -> Vec<usize> {
        let mut positions: Vec<usize> = self
            .positions
            .get(bus)
            .or_else(|| self.positions.get(&bus.to_ascii_lowercase()))
            .map(|terminals| terminals.values().copied().collect())
            .unwrap_or_default();
        positions.sort_unstable();
        positions
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

fn stamp_local_values(
    y: &mut StampedMatrix,
    terminals: &[usize],
    local: &[Vec<Complex64>],
) -> Result<(), String> {
    if local.len() != terminals.len() || local.iter().any(|row| row.len() != terminals.len()) {
        return Err("component primitive and terminal map dimensions differ".to_owned());
    }
    for (r, &i) in terminals.iter().enumerate() {
        for (c, &j) in terminals.iter().enumerate() {
            y.add(i, j, local[r][c]);
        }
    }
    Ok(())
}

#[derive(Debug)]
pub(crate) struct PreparedNetwork {
    pub(crate) index: NetworkIndex,
    pub(crate) passive: CsrMatrix,
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
        let n = index.terminal_ids.len();
        // Name lookups keep the first matching row, exactly as the linear
        // searches they replace did, without their quadratic cost.
        let mut source_rows = BTreeMap::<&str, &powerio_dist::VoltageSource>::new();
        for source in network.sources() {
            source_rows.entry(source.name.as_str()).or_insert(source);
        }
        let mut line_codes = BTreeMap::<String, &DistLineCode>::new();
        for code in network.line_codes() {
            line_codes
                .entry(code.name.to_ascii_lowercase())
                .or_insert(code);
        }
        let mut load_rows = BTreeMap::<&str, &powerio_dist::DistLoad>::new();
        for load in network.loads() {
            load_rows.entry(load.name.as_str()).or_insert(load);
        }
        let mut passive = StampedMatrix::zeros(n);
        let mut yref = StampedMatrix::zeros(n);
        let mut elements = Vec::new();
        let mut fixed = vec![None; n];
        let mut prescribed_source_terminals = BTreeSet::new();
        for bus in network.buses() {
            for terminal in &bus.grounded {
                fixed[index.resolve((&bus.id, terminal))?] = Some(Complex64::new(0.0, 0.0));
            }
        }

        // Ideal sources are exact Dirichlet terminals.  A second source may
        // share a terminal only when it prescribes the same complex voltage.
        for source in instance.sources() {
            if source.terminals.len() != source.v_magnitude.len()
                || source.terminals.len() != source.v_angle.len()
            {
                return Err(format!(
                    "source `{}` terminal and voltage dimensions are inconsistent",
                    source.source
                ));
            }
            for ((terminal, &mag), &angle) in source
                .terminals
                .iter()
                .zip(&source.v_magnitude)
                .zip(&source.v_angle)
            {
                if !mag.is_finite() || !angle.is_finite() || mag < 0.0 {
                    return Err(format!(
                        "source `{}` has a non-finite or negative prescribed voltage",
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
                // The v0.11 typed source is an ideal prescribed source.  A
                // finite source impedance retained in raw extras must not be
                // silently discarded by this ideal-terminal solver.
                for key in [
                    "r1", "x1", "r0", "x0", "r2", "x2", "isc3", "isc1", "mvasc", "mvasc3",
                    "mvasc1", "z_source", "z1", "z0", "puz1", "puz0", "bus2",
                ] {
                    if source_row.extras.contains_key(key) {
                        return Err(format!("source `{}` carries finite impedance metadata `{key}`; normalize it to a Norton source before MC PF", source.source));
                    }
                }
                let i = index.resolve((&source_row.bus, terminal))?;
                if !prescribed_source_terminals.insert(i) {
                    return Err(format!(
                        "multiple ideal sources prescribe terminal `{}`:{}; reaction-current allocation is indeterminate",
                        source_row.bus, terminal
                    ));
                }
                let value = Complex64::from_polar(mag, angle);
                if let Some(previous) = fixed[i] {
                    if (previous - value).norm() > 1e-8 * (1.0 + value.norm()) {
                        return Err(format!(
                            "conflicting ideal source prescriptions at terminal `{terminal}`"
                        ));
                    }
                } else {
                    fixed[i] = Some(value);
                }
            }
        }

        for line in network.lines() {
            let code = line_codes
                .get(&line.linecode.to_ascii_lowercase())
                .copied()
                .ok_or_else(|| {
                    format!(
                        "line `{}` references unresolved linecode `{}`",
                        line.name, line.linecode
                    )
                })?;
            elements.push(stamp_line(&mut passive, line, code, &index)?);
        }
        for switch in network.switches() {
            if !switch.open {
                return Err(format!(
                    "closed switch `{}` is unsupported by the fixed linear snapshot",
                    switch.name
                ));
            }
        }
        for shunt in network.shunts() {
            if shunt.g.len() != shunt.b.len()
                || shunt.g.iter().any(|r| r.len() != shunt.g.len())
                || shunt.b.iter().any(|r| r.len() != shunt.b.len())
                || shunt.g.iter().flatten().any(|value| !value.is_finite())
                || shunt.b.iter().flatten().any(|value| !value.is_finite())
                || shunt.terminal_map.len() != shunt.g.len()
            {
                return Err(format!(
                    "shunt `{}` has inconsistent matrix dimensions",
                    shunt.name
                ));
            }
            let map: Vec<_> = shunt
                .terminal_map
                .iter()
                .map(|t| index.resolve((&shunt.bus, t)))
                .collect::<Result<_, _>>()?;
            let yprim = powerio_matrix::calc_shunt_primitive_admittance(shunt);
            stamp_local_values(&mut passive, &map, &yprim)?;
            elements.push(PreparedElement {
                name: shunt.name.clone(),
                kind: "shunt".to_owned(),
                terminals: map,
                yprim,
            });
        }
        for capacitor in network.capacitors() {
            if !capacitor.q_rated.is_finite()
                || capacitor.v_nom <= 0.0
                || capacitor.terminal_map.len() < 2
            {
                return Err(format!(
                    "capacitor `{}` has invalid fixed shunt data",
                    capacitor.name
                ));
            }
            let map: Vec<_> = capacitor
                .terminal_map
                .iter()
                .map(|t| index.resolve((&capacitor.bus, t)))
                .collect::<Result<_, _>>()?;
            // The fixed point snapshot states a wye bank's neutral
            // explicitly, after exactly three phase terminals.
            if capacitor.configuration == powerio_dist::Configuration::Wye && map.len() != 4 {
                return Err(format!(
                    "wye capacitor `{}` requires exactly three phase terminals followed by an explicit neutral",
                    capacitor.name
                ));
            }
            // PowerIO's capacitor primitive: nameplate vars at nameplate
            // voltage over each coil of the configuration.
            let yprim = powerio_matrix::calc_capacitor_primitive_admittance(capacitor)
                .map_err(|error| error.to_string())?
                .ok_or_else(|| {
                    format!(
                        "capacitor `{}` uses unsupported configuration",
                        capacitor.name
                    )
                })?;
            stamp_local_values(&mut passive, &map, &yprim)?;
            elements.push(PreparedElement {
                name: capacitor.name.clone(),
                kind: "capacitor".to_owned(),
                terminals: map,
                yprim,
            });
        }
        for transformer in network.transformers() {
            let primitive = prepare_transformer(transformer).map_err(|e| e.to_string())?;
            for grounded in &primitive.grounded_terminals {
                let i = index.resolve((&grounded.bus, &grounded.terminal))?;
                if let Some(previous) = fixed[i] {
                    if previous.norm() > 1e-12 {
                        return Err(format!(
                            "solid transformer neutral `{}`:{} conflicts with a prescribed voltage",
                            grounded.bus, grounded.terminal
                        ));
                    }
                } else {
                    fixed[i] = Some(Complex64::new(0.0, 0.0));
                }
            }
            let terminals = primitive
                .terminals
                .iter()
                .map(|t| index.resolve((&t.bus, &t.terminal)))
                .collect::<Result<Vec<_>, _>>()?;
            stamp_local_values(&mut passive, &terminals, &primitive.y_prim)?;
            elements.push(PreparedElement {
                name: primitive.name.clone(),
                kind: "transformer".to_owned(),
                terminals,
                yprim: primitive.y_prim,
            });
        }
        let inferred_nominal_voltage = infer_missing_load_voltages(instance)?;
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
        let mut triplets_by_global = BTreeMap::<(usize, usize), Complex64>::new();
        for (&key, &value) in &passive.values {
            *triplets_by_global.entry(key).or_default() += value;
        }
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

/// Infer the branch-voltage base only for constant-power loads whose source
/// format did not state one. Voltage levels are propagated through lines;
/// transformer winding ratings and explicit load nameplates are authoritative
/// anchors, with source operating voltages used only when a component has no
/// nameplate anchor.
fn infer_missing_load_voltages(
    instance: &McAcPfInstance,
) -> Result<BTreeMap<String, Vec<f64>>, String> {
    let network = instance.network();
    if !network.loads().iter().any(|load| {
        load.voltage_model.v_nom().is_empty()
            && matches!(
                load.voltage_model,
                powerio_dist::DistLoadVoltageModel::ConstantPower { .. }
            )
    }) {
        return Ok(BTreeMap::new());
    }
    let bus_position: BTreeMap<_, _> = network
        .buses()
        .iter()
        .enumerate()
        .map(|(index, bus)| (bus.id.to_ascii_lowercase(), index))
        .collect();
    // Voltage zones: the buses lines and closed switches join, labelled by
    // PowerIO's component labeling.
    let (mut zone_from, mut zone_to) = (Vec::new(), Vec::new());
    for line in network.lines() {
        zone_from.push(
            *bus_position
                .get(&line.bus_from.to_ascii_lowercase())
                .ok_or_else(|| format!("line `{}` references an unknown from bus", line.name))?,
        );
        zone_to.push(
            *bus_position
                .get(&line.bus_to.to_ascii_lowercase())
                .ok_or_else(|| format!("line `{}` references an unknown to bus", line.name))?,
        );
    }
    for switch in network.switches().iter().filter(|switch| !switch.open) {
        zone_from.push(
            *bus_position
                .get(&switch.bus_from.to_ascii_lowercase())
                .ok_or_else(|| {
                    format!("switch `{}` references an unknown from bus", switch.name)
                })?,
        );
        zone_to.push(
            *bus_position
                .get(&switch.bus_to.to_ascii_lowercase())
                .ok_or_else(|| format!("switch `{}` references an unknown to bus", switch.name))?,
        );
    }
    let zone = powerio_tx::calc_dense_island_labels(network.buses().len(), &zone_from, &zone_to);

    let mut rated = BTreeMap::<usize, Vec<(String, f64)>>::new();
    let mut source = BTreeMap::<usize, Vec<(String, f64)>>::new();
    let conventions = network.extras().get("bmopf_terminal_conventions");
    let add = |bus: &str,
               label: String,
               value: f64,
               target: &mut BTreeMap<usize, Vec<(String, f64)>>|
     -> Result<(), String> {
        if !value.is_finite() || value <= 0.0 {
            return Err(format!("{label} provides an invalid nominal voltage"));
        }
        let position = *bus_position
            .get(&bus.to_ascii_lowercase())
            .ok_or_else(|| format!("{label} references unknown bus `{bus}`"))?;
        target
            .entry(zone[position])
            .or_default()
            .push((label, value));
        Ok(())
    };

    for transformer in network.transformers() {
        for (index, winding) in transformer.windings.iter().enumerate() {
            let line_neutral = if transformer.phases >= 2 {
                winding.v_ref / 3f64.sqrt()
            } else {
                winding.v_ref
            };
            add(
                &winding.bus,
                format!("transformer `{}` winding {}", transformer.name, index + 1),
                line_neutral,
                &mut rated,
            )?;
        }
    }
    for capacitor in network.capacitors() {
        let bus = &network.buses()[*bus_position
            .get(&capacitor.bus.to_ascii_lowercase())
            .expect("capacitor bus validated by the network")];
        let phase_to_phase =
            terminals_are_phase_to_phase(bus, &capacitor.terminal_map, conventions);
        let line_neutral =
            branch_base_to_line_neutral(capacitor.configuration, phase_to_phase, capacitor.v_nom)?;
        add(
            &capacitor.bus,
            format!("capacitor `{}`", capacitor.name),
            line_neutral,
            &mut rated,
        )?;
    }
    for load in network.loads() {
        let values = load.voltage_model.v_nom();
        if values.is_empty() {
            continue;
        }
        let bus = &network.buses()[*bus_position
            .get(&load.bus.to_ascii_lowercase())
            .expect("load bus validated by the network")];
        let phase_to_phase = terminals_are_phase_to_phase(bus, &load.terminal_map, conventions);
        for (branch, value) in values.iter().copied().enumerate() {
            let line_neutral =
                branch_base_to_line_neutral(load.configuration, phase_to_phase, value)?;
            add(
                &load.bus,
                format!("load `{}` branch {branch}", load.name),
                line_neutral,
                &mut rated,
            )?;
        }
    }
    for prescribed in instance.sources() {
        let voltage_source = network
            .sources()
            .iter()
            .find(|source| source.name == prescribed.source)
            .ok_or_else(|| {
                format!(
                    "instance voltage source `{}` is absent from its network",
                    prescribed.source
                )
            })?;
        let mut magnitudes = prescribed
            .v_magnitude
            .iter()
            .copied()
            .filter(|value| value.is_finite() && *value > 0.0)
            .collect::<Vec<_>>();
        if magnitudes.is_empty() {
            continue;
        }
        magnitudes.sort_by(f64::total_cmp);
        let line_neutral = magnitudes[magnitudes.len() / 2];
        add(
            &voltage_source.bus,
            format!("voltage source `{}`", voltage_source.name),
            line_neutral,
            &mut source,
        )?;
    }

    let mut bases = BTreeMap::<usize, f64>::new();
    for &root in &zone {
        if bases.contains_key(&root) {
            continue;
        }
        let candidates = rated.get(&root).or_else(|| source.get(&root));
        let Some(candidates) = candidates else {
            continue;
        };
        let base = candidates[0].1;
        for (label, candidate) in &candidates[1..] {
            let relative = (candidate - base).abs() / base.max(*candidate);
            // Ratings imported from different formats commonly mix rounded
            // line-line and line-neutral values (for example 415/sqrt(3)
            // versus 240 V). Treat a one-percent difference as the same zone,
            // while still rejecting genuinely ambiguous voltage levels.
            if relative > 1e-2 {
                return Err(format!(
                    "conflicting nominal voltage anchors in one line-connected component: {} V and {label}={} V",
                    base, candidate
                ));
            }
        }
        bases.insert(root, base);
    }

    let mut inferred = BTreeMap::new();
    for load in network.loads() {
        if !load.voltage_model.v_nom().is_empty()
            || !matches!(
                load.voltage_model,
                powerio_dist::DistLoadVoltageModel::ConstantPower { .. }
            )
        {
            continue;
        }
        let position = *bus_position
            .get(&load.bus.to_ascii_lowercase())
            .ok_or_else(|| format!("load `{}` references an unknown bus", load.name))?;
        let root = zone[position];
        let line_neutral = *bases.get(&root).ok_or_else(|| {
            format!(
                "load `{}` omits nominal voltage and bus `{}` has no source, transformer, or nameplate voltage anchor",
                load.name, load.bus
            )
        })?;
        let bus = &network.buses()[position];
        let phase_to_phase = terminals_are_phase_to_phase(bus, &load.terminal_map, conventions);
        let branch_voltage = match load.configuration {
            powerio_dist::Configuration::Wye => line_neutral,
            powerio_dist::Configuration::Delta => line_neutral * 3f64.sqrt(),
            powerio_dist::Configuration::SinglePhase if phase_to_phase => {
                line_neutral * 3f64.sqrt()
            }
            powerio_dist::Configuration::SinglePhase => line_neutral,
            _ => {
                return Err(format!(
                    "load `{}` has an unsupported configuration for nominal-voltage inference",
                    load.name
                ))
            }
        };
        inferred.insert(load.name.clone(), vec![branch_voltage; load.p_nom.len()]);
    }
    Ok(inferred)
}

fn terminals_are_phase_to_phase(
    bus: &powerio_dist::DistBus,
    terminals: &[String],
    conventions: Option<&serde_json::Value>,
) -> bool {
    let phases: BTreeSet<_> = bus
        .phase_indices(conventions)
        .into_iter()
        .map(|index| bus.terminals[index].as_str())
        .collect();
    terminals.len() == 2
        && terminals
            .iter()
            .all(|terminal| phases.contains(terminal.as_str()))
}

fn branch_base_to_line_neutral(
    configuration: powerio_dist::Configuration,
    phase_to_phase: bool,
    branch_voltage: f64,
) -> Result<f64, String> {
    match configuration {
        powerio_dist::Configuration::Wye => Ok(branch_voltage),
        powerio_dist::Configuration::Delta => Ok(branch_voltage / 3f64.sqrt()),
        powerio_dist::Configuration::SinglePhase if phase_to_phase => {
            Ok(branch_voltage / 3f64.sqrt())
        }
        powerio_dist::Configuration::SinglePhase => Ok(branch_voltage),
        _ => Err("unsupported configuration for nominal-voltage inference".to_owned()),
    }
}

fn stamp_line(
    passive: &mut StampedMatrix,
    line: &powerio_dist::DistLine,
    code: &DistLineCode,
    index: &NetworkIndex,
) -> Result<PreparedElement, String> {
    let n = code.n_conductors;
    if n == 0
        || line.terminal_map_from.len() != n
        || line.terminal_map_to.len() != n
        || line.length <= 0.0
        || !line.length.is_finite()
    {
        return Err(format!(
            "line `{}` has invalid conductor dimensions or length",
            line.name
        ));
    }
    let square = |m: &[Vec<f64>], name: &str| -> Result<(), String> {
        if m.len() != n
            || m.iter()
                .any(|r| r.len() != n || r.iter().any(|v| !v.is_finite()))
        {
            Err(format!(
                "linecode `{}` has invalid {name} matrix",
                code.name
            ))
        } else {
            Ok(())
        }
    };
    square(&code.r_series, "r_series")?;
    square(&code.x_series, "x_series")?;
    square(&code.g_from, "g_from")?;
    square(&code.b_from, "b_from")?;
    square(&code.g_to, "g_to")?;
    square(&code.b_to, "b_to")?;
    // PowerIO's line primitive: the inverse series impedance over the
    // length between the two ends, each end with its shunt half.
    let yprim = powerio_matrix::calc_line_primitive_admittance(line, code)
        .map_err(|error| error.to_string())?;
    let terminals: Vec<_> = line
        .terminal_map_from
        .iter()
        .map(|t| index.resolve((&line.bus_from, t)))
        .chain(
            line.terminal_map_to
                .iter()
                .map(|t| index.resolve((&line.bus_to, t))),
        )
        .collect::<Result<_, _>>()?;
    stamp_local_values(passive, &terminals, &yprim)?;
    Ok(PreparedElement {
        name: line.name.clone(),
        kind: "line".to_owned(),
        terminals,
        yprim,
    })
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
        assert_eq!(index.terminal_ids[0].0, "SourceBus");
        let mut duplicate = powerio_dist::DistBus::default();
        duplicate.id = "sourcebus".into();
        duplicate.terminals = vec!["other".into()];
        network.buses_mut().push(duplicate);
        assert!(NetworkIndex::new(&network)
            .unwrap_err()
            .contains("duplicate bus"));
    }
}
