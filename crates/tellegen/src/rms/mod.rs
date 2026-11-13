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
//! `REPCA1` ([`renewable`]). Loads are constant impedance, constant current
//! or constant power at their power flow voltage.

// Negated comparisons are NaN-safe on purpose (`!(x <= tol)` fails on NaN);
// index loops mirror the row-by-row residual equations.
#![allow(clippy::neg_cmp_op_on_partial_ord, clippy::needless_range_loop)]

pub mod auto;
pub(crate) mod blocks;
mod dyr_params;
pub mod graph;
pub(crate) mod init;
mod integrate;
mod library;
mod linear;
pub(crate) mod models;
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

pub use dyr_params::{
    dyr_edit, dyr_edit_json, dyr_parameters, dyr_parameters_json, DyrEdit, DyrGeneratorView,
    DyrParameter, DyrRecordView,
};
pub use graph::{
    check_graph_json, template, template_json, Block, BlockGraph, DynamicInput, GraphNode,
    ModelOverride, Slot,
};
pub use library::{
    dyr_apply_preset, dyr_dynamize, dyr_dynamize_json, dyr_rekey, dyr_set_model, library,
    library_json, DynamizeRequest, LibraryModel,
};
pub use linear::{
    BodeCurve, BodeSpec, InputSignal, LinearRequest, LinearResult, Mode, OutputSignal,
    Participation,
};

