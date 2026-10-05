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
use std::cell::Cell;
use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

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
    /// Apply the OpenDSS-style bounded-voltage load envelope.
    pub voltage_envelope: bool,
    /// Below this per-unit voltage, loads use their nominal impedance.
    pub v_low_pu: f64,
    /// Lower edge of the load model's normal voltage range.
    pub v_min_pu: f64,
    /// Upper edge of the load model's normal voltage range.
    pub v_max_pu: f64,
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
            voltage_envelope: true,
            v_low_pu: 0.5,
            v_min_pu: 0.85,
            v_max_pu: 1.15,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct McVoltageViolation {
    pub load: String,
    pub branch: usize,
    pub bus: String,
    pub voltage: f64,
    pub nominal_voltage: f64,
    pub voltage_pu: f64,
    pub bound: String,
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
    /// Whether every load branch lies inside the configured normal voltage band.
    pub voltage_valid: bool,
    pub min_voltage_pu: Option<f64>,
    pub max_voltage_pu: Option<f64>,
    pub voltage_violations: Vec<McVoltageViolation>,
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

/// One absolute branch-power override in a retained multiconductor PF session.
///
/// Branches use the prepared load's electrical branch order. This remains
/// unambiguous for explicit-neutral WYE and compact DELTA terminal maps, where
/// a prescribed power does not necessarily correspond to one terminal name.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct McLoadPowerEdit {
    pub load: String,
    pub branch: usize,
    pub p_w: f64,
    pub q_var: f64,
}

/// UI-facing identity and current/base power for one editable load branch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct McLoadBranchState {
    pub load: String,
    pub bus: String,
    pub branch: usize,
    pub p_w: f64,
    pub q_var: f64,
    pub base_p_w: f64,
    pub base_q_var: f64,
}

/// The compact, constant-size view of one converged operating point.
///
/// Scalar fields carry the same names and values as [`McPfResult`]; the
/// aggregates are the sums a results summary displays. It is computed from
/// the converged phasors without building per-terminal or per-port records,
/// so its cost and size do not depend on how the network is displayed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct McPfSummary {
    pub converged: bool,
    /// Whether every load branch lies inside the configured normal voltage band.
    pub voltage_valid: bool,
    pub min_voltage_pu: Option<f64>,
    pub max_voltage_pu: Option<f64>,
    /// Number of load branches outside the normal voltage band.
    pub voltage_violation_count: usize,
    pub iterations: usize,
    pub factorization_count: usize,
    pub matrix_dimension: usize,
    pub matrix_nonzeros: usize,
    pub voltage_change: f64,
    pub physical_kcl_residual: f64,
    pub scaled_kcl_residual: f64,
    pub terminal_count: usize,
    pub element_port_count: usize,
    /// Sum of every source reaction's `V * conj(I_into_network)`, VA.
    pub source_power_into_network: McComplex,
    /// Sum of `V * conj(I_into_element)` over the ports of passive equipment
    /// (lines, shunts, capacitors, transformers), VA.
    pub passive_loss: McComplex,
    /// Solves performed by the producing session; detail views echo it so a
    /// reader can discard values from an older operating point.
    pub solve_count: usize,
}

/// Where one solve spent its time, in milliseconds of the profile clock.
///
/// A phase that did not run is zero. `timed` is false when no clock is
/// available (a WASM build without an injected clock), in which case every
/// duration is zero.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct McPfProfile {
    pub timed: bool,
    /// Parsing the stored module (cold start from a module only).
    pub parse_ms: f64,
    /// Terminal indexing, stamping, and load preparation (cold start only).
    pub prepare_ms: f64,
    /// Sparse assembly and numeric LU factorization (cold start only).
    pub factor_ms: f64,
    /// Load and device current evaluation.
    pub load_evaluation_ms: f64,
    /// Compensated right hand sides and KCL residual products.
    pub kcl_and_matvec_ms: f64,
    /// Retained sparse LU solves.
    pub linear_solve_ms: f64,
    /// Summary aggregation and output finiteness checks.
    pub summary_ms: f64,
    pub total_ms: f64,
    pub iterations: usize,
    pub linear_solves: usize,
}

/// A bounded request for the detailed terminal and equipment values of a
/// session's current operating point.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default)]
pub struct McPfDetailQuery {
    /// Return the terminals of this bus.
    pub bus: Option<String>,
    /// Return only the ports of equipment with this name.
    pub element: Option<String>,
    /// First equipment port to return, in [`McPfResult::element_ports`] order.
    pub port_offset: usize,
    /// Number of equipment ports to return; zero selects the default page.
    pub port_limit: usize,
}

impl McPfDetailQuery {
    pub const DEFAULT_PORT_LIMIT: usize = 20;
    pub const MAX_PORT_LIMIT: usize = 500;
}

/// One page of detailed values, in the same records as [`McPfResult`].
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct McPfDetail {
    pub solve_count: usize,
    pub terminals: Vec<McTerminalResult>,
    pub element_ports: Vec<McElementPort>,
    /// Ports matching the query before paging.
    pub element_port_total: usize,
}

static PROFILE_CLOCK: OnceLock<fn() -> f64> = OnceLock::new();

/// Install the millisecond clock used for solve profiles. Native builds use a
/// monotonic clock by default; a WASM host injects one (for example
/// `performance.now`). Returns false if a clock was already installed.
pub fn set_mc_pf_profile_clock(clock: fn() -> f64) -> bool {
    PROFILE_CLOCK.set(clock).is_ok()
}

pub(crate) fn profile_now() -> Option<f64> {
    match PROFILE_CLOCK.get() {
        Some(clock) => Some(clock()),
        None => default_clock(),
    }
}

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
fn default_clock() -> Option<f64> {
    static EPOCH: OnceLock<std::time::Instant> = OnceLock::new();
    Some(
        EPOCH
            .get_or_init(std::time::Instant::now)
            .elapsed()
            .as_secs_f64()
            * 1_000.0,
    )
}

// `Instant` panics on wasm32-unknown-unknown; profiles stay untimed there
// unless the host installs a clock.
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
fn default_clock() -> Option<f64> {
    None
}

pub(crate) fn elapsed_ms(start: Option<f64>, end: Option<f64>) -> f64 {
    match (start, end) {
        (Some(start), Some(end)) => (end - start).max(0.0),
        _ => 0.0,
    }
}

/// A prepared fixed-point current-injection solve session.
///
/// The passive network, compensation admittance, sparse LU, and last
/// converged voltage are retained. Replacing load powers changes only the
/// prepared physical current laws: the base network and instance are never
/// copied or rebuilt by an ordinary edit, and subsequent solves reuse the
/// exact same factor. The edited network is materialized only when a
/// portable input, snapshot, compatibility accessor, or rebuild is requested.
#[derive(Debug)]
pub struct McPfSession {
    base_instance: McAcPfInstance,
    prepared: PreparedNetwork,
    options: McPfOptions,
    /// The stored module's records (descriptors, diagnostics, history) with
    /// its value detached; the value is supplied again at materialization.
    module: Option<powerio::PioModule<()>>,
    module_holds_network: bool,
    base_power: Vec<Vec<Complex64>>,
    load_index: Result<BTreeMap<String, usize>, String>,
    instance_edit_error: Option<String>,
    state: FixedPoint,
    summary: McPfSummary,
    profile: McPfProfile,
    cold_profile: McPfProfile,
    solve_count: usize,
    materializations: Cell<usize>,
    // The 0.3.0 accessors return borrowed full views. Keep those views lazy so
    // compact callers never allocate or retain them, and discard them only
    // after an edit has successfully committed its new operating point.
    result_cache: OnceLock<McPfResult>,
    instance_cache: OnceLock<McAcPfInstance>,
}

impl McPfSession {
    /// Prepare, factor, and solve the initial operating point once.
    pub fn new(instance: McAcPfInstance, options: McPfOptions) -> Result<Self, String> {
        validate_options(&options)?;
        let started = profile_now();
        // Load edits preserve every identity, so an initial point that can
        // rebind to this network can rebind after any edit. Check once rather
        // than cloning the network on every compact update. Preserve the
        // legacy behavior of accepting an unused, incompatible initial point
        // at construction but rejecting edits that would try to rebind it.
        let instance_edit_error = instance.initial_point().and_then(|_| {
            instance
                .clone()
                .with_network(instance.network().clone())
                .err()
                .map(|error| error.to_string())
        });
        let (prepared, timing) = PreparedNetwork::prepare_timed(&instance)?;
        let base_power = prepared
            .loads
            .iter()
            .map(|load| load.power.clone())
            .collect();
        let mut load_index = BTreeMap::new();
        let mut duplicate = None;
        for (index, load) in prepared.loads.iter().enumerate() {
            if load_index.insert(load.name.clone(), index).is_some() && duplicate.is_none() {
                duplicate = Some(format!("duplicate prepared load identity `{}`", load.name));
            }
        }
        let mut profile = McPfProfile {
            prepare_ms: timing.assemble_ms,
            factor_ms: timing.factor_ms,
            ..McPfProfile::default()
        };
        let state = iterate(&prepared, &options, None, &mut profile)?;
        let summarized = profile_now();
        let summary = summarize(&prepared, &options, &state, 1)?;
        let finished = profile_now();
        profile.summary_ms = elapsed_ms(summarized, finished);
        profile.total_ms = elapsed_ms(started, finished);
        profile.timed = started.is_some();
        Ok(Self {
            base_instance: instance,
            prepared,
            options,
            module: None,
            module_holds_network: false,
            base_power,
            load_index: match duplicate {
                Some(error) => Err(error),
                None => Ok(load_index),
            },
            instance_edit_error,
            state,
            summary,
            profile,
            cold_profile: profile,
            solve_count: 1,
            materializations: Cell::new(0),
            result_cache: OnceLock::new(),
            instance_cache: OnceLock::new(),
        })
    }

