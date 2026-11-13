//! RMS (phasor-domain, electromechanical) dynamic simulation.
//!
//! The input is a PowerIO balanced network, for example a PSS/E `.raw` case,
//! paired with PSS/E `.dyr` dynamic data that PowerIO reads into typed models
//! ([`powerio::link_machines`]). The engine initializes every machine from the
//! AC power flow, integrates the network and device equations with the
//! implicit trapezoidal rule, and applies discrete events (branch trips, bus
//! faults, load changes, generator trips).
//!
//! # Provenance
//!
//! The model equations, the network formulation and the PSS/E parameter
//! mapping are translations of PowerSimulationsDynamics.jl v0.16.2 (BSD-3-Clause,
//! Copyright (c) 2020, 2023 Alliance for Sustainable Energy, LLC and The
//! Regents of the University of California; `licenses/PowerSimulationsDynamics.jl-LICENSE`)
//! and PowerSystems.jl v5.12 (BSD-3-Clause, Copyright (c) 2018, 2023, the same
//! holders; `licenses/PowerSystems.jl-LICENSE`),
//! cited per function. Julia is not a dependency: PowerSimulationsDynamics.jl
//! and the PSS/E recordings in its test suite serve as the validation
//! references (`docs/src/rms-validation.md`).
//!
//! Supported models: machines `GENCLS`, `GENROU`, `GENROE`, `GENSAL`,
//! `GENSAE`; exciters `SEXS`, `ESAC1A`, `EXST1`; governors `TGOV1`, `GAST`,
//! `HYGOV`; stabilizer `IEEEST`; renewable plants `REGCA1` + `REECB1` +
//! `REPCA1` ([`renewable`]). Each static load keeps the voltage law the AC
//! power flow solved it with (constant power, ZIP and exponential parts,
//! [`LoadModel::PowerFlow`]), or the whole bus demand follows one law
//! ([`LoadModel`]); constant-power and constant-current parts convert to
//! constant admittance below a breakpoint ([`LowVoltageLoad`]).

// Negated comparisons are NaN-safe on purpose (`!(x <= tol)` fails on NaN);
// index loops mirror the row-by-row residual equations.
#![allow(clippy::neg_cmp_op_on_partial_ord, clippy::needless_range_loop)]

#[cfg(feature = "rms-experimental")]
pub mod auto;
pub(crate) mod blocks;
#[cfg(feature = "rms-experimental")]
mod dyr_params;
#[cfg(feature = "rms-experimental")]
pub mod graph;
pub(crate) mod init;
mod integrate;
#[cfg(feature = "rms-experimental")]
mod library;
#[cfg(feature = "rms-experimental")]
mod linear;
#[cfg(feature = "rms-experimental")]
mod live;
pub(crate) mod models;
#[cfg(feature = "rms-experimental")]
pub mod presets;
mod renewable;
mod schur;
mod system;
mod topology;

pub use topology::{End, TopoBranch, TopoUnit, Topology, UnitKind};

use std::collections::BTreeMap;

use num_complex::Complex;
use powerio::{BalancedNetwork, DynamicCase};
use serde::{Deserialize, Serialize};

use system::{DynSystem, GenOut, C64};

#[cfg(feature = "rms-experimental")]
pub use dyr_params::{
    dyr_edit, dyr_edit_json, dyr_parameters, dyr_parameters_json, DyrEdit, DyrGeneratorView,
    DyrParameter, DyrRecordView,
};
#[cfg(feature = "rms-experimental")]
pub use graph::{
    check_graph_json, template, template_json, Block, BlockGraph, DynamicInput, GraphNode,
    ModelOverride, Slot,
};
#[cfg(feature = "rms-experimental")]
pub use library::{
    dyr_apply_preset, dyr_dynamize, dyr_dynamize_json, dyr_rekey, dyr_set_model, library,
    library_json, DynamizeRequest, LibraryModel,
};
#[cfg(feature = "rms-experimental")]
pub use linear::{
    BodeCurve, BodeSpec, LinearRequest, LinearResult, Mode, OutputSignal, Participation,
};
#[cfg(feature = "rms-experimental")]
pub use live::{
    linear_module_json, live_from_module_json, pack_chunk, pack_result, replay_module_buffers,
    simulate_module_buffers, RmsBuffers, RmsChunk, RmsLive,
};

/// A controller reference a [`RmsEvent::SetReference`] sets (and a frequency
/// response perturbs).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputSignal {
    /// Exciter voltage reference (field voltage for a fixed exciter).
    VRef,
    /// Governor power reference (mechanical torque for a fixed governor).
    PRef,
}

/// How static loads respond to voltage during the simulation.
///
/// The default keeps every load's own law: the constant-power part, the ZIP
/// constant-current and constant-impedance parts and exponential loads, the
/// same decomposition the AC power flow solves (each part `P0 (V / V0)^γ`
/// with `P0` its power at the power flow voltage `V0`), so the loads draw
/// exactly the power flow demand at `t = 0`. The other variants are a global
/// override: the whole bus demand at `V0`, whatever its parts, follows one
/// law (PSS/E `CONL` with 100 % of P and Q in one class).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoadModel {
    /// Each load part keeps the voltage law of the static model.
    #[default]
    PowerFlow,
    /// Constant admittance at the power flow voltage (PSS/E `CONL` 100% Z).
    ConstantImpedance,
    ConstantCurrent,
    ConstantPower,
}

/// How the constant-power and constant-current load parts (and exponential
/// parts with `γ < 2`) behave at low voltage.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LowVoltageLoad {
    /// PSS/E `PQBRAK`: below `v_break` (pu, absolute; 0.7 by default as in
    /// PSS/E) each part draws the constant admittance of its power at the
    /// breakpoint, `P(V) = P(V_b) (V / V_b)²`, continuous in value at `V_b`.
    /// A bus whose power flow voltage `V0` is below `v_break` breaks at `V0`
    /// instead, so it starts on its own law.
    Break { v_break: f64 },
    /// The stated law at every voltage down to 20 % of `V0`, below that a
    /// smooth floor on the magnitude the parts divide by (as
    /// PowerSimulationsDynamics.jl applies the law at every voltage; the
    /// floor only keeps the current's direction defined at 0 V).
    Exact,
}

impl Default for LowVoltageLoad {
    fn default() -> Self {
        Self::Break { v_break: 0.7 }
    }
}

/// A discrete event at `time` seconds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RmsEvent {
    /// Open the branch `(from, to, circuit)`.
    BranchTrip {
        time: f64,
        from: usize,
        to: usize,
        #[serde(default = "default_circuit")]
        circuit: String,
    },
    /// Close the branch `(from, to, circuit)` again.
    BranchClose {
        time: f64,
        from: usize,
        to: usize,
        #[serde(default = "default_circuit")]
        circuit: String,
    },
    /// A balanced fault to ground through `r + jx` (system per unit).
    BusFault {
        time: f64,
        bus: usize,
        #[serde(default)]
        r: f64,
        #[serde(default = "default_fault_x")]
        x: f64,
    },
    /// Remove the fault at `bus`.
    ClearFault { time: f64, bus: usize },
    /// Scale the load at `bus` (all components) by `scale`.
    LoadChange { time: f64, bus: usize, scale: f64 },
    /// Set a generator's exciter (`v_ref`) or governor (`p_ref`) reference.
    SetReference {
        time: f64,
        bus: usize,
        #[serde(default = "default_circuit")]
        machine_id: String,
        signal: InputSignal,
        value: f64,
    },
    /// Disconnect a generator.
    GeneratorTrip {
        time: f64,
        bus: usize,
        #[serde(default = "default_circuit")]
        machine_id: String,
    },
    /// A balanced fault inside branch `(from, to, circuit)` at `location`
    /// (0 at `from`, 1 at `to`) of its series impedance, through `r + jx`.
    /// A transient fault (`permanent: false`) dies out once both ends are
    /// open, so an auto-reclose succeeds.
    LineFault {
        time: f64,
        from: usize,
        to: usize,
        #[serde(default = "default_circuit")]
        circuit: String,
        location: f64,
        #[serde(default)]
        r: f64,
        #[serde(default = "default_fault_x")]
        x: f64,
        #[serde(default = "default_true")]
        permanent: bool,
    },
    /// Remove the fault inside a branch.
    ClearLineFault {
        time: f64,
        from: usize,
        to: usize,
        #[serde(default = "default_circuit")]
        circuit: String,
    },
    /// Open the breaker of branch `(from, to, circuit)` at bus `at` (one
    /// end; the other stays connected).
    BreakerOpen {
        time: f64,
        from: usize,
        to: usize,
        #[serde(default = "default_circuit")]
        circuit: String,
        at: usize,
    },
    /// Close that breaker again.
    BreakerClose {
        time: f64,
        from: usize,
        to: usize,
        #[serde(default = "default_circuit")]
        circuit: String,
        at: usize,
    },
}