/// How static loads respond to voltage during the simulation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoadModel {
    /// Constant admittance at the power flow voltage (PSS/E `CONL` 100% Z).
    #[default]
    ConstantImpedance,
    ConstantCurrent,
    ConstantPower,
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
    #[serde(default)]
    pub load_model: LoadModel,
    #[serde(default)]
    pub events: Vec<RmsEvent>,
    /// Compute the eigenvalues of the linearized system at the initial point.
    #[serde(default = "default_true")]
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
            load_model: LoadModel::default(),
            events: Vec::new(),
            small_signal: true,
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
/// `pe`, `qe` on the system base) and `quantity:from-to` for branch flows
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
    /// Eigenvalues `[re, im]` at the initial operating point.
    pub eigenvalues: Vec<[f64; 2]>,
    pub events: Vec<String>,
    pub notes: Vec<String>,
    pub steps: usize,
    pub newton_iterations: usize,
    /// The channels of [`RecordSpec::frames`] at their own step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frames: Option<RmsSeries>,
    /// The full state at [`RecordSpec::checkpoint_step`] intervals.
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
        Self::with_models(network, dynamics, &[], load_model)
    }

    /// Assemble and initialize with controller graphs replacing (or adding)
    /// the DYR controllers of the generators they name.
    ///
    /// # Errors
    /// As [`Self::new`], or when a graph does not compile.
    pub fn with_models(
        network: &BalancedNetwork,
        dynamics: &DynamicCase,
        models: &[ModelOverride],
        load_model: LoadModel,
    ) -> Result<Self, String> {
        Self::with_options(network, dynamics, models, load_model, RmsOptions::default())
    }

    /// As [`Self::with_models`], with the [`RmsOptions`] of this simulation.
    ///
    /// # Errors
    /// As [`Self::with_models`].
    pub fn with_options(
        network: &BalancedNetwork,
        dynamics: &DynamicCase,
        models: &[ModelOverride],
        load_model: LoadModel,
        options: RmsOptions,
    ) -> Result<Self, String> {
        let (sys, x0, report) = init::build(network, dynamics, models, load_model, options)?;
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
    pub fn linear_analysis(&self, request: &LinearRequest) -> Result<LinearResult, String> {
        let (states, modes) = linear::modes(&self.sys, &self.x0, request.participation_top)?;
        let bode = linear::bode(&self.sys, &self.x0, request)?;
        Ok(LinearResult {
            states,
            modes,
            bode,
        })
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
    pub fn replay(&self, request: &RmsRequest, from: &Checkpoint) -> Result<RmsResult, String> {
        self.run_from(request, Some(from), &mut || false)
    }

    fn run_from(
        &self,
        request: &RmsRequest,
        start: Option<&Checkpoint>,
        cancelled: &mut dyn FnMut() -> bool,
    ) -> Result<RmsResult, String> {
        if request.load_model != self.load_model {
            return Err("the request's load model differs from the prepared simulation's".into());
        }
        if !(request.dt > 0.0 && request.output_step >= request.dt && request.t_end > 0.0) {
            return Err("need dt > 0, output_step >= dt and t_end > 0".into());
        }
        let substeps = substeps(request.output_step, request.dt) as usize;
        let h = request.output_step / substeps as f64;
        let mut sys = self.sys.clone();
        let mut x = self.x0.clone();
        let mut f = vec![0.0; sys.nx];
        sys.residual(&x, &mut f, None);
        let mut events = request.events.clone();
        events.sort_by(|a, b| a.time().total_cmp(&b.time()));
        let mut next_event = 0;
        let tol = 1e-9 * h;
        let mut t = 0.0_f64;
        let mut k0 = 0usize;
        let mut stepper = integrate::Stepper::new();
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
            k0 = (cp.t / request.output_step).round() as usize;
        }
        let checkpoint_step = match (start, &request.record) {
            (None, Some(r)) if r.checkpoint_step > 0.0 => r.checkpoint_step,
            _ => 0.0,
        };
        let mut checkpoints: Vec<Checkpoint> = Vec::new();
        let mut next_checkpoint = 0.0_f64;
        let mut rec = Recorder::new(&sys, request.record.clone());
        rec.record(&sys, t, &x);
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
        let n_out = (request.t_end / request.output_step).round() as usize;
        for k_out in (k0 + 1)..=n_out {
            if cancelled() {
                return Err("cancelled".into());
            }
            let t_target = k_out as f64 * request.output_step;
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
                .map_err(|e| format!("integration failed at t = {t:.4} s: {e}"))?;
                steps += 1;
                t = t_next;
            }
            t = t_target;
            rec.record(&sys, t, &x);
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
        let mut eigen_note = None;
        let eigenvalues = if request.small_signal && start.is_none() {
            self.eigenvalues().unwrap_or_else(|e| {
                eigen_note = Some(e);
                Vec::new()
            })
        } else {
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
            let l = &mut sys.loads[k];
            for v in [
                &mut l.p_z, &mut l.q_z, &mut l.p_i, &mut l.q_i, &mut l.p_p, &mut l.q_p,
            ] {
                *v *= scale;
            }
            format!("t = {time} s: load at bus {b} scaled by {scale}")
        }
        RmsEvent::SetReference {
            time,
            bus: b,
            machine_id,
            signal,
            value,
        } => {
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

/// Integration steps per output step: the step is at most `dt` (never
/// coarser than asked), the output step divided evenly.
fn substeps(output_step: f64, dt: f64) -> f64 {
    (output_step / dt * (1.0 - 1e-9)).ceil().max(1.0)
}

/// The admittance of a fault through `r + jx`; a fault needs a finite,
/// nonzero impedance.
fn fault_admittance(r: f64, x: f64) -> Result<C64, String> {
    let z = C64::new(r, x);
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RmsOptions {
    /// Simulate renewable plants (`REGCA1` + `REECB1` + `REPCA1`), the
    /// default, or hold them at their power flow output as constant currents.
    pub renewable_plants: bool,
    /// Use PowerSimulationsDynamics.jl 0.16.2's low-voltage active current
    /// gain for renewable plants (negative between its two points,
    /// `get_LV_current_gain`) instead of the PSS/E one.
    pub psid_lv_gain: bool,
    /// Factorize the step matrices by blocks where every unit allows it, the
    /// default, or always as one sparse LU over all unknowns.
    pub factorize_by_blocks: bool,
}

impl Default for RmsOptions {
    fn default() -> Self {
        Self {
            renewable_plants: true,
            psid_lv_gain: false,
            factorize_by_blocks: true,
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
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PastPoint {
    pub h: f64,
    pub x: Vec<f64>,
}

/// What rebuilds an integrator factorization exactly.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SolverState {
    pub at: Vec<f64>,
    pub h: f64,
    pub theta: f64,
}

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
    let (dynamics, models) = read_dynamics(dyr_text)?;
    let sim = RmsSimulation::with_models(network, &dynamics, &models, request.load_model)?;
    sim.run(request)
}

/// DYR text, or the JSON form with controller graphs ([`DynamicInput`]).
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
    let (dynamics, models) = read_dynamics(dyr_text)?;
    let sim = RmsSimulation::with_models(&network, &dynamics, &models, load_model)?;
    let result = sim.run(&RmsRequest {
        t_end: 0.005,
        dt: 0.005,
        output_step: 0.005,
        load_model,
        events: Vec::new(),
        small_signal: true,
        record: None,
    })?;
    let load_buses = sim
        .sys
        .loads
        .iter()
        .zip(&sim.sys.bus_numbers)
        .filter(|(l, _)| l.p_z != 0.0 || l.q_z != 0.0 || l.is_nonlinear())
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
        dyr_records: dynamics.records().len(),
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

/// A result or live chunk as flat numbers: `meta` is its JSON without the
/// series values, which lie in `values` (the channels of `meta.keys`, each
/// `meta.lengths[i]` long, one after the other), `frame_values` (likewise
/// for `meta.frame_keys`) and `checkpoint_values` (per checkpoint of
/// `meta.checkpoint_times`, `meta.nx` values each: its state, the
/// factorization's iterate where `meta.checkpoint_solver` has one, and one
/// earlier point per step in `meta.checkpoint_past`).
#[derive(Clone, Debug, Default)]
pub struct RmsBuffers {
    pub meta: String,
    pub values: Vec<f64>,
    pub frame_values: Vec<f64>,
    pub checkpoint_values: Vec<f64>,
}

fn pack_series(channels: &mut BTreeMap<String, Vec<f64>>) -> (Vec<String>, Vec<usize>, Vec<f64>) {
    let taken = std::mem::take(channels);
    let total = taken.values().map(Vec::len).sum();
    let mut keys = Vec::with_capacity(taken.len());
    let mut lengths = Vec::with_capacity(taken.len());
    let mut values = Vec::with_capacity(total);
    for (k, v) in taken {
        keys.push(k);
        lengths.push(v.len());
        values.extend_from_slice(&v);
    }
    (keys, lengths, values)
}

fn pack<T: Serialize>(
    mut value: T,
    channels: impl FnOnce(
        &mut T,
    ) -> (
        &mut BTreeMap<String, Vec<f64>>,
        Option<&mut RmsSeries>,
        Vec<Checkpoint>,
    ),
) -> Result<RmsBuffers, String> {
    let (ch, frames, checkpoints) = channels(&mut value);
    let (keys, lengths, values) = pack_series(ch);
    let (frame_keys, frame_lengths, frame_values) = match frames {
        Some(f) => pack_series(&mut f.channels),
        None => (Vec::new(), Vec::new(), Vec::new()),
    };
    let nx = checkpoints.first().map_or(0, |c| c.x.len());
    let checkpoint_times: Vec<f64> = checkpoints.iter().map(|c| c.t).collect();
    // Per checkpoint its state, then the factorization's iterate if any,
    // then its earlier points.
    let checkpoint_solver: Vec<Option<[f64; 2]>> = checkpoints
        .iter()
        .map(|c| c.solver.as_ref().map(|s| [s.h, s.theta]))
        .collect();
    // The steps of its earlier points, newest first.
    let checkpoint_past: Vec<Vec<f64>> = checkpoints
        .iter()
        .map(|c| c.past.iter().map(|p| p.h).collect())
        .collect();
    let mut checkpoint_values = Vec::with_capacity(4 * nx * checkpoints.len());
    for c in checkpoints {
        checkpoint_values.extend(c.x);
        if let Some(s) = c.solver {
            checkpoint_values.extend(s.at);
        }
        for p in c.past {
            checkpoint_values.extend(p.x);
        }
    }
    let mut meta = serde_json::to_value(&value).map_err(|e| e.to_string())?;
    if let Some(m) = meta.as_object_mut() {
        m.insert("keys".into(), serde_json::json!(keys));
        m.insert("lengths".into(), serde_json::json!(lengths));
        m.insert("frame_keys".into(), serde_json::json!(frame_keys));
        m.insert("frame_lengths".into(), serde_json::json!(frame_lengths));
        m.insert(
            "checkpoint_times".into(),
            serde_json::json!(checkpoint_times),
        );
        m.insert(
            "checkpoint_solver".into(),
            serde_json::json!(checkpoint_solver),
        );
        m.insert("checkpoint_past".into(), serde_json::json!(checkpoint_past));
        m.insert("nx".into(), serde_json::json!(nx));
    }
    Ok(RmsBuffers {
        meta: serde_json::to_string(&meta).map_err(|e| e.to_string())?,
        values,
        frame_values,
        checkpoint_values,
    })
}

/// [`RmsResult`] as [`RmsBuffers`].
///
/// # Errors
/// When the result does not serialize.
pub fn pack_result(result: RmsResult) -> Result<RmsBuffers, String> {
    pack(result, |r| {
        let cps = std::mem::take(&mut r.checkpoints);
        (&mut r.channels, r.frames.as_mut(), cps)
    })
}

/// [`RmsChunk`] as [`RmsBuffers`].
///
/// # Errors
/// When the chunk does not serialize.
pub fn pack_chunk(chunk: RmsChunk) -> Result<RmsBuffers, String> {
    pack(chunk, |c| (&mut c.channels, c.frames.as_mut(), Vec::new()))
}

/// [`simulate_module_json`] with the series as flat numbers.
///
/// # Errors
/// As [`simulate_module_json`].
pub fn simulate_module_buffers(
    module_json: &str,
    dyr_text: &str,
    request_json: &str,
) -> Result<RmsBuffers, String> {
    let network = network_from_module_json(module_json)?;
    let request: RmsRequest = if request_json.trim().is_empty() {
        RmsRequest::default()
    } else {
        serde_json::from_str(request_json).map_err(|e| format!("request: {e}"))?
    };
    pack_result(simulate(&network, dyr_text, &request)?)
}

/// Replay a run from a checkpoint ([`RmsSimulation::replay`]) of a stored
/// PowerIO module with DYR text; `request_json` is the run's request with
/// the channels wanted now in `record`.
///
/// # Errors
/// As [`RmsSimulation::replay`].
#[allow(clippy::too_many_arguments)]
pub fn replay_module_buffers(
    module_json: &str,
    dyr_text: &str,
    request_json: &str,
    t: f64,
    x: &[f64],
    solver_at: &[f64],
    solver_h: f64,
    solver_theta: f64,
    past_h: &[f64],
    past_x: &[f64],
) -> Result<RmsBuffers, String> {
    if x.is_empty() || past_x.len() != past_h.len() * x.len() {
        return Err("the checkpoint's earlier points do not fit its state".into());
    }
    let past = past_h
        .iter()
        .zip(past_x.chunks(x.len()))
        .map(|(&h, x)| PastPoint { h, x: x.to_vec() })
        .collect();
    let network = network_from_module_json(module_json)?;
    let request: RmsRequest =
        serde_json::from_str(request_json).map_err(|e| format!("request: {e}"))?;
    let (dynamics, models) = read_dynamics(dyr_text)?;
    let sim = RmsSimulation::with_models(&network, &dynamics, &models, request.load_model)?;
    let solver = (!solver_at.is_empty()).then(|| SolverState {
        at: solver_at.to_vec(),
        h: solver_h,
        theta: solver_theta,
    });
    pack_result(sim.replay(
        &request,
        &Checkpoint {
            t,
            x: x.to_vec(),
            solver,
            past,
        },
    )?)
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

/// Small-signal analysis of a stored PowerIO module with DYR text: a
/// [`LinearRequest`] JSON in, a [`LinearResult`] JSON out.
///
/// # Errors
/// When the inputs do not parse, initialization fails or a response names a
/// missing element.
pub fn linear_module_json(
    module_json: &str,
    dyr_text: &str,
    load_model: LoadModel,
    request_json: &str,
) -> Result<String, String> {
    let network = network_from_module_json(module_json)?;
    let (dynamics, models) = read_dynamics(dyr_text)?;
    let request: LinearRequest = if request_json.trim().is_empty() {
        LinearRequest::default()
    } else {
        serde_json::from_str(request_json).map_err(|e| format!("request: {e}"))?
    };
    let sim = RmsSimulation::with_models(&network, &dynamics, &models, load_model)?;
    serde_json::to_string(&sim.linear_analysis(&request)?).map_err(|e| e.to_string())
}

/// New samples of a live session since the previous call.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RmsChunk {
    pub time: Vec<f64>,
    pub channels: BTreeMap<String, Vec<f64>>,
    /// Events applied since the previous chunk.
    pub events: Vec<String>,
    /// Simulated time reached.
    pub t: f64,
    /// The watch set's frame channels since the previous chunk.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frames: Option<RmsSeries>,
}

/// A running simulation that advances on demand and takes events at its
/// current time: the engine of the live view.
pub struct RmsLive {
    sys: DynSystem,
    x: Vec<f64>,
    f: Vec<f64>,
    t: f64,
    h: f64,
    output_step: f64,
    since_output: f64,
    rec: Recorder,
    applied: Vec<String>,
    /// Every event with the time it was applied, for replaying the session.
    log: Vec<RmsEvent>,
    stepper: integrate::Stepper,
    topology: Topology,
    /// Loads at the operating point, the base of [`RmsLive::set_load_scales`].
    base_loads: Vec<system::BusLoad>,
    load_scales: Vec<(f64, f64)>,
    /// `(admittance, source voltage)` of the voltage sources at the
    /// operating point, to bring a tripped one back.
    base_sources: Vec<(C64, C64)>,
}

impl RmsSimulation {
    /// Start a live session at the initial operating point.
    #[must_use]
    pub fn live(&self, dt: f64, output_step: f64) -> RmsLive {
        let mut f = vec![0.0; self.sys.nx];
        self.sys.residual(&self.x0, &mut f, None);
        let output_step = output_step.max(dt);
        let substeps = substeps(output_step, dt);
        let mut rec = Recorder::new(&self.sys, None);
        rec.record(&self.sys, 0.0, &self.x0);
        RmsLive {
            sys: self.sys.clone(),
            x: self.x0.clone(),
            f,
            t: 0.0,
            h: output_step / substeps,
            output_step,
            since_output: 0.0,
            rec,
            applied: Vec::new(),
            log: Vec::new(),
            stepper: integrate::Stepper::new(),
            topology: self.topology.clone(),
            base_loads: self.sys.loads.clone(),
            load_scales: vec![(1.0, 1.0); self.sys.loads.len()],
            base_sources: self.sys.sources.iter().map(|s| (s.y, s.e)).collect(),
        }
    }
}

impl RmsLive {
    #[must_use]
    pub fn time(&self) -> f64 {
        self.t
    }

    /// The integration step in use.
    #[must_use]
    pub fn step(&self) -> f64 {
        self.h
    }

    /// Integrate with a step of about `dt` from the next step on: the output
    /// step divided into equal steps, as at the start, so the samples keep
    /// their times (at most one step per output sample).
    ///
    /// # Errors
    /// When `dt` is not a positive number.
    pub fn set_step(&mut self, dt: f64) -> Result<(), String> {
        if !(dt.is_finite() && dt > 0.0) {
            return Err(format!("the step must be positive, not {dt}"));
        }
        let substeps = substeps(self.output_step, dt);
        self.h = self.output_step / substeps;
        Ok(())
    }

    /// Record only `spec` from the next sample on (`None`: every channel).
    /// The chunks then carry the watch set, and its frames when it names
    /// some; [`Self::snapshot`] gives any other channel at the current time.
    pub fn set_record(&mut self, spec: Option<RecordSpec>) {
        self.rec.set_spec(spec);
    }

    /// The channels matching `patterns` (keys or `prefix:`; empty: all) at
    /// the current time.
    pub fn snapshot(&mut self, patterns: &[String]) -> BTreeMap<String, f64> {
        self.rec.snapshot(&self.sys, &self.x, patterns)
    }

    /// The events applied so far, stamped with their simulated time.
    #[must_use]
    pub fn event_log(&self) -> &[RmsEvent] {
        &self.log
    }

    /// The elements of the simulated network, as the engine numbers them.
    #[must_use]
    pub fn topology(&self) -> &Topology {
        &self.topology
    }

    /// Apply an event now; its own `time` is replaced by the session time.
    ///
    /// # Errors
    /// When the event names a missing element or the network cannot be
    /// re-solved.
    pub fn apply(&mut self, mut event: RmsEvent) -> Result<String, String> {
        let now = self.t;
        match &mut event {
            RmsEvent::BranchTrip { time, .. }
            | RmsEvent::BranchClose { time, .. }
            | RmsEvent::BusFault { time, .. }
            | RmsEvent::ClearFault { time, .. }
            | RmsEvent::LoadChange { time, .. }
            | RmsEvent::SetReference { time, .. }
            | RmsEvent::GeneratorTrip { time, .. }
            | RmsEvent::LineFault { time, .. }
            | RmsEvent::ClearLineFault { time, .. }
            | RmsEvent::BreakerOpen { time, .. }
            | RmsEvent::BreakerClose { time, .. } => *time = (now * 1e6).round() / 1e6,
        }
        let mut sys = self.sys.clone();
        let mut x = self.x.clone();
        let mut text = apply_event(&mut sys, &mut x, &event)?;
        for note in topology::extinguish(&mut sys, now) {
            text.push('\n');
            text.push_str(&note);
        }
        sys.rebuild_ybus();
        sys.rebuild_mass();
        let mut f = vec![0.0; sys.nx];
        sys.residual(&x, &mut f, None);
        self.stepper
            .reinit(&sys, &mut x, &mut f)
            .map_err(|e| format!("re-solving the network: {e}"))?;
        self.sys = sys;
        self.x = x;
        self.f = f;
        self.applied.push(text.clone());
        self.log.push(event);
        Ok(text)
    }

    /// Close the breaker of a disconnected unit (topology order) again.
    ///
    /// A machine comes in as running at no load, synchronized by its
    /// operator: its own terminal voltage is `v_ratio` times the bus
    /// voltage, `angle` radians ahead of it, at `speed` per unit. The
    /// network then pulls it into step — a poor synchronization shows as a
    /// current and torque surge. Its governor and exciter start from that
    /// no-load state (references for zero output at that voltage). An
    /// inverter or source reconnects at its operating point settings (the
    /// caller ramps its reference from zero).
    ///
    /// # Errors
    /// When the unit is in service or does not exist, or the network cannot
    /// be re-solved.
    pub fn synchronize_unit(
        &mut self,
        unit: usize,
        v_ratio: f64,
        angle: f64,
        speed: f64,
    ) -> Result<String, String> {
        let bus = self
            .topology
            .units
            .get(unit)
            .ok_or_else(|| format!("no unit {unit}"))?
            .bus;
        let v_bus = self.measure().bus_voltage(bus);
        let nd = self.sys.devices.len();
        let ns = self.sys.sources.len();
        let mut sys = self.sys.clone();
        let mut x = self.x.clone();
        let text = if unit < nd {
            let d = &mut sys.devices[unit];
            if d.tripped.is_none() {
                return Err(format!("generator {} is in service", d.name()));
            }
            d.tripped = None;
            let u = v_bus * v_ratio * C64::from_polar(1.0, angle);
            let local = init::init_device(d, u, C64::new(0.0, 0.0))?;
            let [_, k, ..] = d.layout();
            x[d.offset..d.offset + local.len()].copy_from_slice(&local);
            x[d.offset + k + 1] = speed;
            format!(
                "generator {} synchronized (ΔV {:+.1} %, Δφ {:+.1}°, Δf {:+.3} Hz)",
                d.name(),
                100.0 * (v_ratio - 1.0),
                angle.to_degrees(),
                (speed - self.frequency()) * d.f0
            )
        } else if unit < nd + ns {
            let k = unit - nd;
            let (y, e) = self.base_sources[k];
            let src = &mut sys.sources[k];
            if src.y != C64::new(0.0, 0.0) {
                return Err(format!(
                    "source {}-{} is in service",
                    src.bus_number, src.machine_id
                ));
            }
            src.y = y;
            src.e = e;
            format!("source {}-{} reconnected", src.bus_number, src.machine_id)
        } else {
            let k = unit - nd - ns;
            let h = sys
                .held
                .get_mut(k)
                .ok_or_else(|| format!("no unit {unit}"))?;
            if !h.tripped {
                return Err(format!(
                    "unit {}-{} is in service",
                    h.bus_number, h.machine_id
                ));
            }
            h.tripped = false;
            let (b, s, name) = (h.bus, h.s, format!("{}-{}", h.bus_number, h.machine_id));
            if h.plant.is_some() {
                sys.rebuild_mass();
            } else {
                sys.loads[b].p_i -= s.re;
                sys.loads[b].q_i -= s.im;
            }
            format!("unit {name} reconnected")
        };
        sys.rebuild_ybus();
        sys.rebuild_mass();
        let mut f = vec![0.0; sys.nx];
        sys.residual(&x, &mut f, None);
        self.stepper
            .reinit(&sys, &mut x, &mut f)
            .map_err(|e| format!("re-solving the network: {e}"))?;
        self.sys = sys;
        self.x = x;
        self.f = f;
        let line = format!("t = {:.4} s: {text}", self.t);
        self.applied.push(line.clone());
        Ok(line)
    }

    /// Bus voltages and unit currents of the present state.
    fn measure(&self) -> topology::RmsMeasure<'_> {
        topology::RmsMeasure::new(&self.sys, &self.x)
    }

    /// Re-solve the network after a change of loads, taps or held units.
    fn resolve_network(&mut self, sys: system::DynSystem) -> Result<(), String> {
        let mut sys = sys;
        sys.rebuild_ybus();
        sys.rebuild_mass();
        let mut f = vec![0.0; sys.nx];
        sys.residual(&self.x, &mut f, None);
        let mut x = self.x.clone();
        self.stepper
            .reinit(&sys, &mut x, &mut f)
            .map_err(|e| format!("re-solving the network: {e}"))?;
        self.sys = sys;
        self.x = x;
        self.f = f;
        Ok(())
    }

    /// Set the loads of buses (dense index) to their operating point powers
    /// times `(p, q)` scales, in one re-solve: the driver of load profiles.
    ///
    /// # Errors
    /// When a bus is missing or the network cannot be re-solved.
    pub fn set_load_scales(&mut self, scales: &[(usize, f64, f64)]) -> Result<(), String> {
        if scales.is_empty() {
            return Ok(());
        }
        let mut sys = self.sys.clone();
        for &(b, p, q) in scales {
            let base = *self
                .base_loads
                .get(b)
                .ok_or_else(|| format!("no bus index {b}"))?;
            let (p0, q0) = self.load_scales[b];
            let l = &mut sys.loads[b];
            l.p_z = base.p_z * p;
            l.p_p = base.p_p * p;
            l.q_z = base.q_z * q;
            l.q_p = base.q_p * q;
            // A tripped inverter's current stays in the constant-current part.
            l.p_i += base.p_i * (p - p0);
            l.q_i += base.q_i * (q - q0);
            self.load_scales[b] = (p, q);
        }
        self.resolve_network(sys)
    }

    /// Operating point load per bus (dense), system per unit at `v0`.
    #[must_use]
    pub fn base_loads(&self) -> Vec<C64> {
        self.base_loads
            .iter()
            .map(|l| C64::new(l.p_z + l.p_i + l.p_p, l.q_z + l.q_i + l.q_p))
            .collect()
    }

    /// Power drawn by the loads of each bus (dense) at the present voltage.
    #[must_use]
    pub fn bus_loads(&self) -> Vec<C64> {
        let n = self.sys.n;
        self.sys
            .loads
            .iter()
            .enumerate()
            .map(|(b, l)| {
                let v = C64::new(self.x[b], self.x[n + b]).norm();
                let r = if l.v0 > 0.0 { v / l.v0 } else { 1.0 };
                C64::new(
                    l.p_z * r * r + l.p_i * r + l.p_p,
                    l.q_z * r * r + l.q_i * r + l.q_p,
                )
            })
            .collect()
    }

    /// Complex power out of each unit (topology order), `None` when tripped.
    #[must_use]
    pub fn unit_powers(&self) -> Vec<Option<C64>> {
        let m = self.measure();
        self.topology
            .units
            .iter()
            .enumerate()
            .map(|(u, unit)| {
                if !m.unit_in_service(u) {
                    return None;
                }
                let v = m.bus_voltage(unit.bus);
                let i = m.unit_current(u);
                Some(v * i.conj())
            })
            .collect()
    }

    /// System frequency: the speed of the online machines weighted by
    /// their rating, per unit (1 when there are none).
    #[must_use]
    pub fn frequency(&self) -> f64 {
        let (mut sw, mut s) = (0.0, 0.0);
        for d in self.sys.devices.iter().filter(|d| d.tripped.is_none()) {
            let [_, k, ..] = d.layout();
            s += d.scale * self.x[d.offset + k + 1];
            sw += d.scale;
        }
        if sw > 0.0 {
            s / sw
        } else {
            self.sys.omega_sys(&self.x)
        }
    }

    /// Speed of each machine unit (topology order; `None` for other units
    /// and tripped machines), per unit.
    #[must_use]
    pub fn unit_speeds(&self) -> Vec<Option<f64>> {
        let mut out: Vec<Option<f64>> = self
            .sys
            .devices
            .iter()
            .map(|d| {
                let [_, k, ..] = d.layout();
                d.tripped.is_none().then(|| self.x[d.offset + k + 1])
            })
            .collect();
        out.resize(self.topology.units.len(), None);
        out
    }

    /// The active power reference of a unit (topology order): the governor
    /// reference of a machine (machine base), the power of an inverter held
    /// at its operating point (system base); `None` for a source.
    #[must_use]
    pub fn unit_reference(&self, unit: usize) -> Option<f64> {
        let nd = self.sys.devices.len();
        let ns = self.sys.sources.len();
        if unit < nd {
            Some(self.sys.devices[unit].p_ref)
        } else if unit >= nd + ns {
            self.sys.held.get(unit - nd - ns).map(|h| match &h.plant {
                Some(p) => p.p_ref * p.scale,
                None => h.s.re,
            })
        } else {
            None
        }
    }

    /// Set active power references (see [`RmsLive::unit_reference`]); held
    /// inverters keep their power factor. One re-solve when an inverter
    /// changed.
    ///
    /// # Errors
    /// When the network cannot be re-solved.
    pub fn set_unit_references(&mut self, refs: &[(usize, f64)]) -> Result<(), String> {
        let nd = self.sys.devices.len();
        let ns = self.sys.sources.len();
        let mut held = false;
        let mut sys = None::<system::DynSystem>;
        for &(u, value) in refs {
            if u < nd {
                self.sys.devices[u].p_ref = value;
                // A copy made for an earlier held unit replaces the system.
                if let Some(s) = sys.as_mut() {
                    s.devices[u].p_ref = value;
                }
            } else if u >= nd + ns && u - nd - ns < self.sys.held.len() {
                let k = u - nd - ns;
                if let Some(p) = self.sys.held[k].plant.as_mut() {
                    // A simulated plant follows its own power reference.
                    p.p_ref = value / p.scale;
                    if let Some(s) = sys.as_mut() {
                        s.held[k].plant.clone_from(&self.sys.held[k].plant);
                    }
                    continue;
                }
                let s = sys.get_or_insert_with(|| self.sys.clone());
                let h = &mut s.held[k];
                if h.tripped {
                    continue;
                }
                let pf = if h.s.re.abs() > 1e-9 {
                    h.s.im / h.s.re
                } else {
                    0.0
                };
                h.s = C64::new(value, value * pf);
                held = true;
            }
        }
        match sys {
            Some(s) if held => self.resolve_network(s),
            _ => Ok(()),
        }
    }

    /// Set a machine's voltage reference (topology order).
    ///
    /// # Errors
    /// When the unit is not a dynamic machine.
    pub fn set_voltage_reference(&mut self, unit: usize, v_ref: f64) -> Result<(), String> {
        let d = self
            .sys
            .devices
            .get_mut(unit)
            .ok_or_else(|| format!("unit {unit} has no voltage regulator"))?;
        d.v_ref = v_ref;
        Ok(())
    }

    /// A machine's voltage reference (topology order).
    #[must_use]
    pub fn voltage_reference(&self, unit: usize) -> Option<f64> {
        self.sys.devices.get(unit).map(|d| d.v_ref)
    }

    /// Whether the breaker at a branch end is closed (dense branch index).
    #[must_use]
    pub fn end_closed(&self, e: usize, end: End) -> bool {
        self.sys.ac.sw.get(e).is_some_and(|&s| s != 0.0)
            && !self.sys.end_open[e][usize::from(end == End::To)]
    }

    /// Whether a fault is inside the branch (dense index) now; a transient
    /// one dies out once both ends of the line are open.
    #[must_use]
    pub fn branch_faulted(&self, e: usize) -> bool {
        self.sys.branch_faults.get(e).is_some_and(Option::is_some)
    }

    /// Tap ratio of a branch (dense index; 1 for a line).
    #[must_use]
    pub fn tap(&self, e: usize) -> f64 {
        self.sys.ac.tap.get(e).copied().unwrap_or(1.0)
    }

    /// Set a transformer's tap ratio and re-solve.
    ///
    /// # Errors
    /// When the branch is missing or the network cannot be re-solved.
    pub fn set_tap(&mut self, e: usize, tap: f64) -> Result<(), String> {
        if e >= self.sys.ac.m {
            return Err(format!("no branch index {e}"));
        }
        let mut sys = self.sys.clone();
        sys.ac.tap[e] = tap;
        self.resolve_network(sys)
    }

    /// Integrate `duration` seconds and return the samples recorded on the
    /// way.
    ///
    /// # Errors
    /// When the integration fails; the session stays at the last good point.
    pub fn advance(&mut self, duration: f64) -> Result<RmsChunk, String> {
        self.integrate(duration)?;
        let (time, channels, frames) = self.rec.take();
        Ok(RmsChunk {
            time,
            channels,
            events: std::mem::take(&mut self.applied),
            t: self.t,
            frames,
        })
    }

    /// [`Self::advance`] with the samples as columns, for a caller that
    /// keeps the channel keys: `values` holds the main channels sample
    /// after sample (`meta.time`), `frame_values` the frame channels
    /// (`meta.frame_time`), each in the order of the keys. `meta` (JSON)
    /// also has `t`, `events` and `layout`, a number that changes
    /// when the keys do; it carries `keys` and `frame_keys` only when
    /// `known_layout` is not that number (0 never is). A live view takes a
    /// slice every few tens of milliseconds: naming thousands of channels
    /// and copying each into its own series, as a chunk does, then costs
    /// about as much as the samples' integration.
    ///
    /// # Errors
    /// As [`Self::advance`].
    pub fn advance_columns(
        &mut self,
        duration: f64,
        known_layout: u32,
    ) -> Result<RmsBuffers, String> {
        self.integrate(duration)?;
        let [(time, values), (frame_time, frame_values)] = self.rec.take_columns();
        let mut meta = serde_json::json!({
            "t": self.t,
            "time": time,
            "frame_time": frame_time,
            "events": std::mem::take(&mut self.applied),
            "layout": self.rec.layout,
        });
        // Without a catalog (before the first sample, or after a new record
        // spec) there are no samples to name.
        if self.rec.layout != known_layout && !self.rec.catalog.is_empty() {
            let [keys, frame_keys] = self.rec.keys();
            meta["keys"] = serde_json::json!(keys);
            meta["frame_keys"] = serde_json::json!(frame_keys);
        }
        Ok(RmsBuffers {
            meta: meta.to_string(),
            values,
            frame_values,
            checkpoint_values: Vec::new(),
        })
    }

    /// Integrate `duration` seconds, recording on the way.
    fn integrate(&mut self, duration: f64) -> Result<(), String> {
        let end = self.t + duration.max(0.0);
        let mut euler = 0usize;
        while self.t < end - 1e-12 {
            let h = self.h.min(self.output_step - self.since_output).max(1e-9);
            step_adaptive(
                &self.sys,
                &mut self.x,
                &mut self.f,
                h,
                0,
                &mut euler,
                &mut self.stepper,
            )
            .map_err(|e| format!("integration failed at t = {:.4} s: {e}", self.t))?;
            self.t += h;
            self.since_output += h;
            if self.since_output >= self.output_step - 1e-12 {
                self.since_output = 0.0;
                self.t = (self.t / self.output_step).round() * self.output_step;
                self.rec.record(&self.sys, self.t, &self.x);
            }
        }
        Ok(())
    }
}

/// A live session over a stored PowerIO module and DYR text.
///
/// # Errors
/// When the inputs do not parse or the case cannot be initialized.
pub fn live_from_module_json(
    module_json: &str,
    dyr_text: &str,
    load_model: LoadModel,
    dt: f64,
    output_step: f64,
) -> Result<RmsLive, String> {
    let network = network_from_module_json(module_json)?;
    let (dynamics, models) = read_dynamics(dyr_text)?;
    Ok(RmsSimulation::with_models(&network, &dynamics, &models, load_model)?.live(dt, output_step))
}
