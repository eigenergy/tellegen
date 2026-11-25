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
//!
//! # Blocks
//!
//! The engine is built from the blocks below; `tellegen-blocks` draws them
//! (`just blocks`, or `/docs/blocks/rms` on a running `tellegen-server`).
//! A run goes through them in this order: **building the system** turns a
//! case into the differential-algebraic system and its steady state, the
//! **device models** give each unit its equations, the **numerical solver**
//! steps the system in time, **running a simulation** applies events and
//! records channels, and **analysing a run** screens the result or
//! linearizes the system. **Editing a case** prepares the dynamic data
//! before any of this.
//!
//! @block RMS dynamic simulation

// @block NaN-safe comparisons
// Checks that fail on NaN instead of passing it, used across the engine
// wherever a value is compared with a tolerance or a limit.

/// True unless `a > b`. A NaN on either side counts as not greater, so a
/// check written with this fails on NaN instead of passing it.
pub(crate) fn not_gt(a: f64, b: f64) -> bool {
    a.partial_cmp(&b) != Some(std::cmp::Ordering::Greater)
}

/// True unless `a < b`; NaN on either side counts as not less.
pub(crate) fn not_lt(a: f64, b: f64) -> bool {
    a.partial_cmp(&b) != Some(std::cmp::Ordering::Less)
}

/// True unless `a <= b`; NaN on either side counts as not less or equal.
pub(crate) fn not_le(a: f64, b: f64) -> bool {
    !matches!(
        a.partial_cmp(&b),
        Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
    )
}
// @end

mod api;

// @block Building the system
// From a case to the simulated system: the AC power flow, the devices
// linked to their buses, the differential-algebraic system `M ẋ = f(x)`
// over the bus voltages and device states, and its steady state, where all
// derivatives vanish and every unit injects its power flow current.
pub(crate) mod init;
mod system;
mod topology;

// @block Device models
// The equations of each unit: synchronous machines with their shaft,
// exciter, governor and stabilizer, the WECC renewable plant, the control
// blocks they share, and controllers drawn as block diagrams at run time.
pub(crate) mod blocks;
#[cfg(feature = "rms-experimental")]
pub mod graph;
// Experimental: ESST1A, ESAC6A, PSS2B, OELIT, UELPQ as block graphs.
#[cfg(feature = "rms-experimental")]
pub(crate) mod ext_models;
pub(crate) mod models;
mod renewable;

// @block Numerical solver
// Time integration of `M ẋ = f(x)` by the implicit trapezoidal rule, each
// step a Newton solve; the step matrix is factorized unit by unit (a Schur
// complement onto the network) or as one sparse LU.
mod integrate;
pub(crate) mod clock;
#[cfg(not(all(target_arch = "wasm32", not(target_feature = "atomics"))))]
pub(crate) mod spin_pool;
pub(crate) mod regions;
pub use regions::region_stats;
pub use clock::set_clock;
#[cfg(not(all(target_arch = "wasm32", not(target_feature = "atomics"))))]
pub use spin_pool::{configure as configure_threads, Spawner};
mod schur;

// @block Running a simulation
// What happens during a run besides integration: discrete events change the
// network, the recorder samples channels, and a live session advances on
// demand and takes events while it runs.
mod events;
#[cfg(feature = "rms-experimental")]
mod live;
#[cfg(feature = "protection")]
mod protect;
mod record;

// @block Analysing a run
// What is computed from a prepared system or a finished run: the screening
// verdict (centre-of-inertia frequency, angles, voltages, islands) and the
// small-signal analysis (modes, participation factors, frequency responses).
#[cfg(feature = "rms-experimental")]
mod linear;
pub mod screen;

// @block Editing a case
// Dynamic data before a simulation: published unit data, a library of
// typical models, rules that give every unit a model, and edits of single
// DYR parameters.
#[cfg(feature = "rms-experimental")]
pub mod auto;
#[cfg(feature = "rms-experimental")]
mod dyr_params;
#[cfg(feature = "rms-experimental")]
mod library;
#[cfg(feature = "rms-experimental")]
pub mod presets;
// @end

pub use api::{
    Checkpoint, InputSignal, LoadModel, LowVoltageLoad, PastPoint, RecordSpec, RmsEvent,
    RmsGenerator, RmsInspection, RmsOptions, RmsRequest, RmsResult, RmsSeries, SolverState,
};
pub use screen::{Reason, RmsScreening, ScreeningCriteria, ScreeningVerdict, SwingTrend};
pub use topology::{End, TopoBranch, TopoUnit, Topology, UnitKind};