fn default_circuit() -> String {
    "1".into()
}

fn default_fault_x() -> f64 {
    1e-4
}

impl RmsEvent {
    pub fn time(&self) -> f64 {
        match self {
            Self::BranchTrip { time, .. }
            | Self::BranchClose { time, .. }
            | Self::BusFault { time, .. }
            | Self::ClearFault { time, .. }
            | Self::LoadChange { time, .. }
            | Self::SetReference { time, .. }
            | Self::GeneratorTrip { time, .. }
            | Self::LineFault { time, .. }
            | Self::ClearLineFault { time, .. }
            | Self::BreakerOpen { time, .. }
            | Self::BreakerClose { time, .. } => *time,
        }
    }
}

/// A simulation request.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RmsRequest {
    /// End time in seconds.
    #[serde(default = "default_t_end")]
    pub t_end: f64,
    /// Integration step in seconds.
    #[serde(default = "default_dt")]
    pub dt: f64,
    /// Output sampling interval in seconds, a multiple of `dt`.
    #[serde(default = "default_output_step")]
    pub output_step: f64,
    /// The load model: for [`simulate`] the one the simulation is built with
    /// (default [`LoadModel::PowerFlow`]); for [`RmsSimulation::run`] it
    /// must be the prepared simulation's or `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub load_model: Option<LoadModel>,
    #[serde(default)]
    pub events: Vec<RmsEvent>,
    /// Compute the eigenvalues of the linearized system at the initial point
    /// (dense, `O(n³)` in the differential states). Needs the
    /// `rms-experimental` feature; without it the request is noted and
    /// skipped.
    #[serde(default)]
    pub small_signal: bool,
    /// What to record (default: every channel at every output step).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record: Option<RecordSpec>,
}

fn default_t_end() -> f64 {
    10.0
}
fn default_dt() -> f64 {
    0.0025
}
fn default_output_step() -> f64 {
    0.005
}
fn default_true() -> bool {
    true
}

impl Default for RmsRequest {
    fn default() -> Self {
        Self {
            t_end: default_t_end(),
            dt: default_dt(),
            output_step: default_output_step(),
            load_model: None,
            events: Vec::new(),
            small_signal: false,
            record: None,
        }
    }
}

/// One dynamic generator in the result.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RmsGenerator {
    pub bus: usize,
    pub machine_id: String,
    /// `[machine, exciter, governor, stabilizer]` model names.
    pub models: Vec<String>,
    pub states: Vec<String>,
    pub initial_states: Vec<f64>,
    pub v_ref: f64,
    pub p_ref: f64,
    pub mbase: f64,
}

/// The simulation result. Channels are keyed `quantity:bus` (`vm`, `va` in
/// radians, `delta` in radians, `omega`, `efd`, `tm` on the machine base,
/// `pe`, `qe` on the system base; `pl`, `ql` the power the loads of a bus
/// draw, system base) and `quantity:from-to` for branch flows
/// (`p_from`, `q_from`, `p_to`, `q_to`, system base), with `#id` appended for
/// a machine or circuit id other than `1`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RmsResult {
    pub time: Vec<f64>,
    pub channels: BTreeMap<String, Vec<f64>>,
    pub buses: Vec<usize>,
    pub base_mva: f64,
    pub base_frequency: f64,
    pub generators: Vec<RmsGenerator>,
    /// Generators held as voltage sources behind `ZSORCE`.
    pub sources: Vec<String>,
    /// Eigenvalues `[re, im]` at the initial operating point (with
    /// [`RmsRequest::small_signal`]; empty otherwise).
    pub eigenvalues: Vec<[f64; 2]>,
    pub events: Vec<String>,
    pub notes: Vec<String>,
    pub steps: usize,
    pub newton_iterations: usize,
    /// The channels of [`RecordSpec::frames`] at their own step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frames: Option<RmsSeries>,
    /// The full state at [`RecordSpec::checkpoint_step`] intervals.
    #[cfg(feature = "rms-experimental")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checkpoints: Vec<Checkpoint>,
}

/// A prepared simulation: the system assembled and initialized once, run any
/// number of times with different events.
pub struct RmsSimulation {
    sys: DynSystem,
    x0: Vec<f64>,
    notes: Vec<String>,
    load_model: LoadModel,
    topology: Topology,
    initial_residual: f64,
}

impl RmsSimulation {
    /// Assemble and initialize.
    ///
    /// # Errors
    /// When the power flow fails, a model is unsupported or inconsistent, or
    /// a machine cannot be initialized.
    pub fn new(
        network: &BalancedNetwork,
        dynamics: &DynamicCase,
        load_model: LoadModel,
    ) -> Result<Self, String> {
        Self::with_options(network, dynamics, load_model, RmsOptions::default())
    }

    /// As [`Self::new`], with the [`RmsOptions`] of this simulation.
    ///
    /// # Errors
    /// As [`Self::new`].
    pub fn with_options(
        network: &BalancedNetwork,
        dynamics: &DynamicCase,
        load_model: LoadModel,
        options: RmsOptions,
    ) -> Result<Self, String> {
        Self::assemble(
            network,
            dynamics,
            #[cfg(feature = "rms-experimental")]
            &[],
            load_model,
            options,
        )
    }

    /// Assemble and initialize with controller graphs replacing (or adding)
    /// the DYR controllers of the generators they name.
    ///
    /// # Errors
    /// As [`Self::new`], or when a graph does not compile.
    #[cfg(feature = "rms-experimental")]
    pub fn with_models(
        network: &BalancedNetwork,
        dynamics: &DynamicCase,
        models: &[ModelOverride],
        load_model: LoadModel,
    ) -> Result<Self, String> {
        Self::with_models_and_options(network, dynamics, models, load_model, RmsOptions::default())
    }

    /// As [`Self::with_models`], with the [`RmsOptions`] of this simulation.
    ///
    /// # Errors
    /// As [`Self::with_models`].
    #[cfg(feature = "rms-experimental")]
    pub fn with_models_and_options(
        network: &BalancedNetwork,
        dynamics: &DynamicCase,
        models: &[ModelOverride],
        load_model: LoadModel,
        options: RmsOptions,
    ) -> Result<Self, String> {
        Self::assemble(network, dynamics, models, load_model, options)
    }

    fn assemble(
        network: &BalancedNetwork,
        dynamics: &DynamicCase,
        #[cfg(feature = "rms-experimental")] models: &[ModelOverride],
        load_model: LoadModel,
        options: RmsOptions,
    ) -> Result<Self, String> {
        let (sys, x0, report) = init::build(
            network,
            dynamics,
            #[cfg(feature = "rms-experimental")]
            models,
            load_model,
            options,
        )?;
        let mut notes = report.notes;
        notes.push(format!(
            "power flow converged in {} iterations (mismatch {:.1e}); steady-state residual {:.1e}",
            report.pf_iterations, report.pf_residual, report.steady_residual
        ));
        let topology = topology::topology(network, &sys, &x0);
        Ok(Self {
            sys,
            x0,
            notes,
            load_model,
            topology,
            initial_residual: report.steady_residual,
        })
    }

    /// The elements of the simulated network, as the engine numbers them.
    #[must_use]
    pub fn topology(&self) -> &Topology {
        &self.topology
    }

    /// The apparent power at the more loaded end of every branch at the
    /// operating point, system per unit (the branches of
    /// [`Self::topology`]).
    #[must_use]
    pub fn branch_loading(&self) -> Vec<f64> {
        (0..self.sys.ac.m)
            .map(|e| {
                let (from, to) = self.sys.branch_flow(&self.x0, e);
                from.norm().max(to.norm())
            })
            .collect()
    }

