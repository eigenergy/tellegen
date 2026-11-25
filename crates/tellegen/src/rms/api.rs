//! The public types of the RMS engine: what a caller asks for (the request
//! with its events and recording choices, the options of a prepared
//! simulation) and what it gets back (the result, checkpoints, the
//! inspection of a case before a run). Plain data, serializable as JSON; the
//! engine's behaviour lives elsewhere.
//!
//! @block Requests and results

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A controller reference a [`RmsEvent::SetReference`] sets (and a frequency
/// response perturbs).
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputSignal {
    /// Exciter voltage reference (field voltage for a fixed exciter).
    VRef,
    /// Governor power reference (mechanical torque for a fixed governor).
    PRef,
    /// Experimental: signal added at the exciter's summing point, where the
    /// stabilizer enters (`vs`).
    Vs,
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
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
    /// Inertia constant `H` on the machine base, s (the weight of the
    /// machine in a centre of inertia is `H · MBASE`).
    #[serde(default)]
    pub h: f64,
}

/// The simulation result. Channels are keyed `quantity:bus` (`vm`, `va` in
/// radians, `delta` in radians, `omega`, `efd`, `tm` on the machine base,
/// `pe`, `qe` on the system base; `pl`, `ql` the power the loads of a bus
/// draw, system base) and `quantity:from-to` for branch flows
/// (`p_from`, `q_from`, `p_to`, `q_to`, system base), with `#id` appended for
/// a machine or circuit id other than `1`.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
    /// The full state at [`RecordSpec::checkpoint_step`] intervals (empty
    /// without the `rms-experimental` feature, with a note).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checkpoints: Vec<Checkpoint>,
}

/// Choices of one prepared simulation, fixed when it is built. The defaults
/// are tellegen's own; the others exist to compare with another tool or
/// method, and affect only the simulation they are given to.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
    /// Refactorize a step matrix whose pattern was factorized before in that
    /// factorization's pivot order, recomputing the values only, the
    /// default, or pivot every factorization afresh. A pivot that has become
    /// too small against its column falls back to a pivoted factorization.
    pub refactor_values: bool,
    /// Keep a step matrix's factorization until Newton stalls, also across
    /// events, the default, or refresh it on a fixed schedule and after
    /// every event. Newton's stopping test decides convergence either way.
    pub keep_factorization: bool,
    /// Run the relays beside the integration step and split work over
    /// parallel threads where the platform has them, the default, or run
    /// everything on one thread. The results are the same.
    pub threads: bool,
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
    /// Hold each non-windup limit (SEXS field voltage, the limited lags of
    /// block graphs) in its state at the start of a step while Newton solves
    /// the step, and let it switch after the step, with a small hysteresis,
    /// the default; or let the limits switch within Newton's iterations. At
    /// a limit's kink the latter often fails steps and falls back to shorter
    /// ones. Validated against 1 ms references: the held limits are as close
    /// or closer, a limit switches at most one step later.
    pub freeze_limits: bool,
}

impl Default for RmsOptions {
    fn default() -> Self {
        Self {
            renewable_plants: true,
            psid_lv_gain: false,
            psid_converter_ramp: false,
            psid_saturation: false,
            factorize_by_blocks: true,
            refactor_values: true,
            keep_factorization: true,
            threads: true,
            low_voltage_load: LowVoltageLoad::default(),
            reactive_limits: true,
            freeze_limits: true,
        }
    }
}

/// What a run records. Without one ([`RmsRequest::record`] `None`) every
/// channel is recorded at every output step.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
    /// (`RmsSimulation::replay`). Checkpoints need the `rms-experimental`
    /// feature; without it a request for them is noted and none are taken.
    /// The field is there under every feature, so the features stay
    /// additive.
    #[serde(default)]
    pub checkpoint_step: f64,
}

impl RecordSpec {
    /// The channels a display of a run needs: every bus voltage magnitude
    /// and angle, every machine's speed, rotor angle and electrical power,
    /// and every bus's load (what [`screen`] and an app's map and traces
    /// read), at every output step.
    #[must_use]
    pub fn display() -> Self {
        Self {
            channels: ["vm:", "va:", "omega:", "delta:", "pe:", "pl:", "ql:"]
                .map(str::to_owned)
                .to_vec(),
            ..Self::default()
        }
    }

    pub(super) fn matches(patterns: &[String], key: &str) -> bool {
        patterns
            .iter()
            .any(|p| key == p || (p.ends_with(':') && key.starts_with(p.as_str())))
    }
}

/// Channels sampled at their own step (the map frames of a run).
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RmsSeries {
    pub time: Vec<f64>,
    pub channels: BTreeMap<String, Vec<f64>>,
}

/// The full state at `t` (differential and algebraic, in the engine's order)
/// and the integrator's reusable factorization there, so a replay from it
/// continues the run sample for sample.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PastPoint {
    pub h: f64,
    pub x: Vec<f64>,
}

/// What rebuilds an integrator factorization exactly.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SolverState {
    pub at: Vec<f64>,
    pub h: f64,
    pub theta: f64,
}

/// What a case's dynamic data resolves to, for a UI to show before a run.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
    /// `‖f(x0)‖∞` at the initial point ([`RmsSimulation::initial_residual`]).
    #[serde(default)]
    pub initial_residual: f64,
    /// The unit whose speed turns the angle reference frame
    /// ([`RmsSimulation::reference_unit`]).
    #[serde(default)]
    pub reference_unit: Option<String>,
    /// Units held at their power flow output, not simulated
    /// ([`RmsSimulation::held_units`]).
    #[serde(default)]
    pub held_units: Vec<String>,
    /// Simulated renewable plants (`REGCA1` + `REECB1` + `REPCA1`), keyed
    /// `bus` or `bus#id`.
    #[serde(default)]
    pub plants: Vec<String>,
    /// The options the simulation was prepared with.
    #[serde(default)]
    pub options: RmsOptions,
    #[serde(default)]
    pub load_model: LoadModel,
}