    /// Parse a stored PowerIO multiconductor module and create a retained session.
    pub fn from_module_json(module_json: &str, options: McPfOptions) -> Result<Self, String> {
        let started = profile_now();
        let mut module = crate::ir::deserialize_module(module_json)?;
        input::validate_mc_module(&module)?;
        let module_holds_network =
            matches!(module.value(), powerio::PioValue::MulticonductorNetwork(_));
        let instance = input::instance_from_value(module.value())?;
        let parsed = profile_now();
        let mut session = Self::new(instance, options)?;
        // Every materialization replaces the value, which releases the
        // retained source bytes and value source map. Do that once here so
        // the session does not hold a second copy of the stored document.
        let _ = module.value_mut();
        session.module = Some(module.map_value(|_| ()));
        session.module_holds_network = module_holds_network;
        session.cold_profile.parse_ms = elapsed_ms(started, parsed);
        session.cold_profile.total_ms += session.cold_profile.parse_ms;
        session.profile = session.cold_profile;
        Ok(session)
    }

    /// The complete result for the current operating point.
    ///
    /// This compatibility view is built on first use and retained until the
    /// next successful load edit. Use [`Self::summary`] or [`Self::detail`]
    /// for compact output, or [`Self::build_result`] for an owned full result
    /// that is not retained by the session.
    pub fn result(&self) -> &McPfResult {
        self.result_cache.get_or_init(|| {
            // `summarize` applies the same output checks before a fixed point
            // can be committed, and the prepared state cannot change here.
            self.build_result()
                .expect("accepted operating point has a valid full result")
        })
    }

    /// The current instance, including load edits.
    ///
    /// This compatibility view is materialized on first use and retained
    /// until the next successful load edit. Use [`Self::base_instance`] for
    /// the unedited input, or [`Self::edited_instance`] for an owned view that
    /// is not retained by the session.
    pub fn instance(&self) -> &McAcPfInstance {
        self.instance_cache.get_or_init(|| {
            self.edited_instance()
                .expect("accepted load powers preserve a valid instance")
        })
    }

    /// The compact summary of the current operating point.
    pub fn summary(&self) -> &McPfSummary {
        &self.summary
    }

    /// Build the complete terminal and equipment result for the current
    /// operating point. This is materialized on demand and not retained.
    pub fn build_result(&self) -> Result<McPfResult, String> {
        build_result(&self.prepared, &self.options, &self.state)
    }

    /// The instance this session was prepared from, without load edits.
    pub fn base_instance(&self) -> &McAcPfInstance {
        &self.base_instance
    }

    pub fn options(&self) -> McPfOptions {
        self.options
    }

    pub fn solve_count(&self) -> usize {
        self.solve_count
    }

    /// Phase timings of the most recent solve.
    pub fn profile(&self) -> McPfProfile {
        self.profile
    }

    /// Phase timings of the initial parse, preparation, factorization, and solve.
    pub fn cold_profile(&self) -> McPfProfile {
        self.cold_profile
    }

    /// Edited networks built for portable output since the session started.
    /// Ordinary load edits never build one.
    pub fn materialization_count(&self) -> usize {
        self.materializations.get()
    }

    /// Total numeric factorizations performed by this session.
    pub fn factorization_count(&self) -> usize {
        self.prepared
            .factor
            .as_ref()
            .map_or(0, |factor| factor.factorization_count())
    }

    pub fn load_branches(&self) -> Vec<McLoadBranchState> {
        self.prepared
            .loads
            .iter()
            .zip(&self.base_power)
            .flat_map(|(load, base)| {
                load.power
                    .iter()
                    .zip(base)
                    .enumerate()
                    .map(|(branch, (&power, &base_power))| McLoadBranchState {
                        load: load.name.clone(),
                        bus: load.bus.clone(),
                        branch,
                        p_w: power.re,
                        q_var: power.im,
                        base_p_w: base_power.re,
                        base_q_var: base_power.im,
                    })
            })
            .collect()
    }

    /// Terminal identities in calculation order, which is also the order of
    /// [`McPfResult::terminals`] and of the interleaved terminal arrays.
    pub fn terminal_ids(&self) -> &[(String, String)] {
        &self.prepared.index.terminal_ids
    }

    /// Terminal voltages as interleaved `[re, im]` pairs, volts.
    pub fn terminal_voltages(&self) -> Vec<f64> {
        self.state
            .voltage
            .iter()
            .flat_map(|value| [value.re, value.im])
            .collect()
    }

    /// Currents injected into the network as interleaved `[re, im]` pairs, amperes.
    pub fn terminal_currents(&self) -> Vec<f64> {
        (0..self.state.voltage.len())
            .flat_map(|row| {
                let current = passive_current(&self.prepared, &self.state.voltage, row);
                [current.re, current.im]
            })
            .collect()
    }

    /// One bounded page of terminal and equipment detail.
    pub fn detail(&self, query: &McPfDetailQuery) -> Result<McPfDetail, String> {
        let limit = match query.port_limit {
            0 => McPfDetailQuery::DEFAULT_PORT_LIMIT,
            limit if limit > McPfDetailQuery::MAX_PORT_LIMIT => {
                return Err(format!(
                    "detail pages hold at most {} equipment ports",
                    McPfDetailQuery::MAX_PORT_LIMIT
                ))
            }
            limit => limit,
        };
        let voltage = &self.state.voltage;
        let prepared = &self.prepared;
        let terminals = match &query.bus {
            Some(bus) => prepared
                .index
                .bus_terminals(bus)
                .into_iter()
                .map(|i| terminal_result(prepared, voltage, i))
                .collect(),
            None => Vec::new(),
        };
        let wanted = |name: &str| {
            query
                .element
                .as_deref()
                .is_none_or(|element| element == name)
        };
        let end = query.port_offset.saturating_add(limit);
        let mut total = 0usize;
        let mut element_ports = Vec::new();
        for load in &prepared.loads {
            if !wanted(&load.name) {
                continue;
            }
            let count: usize = load.incidence.iter().map(Vec::len).sum();
            if total + count > query.port_offset && total < end {
                let mut position = total;
                for (branch, incidence) in load.incidence.iter().enumerate() {
                    let current = load.branch_current(branch, voltage, &self.options)?;
                    for &(i, c) in incidence {
                        if (query.port_offset..end).contains(&position) {
                            element_ports
                                .push(load_port(prepared, voltage, load, branch, i, c, current));
                        }
                        position += 1;
                    }
                }
            }
            total += count;
        }
        for element in &prepared.elements {
            if !wanted(&element.name) {
                continue;
            }
            let count = element.terminals.len();
            if total + count > query.port_offset && total < end {
                for row in 0..count {
                    if (query.port_offset..end).contains(&(total + row)) {
                        element_ports.push(passive_port(prepared, voltage, element, row));
                    }
                }
            }
            total += count;
        }
        Ok(McPfDetail {
            solve_count: self.solve_count,
            terminals,
            element_ports,
            element_port_total: total,
        })
    }