    /// `‖f(x0)‖∞` at the initial point: every derivative and every network
    /// current balance, the steadiness of the initialization (system per
    /// unit; the build refuses more than 1e-6).
    #[must_use]
    pub fn initial_residual(&self) -> f64 {
        self.initial_residual
    }

    pub fn load_model(&self) -> LoadModel {
        self.load_model
    }

    /// Bus numbers in state order.
    pub fn buses(&self) -> &[usize] {
        &self.sys.bus_numbers
    }

    /// `(from, to, circuit)` of every branch.
    pub fn branches(&self) -> Vec<(usize, usize, String)> {
        self.sys.branch_keys.clone()
    }

    /// Eigenvalues of the linearized system at the initial point:
    /// `M_d⁻¹ (f_dd − f_da f_aa⁻¹ f_ad)` over the differential states.
    ///
    /// # Errors
    /// When the system is too large for the dense eigensolver or it fails.
    #[cfg(feature = "rms-experimental")]
    pub fn eigenvalues(&self) -> Result<Vec<[f64; 2]>, String> {
        linear::check_eigen_size(&self.sys)?;
        let (a, _) = linear::reduced_a(&self.sys, &self.x0);
        if a.nrows() == 0 {
            return Ok(Vec::new());
        }
        let mut eig: Vec<[f64; 2]> = a
            .eigenvalues()
            .map_err(|e| format!("eigenvalue computation failed: {e:?}"))?
            .into_iter()
            .map(|z| [z.re, z.im])
            .collect();
        eig.sort_by(|p, q| p[0].total_cmp(&q[0]).then(p[1].total_cmp(&q[1])));
        Ok(eig)
    }

    /// Modes with participation factors and frequency responses at the
    /// initial operating point.
    ///
    /// # Errors
    /// When a frequency response names a missing generator or bus.
    #[cfg(feature = "rms-experimental")]
    pub fn linear_analysis(&self, request: &LinearRequest) -> Result<LinearResult, String> {
        let (states, modes) = linear::modes(&self.sys, &self.x0, request.participation_top)?;
        let bode = linear::bode(&self.sys, &self.x0, request)?;
        Ok(LinearResult {
            states,
            modes,
            bode,
        })
    }

    /// Check every event before a run: a finite time in `[0, t_end)` (an
    /// event at the end would act on nothing the run records), and
    /// an element the system has (each event applied in time order to a
    /// copy of the system, so an element an earlier event adds or removes
    /// is judged as the run would).
    fn check_events(&self, events: &[RmsEvent], t_end: f64) -> Result<(), String> {
        for (k, e) in events.iter().enumerate() {
            let t = e.time();
            if !t.is_finite() {
                return Err(format!("event {k} has a non-finite time {t}"));
            }
            if t < 0.0 || t >= t_end {
                return Err(format!(
                    "event {k} at t = {t} s lies outside the run [0, {t_end}) s"
                ));
            }
        }
        let mut order: Vec<&RmsEvent> = events.iter().collect();
        order.sort_by(|a, b| a.time().total_cmp(&b.time()));
        let mut sys = self.sys.clone();
        let mut x = self.x0.clone();
        for e in order {
            apply_event(&mut sys, &mut x, e)
                .map_err(|err| format!("event at t = {} s: {err}", e.time()))?;
        }
        Ok(())
    }

    /// Run the simulation.
    ///
    /// # Errors
    /// When an event names a missing element or the integration fails.
    pub fn run(&self, request: &RmsRequest) -> Result<RmsResult, String> {
        self.run_cancellable(request, &mut || false)
    }

    /// Run, polling `cancelled` between output samples.
    ///
    /// # Errors
    /// As [`Self::run`], or when `cancelled` returns true.
    pub fn run_cancellable(
        &self,
        request: &RmsRequest,
        cancelled: &mut dyn FnMut() -> bool,
    ) -> Result<RmsResult, String> {
        self.run_from(request, None, cancelled)
    }

    /// Run `request` from a checkpoint of an earlier run of the same
    /// request: its events before the checkpoint are applied to the network,
    /// the state is the checkpoint's, and the samples from there on are the
    /// earlier run's to the solver's tolerance. With [`RmsRequest::record`]
    /// naming other channels, this records them after the fact.
    ///
    /// # Errors
    /// As [`Self::run`], or a checkpoint of another system.
    #[cfg(feature = "rms-experimental")]
    pub fn replay(&self, request: &RmsRequest, from: &Checkpoint) -> Result<RmsResult, String> {
        self.run_from(request, Some(from), &mut || false)
    }