use powerio::{BalancedNetwork, DynamicCase};

use events::apply_event;
use integrate::step_adaptive;
use record::Recorder;
use system::DynSystem;
// Experimental: model switches, set by the caller before a run.
pub use system::set_gast_rate;
pub use system::set_load_recovery;
pub use system::set_plant_q_exponent;
pub use system::set_load_rise;
pub use system::set_load_saturation;
pub use system::set_uel;
pub use system::set_vcomp;
pub use integrate::lu_stats;

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

// @block Prepared simulation
// The engine's centre: a case assembled and initialized once, then run any
// number of times with different events. It holds the system, its steady
// state and the network elements events can name.

/// A prepared simulation: the system assembled and initialized once, run any
/// number of times with different events.
pub struct RmsSimulation {
    sys: DynSystem,
    x0: Vec<f64>,
    notes: Vec<String>,
    load_model: LoadModel,
    topology: Topology,
    initial_residual: f64,
    options: RmsOptions,
    dyr_records: usize,
}

/// A unit's channel key: its bus, with `#id` for a machine id other than `1`.
pub(crate) fn unit_key(bus: usize, id: &str) -> String {
    if id == "1" {
        bus.to_string()
    } else {
        format!("{bus}#{id}")
    }
}

impl RmsSimulation {
    // @block Build once
    // Every constructor ends in `assemble`: the system is built from the
    // case and initialized from the AC power flow, and the steady state, the
    // notes of the build and the topology are kept for every later run.

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
            options,
            dyr_records: dynamics.records().len(),
        })
    }

    // @block Inspect without running
    // What the prepared system shows before any run: its elements as the
    // engine numbers them, the branch loading at the operating point, the
    // generators with their models, the unit that turns the reference frame,
    // the units held at their power flow output, and (with
    // `rms-experimental`) the eigenvalues and the small-signal analysis.

    /// The elements of the simulated network, as the engine numbers them.
    #[must_use]
    pub fn topology(&self) -> &Topology {
        &self.topology
    }

    /// Run with protection relays: as [`Self::run`], with the relays of
    /// `scheme` measuring the network at every step and their breakers
    /// opening, reclosing and tripping units as they decide. The relay
    /// channels (`z_r:`, `z_x:` and the like) join the result's channels.
    ///
    /// # Errors
    /// As [`Self::run`], or when a relay does not bind to this network.
    #[cfg(feature = "protection")]
    pub fn run_with_protection(
        &self,
        request: &RmsRequest,
        scheme: &crate::protection::ProtectionScheme,
    ) -> Result<(RmsResult, crate::protection::ProtectionReport), String> {
        self.run_with_protection_cancellable(request, scheme, &mut || false)
    }

    /// [`Self::run_with_protection`], polling `cancelled` between output
    /// samples (a caller reports progress from it).
    ///
    /// # Errors
    /// As [`Self::run_with_protection`], or when `cancelled` returns true.
    #[cfg(feature = "protection")]
    pub fn run_with_protection_cancellable(
        &self,
        request: &RmsRequest,
        scheme: &crate::protection::ProtectionScheme,
        cancelled: &mut dyn FnMut() -> bool,
    ) -> Result<(RmsResult, crate::protection::ProtectionReport), String> {
        // Series capacitor banks act without relays too.
        if scheme.relays.is_empty() && scheme.series_capacitors.is_empty() {
            return Ok((
                self.run_cancellable(request, cancelled)?,
                crate::protection::ProtectionReport::default(),
            ));
        }
        let mut relays = crate::protection::ProtectionRuntime::bind(scheme, &self.topology, true)?;
        let result = self.run_from(request, None, cancelled, Some(&mut relays))?;
        Ok((result, relays.into_report()))
    }

    /// The relays of `scheme` at the operating point (the power flow): what
    /// each measures and which functions pick up.
    ///
    /// # Errors
    /// When a relay does not bind to this network.
    #[cfg(feature = "protection")]
    pub fn relays_at_operating_point(
        &self,
        scheme: &crate::protection::ProtectionScheme,
    ) -> Result<crate::protection::statics::OperatingPoint, String> {
        let mut r = crate::protection::ProtectionRuntime::bind(scheme, &self.topology, true)?;
        let m = protect::RelayMeasure::new(&self.sys, &self.x0, 0.0);
        r.evaluate(&m);
        let topo = &self.topology;
        let seen = r.seen(&m, |b, i| {
            let kv = topo.base_kv.get(b).copied().unwrap_or(1.0).max(1e-9);
            i.norm() * topo.sbase / (3f64.sqrt() * kv)
        });
        let pickups = r
            .report()
            .operations
            .iter()
            .filter(|o| o.kind == crate::protection::OperationKind::Pickup)
            .cloned()
            .collect();
        Ok(crate::protection::statics::OperatingPoint { pickups, seen })
    }

    /// The apparent power at the more loaded end of every branch at the
    /// operating point, system per unit (the branches of
    /// [`Self::topology`]).
    #[must_use]
    pub fn branch_loading(&self) -> Vec<f64> {
        let flows = self.sys.branch_flows(&self.x0);
        (0..self.sys.ac.m)
            .map(|e| {
                flows.pf[e]
                    .hypot(flows.qf[e])
                    .max(flows.pt[e].hypot(flows.qt[e]))
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

    /// The options this simulation was prepared with.
    #[must_use]
    pub fn options(&self) -> RmsOptions {
        self.options
    }

    /// Bus numbers in state order.
    pub fn buses(&self) -> &[usize] {
        &self.sys.bus_numbers
    }

    /// `(from, to, circuit)` of every branch.
    pub fn branches(&self) -> Vec<(usize, usize, String)> {
        self.sys.branch_keys.clone()
    }

    /// The dynamic generators with their models and initial states.
    #[must_use]
    pub fn generators(&self) -> Vec<RmsGenerator> {
        self.sys
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
                h: d.shaft.h,
            })
            .collect()
    }

    /// The unit whose rotor speed turns the reference frame of the bus
    /// angles (`va:`) and rotor angles (`delta:`), keyed as its channels are
    /// (`bus` or `bus#id`); `None` when the frame turns at the synchronous
    /// speed (a voltage source holds the reference bus).
    #[must_use]
    pub fn reference_unit(&self) -> Option<String> {
        let r = self.sys.omega_ref?;
        self.sys
            .devices
            .iter()
            .find(|d| {
                let [_, s, ..] = d.layout();
                d.offset + s + 1 == r
            })
            .map(|d| unit_key(d.bus_number, &d.machine_id))
    }

    /// The units held at their power flow output (constant currents, not
    /// simulated), keyed `bus` or `bus#id`.
    #[must_use]
    pub fn held_units(&self) -> Vec<String> {
        self.sys
            .held
            .iter()
            .filter(|h| h.plant.is_none())
            .map(|h| unit_key(h.bus_number, &h.machine_id))
            .collect()
    }

    /// What the dynamic data resolves to, without running: the linked
    /// models, the elements events can name, the notes of the
    /// initialization and its residual. The eigenvalues are left empty
    /// ([`Self::eigenvalues`] computes them).
    #[must_use]
    pub fn inspection(&self) -> RmsInspection {
        RmsInspection {
            buses: self.sys.bus_numbers.clone(),
            base_mva: self.sys.sbase,
            base_frequency: self.sys.f0,
            generators: self.generators(),
            sources: self
                .sys
                .sources
                .iter()
                .map(|s| format!("{}-{}", s.bus_number, s.machine_id))
                .collect(),
            branches: self.branches(),
            load_buses: self
                .sys
                .loads
                .iter()
                .zip(&self.sys.bus_numbers)
                .filter(|(l, _)| l.has_load())
                .map(|(_, &b)| b)
                .collect(),
            dyr_records: self.dyr_records,
            eigenvalues: Vec::new(),
            notes: self.notes.clone(),
            supported_models: powerio::SUPPORTED_MODELS
                .iter()
                .map(|(m, _)| (*m).to_owned())
                .collect(),
            initial_residual: self.initial_residual,
            reference_unit: self.reference_unit(),
            held_units: self.held_units(),
            plants: self
                .sys
                .held
                .iter()
                .filter(|h| h.plant.is_some())
                .map(|h| unit_key(h.bus_number, &h.machine_id))
                .collect(),
            options: self.options,
            load_model: self.load_model,
        }
    }

    /// Eigenvalues of the linearized system at the initial point:
    /// `M_d⁻¹ (f_dd − f_da f_aa⁻¹ f_ad)` over the differential states.
    ///
    /// # Errors
    /// When the system is too large for the dense eigensolver or it fails.
    #[cfg(feature = "rms-experimental")]
    pub fn eigenvalues(&self) -> Result<Vec<[f64; 2]>, String> {
        self.eigenvalues_at(models::REST_MARGIN)
    }

    /// [`Self::eigenvalues`] of [`Self::linearized`]`(rel)`.
    #[cfg(feature = "rms-experimental")]
    fn eigenvalues_at(&self, rel: f64) -> Result<Vec<[f64; 2]>, String> {
        let sys = self.linearized(rel);
        linear::check_eigen_size(&sys)?;
        let (a, _) = linear::reduced_a(&sys, &self.x0);
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

    /// The system as a linearization at the initial point takes it: an
    /// exciter output clamp a state rests past opens `rel` beyond it
    /// ([`models::rest_clamp`]), so the forward differences keep the
    /// coupling through it.
    #[cfg(feature = "rms-experimental")]
    fn linearized(&self, rel: f64) -> DynSystem {
        let mut sys = self.sys.clone();
        for d in &mut sys.devices {
            let [_, _, a, g, _] = d.layout();
            d.avr.rest_at(&self.x0[d.offset + a..d.offset + g], rel);
        }
        sys
    }

    /// Modes with participation factors and frequency responses at the
    /// initial operating point.
    ///
    /// # Errors
    /// When a frequency response names a missing generator or bus.
    #[cfg(feature = "rms-experimental")]
    pub fn linear_analysis(&self, request: &LinearRequest) -> Result<LinearResult, String> {
        let sys = self.linearized(models::REST_MARGIN);
        // experimental: a system too large for the dense eigensolver still gets its frequency responses
        let (states, modes) = if linear::check_eigen_size(&sys).is_ok() {
            linear::modes(&sys, &self.x0, request.participation_top)?
        } else {
            (Vec::new(), Vec::new())
        };
        let bode = linear::bode(&sys, &self.x0, request)?;
        Ok(LinearResult {
            states,
            modes,
            bode,
        })
    }

    // @block Run
    // A run integrates the prepared system over `[0, t_end]` with the
    // request's events, from the start or from a checkpoint of an earlier
    // run, and returns the recorded channels. The prepared system is never
    // changed: each run works on its own copy.

    /// Check a request against this simulation and return the integration
    /// step: at most `dt`, dividing the output step evenly.
    fn check_request(&self, request: &RmsRequest) -> Result<f64, String> {
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
        // Bounded before the count is cast: a huge output step would
        // saturate it and leave a step coarser than `dt`.
        let substeps = substeps(request.output_step, request.dt);
        if substeps > MAX_STEPS {
            return Err(format!(
                "output_step / dt = {:.3e} steps per output step exceeds the limit of {MAX_STEPS:e}; use a smaller output_step",
                request.output_step / request.dt
            ));
        }
        let h = request.output_step / substeps;
        self.check_events(&request.events, request.t_end, 1e-9 * h)?;
        if request.t_end / h > MAX_STEPS {
            return Err(format!(
                "t_end / dt = {:.3e} steps exceeds the limit of {MAX_STEPS:e}; use a larger dt or a shorter run",
                request.t_end / h
            ));
        }
        Ok(h)
    }

    /// Check every event before a run: a finite time in `[0, t_end − tol)`
    /// (an event at the end would act on nothing the run records, and the
    /// run reaches `t_end` once within `tol`, the step's time tolerance), and
    /// an element the system has (each event applied in time order to a
    /// copy of the system, so an element an earlier event adds or removes
    /// is judged as the run would).
    fn check_events(&self, events: &[RmsEvent], t_end: f64, tol: f64) -> Result<(), String> {
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
            if t >= t_end - tol {
                return Err(format!(
                    "event {k} at t = {t} s lies within {tol:.1e} s of the run's end t_end = {t_end} s, where it would act on nothing the run records"
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
        self.run_from(
            request,
            None,
            cancelled,
            #[cfg(feature = "protection")]
            None,
        )
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
        self.run_from(
            request,
            Some(from),
            &mut || false,
            #[cfg(feature = "protection")]
            None,
        )
    }

    /// A run from the start, or from a checkpoint of an earlier run of the
    /// same request; `cancelled` is polled at every output sample.
    // @block The run loop
    fn run_from(
        &self,
        request: &RmsRequest,
        start: Option<&Checkpoint>,
        cancelled: &mut dyn FnMut() -> bool,
        #[cfg(feature = "protection")] mut relays: Option<
            &mut crate::protection::ProtectionRuntime,
        >,
    ) -> Result<RmsResult, String> {
        // @block Check the request
        // The load model must be the prepared one, the steps positive and
        // bounded, and every event must name an element the system has at
        // its time. The integration step `h` divides the output step evenly.
        let h = self.check_request(request)?;

        // @block Start state
        // A copy of the prepared system and its steady state, the output
        // times, the events in time order, and a fresh integrator.
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
        #[cfg(feature = "protection")]
        if let Some(r) = relays.as_deref_mut() {
            r.set_threads(sys.threads);
        }
        // Experimental: with `TELLEGEN_REINIT_SKIP=1`, a re-solve on copies before the
        // run learns whether the one with the device states free is singular, so the first event
        // does not pay for finding out.
        integrate::probe_reinit(&mut stepper, &sys, &x, &f);

        // @block Resume from a checkpoint
        // Replay only: the events before the checkpoint are applied to the
        // network (not the state, which is the checkpoint's), and the
        // integrator gets back the factorization and history it had there,
        // so the samples that follow are the earlier run's.
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

        // @block Prepare recording
        // The recorder takes the first sample at the start time; a run that
        // asks for checkpoints keeps its first one there.
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
        #[cfg(feature = "protection")]
        if let Some(r) = relays.as_deref_mut() {
            let m = protect::RelayMeasure::new(&sys, &x, t);
            r.evaluate(&m);
            r.channels(&m);
        }
        // The relays' evaluation of the state just reached, not yet run.
        #[cfg(feature = "protection")]
        let mut pending: Option<protect::Pending> = None;
        let mut applied: Vec<String> = Vec::new();
        let mut steps = 0usize;
        let mut iterations = 0usize;
        let mut euler_steps = 0usize;

        // @block Time loop
        // Output step by output step up to `t_end`: integrate to the next
        // output time, stopping at every event on the way, then record.
        // Experimental: with `TELLEGEN_PARTIAL=1` a run that cannot go on (a failed
        // step, a network that cannot be re-solved) ends there and returns what it recorded up to
        // the last output time, with the reason as a note, instead of an error.
        let partial = std::env::var("TELLEGEN_PARTIAL").is_ok_and(|v| v == "1");
        let mut stopped: Option<String> = None;
        'time: for &t_target in &targets[k0..] {
            // @block Step to the next output time
            // Steps of at most `h`, cut short at the next event.
            if cancelled() {
                return Err("cancelled".into());
            }
            while t < t_target - tol {
                // @block Relays beside the step
                // The relays' evaluation of the state at `t` runs beside the
                // step from `t` when that acts exactly as running it first
                // (`ProtectionRuntime::can_run_beside`: nothing due now, and
                // nothing it can start falls due within the step); otherwise
                // it runs first, and the step proceeds as below.
                #[cfg(feature = "protection")]
                let beside = {
                    let mut t_next = (t + h).min(t_target);
                    if next_event < events.len() {
                        t_next = t_next.min(events[next_event].time());
                    }
                    let event_due =
                        next_event < events.len() && events[next_event].time() <= t + tol;
                    match (&pending, relays.as_deref()) {
                        (Some(_), Some(r)) => {
                            cfg!(not(all(target_arch = "wasm32", not(target_feature = "atomics"))))
                                && sys.threads
                                && !event_due
                                && r.can_run_beside(t, t_next, h)
                        }
                        _ => false,
                    }
                };
                #[cfg(feature = "protection")]
                if !beside {
                    if let (Some(p), Some(r)) = (pending.take(), relays.as_deref_mut()) {
                        let t0 = crate::rms::clock::Instant::now();
                        p.run(r, &sys);
                        integrate::time_add(0, t0);
                        integrate::STATS[10].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                }
                // @block Apply due events
                // Every event due now changes the network; units left
                // without a path to a source are switched off, the
                // admittance and mass matrices are rebuilt, and the
                // algebraic part (bus voltages, currents) is re-solved with
                // the differential states held, so the next step starts from
                // a consistent point.
                let mut changed = false;
                while next_event < events.len() && events[next_event].time() <= t + tol {
                    applied.push(apply_event(&mut sys, &mut x, &events[next_event])?);
                    next_event += 1;
                    changed = true;
                }
                // Then the relays' breaker actions due now (none when the
                // evaluation runs beside the step).
                #[cfg(feature = "protection")]
                if let Some(r) = relays.as_deref_mut().filter(|_| !beside) {
                    let actions = r.take_due(t, &protect::RelayMeasure::new(&sys, &x, t));
                    for a in &actions {
                        applied.push(protect::apply_action(&mut sys, &x, a, t));
                        changed = true;
                    }
                    r.applied();
                }
                if changed {
                    applied.extend(topology::extinguish(&mut sys, t));
                    sys.rebuild_ybus();
                    sys.rebuild_mass();
                    sys.residual(&x, &mut f, None);
                    // Experimental: relief in a collapse, see `topology::relieve`.
                    let x_keep = x.clone();
                    let mut solved = stepper.reinit(&sys, &mut x, &mut f);
                    for level in RELIEF_LEVELS {
                        let Err(message) = &solved else { break };
                        let hint = topology::hint_bus(message);
                        x.clone_from(&x_keep);
                        let notes = topology::relieve(&mut sys, &x, t, level, hint);
                        if notes.is_empty() {
                            continue;
                        }
                        applied.extend(notes);
                        applied.extend(topology::extinguish(&mut sys, t));
                        sys.rebuild_ybus();
                        sys.rebuild_mass();
                        sys.residual(&x, &mut f, None);
                        solved = stepper.reinit(&sys, &mut x, &mut f);
                    }
                    match solved.map_err(|e| format!("re-solving the network at t = {t:.4} s: {e}")) {
                        Ok(n) => iterations += n,
                        Err(e) if partial => {
                            stopped = Some(e);
                            break 'time;
                        }
                        Err(e) => return Err(e),
                    }
                    #[cfg(feature = "protection")]
                    if let Some(r) = relays.as_deref_mut() {
                        iterations += protect::settle_banks(
                            r,
                            &mut sys,
                            &mut x,
                            &mut f,
                            &mut stepper,
                            t,
                            &mut applied,
                        )?;
                    }
                }

                // @block Integrate one step
                // A trapezoidal step to the next output time or event, with
                // the integrator's fallbacks (backward Euler, halved steps);
                // a failure names the islands the network has split into.
                let mut t_next = (t + h).min(t_target);
                if next_event < events.len() {
                    t_next = t_next.min(events[next_event].time());
                }
                #[cfg(feature = "protection")]
                if let Some(d) = relays.as_deref().and_then(|r| r.next_deadline()) {
                    if d > t + tol {
                        t_next = t_next.min(d);
                    }
                }
                // Experimental: relief in a collapse, see `topology::relieve`.
                let x_keep = x.clone();
                #[cfg(feature = "protection")]
                let mut stepped = match (beside, pending.take(), relays.as_deref_mut()) {
                    #[cfg(not(target_arch = "wasm32"))]
                    (true, Some(p), Some(r)) => {
                        // Both only read the network: the relays measure
                        // their own copy of the state at `t`.
                        let sys = &sys;
                        integrate::STATS[11].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        std::thread::scope(|s| {
                            let relays_done = s.spawn(move || p.run(r, sys));
                            let t0 = crate::rms::clock::Instant::now();
                            let stepped = step_adaptive(
                                sys,
                                &mut x,
                                &mut f,
                                t_next - t,
                                0,
                                &mut euler_steps,
                                &mut stepper,
                            );
                            integrate::time_add(3, t0);
                            let t1 = crate::rms::clock::Instant::now();
                            relays_done.join().expect("the relay evaluation panicked");
                            integrate::time_add(1, t1);
                            stepped
                        })
                    }
                    // In the browser: the relays on the side pool's Web Worker.
                    #[cfg(all(target_arch = "wasm32", target_feature = "atomics"))]
                    (true, Some(p), Some(r)) => {
                        let sys = &sys;
                        integrate::STATS[11].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let job = std::sync::Mutex::new(Some((p, r)));
                        let side = |_: usize| {
                            if let Some((p, r)) = job.lock().expect("relay job").take() {
                                p.run(r, sys);
                            }
                        };
                        spin_pool::SpinPool::side().beside(&side, || {
                            let t0 = crate::rms::clock::Instant::now();
                            let stepped = step_adaptive(
                                sys,
                                &mut x,
                                &mut f,
                                t_next - t,
                                0,
                                &mut euler_steps,
                                &mut stepper,
                            );
                            integrate::time_add(3, t0);
                            stepped
                        })
                    }
                    (_, p, _) => {
                        pending = p;
                        let t0 = crate::rms::clock::Instant::now();
                        let st = step_adaptive(
                            &sys,
                            &mut x,
                            &mut f,
                            t_next - t,
                            0,
                            &mut euler_steps,
                            &mut stepper,
                        );
                        integrate::time_add(3, t0);
                        st
                    }
                };
                #[cfg(not(feature = "protection"))]
                let mut stepped = step_adaptive(
                    &sys,
                    &mut x,
                    &mut f,
                    t_next - t,
                    0,
                    &mut euler_steps,
                    &mut stepper,
                );
                for level in RELIEF_LEVELS {
                    let Err(message) = &stepped else { break };
                    let hint = topology::hint_bus(message);
                    x.clone_from(&x_keep);
                    let notes = topology::relieve(&mut sys, &x, t, level, hint);
                    if notes.is_empty() {
                        continue;
                    }
                    applied.extend(notes);
                    applied.extend(topology::extinguish(&mut sys, t));
                    sys.rebuild_ybus();
                    sys.rebuild_mass();
                    sys.residual(&x, &mut f, None);
                    match stepper.reinit(&sys, &mut x, &mut f) {
                        Err(e) => stepped = Err(e),
                        Ok(_) => {
                            stepped = step_adaptive(
                                &sys,
                                &mut x,
                                &mut f,
                                t_next - t,
                                0,
                                &mut euler_steps,
                                &mut stepper,
                            );
                        }
                    }
                }
                let stepped = stepped.map_err(|e| {
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
                });
                match stepped {
                    Ok(n) => iterations += n,
                    Err(e) if partial => {
                        stopped = Some(e);
                        break 'time;
                    }
                    Err(e) => return Err(e),
                }
                steps += 1;
                sys.relax_loads(&x, t_next - t);
                t = t_next;
                #[cfg(feature = "protection")]
                if let Some(r) = relays.as_deref_mut() {
                        let _tb = integrate::Timer(2, crate::rms::clock::Instant::now());
                        iterations += protect::settle_banks(
                        r,
                        &mut sys,
                        &mut x,
                        &mut f,
                        &mut stepper,
                        t,
                        &mut applied,
                    )?;
                    // Evaluated at the next step: beside it, or first.
                    pending = Some(protect::Pending::new(t, &x));
                }
            }

            // @block Record a sample
            // The selected channels at the output time.
            t = t_target;
            rec.record(&sys, t, &x);
            #[cfg(feature = "protection")]
            if let Some(r) = relays.as_deref_mut() {
                // The relay channels of this output sample, recorded after
                // the evaluation of the same state.
                match pending.as_mut().filter(|p| p.channels.is_none()) {
                    Some(p) => p.channels = Some(t),
                    None => {
                        if let Some(p) = pending.take() {
                            p.run(r, &sys);
                        }
                        r.channels(&protect::RelayMeasure::new(&sys, &x, t));
                    }
                }
            }

            // @block Take a checkpoint
            // At the requested interval: the full state, the integrator's
            // factorization and its history, enough to replay from here.
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
        #[cfg(feature = "protection")]
        if let Some(r) = relays {
            if let Some(p) = pending.take() {
                p.run(r, &sys);
            }
            r.take_channels(&mut rec.channels);
        }

        // @block Assemble the result
        // The recorded channels with what describes them: buses, bases,
        // generators and sources, the events as applied, the notes of the
        // build and the run, and the eigenvalues when asked for.
        let (time, channels, frames) = rec.take();
        #[cfg_attr(feature = "rms-experimental", allow(unused_mut))]
        let mut run_notes: Vec<String> = Vec::new();
        if let Some(e) = stopped {
            run_notes.push(format!("stopped early (TELLEGEN_PARTIAL): {e}"));
        }
        #[cfg(feature = "rms-experimental")]
        let eigenvalues = if request.small_signal && start.is_none() {
            self.eigenvalues().unwrap_or_else(|e| {
                run_notes.push(e);
                Vec::new()
            })
        } else {
            Vec::new()
        };
        #[cfg(not(feature = "rms-experimental"))]
        let eigenvalues = {
            if request.small_signal {
                run_notes.push(
                    "small-signal analysis needs the rms-experimental feature; not computed"
                        .to_owned(),
                );
            }
            Vec::new()
        };
        let generators = self.generators();
        #[cfg(not(feature = "rms-experimental"))]
        let checkpoints = {
            if request
                .record
                .as_ref()
                .is_some_and(|r| r.checkpoint_step > 0.0)
            {
                run_notes
                    .push("checkpoints need the rms-experimental feature; none taken".to_owned());
            }
            Vec::new()
        };
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
                notes.extend(run_notes);
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

// @block Step and output times
// How a run divides time: the output samples up to `t_end`, the
// integration steps per output step, and the bounds on both.

/// Experimental: the order of the collapse reliefs tried at one instant
/// (see `topology::relieve`): small islands, then the named bus (each failure
/// names a new one), then its neighbours, then everything below 0.3 pu.
const RELIEF_LEVELS: [usize; 12] = [0, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 3];

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

// @block Entry points
// The one-call ways in: parse a case and its dynamic data, prepare the
// simulation, and inspect it or run a JSON request on it. The wasm and
// Python adapters call these.

/// Parse a PSS/E-style case and its DYR text, then simulate.
///
/// # Errors
/// When either input fails to parse or the simulation fails.
pub fn simulate(
    network: &BalancedNetwork,
    dyr_text: &str,
    request: &RmsRequest,
) -> Result<RmsResult, String> {
    let sim = prepare(
        network,
        dyr_text,
        request.load_model.unwrap_or_default(),
        RmsOptions::default(),
    )?;
    sim.run(request)
}

/// Read the dynamic data and assemble the simulation. With
/// `rms-experimental` the text may be the JSON form with controller graphs
/// ([`DynamicInput`]).
fn prepare(
    network: &BalancedNetwork,
    dyr_text: &str,
    load_model: LoadModel,
    options: RmsOptions,
) -> Result<RmsSimulation, String> {
    #[cfg(feature = "rms-experimental")]
    {
        let (dynamics, models) = read_dynamics(dyr_text)?;
        RmsSimulation::with_models_and_options(network, &dynamics, &models, load_model, options)
    }
    #[cfg(not(feature = "rms-experimental"))]
    {
        let dynamics = powerio::parse_dyr(dyr_text).map_err(|e| format!("DYR: {e}"))?;
        RmsSimulation::with_options(network, &dynamics, load_model, options)
    }
}

/// Prepare a simulation of a stored PowerIO module (JSON) with DYR text,
/// once, to run any number of times ([`RmsSimulation::run`]) and inspect
/// without a run ([`RmsSimulation::inspection`]). With `rms-experimental`
/// the dynamic data may be the JSON form with controller graphs
/// ([`DynamicInput`]).
///
/// # Errors
/// When the inputs do not parse or the case cannot be initialized.
pub fn prepare_module_json(
    module_json: &str,
    dyr_text: &str,
    load_model: LoadModel,
    options: RmsOptions,
) -> Result<RmsSimulation, String> {
    let network = network_from_module_json(module_json)?;
    prepare(&network, dyr_text, load_model, options)
}

/// DYR text, or the JSON form with controller graphs ([`DynamicInput`]).
#[cfg(feature = "rms-experimental")]
pub(crate) fn read_dynamics(text: &str) -> Result<(DynamicCase, Vec<ModelOverride>), String> {
    let input = DynamicInput::from_text(text)?;
    let dynamics = powerio::parse_dyr(&input.dyr).map_err(|e| format!("DYR: {e}"))?;
    Ok((dynamics, input.models))
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
    let sim = prepare_module_json(module_json, dyr_text, load_model, RmsOptions::default())?;
    #[cfg_attr(not(feature = "rms-experimental"), allow(unused_mut))]
    let mut inspection = sim.inspection();
    #[cfg(feature = "rms-experimental")]
    match sim.eigenvalues() {
        Ok(eig) => inspection.eigenvalues = eig,
        Err(e) => inspection.notes.push(e),
    }
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