    /// Materialize the current edited operating point as a typed instance.
    ///
    /// This is the only place an edited network is built. It runs for
    /// portable output (input module, snapshot), the compatibility instance
    /// accessor, or an explicit rebuild, never
    /// for an ordinary load edit.
    pub fn edited_instance(&self) -> Result<McAcPfInstance, String> {
        let edited = self
            .prepared
            .loads
            .iter()
            .zip(&self.base_power)
            .any(|(load, base)| load.power != *base);
        if !edited {
            return Ok(self.base_instance.clone());
        }
        let mut network = self.base_instance.network().clone();
        let mut row_index = BTreeMap::new();
        for (index, row) in network.loads().iter().enumerate() {
            row_index.entry(row.name.as_str()).or_insert(index);
        }
        let row_positions = self
            .prepared
            .loads
            .iter()
            .map(|load| {
                row_index
                    .get(load.name.as_str())
                    .copied()
                    .ok_or_else(|| format!("load `{}` is absent from its network", load.name))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let rows = network.loads_mut();
        for (position, load) in row_positions.into_iter().zip(&self.prepared.loads) {
            let row = &mut rows[position];
            row.p_nom = load.power.iter().map(|value| value.re).collect();
            row.q_nom = load.power.iter().map(|value| value.im).collect();
        }
        self.materializations.set(self.materializations.get() + 1);
        self.base_instance
            .clone()
            .with_network(network)
            .map_err(|e| e.to_string())
    }

    /// Materialize the current edited operating point as portable PowerIO IR.
    pub fn input_module_json(&self) -> Result<String, String> {
        self.module_json_for(self.edited_instance()?)
    }

    fn module_json_for(&self, instance: McAcPfInstance) -> Result<String, String> {
        let value = if self.module_holds_network {
            powerio::PioValue::MulticonductorNetwork(instance.network().clone())
        } else {
            powerio::PioValue::McAcPfInstance(instance)
        };
        let module = match &self.module {
            Some(records) => records.clone().map_value(|()| value),
            None => powerio::PioModule::new(value),
        };
        crate::ir::serialize_module(&module)
    }

    /// Build a portable saved Study at the current edited operating point
    /// without preparing, factoring, or solving again.
    pub fn snapshot(
        &self,
        id: impl Into<String>,
        title: impl Into<String>,
    ) -> Result<McStudySnapshot, String> {
        let id = id.into();
        let title = title.into();
        if id.trim().is_empty() || title.trim().is_empty() {
            return Err("multiconductor Study snapshot requires an id and title".to_owned());
        }
        // The input module and the solution must embed the same edited
        // instance; replay validation compares them.
        let instance = self.edited_instance()?;
        let result = self.build_result()?;
        let solution = result.to_powerio_solution(&instance)?;
        let solution_module = crate::ir::serialize_module(&powerio::PioModule::new(
            powerio::PioValue::McAcPfSolution(solution),
        ))?;
        Ok(McStudySnapshot {
            schema: McStudySnapshot::SCHEMA.to_owned(),
            version: McStudySnapshot::VERSION,
            id,
            title,
            formulation: "mc_ac_pf".to_owned(),
            input_module: self.module_json_for(instance)?,
            solution_module,
            options: self.options,
            result,
        })
    }

    /// Replace the absolute edit set, re-solve from the last converged voltage,
    /// and retain the previous operating point if validation or convergence fails.
    /// An empty edit set restores the base load powers.
    ///
    /// Returns the complete result, as in 0.3.0. For interactive updates that
    /// do not materialize a full result, use [`Self::replace_load_powers_summary`].
    pub fn replace_load_powers(
        &mut self,
        edits: &[McLoadPowerEdit],
    ) -> Result<&McPfResult, String> {
        self.replace_load_powers_summary(edits)?;
        Ok(self.result())
    }

    /// Replace the absolute edit set and return a compact summary without
    /// materializing or retaining a complete result or an edited network.
    ///
    /// Re-solves from the last converged voltage and retains the previous
    /// operating point if validation or convergence fails. An empty edit set
    /// restores the base load powers.
    pub fn replace_load_powers_summary(
        &mut self,
        edits: &[McLoadPowerEdit],
    ) -> Result<&McPfSummary, String> {
        let started = profile_now();
        let load_index = self.load_index.as_ref().map_err(Clone::clone)?;
        if let Some(error) = &self.instance_edit_error {
            return Err(error.clone());
        }
        let mut next_power = self.base_power.clone();
        let mut seen = std::collections::BTreeSet::new();
        for edit in edits {
            if edit.load.trim().is_empty() || !edit.p_w.is_finite() || !edit.q_var.is_finite() {
                return Err("load-power edits require a name and finite P/Q values".to_owned());
            }
            let Some(&load) = load_index.get(&edit.load) else {
                return Err(format!("unknown load `{}`", edit.load));
            };
            if edit.branch >= next_power[load].len() {
                return Err(format!(
                    "load `{}` has no branch {}; expected 0..{}",
                    edit.load,
                    edit.branch,
                    next_power[load].len()
                ));
            }
            if !seen.insert((load, edit.branch)) {
                return Err(format!(
                    "load `{}` branch {} is edited more than once",
                    edit.load, edit.branch
                ));
            }
            next_power[load][edit.branch] = Complex64::new(edit.p_w, edit.q_var);
        }

        // Only the prepared current laws change. A load whose power is
        // unchanged keeps its law (and nominal admittance) exactly.
        let mut previous = Vec::new();
        let mut applied = Ok(());
        for (index, (load, power)) in self.prepared.loads.iter_mut().zip(next_power).enumerate() {
            if load.power == power {
                continue;
            }
            let old = load.power.clone();
            if let Err(error) = load.replace_power(power) {
                applied = Err(error);
                break;
            }
            previous.push((index, old));
        }
        let mut profile = McPfProfile::default();
        let solved = applied
            .and_then(|()| {
                iterate(
                    &self.prepared,
                    &self.options,
                    Some(&self.state.voltage),
                    &mut profile,
                )
            })
            .and_then(|state| {
                let summarized = profile_now();
                let summary =
                    summarize(&self.prepared, &self.options, &state, self.solve_count + 1)?;
                profile.summary_ms = elapsed_ms(summarized, profile_now());
                Ok((state, summary))
            });
        let (state, summary) = match solved {
            Ok(solved) => solved,
            Err(error) => {
                for (index, power) in previous {
                    self.prepared.loads[index]
                        .replace_power(power)
                        .expect("previous prepared load power is valid");
                }
                return Err(error);
            }
        };
        self.result_cache.take();
        self.instance_cache.take();
        self.state = state;
        self.summary = summary;
        self.solve_count += 1;
        profile.total_ms = elapsed_ms(started, profile_now());
        profile.timed = started.is_some();
        self.profile = profile;
        Ok(&self.summary)
    }
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

    /// Attach equipment positions to both saved modules without recalculating electrical results.
    pub fn apply_geo_layer(&mut self, layer: &powerio::GeoLayer) -> Result<(), String> {
        self.validate()?;
        let mut input = crate::ir::deserialize_module(&self.input_module)?;
        let instance = input::instance_from_value(input.value())?;
        let mut network = instance.network().clone();
        let report = powerio::dist_geo::apply_dist_geo_layer(&mut network, layer);
        if report.matched_buses == 0 && report.matched_branches == 0 {
            return Err("no saved case equipment matched the geographic file".to_owned());
        }
        let instance = instance
            .with_network(network.clone())
            .map_err(|e| e.to_string())?;
        *input.value_mut() = match input.value() {
            powerio::PioValue::MulticonductorNetwork(_) => {
                powerio::PioValue::MulticonductorNetwork(network)
            }
            powerio::PioValue::McAcPfInstance(_) => {
                powerio::PioValue::McAcPfInstance(instance.clone())
            }
            _ => {
                return Err("saved power flow requires a network or AC power flow input".to_owned())
            }
        };
        let mut solution = crate::ir::deserialize_module(&self.solution_module)?;
        *solution.value_mut() =
            powerio::PioValue::McAcPfSolution(self.result.to_powerio_solution(&instance)?);
        let mut updated = self.clone();
        updated.input_module = crate::ir::serialize_module(&input)?;
        updated.solution_module = crate::ir::serialize_module(&solution)?;
        updated.validate()?;
        *self = updated;
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
        || result.voltage_valid != result.voltage_violations.is_empty()
        || result
            .min_voltage_pu
            .is_some_and(|value| !value.is_finite() || value < 0.0)
        || result
            .max_voltage_pu
            .is_some_and(|value| !value.is_finite() || value < 0.0)
        || result.min_voltage_pu.is_some() != result.max_voltage_pu.is_some()
        || result
            .min_voltage_pu
            .zip(result.max_voltage_pu)
            .is_some_and(|(min, max)| min > max)
        || result.voltage_violations.iter().any(|violation| {
            !violation.voltage.is_finite()
                || !violation.nominal_voltage.is_finite()
                || !violation.voltage_pu.is_finite()
                || violation.voltage < 0.0
                || violation.nominal_voltage <= 0.0
                || !matches!(violation.bound.as_str(), "minimum" | "maximum")
        })
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
    let prepared = PreparedNetwork::prepare(solution.instance())?;
    let prepared_voltage = prepared
        .index
        .terminal_ids
        .iter()
        .map(|(bus, terminal)| {
            terminal_voltages
                .get(&(bus.clone(), terminal.clone()))
                .copied()
                .map(McComplexValue::into_complex)
                .ok_or_else(|| {
                    format!("multiconductor Study result omits terminal `{bus}:{terminal}`")
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let (expected_min, expected_max, expected_violations) =
        assess_load_voltages(&prepared, &prepared_voltage, options);
    if result.min_voltage_pu != expected_min
        || result.max_voltage_pu != expected_max
        || result.voltage_violations != expected_violations
        || result.voltage_valid != expected_violations.is_empty()
    {
        return Err(
            "multiconductor Study voltage-band assessment differs from its terminal voltages"
                .to_owned(),
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
    let mut load_rows = BTreeMap::<&str, &powerio_dist::DistLoad>::new();
    for load in network.loads() {
        load_rows.entry(load.name.as_str()).or_insert(load);
    }
    for load_instance in instance.loads() {
        let load = load_rows
            .get(load_instance.load.as_str())
            .copied()
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
    let state = iterate(&prepared, options, None, &mut McPfProfile::default())?;
    build_result(&prepared, options, &state)
}

/// A converged fixed point: the phasors and the convergence evidence that
/// every output view is derived from.
#[derive(Clone, Debug)]
struct FixedPoint {
    voltage: Vec<Complex64>,
    load_current: Vec<Complex64>,
    iterations: usize,
    voltage_change: f64,
    physical_kcl_residual: f64,
    scaled_kcl_residual: f64,
}

/// Run the fixed-point current-injection iteration on a prepared network.
///
/// All iteration buffers are allocated once per call. The final load
/// currents and KCL metrics are those of the accepting convergence check,
/// which ran at exactly the returned voltage.
fn iterate(
    prepared: &PreparedNetwork,
    options: &McPfOptions,
    initial_voltage: Option<&[Complex64]>,
    profile: &mut McPfProfile,
) -> Result<FixedPoint, String> {
    validate_options(options)?;
    let n = prepared.index.terminal_ids.len();
    let mut voltage = if let Some(initial) = initial_voltage {
        if initial.len() != n {
            return Err(format!(
                "initial voltage has {} terminals; expected {n}",
                initial.len(),
            ));
        }
        if initial
            .iter()
            .any(|value| !value.re.is_finite() || !value.im.is_finite())
        {
            return Err("initial voltage contains a non-finite phasor".to_owned());
        }
        initial.to_vec()
    } else {
        prepared.fixed_voltage_vector()
    };
    for (i, fixed) in prepared.fixed.iter().enumerate() {
        if let Some(value) = fixed {
            voltage[i] = *value;
        }
    }
    let base_rhs = &prepared.source_rhs;
    let mut solved = vec![Complex64::default(); prepared.unknown.len()];
    if initial_voltage.is_none() {
        if let Some(factor) = &prepared.factor {
            let started = profile_now();
            factor.solve_into(base_rhs, &mut solved)?;
            profile.linear_solve_ms += elapsed_ms(started, profile_now());
            profile.linear_solves += 1;
            for (u, &i) in prepared.unknown.iter().enumerate() {
                voltage[i] = solved[u];
            }
        }
    }
    let mut load_current = vec![Complex64::default(); n];
    let mut compensated = vec![Complex64::default(); prepared.unknown.len()];
    let mut scratch = KclScratch::new(n);
    // KCL metrics of `load_current`, when both describe the current voltage.
    let mut evaluated: Option<(f64, f64)> = None;
    let mut final_change = f64::INFINITY;
    let mut iterations = 0;
    for k in 0..options.max_iterations {
        iterations = k + 1;
        if evaluated.is_none() {
            let started = profile_now();
            total_load_current_into(prepared, &voltage, options, &mut load_current)?;
            profile.load_evaluation_ms += elapsed_ms(started, profile_now());
        }
        let started = profile_now();
        for (u, &i) in prepared.unknown.iter().enumerate() {
            let yv = prepared.yref.row_mul(i, &voltage);
            compensated[u] = base_rhs[prepared.unknown_pos[i]] + yv - load_current[i];
        }
        profile.kcl_and_matvec_ms += elapsed_ms(started, profile_now());
        if let Some(factor) = &prepared.factor {
            let started = profile_now();
            factor.solve_into(&compensated, &mut solved)?;
            profile.linear_solve_ms += elapsed_ms(started, profile_now());
            profile.linear_solves += 1;
        }
        final_change = 0.0;
        for (u, &i) in prepared.unknown.iter().enumerate() {
            let next = voltage[i] + options.damping * (solved[u] - voltage[i]);
            final_change = final_change.max((next - voltage[i]).norm());
            voltage[i] = next;
        }
        evaluated = None;
        if final_change <= options.tolerance {
            let started = profile_now();
            total_load_current_into(prepared, &voltage, options, &mut load_current)?;
            let loaded = profile_now();
            let metrics = kcl_metrics(prepared, &voltage, &load_current, options, &mut scratch);
            profile.load_evaluation_ms += elapsed_ms(started, loaded);
            profile.kcl_and_matvec_ms += elapsed_ms(loaded, profile_now());
            evaluated = Some(metrics);
            if metrics.1 <= 1.0 {
                break;
            }
        }
    }
    let (residual, scaled_residual) = match evaluated {
        Some(metrics) => metrics,
        None => {
            total_load_current_into(prepared, &voltage, options, &mut load_current)?;
            kcl_metrics(prepared, &voltage, &load_current, options, &mut scratch)
        }
    };
    profile.iterations = iterations;
    if !final_change.is_finite() || !residual.is_finite() || !scaled_residual.is_finite() {
        return Err("fixed-point iteration produced a non-finite residual".to_owned());
    }
    if final_change > options.tolerance || scaled_residual > 1.0 {
        return Err(format!("multiconductor PF did not converge after {iterations} iterations (voltage change {final_change:.3e}, KCL residual {residual:.3e})"));
    }
    Ok(FixedPoint {
        voltage,
        load_current,
        iterations,
        voltage_change: final_change,
        physical_kcl_residual: residual,
        scaled_kcl_residual: scaled_residual,
    })
}

const NON_FINITE_OUTPUT: &str =
    "fixed-point PF produced a non-finite voltage, current, or power output";

/// Reduce a converged fixed point to its compact summary.
///
/// This applies the same output finiteness gate as [`build_result`] to every
/// terminal, equipment port, and source reaction, numerically and without
/// building records, so a session commits an operating point under exactly
/// the conditions a full solve would accept it.
fn summarize(
    prepared: &PreparedNetwork,
    options: &McPfOptions,
    state: &FixedPoint,
    solve_count: usize,
) -> Result<McPfSummary, String> {
    let voltage = &state.voltage;
    let mut finite = true;
    for (i, &v) in voltage.iter().enumerate() {
        let current = passive_current(prepared, voltage, i);
        finite &= phasors_finite(&[v, current, v * current.conj()]);
    }
    let mut element_port_count = 0usize;
    for load in &prepared.loads {
        for (branch, incidence) in load.incidence.iter().enumerate() {
            let current = load.branch_current(branch, voltage, options)?;
            for &(i, c) in incidence {
                let terminal_current = c.conj() * current;
                finite &= phasors_finite(&[terminal_current, voltage[i] * terminal_current.conj()]);
                element_port_count += 1;
            }
        }
    }
    // Summed in port order, matching a sum over `McPfResult::element_ports`.
    let mut passive_loss = Complex64::default();
    let mut port_voltages = Vec::new();
    for element in &prepared.elements {
        port_voltages.clear();
        port_voltages.extend(element.terminals.iter().map(|&i| voltage[i]));
        for (row, &i) in element.terminals.iter().enumerate() {
            let current = element_port_current(element, row, &port_voltages);
            let power = voltage[i] * current.conj();
            finite &= phasors_finite(&[current, power]);
            passive_loss.re += power.re;
            passive_loss.im += power.im;
            element_port_count += 1;
        }
    }
    let mut source_power = Complex64::default();
    for source in &prepared.source_terminals {
        let i = source.index;
        let current = passive_current(prepared, voltage, i) + state.load_current[i];
        let power = voltage[i] * current.conj();
        finite &= phasors_finite(&[current, power]);
        source_power.re += power.re;
        source_power.im += power.im;
    }
    if !finite {
        return Err(NON_FINITE_OUTPUT.to_owned());
    }
    let mut minimum: Option<f64> = None;
    let mut maximum: Option<f64> = None;
    let mut violations = 0usize;
    visit_load_voltages(prepared, voltage, options, |_, _, _, _, pu, bound| {
        minimum = Some(minimum.map_or(pu, |value| value.min(pu)));
        maximum = Some(maximum.map_or(pu, |value| value.max(pu)));
        violations += usize::from(bound.is_some());
    });
    Ok(McPfSummary {
        converged: true,
        voltage_valid: violations == 0,
        min_voltage_pu: minimum,
        max_voltage_pu: maximum,
        voltage_violation_count: violations,
        iterations: state.iterations,
        factorization_count: prepared
            .factor
            .as_ref()
            .map_or(0, |factor| factor.factorization_count()),
        matrix_dimension: prepared.factor.as_ref().map_or(0, |factor| factor.dim()),
        matrix_nonzeros: prepared
            .factor
            .as_ref()
            .map_or(0, |factor| factor.nonzeros()),
        voltage_change: state.voltage_change,
        physical_kcl_residual: state.physical_kcl_residual,
        scaled_kcl_residual: state.scaled_kcl_residual,
        terminal_count: voltage.len(),
        element_port_count,
        source_power_into_network: source_power.into(),
        passive_loss: passive_loss.into(),
        solve_count,
    })
}

/// Build the complete terminal and equipment result for a fixed point.
fn build_result(
    prepared: &PreparedNetwork,
    options: &McPfOptions,
    state: &FixedPoint,
) -> Result<McPfResult, String> {
    let voltage = &state.voltage;
    let terminals: Vec<_> = (0..prepared.index.terminal_ids.len())
        .map(|i| terminal_result(prepared, voltage, i))
        .collect();
    let mut element_ports = Vec::new();
    for load in &prepared.loads {
        for (branch, incidence) in load.incidence.iter().enumerate() {
            let current = load.branch_current(branch, voltage, options)?;
            for &(i, c) in incidence {
                element_ports.push(load_port(prepared, voltage, load, branch, i, c, current));
            }
        }
    }
    for element in &prepared.elements {
        for row in 0..element.terminals.len() {
            element_ports.push(passive_port(prepared, voltage, element, row));
        }
    }
    let source_reactions: Vec<_> = prepared
        .source_terminals
        .iter()
        .map(|source| {
            let i = source.index;
            let current = passive_current(prepared, voltage, i) + state.load_current[i];
            McSourceReaction {
                source: source.source.clone(),
                terminal: source.terminal.clone(),
                current_into_network: current.into(),
                power_into_network: (voltage[i] * current.conj()).into(),
            }
        })
        .collect();
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
        return Err(NON_FINITE_OUTPUT.to_owned());
    }
    let (min_voltage_pu, max_voltage_pu, voltage_violations) =
        assess_load_voltages(prepared, voltage, options);
    Ok(McPfResult {
        converged: true,
        voltage_valid: voltage_violations.is_empty(),
        min_voltage_pu,
        max_voltage_pu,
        voltage_violations,
        iterations: state.iterations,
        factorization_count: prepared
            .factor
            .as_ref()
            .map_or(0, |factor| factor.factorization_count()),
        matrix_dimension: prepared.factor.as_ref().map_or(0, |factor| factor.dim()),
        matrix_nonzeros: prepared
            .factor
            .as_ref()
            .map_or(0, |factor| factor.nonzeros()),
        voltage_change: state.voltage_change,
        physical_kcl_residual: state.physical_kcl_residual,
        scaled_kcl_residual: state.scaled_kcl_residual,
        terminals,
        element_ports,
        source_reactions,
    })
}

fn terminal_result(
    prepared: &PreparedNetwork,
    voltage: &[Complex64],
    i: usize,
) -> McTerminalResult {
    let current = passive_current(prepared, voltage, i);
    let (bus, terminal) = &prepared.index.terminal_ids[i];
    McTerminalResult {
        bus: bus.clone(),
        terminal: terminal.clone(),
        voltage: voltage[i].into(),
        power_into_network: (voltage[i] * current.conj()).into(),
        current_into_network: current.into(),
    }
}

fn load_port(
    prepared: &PreparedNetwork,
    voltage: &[Complex64],
    load: &loads::BranchLoad,
    branch: usize,
    i: usize,
    c: Complex64,
    branch_current: Complex64,
) -> McElementPort {
    let terminal_current = c.conj() * branch_current;
    let (bus, terminal) = &prepared.index.terminal_ids[i];
    McElementPort {
        element: load.name.clone(),
        kind: "load".to_owned(),
        branch,
        bus: bus.clone(),
        terminal: terminal.clone(),
        current_into_element: terminal_current.into(),
        power_into_element: (voltage[i] * terminal_current.conj()).into(),
    }
}

fn passive_port(
    prepared: &PreparedNetwork,
    voltage: &[Complex64],
    element: &network::PreparedElement,
    row: usize,
) -> McElementPort {
    let port_voltages: Vec<_> = element.terminals.iter().map(|&i| voltage[i]).collect();
    let current = element_port_current(element, row, &port_voltages);
    let i = element.terminals[row];
    let (bus, terminal) = &prepared.index.terminal_ids[i];
    McElementPort {
        element: element.name.clone(),
        kind: element.kind.clone(),
        branch: 0,
        bus: bus.clone(),
        terminal: terminal.clone(),
        current_into_element: current.into(),
        power_into_element: (voltage[i] * current.conj()).into(),
    }
}

fn element_port_current(
    element: &network::PreparedElement,
    row: usize,
    port_voltages: &[Complex64],
) -> Complex64 {
    element.yprim[row]
        .iter()
        .zip(port_voltages)
        .map(|(&y, &v)| y * v)
        .sum()
}

fn phasors_finite(values: &[Complex64]) -> bool {
    values
        .iter()
        .all(|value| value.re.is_finite() && value.im.is_finite())
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
        || !options.v_low_pu.is_finite()
        || !options.v_min_pu.is_finite()
        || !options.v_max_pu.is_finite()
        || options.v_low_pu <= 0.0
        || options.v_low_pu >= options.v_min_pu
        || options.v_min_pu >= options.v_max_pu
    {
        return Err("invalid multiconductor PF options".to_owned());
    }
    Ok(())
}

fn passive_current(network: &PreparedNetwork, voltage: &[Complex64], row: usize) -> Complex64 {
    network.passive.row_mul(row, voltage)
}

/// Buffers reused by every KCL evaluation of one solve.
struct KclScratch {
    incident: Vec<f64>,
    port_voltages: Vec<Complex64>,
}

impl KclScratch {
    fn new(n: usize) -> Self {
        Self {
            incident: vec![0.0; n],
            port_voltages: Vec::new(),
        }
    }
}

fn kcl_metrics(
    network: &PreparedNetwork,
    voltage: &[Complex64],
    load: &[Complex64],
    options: &McPfOptions,
    scratch: &mut KclScratch,
) -> (f64, f64) {
    let incident = &mut scratch.incident;
    incident.fill(0.0);
    for element in &network.elements {
        scratch.port_voltages.clear();
        scratch
            .port_voltages
            .extend(element.terminals.iter().map(|&i| voltage[i]));
        for (row, &terminal) in element.terminals.iter().enumerate() {
            incident[terminal] += element_port_current(element, row, &scratch.port_voltages).norm();
        }
    }
    for load in &network.loads {
        for branch in 0..load.power.len() {
            let current = match load.branch_current(branch, voltage, options) {
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

fn total_load_current_into(
    network: &PreparedNetwork,
    voltage: &[Complex64],
    options: &McPfOptions,
    current: &mut [Complex64],
) -> Result<(), String> {
    current.fill(Complex64::new(0.0, 0.0));
    for load in &network.loads {
        load.add_current(voltage, options, current)?;
    }
    Ok(())
}

/// Visit every load branch's voltage-band assessment in load and branch
/// order. The summary and the full violation list share this arithmetic.
fn visit_load_voltages(
    network: &PreparedNetwork,
    voltage: &[Complex64],
    options: &McPfOptions,
    mut visit: impl FnMut(&loads::BranchLoad, usize, f64, f64, f64, Option<&'static str>),
) {
    for load in &network.loads {
        for branch in 0..load.power.len() {
            let magnitude = load.branch_voltage(branch, voltage).norm();
            let nominal = load.nominal_voltage[branch];
            let pu = magnitude / nominal;
            let bound = if pu < options.v_min_pu {
                Some("minimum")
            } else if pu > options.v_max_pu {
                Some("maximum")
            } else {
                None
            };
            visit(load, branch, magnitude, nominal, pu, bound);
        }
    }
}

fn assess_load_voltages(
    network: &PreparedNetwork,
    voltage: &[Complex64],
    options: &McPfOptions,
) -> (Option<f64>, Option<f64>, Vec<McVoltageViolation>) {
    let mut minimum: Option<f64> = None;
    let mut maximum: Option<f64> = None;
    let mut violations = Vec::new();
    visit_load_voltages(
        network,
        voltage,
        options,
        |load, branch, magnitude, nominal, pu, bound| {
            minimum = Some(minimum.map_or(pu, |value| value.min(pu)));
            maximum = Some(maximum.map_or(pu, |value| value.max(pu)));
            if let Some(bound) = bound {
                violations.push(McVoltageViolation {
                    load: load.name.clone(),
                    branch,
                    bus: load.bus.clone(),
                    voltage: magnitude,
                    nominal_voltage: nominal,
                    voltage_pu: pu,
                    bound: bound.to_owned(),
                });
            }
        },
    );
    (minimum, maximum, violations)
}

#[cfg(test)]
mod tests {
    use super::*;
    use powerio_dist::{
        Configuration, DistBus, DistLine, DistLineCode, DistLoad, DistLoadVoltageModel,
        DistTransformer, DistWinding, DistWindingConn, MulticonductorNetwork, VoltageSource,
    };
    use powerio_prob::McAcPfInstance;

    fn raw_load_options() -> McPfOptions {
        McPfOptions {
            voltage_envelope: false,
            ..Default::default()
        }
    }

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
        assert!(result.voltage_valid);
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

    fn load_bus_voltage(result: &McPfResult) -> Complex64 {
        result
            .terminals
            .iter()
            .find(|terminal| terminal.bus == "load" && terminal.terminal == "1")
            .map(|terminal| terminal.voltage.into_complex())
            .expect("load terminal")
    }

    #[test]
    fn legacy_session_accessors_materialize_only_on_request() {
        let mut session = McPfSession::new(one_phase(1.0), McPfOptions::default()).unwrap();
        assert!(session.result_cache.get().is_none());
        assert!(session.instance_cache.get().is_none());

        // Keep the exact 0.3.0 borrowed signatures usable by downstream code.
        let result: &McPfResult = session.result();
        assert!(std::ptr::eq(result, session.result()));
        let instance: &McAcPfInstance = session.instance();
        assert!(std::ptr::eq(instance, session.instance()));
        assert_eq!(instance.loads()[0].p_w, vec![1.0]);
        assert_eq!(session.materialization_count(), 0);

        session
            .replace_load_powers_summary(&[McLoadPowerEdit {
                load: "pl".into(),
                branch: 0,
                p_w: 1.2,
                q_var: 0.1,
            }])
            .unwrap();
        assert!(session.result_cache.get().is_none());
        assert!(session.instance_cache.get().is_none());
        assert_eq!(session.materialization_count(), 0);
        assert_eq!(
            serde_json::to_value(session.result()).unwrap(),
            serde_json::to_value(session.build_result().unwrap()).unwrap()
        );
        assert_eq!(session.instance().loads()[0].p_w, vec![1.2]);
        assert_eq!(session.instance().loads()[0].q_var, vec![0.1]);
        assert_eq!(session.materialization_count(), 1);
        // Repeated borrowed reads use the same edited network.
        assert!(std::ptr::eq(session.instance(), session.instance()));
        assert_eq!(session.materialization_count(), 1);
        assert_eq!(session.base_instance().loads()[0].p_w, vec![1.0]);
        assert_eq!(session.factorization_count(), 1);
    }

    #[test]
    fn legacy_session_updates_return_full_results_and_reset_cached_views() {
        let mut session = McPfSession::new(one_phase(1.0), McPfOptions::default()).unwrap();
        let base = session.result().clone();
        let _ = session.instance();
        let edited: &McPfResult = session
            .replace_load_powers(&[McLoadPowerEdit {
                load: "pl".into(),
                branch: 0,
                p_w: 1.2,
                q_var: 0.1,
            }])
            .unwrap();
        assert_ne!(load_bus_voltage(edited), load_bus_voltage(&base));
        assert_eq!(edited.terminals.len(), base.terminals.len());
        assert_summary_matches(session.summary(), session.result());
        assert_eq!(session.instance().loads()[0].p_w, vec![1.2]);
        assert_eq!(session.instance().loads()[0].q_var, vec![0.1]);

        let reset: &McPfResult = session.replace_load_powers(&[]).unwrap();
        assert!((load_bus_voltage(reset) - load_bus_voltage(&base)).norm() < 1e-8);
        assert_eq!(session.instance().loads()[0].p_w, vec![1.0]);
        assert_eq!(session.instance().loads()[0].q_var, vec![0.0]);
        assert_eq!(session.solve_count(), 3);
        assert_eq!(session.factorization_count(), 1);
    }

    #[test]
    fn legacy_session_failed_edits_preserve_cached_views() {
        let mut session = McPfSession::new(
            one_phase(1.0),
            McPfOptions {
                max_iterations: 40,
                voltage_envelope: false,
                ..Default::default()
            },
        )
        .unwrap();
        let result_ptr = session.result() as *const McPfResult;
        let instance_ptr = session.instance() as *const McAcPfInstance;
        let before = serde_json::to_value(session.result()).unwrap();
        for (branch, p_w, expected) in [(1, 2.0, "no branch"), (0, 30.0, "did not converge")] {
            let error = session
                .replace_load_powers(&[McLoadPowerEdit {
                    load: "pl".into(),
                    branch,
                    p_w,
                    q_var: 0.0,
                }])
                .unwrap_err();
            assert!(error.contains(expected), "{error}");
            assert!(session.result_cache.get().is_some());
            assert!(session.instance_cache.get().is_some());
            assert!(std::ptr::eq(result_ptr, session.result()));
            assert!(std::ptr::eq(instance_ptr, session.instance()));
            assert_eq!(serde_json::to_value(session.result()).unwrap(), before);
            assert_eq!(session.instance().loads()[0].p_w, vec![1.0]);
            assert_eq!(session.solve_count(), 1);
        }
    }

    #[test]
    fn session_edits_preserve_initial_point_rebinding_checks() {
        use powerio_prob::operating::MulticonductorOperatingPointBuilder;

        let base = one_phase(1.0);
        let initial = MulticonductorOperatingPointBuilder::for_point(base.network().clone())
            .terminal_voltage_magnitudes(vec![10.0, 9.9])
            .build_point()
            .unwrap();
        let mut session = McPfSession::new(
            base.clone().with_initial_point(initial),
            McPfOptions::default(),
        )
        .unwrap();
        let edits = [McLoadPowerEdit {
            load: "pl".into(),
            branch: 0,
            p_w: 1.2,
            q_var: 0.0,
        }];
        session.replace_load_powers_summary(&edits).unwrap();
        assert_eq!(session.materialization_count(), 0);
        let instance = session.instance();
        assert_eq!(instance.loads()[0].p_w, vec![1.2]);
        assert_eq!(
            instance.initial_point().unwrap().network().loads()[0].p_nom,
            vec![1.2]
        );

        let mut foreign = base.network().clone();
        foreign
            .buses_mut()
            .push(DistBus::new("foreign", vec!["1".into()]));
        let incompatible = MulticonductorOperatingPointBuilder::for_point(foreign)
            .terminal_voltage_magnitudes(vec![10.0, 9.9, 9.8])
            .build_point()
            .unwrap();
        let mut session = McPfSession::new(
            base.with_initial_point(incompatible),
            McPfOptions::default(),
        )
        .unwrap();
        let result = serde_json::to_value(session.result()).unwrap();
        let error = session.replace_load_powers(&edits).unwrap_err();
        assert!(error.contains("identity order"), "{error}");
        assert!(session.replace_load_powers_summary(&edits).is_err());
        assert_eq!(session.instance().loads()[0].p_w, vec![1.0]);
        assert_eq!(serde_json::to_value(session.result()).unwrap(), result);
        assert_eq!(session.solve_count(), 1);
    }

    #[test]
    fn retained_session_warm_starts_load_edits_without_refactorization() {
        let options = McPfOptions {
            tolerance: 1e-10,
            ..Default::default()
        };
        let mut session = McPfSession::new(one_phase(1.0), options).unwrap();
        assert_eq!(session.factorization_count(), 1);
        assert_eq!(session.solve_count(), 1);
        let base_iterations = session.summary().iterations;

        let edited = session
            .replace_load_powers_summary(&[McLoadPowerEdit {
                load: "pl".into(),
                branch: 0,
                p_w: 1.001,
                q_var: 0.0,
            }])
            .unwrap()
            .clone();
        let fresh = solve_mc_ac_pf_instance(&one_phase(1.001), &options).unwrap();
        let warm = session.build_result().unwrap();
        assert!((load_bus_voltage(&warm) - load_bus_voltage(&fresh)).norm() < 1e-10);
        assert_eq!(session.factorization_count(), 1);
        assert_eq!(session.solve_count(), 2);
        assert_eq!(edited.solve_count, 2);
        assert!(
            edited.iterations < base_iterations,
            "warm {} vs cold {base_iterations}",
            edited.iterations
        );
        assert_eq!(session.load_branches()[0].p_w, 1.001);
        // The edit is an overlay: the base instance is untouched and no
        // edited network was built.
        assert_eq!(session.base_instance().loads()[0].p_w, vec![1.0]);
        assert_eq!(session.materialization_count(), 0);

        session.replace_load_powers_summary(&[]).unwrap();
        let reset = session.build_result().unwrap();
        let fresh_base = solve_mc_ac_pf_instance(&one_phase(1.0), &options).unwrap();
        assert!((load_bus_voltage(&reset) - load_bus_voltage(&fresh_base)).norm() < 1e-10);
        assert_eq!(session.factorization_count(), 1);
        assert_eq!(session.materialization_count(), 0);
    }

    #[test]
    fn retained_session_updates_physical_impedance_while_freezing_reference_factor() {
        let mut instance = one_phase(1.0);
        let mut network = instance.network().clone();
        network.loads_mut()[0].voltage_model =
            DistLoadVoltageModel::ConstantImpedance { v_nom: vec![10.0] };
        instance = McAcPfInstance::from_network(network).unwrap();
        let mut session = McPfSession::new(instance, McPfOptions::default()).unwrap();
        session
            .replace_load_powers_summary(&[McLoadPowerEdit {
                load: "pl".into(),
                branch: 0,
                p_w: 2.0,
                q_var: 0.0,
            }])
            .unwrap();
        let expected = 10.0 / (1.0 + 2.0 / 100.0);
        let edited = session.build_result().unwrap();
        assert!((load_bus_voltage(&edited).re - expected).abs() < 1e-10);
        assert_eq!(session.factorization_count(), 1);
    }

    #[test]
    fn retained_session_rejects_invalid_edits_without_changing_state() {
        let mut session = McPfSession::new(one_phase(1.0), McPfOptions::default()).unwrap();
        let before = session.build_result().unwrap();
        let summary = session.summary().clone();
        let error = session
            .replace_load_powers_summary(&[McLoadPowerEdit {
                load: "pl".into(),
                branch: 1,
                p_w: 2.0,
                q_var: 0.0,
            }])
            .unwrap_err();
        assert!(error.contains("no branch 1"), "{error}");
        assert_eq!(session.solve_count(), 1);
        assert_eq!(session.load_branches()[0].p_w, 1.0);
        assert_eq!(
            load_bus_voltage(&session.build_result().unwrap()),
            load_bus_voltage(&before)
        );
        assert_eq!(session.summary(), &summary);
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
    fn voltage_envelope_has_a_finite_zero_voltage_limit() {
        let mut instance = one_phase(1.0);
        // The source is the only fixed terminal; force a zero source voltage.
        let mut net = instance.network().clone();
        net.sources_mut()[0].v_magnitude[0] = 0.0;
        instance = McAcPfInstance::from_network(net).expect("instance");
        let result = solve_mc_ac_pf_instance(&instance, &McPfOptions::default()).unwrap();
        assert!(result.converged);
        assert!(!result.voltage_valid);
        assert_eq!(result.min_voltage_pu, Some(0.0));
    }

    #[test]
    fn constant_power_current_conjugates_the_voltage_quotient() {
        let angle = std::f64::consts::FRAC_PI_4;
        let voltage = Complex64::from_polar(2.0, angle);
        let load = crate::mc_pf::loads::BranchLoad {
            name: "rotated".into(),
            bus: "load".into(),
            incidence: vec![vec![(0, Complex64::new(1.0, 0.0))]],
            power: vec![Complex64::new(3.0, 2.0)],
            nominal_admittance: vec![Complex64::new(0.0, 0.0)],
            reference_admittance: vec![Complex64::new(0.0, 0.0)],
            nominal_voltage: vec![1.0],
            model: crate::mc_pf::loads::BranchVoltageModel::ConstantPower,
        };
        let mut actual_values = vec![Complex64::default()];
        load.add_current(&[voltage], &raw_load_options(), &mut actual_values)
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
            bus: "load".into(),
            incidence: vec![vec![(0, Complex64::new(1.0, 0.0))]],
            power: vec![Complex64::new(10.0, 5.0)],
            nominal_admittance: vec![Complex64::new(0.1, -0.05)],
            reference_admittance: vec![Complex64::new(0.1, -0.05)],
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
            let actual = load
                .branch_current(0, &voltage, &raw_load_options())
                .unwrap();
            assert!(
                (actual - expected).norm() < 1e-12,
                "{actual:?} vs {expected:?}"
            );
        }
    }

    #[test]
    fn opendss_voltage_envelope_matches_each_reference_region() {
        let load = direct_branch_load(crate::mc_pf::loads::BranchVoltageModel::ConstantPower);
        let options = McPfOptions::default();
        let y_ref = Complex64::new(0.1, -0.05);
        let at_low = y_ref * (10.0 * options.v_low_pu);
        let at_min = y_ref * (10.0 / options.v_min_pu);

        let cases = [
            (4.0, y_ref * 4.0),
            (5.0, at_low),
            (
                7.0,
                at_low
                    + (at_min - at_low) * (0.7 - options.v_low_pu)
                        / (options.v_min_pu - options.v_low_pu),
            ),
            (8.5, at_min),
            (10.0, Complex64::new(1.0, -0.5)),
            (12.0, y_ref * 12.0 / options.v_max_pu.powi(2)),
        ];
        for (voltage, expected) in cases {
            let actual = load
                .branch_current(0, &[Complex64::new(voltage, 0.0)], &options)
                .unwrap();
            assert!(
                (actual - expected).norm() < 1e-12,
                "at {voltage} V: {actual:?} vs {expected:?}"
            );
        }

        // Both transitions are continuous, including for a rotated phasor.
        for boundary in [options.v_low_pu, options.v_min_pu, options.v_max_pu] {
            let angle = 0.37;
            let epsilon = 1e-9;
            let below = Complex64::from_polar(10.0 * (boundary - epsilon), angle);
            let above = Complex64::from_polar(10.0 * (boundary + epsilon), angle);
            let below = load.branch_current(0, &[below], &options).unwrap();
            let above = load.branch_current(0, &[above], &options).unwrap();
            assert!((below - above).norm() < 1e-7, "boundary {boundary}");
        }
    }

    #[test]
    fn voltage_dependent_laws_have_controlled_zero_voltage_limits() {
        let zero = [Complex64::default()];
        let mut load =
            direct_branch_load(crate::mc_pf::loads::BranchVoltageModel::ConstantImpedance);
        assert_eq!(
            load.branch_current(0, &zero, &raw_load_options()).unwrap(),
            Complex64::default()
        );

        load.model = crate::mc_pf::loads::BranchVoltageModel::ConstantCurrent;
        assert!(load.branch_current(0, &zero, &raw_load_options()).is_err());

        load.model = crate::mc_pf::loads::BranchVoltageModel::Zip {
            alpha_z: vec![0.0],
            alpha_i: vec![1.0],
            alpha_p: vec![0.0],
            beta_z: vec![0.0],
            beta_i: vec![1.0],
            beta_p: vec![0.0],
        };
        assert!(load.branch_current(0, &zero, &raw_load_options()).is_err());

        load.model = crate::mc_pf::loads::BranchVoltageModel::Exponential {
            gamma_p: vec![2.0],
            gamma_q: vec![2.0],
        };
        assert_eq!(
            load.branch_current(0, &zero, &raw_load_options()).unwrap(),
            Complex64::default()
        );

        load.model = crate::mc_pf::loads::BranchVoltageModel::Exponential {
            gamma_p: vec![0.7],
            gamma_q: vec![2.0],
        };
        assert!(load.branch_current(0, &zero, &raw_load_options()).is_err());

        // A very small nonzero voltage remains evaluable for a bounded
        // current law; the voltage check does not round it to zero.
        load.model = crate::mc_pf::loads::BranchVoltageModel::ConstantCurrent;
        let tiny = [Complex64::new(1e-200, 1e-200)];
        let ci_current = load.branch_current(0, &tiny, &raw_load_options()).unwrap();
        assert!(ci_current.norm() > 0.0);
        load.model = crate::mc_pf::loads::BranchVoltageModel::Exponential {
            gamma_p: vec![1.0],
            gamma_q: vec![1.0],
        };
        let exp_current = load.branch_current(0, &tiny, &raw_load_options()).unwrap();
        assert!((exp_current - ci_current).norm() < 1e-12);

        // An inactive P or Q component must not evaluate its unused exponent:
        // 0 * infinity would otherwise turn a finite current into NaN.
        load.power = vec![Complex64::new(10.0, 0.0)];
        load.model = crate::mc_pf::loads::BranchVoltageModel::Exponential {
            gamma_p: vec![2.0],
            gamma_q: vec![-1_000_000.0],
        };
        let current = load
            .branch_current(0, &[Complex64::new(8.0, 0.0)], &raw_load_options())
            .unwrap();
        assert!(current.re.is_finite() && current.im.is_finite());

        let huge = crate::mc_pf::loads::BranchLoad {
            name: "huge".into(),
            bus: "load".into(),
            incidence: vec![vec![(0, Complex64::new(1.0, 0.0))]],
            power: vec![Complex64::new(1e308, 0.0)],
            nominal_admittance: vec![Complex64::new(1e306, 0.0)],
            reference_admittance: vec![Complex64::new(1e306, 0.0)],
            nominal_voltage: vec![10.0],
            model: crate::mc_pf::loads::BranchVoltageModel::ConstantImpedance,
        };
        assert!(huge
            .branch_current(0, &[Complex64::new(20.0, 0.0)], &raw_load_options())
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
    fn omitted_constant_power_nominal_voltage_is_inferred_from_its_voltage_zone() {
        let mut network = one_phase(1.0).network().clone();
        network.loads_mut()[0].voltage_model =
            DistLoadVoltageModel::ConstantPower { v_nom: Vec::new() };
        let instance = McAcPfInstance::from_network(network).unwrap();
        let result = solve_mc_ac_pf_instance(&instance, &McPfOptions::default()).unwrap();
        assert!(result.voltage_valid);
        let minimum = result.min_voltage_pu.expect("one load branch");
        assert!((minimum - 0.9898979486).abs() < 1e-8, "{minimum}");
    }

    #[test]
    fn omitted_nominal_voltage_uses_the_local_transformer_winding_rating() {
        let mut network = MulticonductorNetwork::new();
        network
            .buses_mut()
            .push(DistBus::new("hv", vec!["p".into(), "n".into()]));
        network
            .buses_mut()
            .push(DistBus::new("lv", vec!["p".into(), "n".into()]));
        network.buses_mut()[0].grounded.push("n".into());
        network.buses_mut()[1].grounded.push("n".into());
        network.sources_mut().push(VoltageSource::new(
            "source",
            "hv",
            vec!["p".into(), "n".into()],
            vec![100.0, 0.0],
            vec![0.0, 0.0],
        ));
        let mut high = DistWinding::new(
            "hv",
            vec!["p".into(), "n".into()],
            DistWindingConn::Wye,
            100.0,
            1_000.0,
        );
        high.r_pct = 0.5;
        let mut low = DistWinding::new(
            "lv",
            vec!["p".into(), "n".into()],
            DistWindingConn::Wye,
            10.0,
            1_000.0,
        );
        low.r_pct = 0.5;
        network.transformers_mut().push(DistTransformer::new(
            "step-down",
            vec![high, low],
            vec![2.0],
            1,
        ));
        network.loads_mut().push(DistLoad::new(
            "secondary-load",
            "lv",
            vec!["p".into(), "n".into()],
            Configuration::SinglePhase,
            vec![10.0],
            vec![2.0],
        ));

        let result = solve_mc_ac_pf_instance(
            &McAcPfInstance::from_network(network).unwrap(),
            &McPfOptions::default(),
        )
        .unwrap();
        let load_voltage = result
            .terminals
            .iter()
            .find(|terminal| terminal.bus == "lv" && terminal.terminal == "p")
            .unwrap()
            .voltage;
        let inferred = load_voltage.re.hypot(load_voltage.im) / result.min_voltage_pu.unwrap();
        assert!((inferred - 10.0).abs() < 1e-9, "{inferred}");
    }

    #[test]
    fn invalid_voltage_envelope_options_are_rejected() {
        let error = solve_mc_ac_pf_instance(
            &one_phase(1.0),
            &McPfOptions {
                v_min_pu: 1.2,
                v_max_pu: 1.1,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(error.contains("invalid multiconductor PF options"));
    }

    #[test]
    fn envelope_can_converge_to_a_solution_that_is_flagged_outside_the_normal_band() {
        let stressed = one_phase(30.0);
        let raw_error = solve_mc_ac_pf_instance(
            &stressed,
            &McPfOptions {
                max_iterations: 40,
                voltage_envelope: false,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(raw_error.contains("did not converge"), "{raw_error}");

        let result = solve_mc_ac_pf_instance(
            &stressed,
            &McPfOptions {
                max_iterations: 200,
                ..Default::default()
            },
        )
        .expect("bounded-voltage solve");
        assert!(result.converged);
        assert!(!result.voltage_valid);
        assert!(result.min_voltage_pu.unwrap() < 0.85);
        assert_eq!(result.voltage_violations.len(), 1);
        assert_eq!(result.voltage_violations[0].bound, "minimum");
        assert_eq!(result.voltage_violations[0].load, "pl");
    }

    #[test]
    fn voltage_assessment_reports_the_upper_band_separately_from_convergence() {
        let mut network = one_phase(1.0).network().clone();
        network.loads_mut()[0].voltage_model =
            DistLoadVoltageModel::ConstantPower { v_nom: vec![8.0] };
        let result = solve_mc_ac_pf_instance(
            &McAcPfInstance::from_network(network).unwrap(),
            &McPfOptions::default(),
        )
        .unwrap();
        assert!(result.converged);
        assert!(!result.voltage_valid);
        assert!(result.max_voltage_pu.unwrap() > 1.15);
        assert_eq!(result.voltage_violations[0].bound, "maximum");
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
    fn retained_session_snapshot_materializes_edited_loads_without_refactoring() {
        let mut session =
            McPfSession::from_module_json(&one_phase_module(1.0), McPfOptions::default()).unwrap();
        session
            .replace_load_powers_summary(&[McLoadPowerEdit {
                load: "pl".into(),
                branch: 0,
                p_w: 1.2,
                q_var: 0.1,
            }])
            .unwrap();

        assert_eq!(session.materialization_count(), 0);
        let snapshot = session.snapshot("edited", "Edited feeder").unwrap();
        // One edited instance feeds both the input module and the solution.
        assert_eq!(session.materialization_count(), 1);
        let replayed = McStudySnapshot::from_json(&snapshot.to_json().unwrap()).unwrap();
        let module = crate::ir::deserialize_module(&replayed.input_module).unwrap();
        let input = input::instance_from_value(module.value()).unwrap();
        assert_eq!(input.loads()[0].p_w, vec![1.2]);
        assert_eq!(input.loads()[0].q_var, vec![0.1]);
        assert_eq!(session.factorization_count(), 1);
        assert_eq!(
            load_bus_voltage(&replayed.result),
            load_bus_voltage(&session.build_result().unwrap())
        );
        assert!(session.snapshot("", "Edited feeder").is_err());
    }

    #[test]
    fn saved_geography_updates_both_modules_without_changing_results() {
        let mut snapshot = McStudySnapshot::solve(
            "geo-study",
            "Feeder",
            &one_phase_module(1.0),
            McPfOptions::default(),
        )
        .unwrap();
        let result = serde_json::to_value(&snapshot.result).unwrap();
        let layer = powerio::GeoLayer::parse(r#"{"type":"FeatureCollection","features":[
          {"type":"Feature","properties":{"bus":"source"},"geometry":{"type":"Point","coordinates":[-83.9,35.9]}},
          {"type":"Feature","properties":{"bus":"load"},"geometry":{"type":"Point","coordinates":[-83.8,35.8]}},
          {"type":"Feature","properties":{"bus_from":"source","bus_to":"load"},"geometry":{"type":"LineString","coordinates":[[-83.9,35.9],[-83.85,35.87],[-83.8,35.8]]}}
        ]}"#, Some("case.geo.json")).unwrap();
        snapshot.apply_geo_layer(&layer.layer).unwrap();
        let restored = McStudySnapshot::from_json(&snapshot.to_json().unwrap()).unwrap();
        assert_eq!(serde_json::to_value(&restored.result).unwrap(), result);
        for text in [&restored.input_module, &restored.solution_module] {
            assert!(text.contains("-83.85"), "saved route midpoint is absent");
        }
        let saved = snapshot.to_json().unwrap();
        let unrelated = powerio::GeoLayer::parse(r#"{"type":"FeatureCollection","features":[
          {"type":"Feature","properties":{"bus":"absent"},"geometry":{"type":"Point","coordinates":[-70,30]}}
        ]}"#, Some("case.geo.json")).unwrap();
        assert!(snapshot.apply_geo_layer(&unrelated.layer).is_err());
        assert_eq!(snapshot.to_json().unwrap(), saved);
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
        value["result"]["min_voltage_pu"] = serde_json::json!(0.1);
        assert!(McStudySnapshot::from_json(&value.to_string()).is_err());

        value = serde_json::from_str(&snapshot.to_json().unwrap()).unwrap();
        value["input_module"] = serde_json::Value::String(one_phase_module(2.0));
        assert!(McStudySnapshot::from_json(&value.to_string()).is_err());
    }

    fn oracle_instance(text: &str) -> McAcPfInstance {
        parse_bmopf_instance(text).expect("oracle BMOPF input")
    }

    const ORACLE_INPUTS: [&str; 4] = [
        include_str!("../../tests/data/mc_pf/oracle_inputs/pf_dy_xfmr.json"),
        include_str!("../../tests/data/mc_pf/oracle_inputs/pf_delta_load.json"),
        include_str!("../../tests/data/mc_pf/oracle_inputs/pf_center_tap_multi_feeder.json"),
        include_str!("../../tests/data/mc_pf/oracle_inputs/pf_3ph_line.json"),
    ];

    fn assert_summary_matches(summary: &McPfSummary, result: &McPfResult) {
        assert_eq!(summary.converged, result.converged);
        assert_eq!(summary.voltage_valid, result.voltage_valid);
        assert_eq!(summary.min_voltage_pu, result.min_voltage_pu);
        assert_eq!(summary.max_voltage_pu, result.max_voltage_pu);
        assert_eq!(
            summary.voltage_violation_count,
            result.voltage_violations.len()
        );
        assert_eq!(summary.iterations, result.iterations);
        assert_eq!(summary.factorization_count, result.factorization_count);
        assert_eq!(summary.matrix_dimension, result.matrix_dimension);
        assert_eq!(summary.matrix_nonzeros, result.matrix_nonzeros);
        assert_eq!(summary.voltage_change, result.voltage_change);
        assert_eq!(summary.physical_kcl_residual, result.physical_kcl_residual);
        assert_eq!(summary.scaled_kcl_residual, result.scaled_kcl_residual);
        assert_eq!(summary.terminal_count, result.terminals.len());
        assert_eq!(summary.element_port_count, result.element_ports.len());
        // The UI's former aggregation over the full result, summed in the
        // same order, must reproduce the summary bit for bit.
        let mut source = McComplex::default();
        for reaction in &result.source_reactions {
            source.re += reaction.power_into_network.re;
            source.im += reaction.power_into_network.im;
        }
        assert_eq!(summary.source_power_into_network, source);
        let mut loss = McComplex::default();
        for port in result
            .element_ports
            .iter()
            .filter(|port| !["load", "generator", "ibr"].contains(&port.kind.as_str()))
        {
            loss.re += port.power_into_element.re;
            loss.im += port.power_into_element.im;
        }
        assert_eq!(summary.passive_loss, loss);
    }

    #[test]
    fn session_summary_equals_the_full_result_it_summarizes() {
        for text in ORACLE_INPUTS {
            let mut session =
                McPfSession::new(oracle_instance(text), McPfOptions::default()).unwrap();
            assert_summary_matches(session.summary(), &session.build_result().unwrap());
            let edits: Vec<_> = session
                .load_branches()
                .into_iter()
                .map(|branch| McLoadPowerEdit {
                    load: branch.load,
                    branch: branch.branch,
                    p_w: branch.base_p_w * 1.07,
                    q_var: branch.base_q_var * 0.9,
                })
                .collect();
            let summary = session.replace_load_powers_summary(&edits).unwrap().clone();
            assert_summary_matches(&summary, &session.build_result().unwrap());
            assert_eq!(summary.solve_count, 2);
            assert_eq!(session.materialization_count(), 0);
            assert!(session.result_cache.get().is_none());
            assert!(session.instance_cache.get().is_none());
        }
    }

    #[test]
    fn terminal_arrays_and_detail_pages_match_the_full_result() {
        let session =
            McPfSession::new(oracle_instance(ORACLE_INPUTS[2]), McPfOptions::default()).unwrap();
        let result = session.build_result().unwrap();
        let voltages = session.terminal_voltages();
        let currents = session.terminal_currents();
        assert_eq!(session.terminal_ids().len(), result.terminals.len());
        for (k, terminal) in result.terminals.iter().enumerate() {
            assert_eq!(
                session.terminal_ids()[k],
                (terminal.bus.clone(), terminal.terminal.clone())
            );
            assert_eq!(voltages[2 * k], terminal.voltage.re);
            assert_eq!(voltages[2 * k + 1], terminal.voltage.im);
            assert_eq!(currents[2 * k], terminal.current_into_network.re);
            assert_eq!(currents[2 * k + 1], terminal.current_into_network.im);
        }
        let json = |value: &dyn erased::Json| value.json();
        // Every page of the unfiltered equipment list, then per element.
        let mut paged = Vec::new();
        let mut offset = 0;
        loop {
            let page = session
                .detail(&McPfDetailQuery {
                    port_offset: offset,
                    port_limit: 7,
                    ..Default::default()
                })
                .unwrap();
            assert_eq!(page.element_port_total, result.element_ports.len());
            assert_eq!(page.solve_count, 1);
            if page.element_ports.is_empty() {
                break;
            }
            offset += page.element_ports.len();
            paged.extend(page.element_ports);
        }
        assert_eq!(json(&paged), json(&result.element_ports));
        for element in ["mv_2", "ab_1", "ct_ab", "missing"] {
            let detail = session
                .detail(&McPfDetailQuery {
                    element: Some(element.to_owned()),
                    port_limit: McPfDetailQuery::MAX_PORT_LIMIT,
                    ..Default::default()
                })
                .unwrap();
            let expected: Vec<_> = result
                .element_ports
                .iter()
                .filter(|port| port.element == element)
                .cloned()
                .collect();
            assert_eq!(detail.element_port_total, expected.len());
            assert_eq!(json(&detail.element_ports), json(&expected));
        }
        for bus in session
            .terminal_ids()
            .iter()
            .map(|(bus, _)| bus.clone())
            .collect::<std::collections::BTreeSet<_>>()
        {
            let detail = session
                .detail(&McPfDetailQuery {
                    bus: Some(bus.clone()),
                    ..Default::default()
                })
                .unwrap();
            let expected: Vec<_> = result
                .terminals
                .iter()
                .filter(|terminal| terminal.bus == bus)
                .cloned()
                .collect();
            assert_eq!(json(&detail.terminals), json(&expected));
        }
        assert!(session
            .detail(&McPfDetailQuery {
                port_limit: McPfDetailQuery::MAX_PORT_LIMIT + 1,
                ..Default::default()
            })
            .is_err());
    }

    mod erased {
        pub trait Json {
            fn json(&self) -> serde_json::Value;
        }
        impl<T: serde::Serialize> Json for T {
            fn json(&self) -> serde_json::Value {
                serde_json::to_value(self).unwrap()
            }
        }
    }

    #[test]
    fn module_session_materializes_the_same_bytes_as_reparsing_the_module() {
        let text = ORACLE_INPUTS[0];
        let source =
            powerio::Source::from_memory("case.bmopf.json", text.as_bytes().to_vec()).unwrap();
        let parsed = powerio::parse_with_options(
            source,
            &powerio::ParseOptions::default()
                .format("bmopf-json")
                .unwrap(),
        )
        .unwrap();
        let module_json = crate::ir::serialize_module(&parsed).unwrap();
        // The pre-overlay session re-parsed its stored text for every
        // materialization and replaced the value in place.
        let reparsed = |instance: &McAcPfInstance| {
            let mut module = crate::ir::deserialize_module(&module_json).unwrap();
            *module.value_mut() =
                powerio::PioValue::MulticonductorNetwork(instance.network().clone());
            crate::ir::serialize_module(&module).unwrap()
        };
        let mut session =
            McPfSession::from_module_json(&module_json, McPfOptions::default()).unwrap();
        assert!(session.cold_profile().parse_ms >= 0.0);
        assert_eq!(
            session.input_module_json().unwrap(),
            reparsed(&session.edited_instance().unwrap())
        );
        let branch = session.load_branches()[0].clone();
        session
            .replace_load_powers_summary(&[McLoadPowerEdit {
                load: branch.load,
                branch: branch.branch,
                p_w: branch.base_p_w * 1.3,
                q_var: branch.base_q_var,
            }])
            .unwrap();
        assert_eq!(session.materialization_count(), 0);
        let edited = session.input_module_json().unwrap();
        assert_eq!(edited, reparsed(&session.edited_instance().unwrap()));
        assert_eq!(session.materialization_count(), 2);
    }

    #[test]
    fn failed_edit_restores_powers_summary_and_profile() {
        let mut session = McPfSession::new(
            one_phase(1.0),
            McPfOptions {
                max_iterations: 40,
                voltage_envelope: false,
                ..Default::default()
            },
        )
        .unwrap();
        let summary = session.summary().clone();
        let profile = session.profile();
        let voltages = session.terminal_voltages();
        let error = session
            .replace_load_powers_summary(&[McLoadPowerEdit {
                load: "pl".into(),
                branch: 0,
                p_w: 30.0,
                q_var: 0.0,
            }])
            .unwrap_err();
        assert!(error.contains("did not converge"), "{error}");
        assert_eq!(session.summary(), &summary);
        assert_eq!(session.profile(), profile);
        assert_eq!(session.terminal_voltages(), voltages);
        assert_eq!(session.solve_count(), 1);
        assert_eq!(session.load_branches()[0].p_w, 1.0);
        // The restored law still solves warm with the retained factor.
        session.replace_load_powers_summary(&[]).unwrap();
        assert_eq!(session.factorization_count(), 1);
    }

    #[test]
    fn native_session_profiles_each_phase() {
        let mut session =
            McPfSession::new(oracle_instance(ORACLE_INPUTS[0]), McPfOptions::default()).unwrap();
        let cold = session.cold_profile();
        assert!(cold.timed);
        assert!(cold.iterations > 0);
        assert!(cold.linear_solves > cold.iterations);
        assert!(cold.total_ms >= cold.prepare_ms + cold.factor_ms);
        let branch = session.load_branches()[0].clone();
        session
            .replace_load_powers_summary(&[McLoadPowerEdit {
                load: branch.load,
                branch: branch.branch,
                p_w: branch.base_p_w * 1.1,
                q_var: branch.base_q_var,
            }])
            .unwrap();
        let warm = session.profile();
        assert!(warm.timed);
        assert_eq!(warm.prepare_ms, 0.0);
        assert_eq!(warm.factor_ms, 0.0);
        assert_eq!(warm.linear_solves, warm.iterations);
        assert_eq!(warm.iterations, session.summary().iterations);
    }
}