    fn run_from(
        &self,
        request: &RmsRequest,
        start: Option<&Checkpoint>,
        cancelled: &mut dyn FnMut() -> bool,
    ) -> Result<RmsResult, String> {
        if request.load_model.is_some_and(|m| m != self.load_model) {
            return Err("the request's load model differs from the prepared simulation's".into());
        }
        if !(request.dt >= MIN_DT
            && request.output_step >= request.dt
            && request.output_step.is_finite()
            && request.t_end > 0.0
            && request.t_end.is_finite())
        {
            return Err(format!(
                "need dt >= {MIN_DT} s, a finite output_step >= dt and a finite t_end > 0"
            ));
        }
        self.check_events(&request.events, request.t_end)?;
        let substeps = substeps(request.output_step, request.dt) as usize;
        let h = request.output_step / substeps as f64;
        if request.t_end / h > MAX_STEPS {
            return Err(format!(
                "t_end / dt = {:.3e} steps exceeds the limit of {MAX_STEPS:e}; use a larger dt or a shorter run",
                request.t_end / h
            ));
        }
        let targets = output_times(request.t_end, request.output_step);
        let mut sys = self.sys.clone();
        let mut x = self.x0.clone();
        let mut f = vec![0.0; sys.nx];
        sys.residual(&x, &mut f, None);
        let mut events = request.events.clone();
        events.sort_by(|a, b| a.time().total_cmp(&b.time()));
        let mut next_event = 0;
        let tol = 1e-9 * h;
        let mut t = 0.0_f64;
        #[cfg_attr(not(feature = "rms-experimental"), allow(unused_mut))]
        let mut k0 = 0usize;
        let mut stepper = integrate::Stepper::new();
        #[cfg(not(feature = "rms-experimental"))]
        let _ = start;
        #[cfg(feature = "rms-experimental")]
        if let Some(cp) = start {
            if cp.x.len() != sys.nx {
                return Err(format!(
                    "the checkpoint has {} states, this system {}",
                    cp.x.len(),
                    sys.nx
                ));
            }
            // The earlier events act on the checkpoint's state: a unit
            // tripped before it keeps the states it was tripped with, which
            // the run held from then on.
            let mut scratch = cp.x.clone();
            let mut changed = false;
            while next_event < events.len() && events[next_event].time() < cp.t - tol {
                let at = events[next_event].time();
                apply_event(&mut sys, &mut scratch, &events[next_event])?;
                topology::extinguish(&mut sys, at);
                next_event += 1;
                changed = true;
            }
            if changed {
                sys.rebuild_ybus();
                sys.rebuild_mass();
            }
            x = cp.x.clone();
            sys.residual(&x, &mut f, None);
            if let Some(s) = &cp.solver {
                if s.at.len() != sys.nx {
                    return Err("the checkpoint's solver state does not fit this system".into());
                }
                stepper.restore_factor(&sys, &s.at, s.h, s.theta)?;
            }
            if cp.past.iter().any(|p| p.x.len() != sys.nx) {
                return Err("the checkpoint's earlier points do not fit this system".into());
            }
            stepper.restore_past(&sys, cp.past.iter().map(|p| (p.h, p.x.as_slice())));
            t = cp.t;
            k0 = targets.partition_point(|&tk| tk <= cp.t + tol);
        }
        #[cfg(feature = "rms-experimental")]
        let checkpoint_step = match (start, &request.record) {
            (None, Some(r)) if r.checkpoint_step > 0.0 => r.checkpoint_step,
            _ => 0.0,
        };
        #[cfg(feature = "rms-experimental")]
        let mut checkpoints: Vec<Checkpoint> = Vec::new();
        #[cfg(feature = "rms-experimental")]
        let mut next_checkpoint = 0.0_f64;
        let mut rec = Recorder::new(&sys, request.record.clone());
        rec.record(&sys, t, &x);
        #[cfg(feature = "rms-experimental")]
        if checkpoint_step > 0.0 {
            checkpoints.push(Checkpoint {
                t,
                x: x.clone(),
                solver: None,
                past: Vec::new(),
            });
            next_checkpoint = t + checkpoint_step;
        }
        let mut applied: Vec<String> = Vec::new();
        let mut steps = 0usize;
        let mut iterations = 0usize;
        let mut euler_steps = 0usize;
        for &t_target in &targets[k0..] {
            if cancelled() {
                return Err("cancelled".into());
            }
            while t < t_target - tol {
                // Apply every event due now, then re-solve the algebraic part.
                let mut changed = false;
                while next_event < events.len() && events[next_event].time() <= t + tol {
                    applied.push(apply_event(&mut sys, &mut x, &events[next_event])?);
                    next_event += 1;
                    changed = true;
                }
                if changed {
                    applied.extend(topology::extinguish(&mut sys, t));
                    sys.rebuild_ybus();
                    sys.rebuild_mass();
                    sys.residual(&x, &mut f, None);
                    iterations += stepper
                        .reinit(&sys, &mut x, &mut f)
                        .map_err(|e| format!("re-solving the network at t = {t:.4} s: {e}"))?;
                }
                let mut t_next = (t + h).min(t_target);
                if next_event < events.len() {
                    t_next = t_next.min(events[next_event].time());
                }
                iterations += step_adaptive(
                    &sys,
                    &mut x,
                    &mut f,
                    t_next - t,
                    0,
                    &mut euler_steps,
                    &mut stepper,
                )
                .map_err(|e| {
                    let islands: Vec<String> =
                        sys.detached_islands().into_iter().map(|(_, s)| s).collect();
                    if islands.is_empty() {
                        format!("integration failed at t = {t:.4} s: {e}")
                    } else {
                        format!(
                            "integration failed at t = {t:.4} s: {e}; for reference, the network is \
                             split at that time, with an island of {}",
                            islands.join("; an island of ")
                        )
                    }
                })?;
                steps += 1;
                t = t_next;
            }
            t = t_target;
            rec.record(&sys, t, &x);
            #[cfg(feature = "rms-experimental")]
            if checkpoint_step > 0.0 && t >= next_checkpoint - tol {
                let solver = stepper
                    .solver_state(&sys)
                    .map(|(at, h, theta)| SolverState { at, h, theta });
                let past = stepper
                    .past(&sys)
                    .into_iter()
                    .map(|(h, x)| PastPoint { h, x })
                    .collect();
                checkpoints.push(Checkpoint {
                    t,
                    x: x.clone(),
                    solver,
                    past,
                });
                next_checkpoint = t + checkpoint_step;
            }
        }
        let (time, channels, frames) = rec.take();
        #[cfg_attr(feature = "rms-experimental", allow(unused_mut))]
        let mut eigen_note: Vec<String> = Vec::new();
        #[cfg(feature = "rms-experimental")]
        let eigenvalues = if request.small_signal && start.is_none() {
            self.eigenvalues().unwrap_or_else(|e| {
                eigen_note.push(e);
                Vec::new()
            })
        } else {
            Vec::new()
        };
        #[cfg(not(feature = "rms-experimental"))]
        let eigenvalues = {
            if request.small_signal {
                eigen_note.push(
                    "small-signal analysis needs the rms-experimental feature; not computed"
                        .to_owned(),
                );
            }
            Vec::new()
        };
        let generators = self
            .sys
            .devices
            .iter()
            .map(|d| RmsGenerator {
                bus: d.bus_number,
                machine_id: d.machine_id.clone(),
                models: vec![
                    d.machine.name().into(),
                    d.avr.name(),
                    d.gov.name(),
                    d.pss.name(),
                ],
                states: d.state_names(),
                initial_states: self.x0[d.offset..d.offset + d.n_states()].to_vec(),
                v_ref: d.v_ref,
                p_ref: d.p_ref,
                mbase: d.scale * self.sys.sbase,
            })
            .collect();
        Ok(RmsResult {
            time,
            channels,
            frames,
            #[cfg(feature = "rms-experimental")]
            checkpoints,
            buses: self.sys.bus_numbers.clone(),
            base_mva: self.sys.sbase,
            base_frequency: self.sys.f0,
            generators,
            sources: self
                .sys
                .sources
                .iter()
                .map(|s| format!("{}-{}", s.bus_number, s.machine_id))
                .collect(),
            eigenvalues,
            events: applied,
            notes: {
                let mut notes = self.notes.clone();
                notes.extend(eigen_note);
                if euler_steps > 0 {
                    notes.push(format!(
                        "{euler_steps} step(s) fell back from the trapezoidal rule to backward Euler (limiter or rectifier switching inside the step)"
                    ));
                }
                notes
            },
            steps,
            newton_iterations: iterations,
        })
    }
}

/// Take a trapezoidal step in place; on Newton failure fall back to an
/// L-stable backward-Euler step (which damps the chatter of limiters and
/// rectifiers that switch inside the step), then halve the step. On an error
/// `x` and `f` are unchanged.
fn step_adaptive(
    sys: &DynSystem,
    x: &mut Vec<f64>,
    f: &mut Vec<f64>,
    h: f64,
    depth: usize,
    euler_steps: &mut usize,
    stepper: &mut integrate::Stepper,
) -> Result<usize, String> {
    match stepper.step(sys, x, f, h, 0.5) {
        Ok(ok) => Ok(ok),
        Err(trap) => {
            if let Ok(ok) = stepper.step(sys, x, f, h, 1.0) {
                *euler_steps += 1;
                return Ok(ok);
            }
            if depth >= 10 {
                return Err(trap);
            }
            let start = (x.clone(), f.clone());
            let halves = step_adaptive(sys, x, f, 0.5 * h, depth + 1, euler_steps, stepper)
                .and_then(|i1| {
                    step_adaptive(sys, x, f, 0.5 * h, depth + 1, euler_steps, stepper)
                        .map(|i2| i1 + i2)
                });
            if halves.is_err() {
                (*x, *f) = start;
                stepper.forget();
            }
            halves
        }
    }
}

fn find_branch(sys: &DynSystem, from: usize, to: usize, circuit: &str) -> Result<usize, String> {
    sys.branch_keys
        .iter()
        .position(|(f, t, c)| {
            ((*f == from && *t == to) || (*f == to && *t == from)) && c.trim() == circuit.trim()
        })
        .ok_or_else(|| format!("no branch {from}-{to} circuit {circuit}"))
}

fn apply_event(sys: &mut DynSystem, x: &mut [f64], event: &RmsEvent) -> Result<String, String> {
    let bus = |sys: &DynSystem, b: usize| sys.dense_bus(b).ok_or_else(|| format!("no bus {b}"));
    Ok(match event {
        RmsEvent::BranchTrip {
            time,
            from,
            to,
            circuit,
        } => {
            let e = find_branch(sys, *from, *to, circuit)?;
            sys.ac.sw[e] = 0.0;
            format!("t = {time} s: branch {from}-{to} circuit {circuit} opened")
        }
        RmsEvent::BranchClose {
            time,
            from,
            to,
            circuit,
        } => {
            let e = find_branch(sys, *from, *to, circuit)?;
            sys.ac.sw[e] = 1.0;
            format!("t = {time} s: branch {from}-{to} circuit {circuit} closed")
        }
        RmsEvent::BusFault {
            time,
            bus: b,
            r,
            x: xf,
        } => {
            let k = bus(sys, *b)?;
            sys.fault_y[k] = fault_admittance(*r, *xf)?;
            format!("t = {time} s: fault at bus {b} through {r} + j{xf} pu")
        }
        RmsEvent::ClearFault { time, bus: b } => {
            let k = bus(sys, *b)?;
            sys.fault_y[k] = C64::new(0.0, 0.0);
            format!("t = {time} s: fault at bus {b} cleared")
        }
        RmsEvent::LoadChange {
            time,
            bus: b,
            scale,
        } => {
            let k = bus(sys, *b)?;
            if !(scale.is_finite() && *scale >= 0.0) {
                return Err(format!(
                    "load scale {scale} is not a finite number >= 0 (a negative scale turns loads into sources)"
                ));
            }
            sys.loads[k].scale(*scale, *scale);
            format!("t = {time} s: load at bus {b} scaled by {scale}")
        }
        RmsEvent::SetReference {
            time,
            bus: b,
            machine_id,
            signal,
            value,
        } => {
            if !value.is_finite() {
                return Err(format!("reference value {value} is not finite"));
            }
            let d = sys
                .devices
                .iter_mut()
                .find(|d| d.bus_number == *b && d.machine_id == machine_id.trim())
                .ok_or_else(|| format!("no dynamic generator {b} '{machine_id}'"))?;
            let name = match signal {
                InputSignal::VRef => {
                    d.v_ref = *value;
                    "V_ref"
                }
                InputSignal::PRef => {
                    d.p_ref = *value;
                    "P_ref"
                }
            };
            format!("t = {time} s: generator {b} '{machine_id}' {name} = {value}")
        }
        RmsEvent::GeneratorTrip {
            time,
            bus: b,
            machine_id,
        } => {
            if let Some(d) = sys
                .devices
                .iter_mut()
                .find(|d| d.bus_number == *b && d.machine_id == machine_id.trim())
            {
                d.tripped = Some(x[d.offset..d.offset + d.n_states()].to_vec());
            } else if let Some(s) = sys
                .sources
                .iter_mut()
                .find(|s| s.bus_number == *b && s.machine_id == machine_id.trim())
            {
                s.y = C64::new(0.0, 0.0);
                s.e = C64::new(0.0, 0.0);
            } else if let Some(k) = sys
                .held
                .iter()
                .position(|u| u.bus_number == *b && u.machine_id == machine_id.trim())
            {
                topology::trip_held(sys, k);
            } else {
                return Err(format!("no generator {b} '{machine_id}'"));
            }
            format!("t = {time} s: generator {b} '{machine_id}' tripped")
        }
        RmsEvent::LineFault {
            time,
            from,
            to,
            circuit,
            location,
            r,
            x: xf,
            permanent,
        } => {
            let e = find_branch(sys, *from, *to, circuit)?;
            if !(0.0..=1.0).contains(location) {
                return Err(format!("fault location {location} must lie in [0, 1]"));
            }
            // Measured from the branch's own from end.
            let alpha = if sys.branch_keys[e].0 == *from {
                *location
            } else {
                1.0 - location
            };
            sys.branch_faults[e] = Some(system::BranchFault {
                alpha,
                y: fault_admittance(*r, *xf)?,
                permanent: *permanent,
            });
            format!(
                "t = {time} s: {} fault at {:.0} % of {from}-{to} circuit {circuit} through {r} + j{xf} pu",
                if *permanent { "permanent" } else { "transient" },
                location * 100.0
            )
        }
        RmsEvent::ClearLineFault {
            time,
            from,
            to,
            circuit,
        } => {
            let e = find_branch(sys, *from, *to, circuit)?;
            sys.branch_faults[e] = None;
            format!("t = {time} s: fault inside {from}-{to} circuit {circuit} cleared")
        }
        RmsEvent::BreakerOpen {
            time,
            from,
            to,
            circuit,
            at,
        }
        | RmsEvent::BreakerClose {
            time,
            from,
            to,
            circuit,
            at,
        } => {
            let e = find_branch(sys, *from, *to, circuit)?;
            let k = if sys.branch_keys[e].0 == *at {
                0
            } else if sys.branch_keys[e].1 == *at {
                1
            } else {
                return Err(format!("bus {at} is not an end of {from}-{to}"));
            };
            let open = matches!(event, RmsEvent::BreakerOpen { .. });
            sys.end_open[e][k] = open;
            format!(
                "t = {time} s: breaker of {from}-{to} circuit {circuit} at bus {at} {}",
                if open { "opened" } else { "closed" }
            )
        }
    })
}

/// The smallest integration step a run takes, s.
const MIN_DT: f64 = 1e-6;

/// The most integration steps a run takes (`t_end / dt`).
const MAX_STEPS: f64 = 1e7;

/// The output times after `t = 0`: every whole output step up to `t_end`,
/// then `t_end` itself when it is not one (a last, shorter interval), so a
/// run ends exactly at `t_end`.
fn output_times(t_end: f64, output_step: f64) -> Vec<f64> {
    let tol = 1e-9 * output_step;
    let n = ((t_end + tol) / output_step).floor() as usize;
    let mut times: Vec<f64> = (1..=n).map(|k| k as f64 * output_step).collect();
    if times.last().is_none_or(|&t| t_end - t > tol) {
        times.push(t_end);
    }
    times
}

/// Integration steps per output step: the step is at most `dt` (never
/// coarser than asked), the output step divided evenly.
fn substeps(output_step: f64, dt: f64) -> f64 {
    (output_step / dt * (1.0 - 1e-9)).ceil().max(1.0)
}

/// The admittance of a fault through `r + jx`; a fault needs a finite,
/// nonzero impedance.
fn fault_admittance(r: f64, x: f64) -> Result<C64, String> {
    let z = C64::new(r, x);
    if r < 0.0 || x < 0.0 {
        return Err(format!(
            "a fault impedance needs r >= 0 and x >= 0, not {r} + j{x} (a negative part makes the fault a source)"
        ));
    }
    if !(z.norm() > 0.0 && z.norm().is_finite()) {
        return Err(format!(
            "a fault needs a finite, nonzero impedance r + jx, not {r} + j{x} (use a small reactance for a bolted fault)"
        ));
    }
    Ok(C64::new(1.0, 0.0) / z)
}

/// Choices of one prepared simulation, fixed when it is built. The defaults
/// are tellegen's own; the others exist to compare with another tool or
/// method, and affect only the simulation they are given to.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RmsOptions {
    /// Simulate renewable plants (`REGCA1` + `REECB1` + `REPCA1`), the
    /// default, or hold them at their power flow output as constant currents.
    pub renewable_plants: bool,
    /// Use PowerSimulationsDynamics.jl 0.16.2's low-voltage active current
    /// gain for renewable plants (negative between its two points,
    /// `get_LV_current_gain`) instead of the PSS/E one.
    pub psid_lv_gain: bool,
    /// Apply a renewable plant's converter ramp limits (`REGCA1` `Rrpwr`,
    /// `Iqrmax`, `Iqrmin`) to the `Tg` lag's error as
    /// PowerSimulationsDynamics.jl 0.16.2 does (an effective rate
    /// `Rrpwr/Tg`) instead of to the currents' rate in pu/s (PSS/E).
    pub psid_converter_ramp: bool,
    /// Evaluate the quadratic saturation of machines and `ESAC1A` below its
    /// knee `A` as PowerSimulationsDynamics.jl 0.16.2 does (the formula
    /// `B (x − A)² / x`, which rises again below `A`) instead of PSS/E's
    /// `Se = 0` there.
    pub psid_saturation: bool,
    /// Factorize the step matrices by blocks where every unit allows it, the
    /// default, or always as one sparse LU over all unknowns.
    pub factorize_by_blocks: bool,
    /// Low-voltage behaviour of the constant-power and constant-current load
    /// parts: PSS/E's `PQBRAK` conversion at 0.7 pu by default, or the exact
    /// law (PowerSimulationsDynamics.jl).
    pub low_voltage_load: LowVoltageLoad,
    /// Initialize from the AC power flow with the generators' reactive
    /// limits enforced (PV→PQ switching), the default and the same solve as
    /// the static study, or with every generator at its voltage setpoint
    /// whatever reactive power that takes (PowerSimulationsDynamics.jl, and
    /// PSS/E from a case solved without limits). Either way the result's
    /// notes name every unit whose solved reactive power and setpoint
    /// disagree, and by how much.
    pub reactive_limits: bool,
}

impl Default for RmsOptions {
    fn default() -> Self {
        Self {
            renewable_plants: true,
            psid_lv_gain: false,
            psid_converter_ramp: false,
            psid_saturation: false,
            factorize_by_blocks: true,
            low_voltage_load: LowVoltageLoad::default(),
            reactive_limits: true,
        }
    }
}

/// What a run records. Without one ([`RmsRequest::record`] `None`) every
/// channel is recorded at every output step.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RecordSpec {
    /// Channels recorded at every output step: exact keys (`omega:2178`) or
    /// prefixes ending in `:` (`omega:` for every unit's speed). Empty: every
    /// channel.
    #[serde(default)]
    pub channels: Vec<String>,
    /// Channels recorded besides every `frame_step` seconds, in
    /// [`RmsResult::frames`] (the map's bus voltages and angles).
    #[serde(default)]
    pub frames: Vec<String>,
    #[serde(default)]
    pub frame_step: f64,
    /// The full state every `checkpoint_step` seconds (0: none), in
    /// [`RmsResult::checkpoints`]: any channel can be replayed from them
    /// ([`RmsSimulation::replay`]).
    #[cfg(feature = "rms-experimental")]
    #[serde(default)]
    pub checkpoint_step: f64,
}

impl RecordSpec {
    fn matches(patterns: &[String], key: &str) -> bool {
        patterns
            .iter()
            .any(|p| key == p || (p.ends_with(':') && key.starts_with(p.as_str())))
    }
}

/// Channels sampled at their own step (the map frames of a run).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RmsSeries {
    pub time: Vec<f64>,
    pub channels: BTreeMap<String, Vec<f64>>,
}

/// The full state at `t` (differential and algebraic, in the engine's order)
/// and the integrator's reusable factorization there, so a replay from it
/// continues the run sample for sample.
#[cfg(feature = "rms-experimental")]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Checkpoint {
    pub t: f64,
    pub x: Vec<f64>,
    /// The cached Jacobian factorization: the iterate it was taken at, its
    /// step and method (none when the next step factorizes afresh).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub solver: Option<SolverState>,
    /// The points the integrator's predictor extrapolates from, newest
    /// first (none right after an event).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub past: Vec<PastPoint>,
}

/// A point of the run before a checkpoint: the full state and the step that
/// led away from it.
#[cfg(feature = "rms-experimental")]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PastPoint {
    pub h: f64,
    pub x: Vec<f64>,
}

/// What rebuilds an integrator factorization exactly.
#[cfg(feature = "rms-experimental")]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SolverState {
    pub at: Vec<f64>,
    pub h: f64,
    pub theta: f64,
}

/// No checkpoints without `rms-experimental`: a run starts at `t = 0`.
#[cfg(not(feature = "rms-experimental"))]
type Checkpoint = std::convert::Infallible;

/// Where a channel's value comes from.
#[derive(Clone, Copy, Debug)]
enum Source {
    Vm(usize),
    Va(usize),
    Delta(usize),
    Omega(usize),
    Efd(usize),
    Tm(usize),
    Vs(usize),
    Pe(usize),
    Qe(usize),
    /// Device, index into its graph signals.
    Sig(usize, usize),
    /// Held unit (constant current or simulated plant): active, reactive
    /// power out, system base.
    HeldPe(usize),
    HeldQe(usize),
    /// Simulated plant, local state index.
    PlantState(usize, usize),
    /// Power drawn by the loads of a bus (not its held units), system base.
    Pl(usize),
    Ql(usize),
    PFrom(usize),
    QFrom(usize),
    PTo(usize),
    QTo(usize),
}

/// One recorded set of channels: indices into the catalog, its step and the
/// samples (sample-major).
struct Group {
    sel: Vec<usize>,
    /// Record every `step` seconds; 0: at every output sample.
    step: f64,
    next: f64,
    time: Vec<f64>,
    values: Vec<f64>,
    /// Which inputs the selection needs.
    outs: bool,
    sig: bool,
}

struct Recorder {
    /// Further channels recorded at every output sample (aligned with the
    /// main group).
    channels: BTreeMap<String, Vec<f64>>,
    gen_keys: Vec<String>,
    branch_keys: Vec<String>,
    outs: Vec<GenOut>,
    spec: Option<RecordSpec>,
    /// Every channel, built at the first sample (graph signal names need
    /// the state): its key and source.
    catalog: Vec<(String, Source)>,
    /// Counts the catalogs built: the channels of the groups keep their
    /// order while it stays ([`RmsLive::advance_columns`]).
    layout: u32,
    main: Group,
    frames: Option<Group>,
}

impl Recorder {
    fn new(sys: &DynSystem, spec: Option<RecordSpec>) -> Self {
        let gen_keys = sys
            .devices
            .iter()
            .map(|d| {
                if d.machine_id == "1" {
                    d.bus_number.to_string()
                } else {
                    format!("{}#{}", d.bus_number, d.machine_id)
                }
            })
            .collect();
        // Lines and transformers keep separate PSS/E circuit-id spaces, so a
        // line and a transformer between the same buses can share `from-to`;
        // a repeated key takes the branch index, so every channel is one series.
        let mut seen = std::collections::BTreeSet::new();
        let branch_keys = sys
            .branch_keys
            .iter()
            .enumerate()
            .map(|(e, (f, t, c))| {
                let key = if c.trim() == "1" {
                    format!("{f}-{t}")
                } else {
                    format!("{f}-{t}#{}", c.trim())
                };
                if seen.insert(key.clone()) {
                    key
                } else {
                    format!("{key}@{e}")
                }
            })
            .collect();
        let group = |step: f64| Group {
            sel: Vec::new(),
            step,
            next: 0.0,
            time: Vec::new(),
            values: Vec::new(),
            outs: false,
            sig: false,
        };
        let frames = spec
            .as_ref()
            .filter(|s| !s.frames.is_empty() && s.frame_step > 0.0)
            .map(|s| group(s.frame_step));
        Self {
            channels: BTreeMap::new(),
            gen_keys,
            branch_keys,
            outs: Vec::new(),
            spec,
            catalog: Vec::new(),
            layout: 0,
            main: group(0.0),
            frames,
        }
    }

    /// The catalog of every channel and the groups' selections.
    fn build_catalog(&mut self, sys: &DynSystem, x: &[f64]) {
        let n = sys.n;
        let mut cat: Vec<(String, Source)> = Vec::new();
        for (b, &number) in sys.bus_numbers.iter().enumerate() {
            cat.push((format!("vm:{number}"), Source::Vm(b)));
            cat.push((format!("va:{number}"), Source::Va(b)));
        }
        for (k, d) in sys.devices.iter().enumerate() {
            let key = &self.gen_keys[k];
            cat.push((format!("delta:{key}"), Source::Delta(k)));
            cat.push((format!("omega:{key}"), Source::Omega(k)));
            if !matches!(d.machine, models::Machine::Classic { .. }) {
                cat.push((format!("efd:{key}"), Source::Efd(k)));
            }
            cat.push((format!("tm:{key}"), Source::Tm(k)));
            if !matches!(d.pss, models::Stabilizer::None) {
                cat.push((format!("vs:{key}"), Source::Vs(k)));
            }
            cat.push((format!("pe:{key}"), Source::Pe(k)));
            cat.push((format!("qe:{key}"), Source::Qe(k)));
            let (vr, vi) = (x[d.bus], x[n + d.bus]);
            for (j, (name, _)) in d
                .graph_signals(&x[d.offset..d.offset + d.n_states()], vr, vi)
                .into_iter()
                .enumerate()
            {
                cat.push((format!("sig:{name}:{key}"), Source::Sig(k, j)));
            }
        }
        for (k, h) in sys.held.iter().enumerate() {
            let key = if h.machine_id == "1" {
                h.bus_number.to_string()
            } else {
                format!("{}#{}", h.bus_number, h.machine_id)
            };
            cat.push((format!("pe:{key}"), Source::HeldPe(k)));
            cat.push((format!("qe:{key}"), Source::HeldQe(k)));
            if let Some(p) = &h.plant {
                for (j, name) in p.state_names().into_iter().enumerate() {
                    cat.push((format!("inv.{name}:{key}"), Source::PlantState(k, j)));
                }
            }
        }
        for (b, &number) in sys.bus_numbers.iter().enumerate() {
            if sys.loads[b].has_load() {
                cat.push((format!("pl:{number}"), Source::Pl(b)));
                cat.push((format!("ql:{number}"), Source::Ql(b)));
            }
        }
        for e in 0..sys.ac.m {
            let key = &self.branch_keys[e];
            cat.push((format!("p_from:{key}"), Source::PFrom(e)));
            cat.push((format!("q_from:{key}"), Source::QFrom(e)));
            cat.push((format!("p_to:{key}"), Source::PTo(e)));
            cat.push((format!("q_to:{key}"), Source::QTo(e)));
        }
        let select = |patterns: Option<&[String]>| -> Vec<usize> {
            match patterns {
                Some(p) if !p.is_empty() => (0..cat.len())
                    .filter(|&i| RecordSpec::matches(p, &cat[i].0))
                    .collect(),
                _ => (0..cat.len()).collect(),
            }
        };
        let needs = |sel: &[usize]| {
            let outs = sel.iter().any(|&i| {
                matches!(
                    cat[i].1,
                    Source::Efd(_) | Source::Tm(_) | Source::Vs(_) | Source::Pe(_) | Source::Qe(_)
                )
            });
            let sig = sel.iter().any(|&i| matches!(cat[i].1, Source::Sig(..)));
            (outs, sig)
        };
        self.main.sel = select(self.spec.as_ref().map(|s| s.channels.as_slice()));
        (self.main.outs, self.main.sig) = needs(&self.main.sel);
        if let (Some(g), Some(s)) = (self.frames.as_mut(), self.spec.as_ref()) {
            g.sel = select(Some(s.frames.as_slice()));
            (g.outs, g.sig) = needs(&g.sel);
        }
        self.catalog = cat;
        self.layout += 1;
    }

    /// Change what is recorded from the next sample on (a live session's
    /// watch set). Samples recorded so far stay in their groups.
    #[cfg(feature = "rms-experimental")]
    fn set_spec(&mut self, spec: Option<RecordSpec>) {
        let step = spec
            .as_ref()
            .filter(|s| !s.frames.is_empty() && s.frame_step > 0.0)
            .map(|s| s.frame_step);
        self.spec = spec;
        match (step, self.frames.as_mut()) {
            (Some(st), Some(g)) => g.step = st,
            (Some(st), None) => {
                self.frames = Some(Group {
                    sel: Vec::new(),
                    step: st,
                    next: 0.0,
                    time: Vec::new(),
                    values: Vec::new(),
                    outs: false,
                    sig: false,
                });
            }
            (None, _) => self.frames = None,
        }
        // Rebuilt at the next sample, with the new selection; a group whose
        // selection changes starts its samples over.
        self.catalog.clear();
        self.main.time.clear();
        self.main.values.clear();
        if let Some(g) = &mut self.frames {
            g.time.clear();
            g.values.clear();
        }
    }

    /// Move out the samples recorded so far: the main channels and the
    /// frames.
    fn take(&mut self) -> (Vec<f64>, BTreeMap<String, Vec<f64>>, Option<RmsSeries>) {
        let mut channels = std::mem::take(&mut self.channels);
        let time = std::mem::take(&mut self.main.time);
        let values = std::mem::take(&mut self.main.values);
        if !time.is_empty() {
            Self::unpack(
                &self.catalog,
                &self.main.sel,
                &values,
                time.len(),
                &mut channels,
            );
        }
        // No frame in this span: no frame series (a live chunk between frames).
        let frames = self.frames.as_mut().and_then(|g| {
            let t = std::mem::take(&mut g.time);
            let v = std::mem::take(&mut g.values);
            if t.is_empty() {
                return None;
            }
            let mut ch = BTreeMap::new();
            Self::unpack(&self.catalog, &g.sel, &v, t.len(), &mut ch);
            Some(RmsSeries {
                time: t,
                channels: ch,
            })
        });
        (time, channels, frames)
    }

    /// Move out the samples recorded so far as they were recorded: per
    /// group its times and its values, sample after sample in the order of
    /// [`Self::keys`] (nothing is copied per channel).
    #[cfg(feature = "rms-experimental")]
    fn take_columns(&mut self) -> [(Vec<f64>, Vec<f64>); 2] {
        let main = (
            std::mem::take(&mut self.main.time),
            std::mem::take(&mut self.main.values),
        );
        let frames = self.frames.as_mut().map_or_else(Default::default, |g| {
            (std::mem::take(&mut g.time), std::mem::take(&mut g.values))
        });
        [main, frames]
    }

    /// The channel keys of the main group and of the frames, in the order
    /// of their values.
    #[cfg(feature = "rms-experimental")]
    fn keys(&self) -> [Vec<&str>; 2] {
        let of = |sel: &[usize]| sel.iter().map(|&i| self.catalog[i].0.as_str()).collect();
        [
            of(&self.main.sel),
            self.frames.as_ref().map_or_else(Vec::new, |g| of(&g.sel)),
        ]
    }

    fn unpack(
        catalog: &[(String, Source)],
        sel: &[usize],
        values: &[f64],
        samples: usize,
        out: &mut BTreeMap<String, Vec<f64>>,
    ) {
        let w = sel.len();
        for (j, &i) in sel.iter().enumerate() {
            let series: Vec<f64> = (0..samples).map(|s| values[s * w + j]).collect();
            out.insert(catalog[i].0.clone(), series);
        }
    }

    /// One value of every channel at state `x` (the live session's snapshot
    /// for on-demand channels).
    #[cfg(feature = "rms-experimental")]
    fn snapshot(
        &mut self,
        sys: &DynSystem,
        x: &[f64],
        patterns: &[String],
    ) -> BTreeMap<String, f64> {
        if self.catalog.is_empty() {
            self.build_catalog(sys, x);
        }
        let sel: Vec<usize> = (0..self.catalog.len())
            .filter(|&i| patterns.is_empty() || RecordSpec::matches(patterns, &self.catalog[i].0))
            .collect();
        let outs = sel.iter().any(|&i| {
            matches!(
                self.catalog[i].1,
                Source::Efd(_) | Source::Tm(_) | Source::Vs(_) | Source::Pe(_) | Source::Qe(_)
            )
        });
        let sig = sel
            .iter()
            .any(|&i| matches!(self.catalog[i].1, Source::Sig(..)));
        let mut vals = Vec::with_capacity(sel.len());
        self.eval(sys, x, &sel, outs, sig, &mut vals);
        sel.iter()
            .zip(vals)
            .map(|(&i, v)| (self.catalog[i].0.clone(), v))
            .collect()
    }

    /// Append the values of `sel` at state `x` to `out`.
    fn eval(
        &mut self,
        sys: &DynSystem,
        x: &[f64],
        sel: &[usize],
        outs: bool,
        sig: bool,
        out: &mut Vec<f64>,
    ) {
        let n = sys.n;
        if outs {
            let mut f = vec![0.0; sys.nx];
            let mut o = std::mem::take(&mut self.outs);
            sys.residual(x, &mut f, Some(&mut o));
            self.outs = o;
        }
        let _ = sig;
        let mut sigs: Vec<Option<Vec<f64>>> = Vec::new();
        if sel
            .iter()
            .any(|&i| matches!(self.catalog[i].1, Source::Sig(..)))
        {
            sigs = vec![None; sys.devices.len()];
        }
        let mut flows: std::collections::HashMap<usize, (C64, C64)> =
            std::collections::HashMap::new();
        for &i in sel {
            let v = match self.catalog[i].1 {
                Source::Vm(b) => Complex::new(x[b], x[n + b]).norm(),
                Source::Va(b) => Complex::new(x[b], x[n + b]).arg(),
                Source::Delta(k) => {
                    let d = &sys.devices[k];
                    let [_, s, ..] = d.layout();
                    x[d.offset + s]
                }
                Source::Omega(k) => {
                    let d = &sys.devices[k];
                    let [_, s, ..] = d.layout();
                    x[d.offset + s + 1]
                }
                Source::Efd(k) => self.outs[k].vf,
                Source::Tm(k) => self.outs[k].tm,
                Source::Vs(k) => self.outs[k].vs,
                Source::Pe(k) => {
                    let d = &sys.devices[k];
                    let o = &self.outs[k];
                    x[d.bus] * o.ir + x[n + d.bus] * o.ii
                }
                Source::Qe(k) => {
                    let d = &sys.devices[k];
                    let o = &self.outs[k];
                    x[n + d.bus] * o.ir - x[d.bus] * o.ii
                }
                Source::Sig(k, j) => {
                    let s = sigs[k].get_or_insert_with(|| {
                        let d = &sys.devices[k];
                        d.graph_signals(
                            &x[d.offset..d.offset + d.n_states()],
                            x[d.bus],
                            x[n + d.bus],
                        )
                        .into_iter()
                        .map(|(_, v)| v)
                        .collect()
                    });
                    s.get(j).copied().unwrap_or(f64::NAN)
                }
                Source::HeldPe(k) | Source::HeldQe(k) => {
                    let h = &sys.held[k];
                    let v = Complex::new(x[h.bus], x[n + h.bus]);
                    let s = v * h.current_at(x, v).conj();
                    if matches!(self.catalog[i].1, Source::HeldPe(_)) {
                        s.re
                    } else {
                        s.im
                    }
                }
                Source::PlantState(k, j) => x[sys.held[k].offset + j],
                Source::Pl(b) => {
                    sys.loads[b]
                        .power(Complex::new(x[b], x[n + b]), sys.low_voltage)
                        .re
                }
                Source::Ql(b) => {
                    sys.loads[b]
                        .power(Complex::new(x[b], x[n + b]), sys.low_voltage)
                        .im
                }
                Source::PFrom(e) => flows.entry(e).or_insert_with(|| sys.branch_flow(x, e)).0.re,
                Source::QFrom(e) => flows.entry(e).or_insert_with(|| sys.branch_flow(x, e)).0.im,
                Source::PTo(e) => flows.entry(e).or_insert_with(|| sys.branch_flow(x, e)).1.re,
                Source::QTo(e) => flows.entry(e).or_insert_with(|| sys.branch_flow(x, e)).1.im,
            };
            out.push(v);
        }
    }

    fn record(&mut self, sys: &DynSystem, t: f64, x: &[f64]) {
        if self.catalog.is_empty() {
            self.build_catalog(sys, x);
        }
        let mut main = std::mem::replace(
            &mut self.main,
            Group {
                sel: Vec::new(),
                step: 0.0,
                next: 0.0,
                time: Vec::new(),
                values: Vec::new(),
                outs: false,
                sig: false,
            },
        );
        let mut frames = self.frames.take();
        let frame_due = frames.as_ref().is_some_and(|g| t >= g.next - 1e-9);
        let outs = main.outs || (frame_due && frames.as_ref().is_some_and(|g| g.outs));
        let sig = main.sig || (frame_due && frames.as_ref().is_some_and(|g| g.sig));
        // Device outputs once for both groups.
        if outs {
            let mut f = vec![0.0; sys.nx];
            let mut o = std::mem::take(&mut self.outs);
            sys.residual(x, &mut f, Some(&mut o));
            self.outs = o;
        }
        main.time.push(t);
        let sel = std::mem::take(&mut main.sel);
        self.eval(sys, x, &sel, false, sig && main.sig, &mut main.values);
        main.sel = sel;
        if let Some(g) = frames.as_mut().filter(|_| frame_due) {
            g.time.push(t);
            let sel = std::mem::take(&mut g.sel);
            self.eval(sys, x, &sel, false, g.sig, &mut g.values);
            g.sel = sel;
            g.next = t + g.step;
        }
        self.main = main;
        self.frames = frames;
    }
}

/// Parse a PSS/E-style case and its DYR text, then simulate.
///
/// # Errors
/// When either input fails to parse or the simulation fails.
pub fn simulate(
    network: &BalancedNetwork,
    dyr_text: &str,
    request: &RmsRequest,
) -> Result<RmsResult, String> {
    let (_, sim) = prepare(network, dyr_text, request.load_model.unwrap_or_default())?;
    sim.run(request)
}

/// Read the dynamic data and assemble the simulation; returns the number of
/// DYR records too. With `rms-experimental` the text may be the JSON form
/// with controller graphs ([`DynamicInput`]).
fn prepare(
    network: &BalancedNetwork,
    dyr_text: &str,
    load_model: LoadModel,
) -> Result<(usize, RmsSimulation), String> {
    #[cfg(feature = "rms-experimental")]
    {
        let (dynamics, models) = read_dynamics(dyr_text)?;
        let sim = RmsSimulation::with_models(network, &dynamics, &models, load_model)?;
        Ok((dynamics.records().len(), sim))
    }
    #[cfg(not(feature = "rms-experimental"))]
    {
        let dynamics = powerio::parse_dyr(dyr_text).map_err(|e| format!("DYR: {e}"))?;
        let sim = RmsSimulation::new(network, &dynamics, load_model)?;
        Ok((dynamics.records().len(), sim))
    }
}

/// DYR text, or the JSON form with controller graphs ([`DynamicInput`]).
#[cfg(feature = "rms-experimental")]
pub(crate) fn read_dynamics(text: &str) -> Result<(DynamicCase, Vec<ModelOverride>), String> {
    let input = DynamicInput::from_text(text)?;
    let dynamics = powerio::parse_dyr(&input.dyr).map_err(|e| format!("DYR: {e}"))?;
    Ok((dynamics, input.models))
}

/// What a case's dynamic data resolves to, for a UI to show before a run.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RmsInspection {
    pub buses: Vec<usize>,
    pub base_mva: f64,
    pub base_frequency: f64,
    pub generators: Vec<RmsGenerator>,
    pub sources: Vec<String>,
    /// `(from, to, circuit)` of every in-service branch.
    pub branches: Vec<(usize, usize, String)>,
    /// Buses carrying load.
    pub load_buses: Vec<usize>,
    pub dyr_records: usize,
    pub eigenvalues: Vec<[f64; 2]>,
    pub notes: Vec<String>,
    pub supported_models: Vec<String>,
}

pub(crate) fn network_from_module_json(module_json: &str) -> Result<BalancedNetwork, String> {
    let module = crate::ir::balanced_module(crate::ir::deserialize_module(module_json)?)?;
    Ok(module.value().clone())
}

/// Assemble and initialize a stored PowerIO module with DYR text and report
/// the linked models, the network elements events can name, and the
/// eigenvalues at the operating point.
///
/// # Errors
/// When the inputs do not parse or the case cannot be initialized.
pub fn inspect_module_json(
    module_json: &str,
    dyr_text: &str,
    load_model: LoadModel,
) -> Result<String, String> {
    let network = network_from_module_json(module_json)?;
    let (dyr_records, sim) = prepare(&network, dyr_text, load_model)?;
    let result = sim.run(&RmsRequest {
        t_end: 0.005,
        dt: 0.005,
        output_step: 0.005,
        load_model: Some(load_model),
        events: Vec::new(),
        small_signal: cfg!(feature = "rms-experimental"),
        record: None,
    })?;
    let load_buses = sim
        .sys
        .loads
        .iter()
        .zip(&sim.sys.bus_numbers)
        .filter(|(l, _)| l.has_load())
        .map(|(_, &b)| b)
        .collect();
    let inspection = RmsInspection {
        buses: result.buses,
        base_mva: result.base_mva,
        base_frequency: result.base_frequency,
        generators: result.generators,
        sources: result.sources,
        branches: sim.branches(),
        load_buses,
        dyr_records,
        eigenvalues: result.eigenvalues,
        notes: result.notes,
        supported_models: powerio::SUPPORTED_MODELS
            .iter()
            .map(|(m, _)| (*m).to_owned())
            .collect(),
    };
    serde_json::to_string(&inspection).map_err(|e| e.to_string())
}

/// Simulate a stored PowerIO module with DYR text and an [`RmsRequest`] JSON;
/// returns the [`RmsResult`] JSON.
///
/// # Errors
/// When the inputs do not parse, initialization fails or the run fails.
pub fn simulate_module_json(
    module_json: &str,
    dyr_text: &str,
    request_json: &str,
) -> Result<String, String> {
    let network = network_from_module_json(module_json)?;
    let request: RmsRequest = if request_json.trim().is_empty() {
        RmsRequest::default()
    } else {
        serde_json::from_str(request_json).map_err(|e| format!("request: {e}"))?
    };
    let result = simulate(&network, dyr_text, &request)?;
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

/// Whether text is PSS/E dynamic data: it parses as DYR and at least one
/// record names a model with a typed view.
pub fn looks_like_dyr(text: &str) -> bool {
    powerio::parse_dyr(text).is_ok_and(|case| {
        case.records()
            .iter()
            .any(|r| powerio::typed_model(r).is_ok())
    })
}
