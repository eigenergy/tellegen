//! Protection relays for the RMS engine.
//!
//! A [`ProtectionScheme`] lists relays, each at one location (a line end, a
//! transformer, a busbar, a generating unit or an inverter) with the
//! protection functions it carries. Device numbers are IEEE C37.2's. Each
//! function is written from the published equations and logic of the
//! standard or guide named here; no text or table is taken over beyond
//! the published constants it names.
//!
//! | ANSI | Function | Standard or guide |
//! |---|---|---|
//! | 21 | distance (mho, quadrilateral), zones, k0 | IEC 60255-121 |
//! | 21 | POTT/PUTT teleprotection | IEEE C37.113 |
//! | 68 | power swing blocking (concentric circles) | IEEE PSRC WG D6 report (2005); outside IEC 60255-121 |
//! | 50/51, 67 | overcurrent, definite and inverse time, directional | IEC 60255-151 (its IEC and IEEE curves, Annex A); IEEE C37.112 (IEEE curves and reset) |
//! | 50N/51N | residual (earth fault) overcurrent | IEC 60255-151 |
//! | 27/59, 59N | under/overvoltage, neutral overvoltage | IEC 60255-127 |
//! | 81U/81O, 81R | under/overfrequency, rate of change of frequency | IEC 60255-181; the 500 ms RoCoF window after ENTSO-E's RoCoF guidance |
//! | 87T, 87U | biased differential (transformer, generator unit) | IEC 60255-187-1 |
//! | 87L, 87B | the same biased characteristic (line, busbar) | IEC 60255-187-3 and -187-2 are in preparation |
//! | 49 | thermal overload | IEC 60255-149 |
//! | 24 | overexcitation (V/Hz), definite time | IEEE C37.102, C37.91 |
//! | 32 | reverse power, definite time | IEEE C37.102 |
//! | 40 | loss of field (two-zone offset mho) | IEEE C37.102 |
//! | 46 | negative-sequence current (`I2² t = K`) | IEEE C37.102, C50.13 |
//! | 78 | out of step (blinders, slip counting) | IEEE C37.102 |
//! | FRT | voltage-against-time ride-through envelope | Commission Regulation (EU) 2016/631 (RfG), Art. 14(3)(a) and 16(3)(a) |
//! | 50BF | breaker failure | IEEE C37.119 |
//! | 79 | auto-reclose | IEEE C37.104 |
//!
//! Values the standards leave open are this module's choices and named
//! as such where they are set (the quadrilateral's lower and left limits,
//! the power swing band, the differential slopes, the typical ratings).
//!
//! The relays measure fundamental-frequency phasors: in RMS the network
//! phasors themselves (balanced, positive sequence, plus a measuring time for
//! the relay's own filter). Functions that need unbalance (46, 50N/51N, 59N)
//! act only on an engine that reports unbalanced phase quantities
//! ([`Measure::unbalanced`]), never in RMS.
//!
//! Breakers act per line end. A relay trip schedules its breakers after the
//! breaker operating time; a breaker listed in
//! [`ProtectionScheme::stuck_breakers`] does not open, which the breaker-failure
//! function (50BF) of the relay detects.
//!
//! # Blocks
//!
//! The relay engine is built from the blocks below. The **settings** of a
//! scheme say where each relay sits and what its functions pick up at; a run
//! **binds** them to the simulated network, and the **relay runtime**
//! evaluates every relay on the engine's measurements, runs the
//! **measuring elements** and their timers, and schedules breakers,
//! breaker-failure trips and reclosing. Around a run, the **settings from
//! the network** set and audit a scheme by short-circuit calculation, a
//! **static protection study** runs the same relays on a short circuit, and
//! the **protection tests** check a scheme in RMS runs. The **browser entry
//! points** offer all of this as JSON to the app.
//!
//! @block Protection relays

pub mod api;
mod functions;

// @block Setting and checking a scheme
// Work on a scheme outside a dynamic run: the short-circuit calculation and
// the setting rules, the static study of one fault, and the standard test
// cases run in RMS.
pub mod setting;
pub mod statics;
pub mod testing;
// @end

use std::collections::BTreeMap;
use std::f64::consts::PI;

use num_complex::Complex;
use serde::{Deserialize, Serialize};

pub use functions::{curve_time, Curve};

// @block Phasor helpers
// Complex phasors and the symmetrical components every function measures
// with: the operator `a`, a balanced three-phase set from one phasor, and
// the zero, positive and negative sequences of a phase set.
pub(crate) type C64 = Complex<f64>;

/// `e^{j 120°}`.
pub(crate) fn op_a() -> C64 {
    // Computed once: every phase conversion of every relay asks for it.
    static A: std::sync::OnceLock<C64> = std::sync::OnceLock::new();
    *A.get_or_init(|| C64::from_polar(1.0, 2.0 * PI / 3.0))
}

/// Balanced phase phasors `[a, b, c]` of a positive-sequence phasor.
pub(crate) fn balanced(v: C64) -> [C64; 3] {
    let a = op_a();
    [v, v * a * a, v * a]
}

/// Symmetrical components `[zero, positive, negative]` of phase phasors.
pub(crate) fn sequences(p: [C64; 3]) -> [C64; 3] {
    let a = op_a();
    let a2 = a * a;
    [
        (p[0] + p[1] + p[2]) / 3.0,
        (p[0] + a * p[1] + a2 * p[2]) / 3.0,
        (p[0] + a2 * p[1] + a * p[2]) / 3.0,
    ]
}
// @end

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

// @block Scheme and relays
// The settings a study starts from: the scheme with its relays, the
// breakers made to fail, the measuring time and the series capacitor banks;
// each relay with its location, its functions, its breaker time, and its
// optional breaker-failure and auto-reclose functions.

/// The protection of a case: relays and the breakers that are made to fail.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProtectionScheme {
    #[serde(default)]
    pub relays: Vec<Relay>,
    /// Breakers that do not open when tripped (breaker-failure studies).
    #[serde(default)]
    pub stuck_breakers: Vec<BreakerRef>,
    /// Measuring time of a numerical relay (its phasor filter and processing)
    /// added to every RMS operation. Default one cycle at 50 Hz.
    #[serde(default = "default_measuring_time")]
    pub measuring_time: f64,
    /// The protection of series capacitor banks: their MOV, which conducts
    /// and bypasses the bank while a fault drives its current above the
    /// protective level.
    #[serde(default)]
    pub series_capacitors: Vec<SeriesCapacitor>,
}

/// A series capacitor bank's own protection (IEEE C37.116): its metal-oxide
/// varistor conducts once the bank current exceeds the protective level and
/// bypasses the bank (in RMS: the branch a near short), so a fault does not
/// see the capacitor and its current does not invert; the bank is back in
/// once its current has stayed below its rating for `reinsert`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SeriesCapacitor {
    pub from: usize,
    pub to: usize,
    #[serde(default = "default_circuit")]
    pub circuit: String,
    /// Rated current, system pu.
    pub rated: f64,
    /// Protective level, multiples of the rated current.
    #[serde(default = "default_protective_level")]
    pub protective_level: f64,
    /// Time below the rating before the bank is reinserted, s.
    #[serde(default = "default_reinsert")]
    pub reinsert: f64,
}

fn default_protective_level() -> f64 {
    2.0
}

fn default_reinsert() -> f64 {
    0.05
}

fn default_measuring_time() -> f64 {
    0.02
}

/// How long a condition must hold before it operates on an engine that
/// samples waveforms (not RMS), s.
pub const WAVEFORM_PICKUP_TIME: f64 = 0.005;

/// A breaker: the end of branch `(from, to, circuit)` at bus `at`, or a unit
/// breaker (`machine_id` set, `at` the unit's bus).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BreakerRef {
    #[serde(default)]
    pub from: usize,
    #[serde(default)]
    pub to: usize,
    #[serde(default = "default_circuit")]
    pub circuit: String,
    pub at: usize,
    #[serde(default)]
    pub machine_id: Option<String>,
}

fn default_circuit() -> String {
    "1".into()
}

fn yes() -> bool {
    true
}

/// One relay.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Relay {
    pub id: String,
    pub location: RelayLocation,
    #[serde(default)]
    pub functions: Vec<ProtectionFunction>,
    /// Breaker operating time (trip command to arc extinction), s.
    #[serde(default = "default_breaker_time")]
    pub breaker_time: f64,
    #[serde(default)]
    pub breaker_failure: Option<BreakerFailure>,
    #[serde(default)]
    pub autoreclose: Option<AutoReclose>,
    #[serde(default = "yes")]
    pub enabled: bool,
    /// Rated current of the protected object in system per unit; `None`
    /// takes the branch rating (`rate_a`) or the unit's MVA base.
    #[serde(default)]
    pub i_nom: Option<f64>,
}

fn default_breaker_time() -> f64 {
    0.05
}

/// Where a relay sits and what it protects.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RelayLocation {
    /// The end of a line at bus `at`; trips that end's breaker.
    Line {
        from: usize,
        to: usize,
        #[serde(default = "default_circuit")]
        circuit: String,
        at: usize,
    },
    /// A two-winding transformer; measures at the `from` (HV) side and trips
    /// both ends.
    Transformer {
        from: usize,
        to: usize,
        #[serde(default = "default_circuit")]
        circuit: String,
    },
    /// A busbar zone: every feeder at the bus; trips all of them.
    Busbar { bus: usize },
    /// A synchronous generating unit; trips the unit (and its step-up
    /// transformer for the unit differential).
    Generator {
        bus: usize,
        #[serde(default = "default_circuit")]
        machine_id: String,
    },
    /// An inverter-based unit (interface protection); trips the unit.
    Inverter {
        bus: usize,
        #[serde(default = "default_circuit")]
        machine_id: String,
    },
}

impl RelayLocation {
    /// The bus whose voltage the relay measures.
    #[must_use]
    pub fn bus(&self) -> usize {
        match self {
            Self::Line { at, .. } => *at,
            Self::Transformer { from, .. } => *from,
            Self::Busbar { bus } | Self::Generator { bus, .. } | Self::Inverter { bus, .. } => *bus,
        }
    }
}

/// Direction of a directional element, seen from the relay into the
/// protected object.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    #[default]
    Forward,
    Reverse,
    NonDirectional,
}

// @block Function catalogue
// Every function a relay can carry, by its ANSI device number, and the
// short label that names it in the record and in the tests (`51`, `21 Z2`,
// `87T`, or the label the setting gives it).

/// One protection function.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProtectionFunction {
    /// 50/51 (67 when directional): phase overcurrent.
    Overcurrent(Overcurrent),
    /// 50N/51N (67N): residual overcurrent `|3 I0|`; needs unbalance (not RMS).
    EarthFault(Overcurrent),
    /// 21: distance protection.
    Distance(Distance),
    /// 87: biased differential over the location's zone.
    Differential(Differential),
    /// 27: undervoltage (lowest phase-to-ground voltage).
    Undervoltage(VoltageStage),
    /// 59: overvoltage (highest phase-to-ground voltage).
    Overvoltage(VoltageStage),
    /// 59N: neutral (zero-sequence) overvoltage `|3 V0|`; needs unbalance (not RMS).
    NeutralOvervoltage(VoltageStage),
    /// 81U.
    Underfrequency(FrequencyStage),
    /// 81O.
    Overfrequency(FrequencyStage),
    /// 81R: rate of change of frequency (absolute value).
    Rocof(Rocof),
    /// 24: overexcitation `(V/V_n)/(f/f_n)`.
    Overexcitation(VoltageStage),
    /// 32: reverse (active) power.
    ReversePower(ReversePower),
    /// 40: loss of field.
    LossOfField(LossOfField),
    /// 78: out of step.
    OutOfStep(OutOfStep),
    /// 46: negative-sequence current; needs unbalance (not RMS).
    NegativeSequence(NegativeSequence),
    /// 49: thermal overload.
    Thermal(Thermal),
    /// Fault ride-through envelope: trips when the voltage falls below the
    /// voltage-against-time profile.
    FaultRideThrough(FaultRideThrough),
}

impl ProtectionFunction {
    /// Short label, e.g. `51`, `21 Z2`, `87T`.
    #[must_use]
    pub fn label(&self) -> String {
        let own = |l: &Option<String>, d: &str| l.clone().unwrap_or_else(|| d.to_owned());
        match self {
            Self::Overcurrent(o) => own(
                &o.label,
                match (o.curve, o.direction) {
                    (Curve::Definite, Direction::NonDirectional) => "50",
                    (_, Direction::NonDirectional) => "51",
                    _ => "67",
                },
            ),
            Self::EarthFault(o) => own(
                &o.label,
                if o.direction == Direction::NonDirectional {
                    "51N"
                } else {
                    "67N"
                },
            ),
            Self::Distance(_) => "21".into(),
            Self::Differential(d) => own(&d.label, "87"),
            Self::Undervoltage(s) => own(&s.label, "27"),
            Self::Overvoltage(s) => own(&s.label, "59"),
            Self::NeutralOvervoltage(s) => own(&s.label, "59N"),
            Self::Underfrequency(s) => own(&s.label, "81U"),
            Self::Overfrequency(s) => own(&s.label, "81O"),
            Self::Rocof(s) => own(&s.label, "81R"),
            Self::Overexcitation(s) => own(&s.label, "24"),
            Self::ReversePower(s) => own(&s.label, "32"),
            Self::LossOfField(s) => own(&s.label, "40"),
            Self::OutOfStep(s) => own(&s.label, "78"),
            Self::NegativeSequence(s) => own(&s.label, "46"),
            Self::Thermal(s) => own(&s.label, "49"),
            Self::FaultRideThrough(s) => own(&s.label, "FRT"),
        }
    }
}

// @block Current, voltage and frequency stages
// The settings of the simple stages: an overcurrent stage (definite time or
// an inverse curve, optionally directional), a definite-time voltage stage
// (also used for V/Hz), a frequency stage blocked at low voltage, and the
// rate of change of frequency over a sliding window.

/// Overcurrent stage (IEC 60255-151). Pickup in multiples of the rated
/// current. An inverse stage starts timing at `1.1 ×` pickup: the threshold
/// the standard leaves to the relay, between the setting and `1.3 ×` it; its
/// curve is flat above 20 × pickup.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Overcurrent {
    #[serde(default)]
    pub label: Option<String>,
    pub pickup: f64,
    #[serde(default)]
    pub curve: Curve,
    /// Time multiplier (IEC TMS / IEEE TD).
    #[serde(default = "one")]
    pub tms: f64,
    /// Definite time; for an inverse curve, its minimum operate time, s.
    #[serde(default)]
    pub delay: f64,
    #[serde(default = "non_directional")]
    pub direction: Direction,
}

fn one() -> f64 {
    1.0
}

fn non_directional() -> Direction {
    Direction::NonDirectional
}

/// A definite-time voltage stage (pu of the bus base, phase to ground).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VoltageStage {
    #[serde(default)]
    pub label: Option<String>,
    pub pickup: f64,
    pub delay: f64,
}

/// A definite-time frequency stage. Blocked below `min_voltage` (pu), as is
/// common practice (IEC 60255-181 leaves the blocking to the relay).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrequencyStage {
    #[serde(default)]
    pub label: Option<String>,
    /// Hz.
    pub pickup: f64,
    pub delay: f64,
    #[serde(default = "default_min_voltage")]
    pub min_voltage: f64,
}

fn default_min_voltage() -> f64 {
    0.3
}

/// Rate of change of frequency over a sliding window.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rocof {
    #[serde(default)]
    pub label: Option<String>,
    /// Hz/s (absolute value).
    pub pickup: f64,
    /// Measuring window, s.
    #[serde(default = "default_rocof_window")]
    pub window: f64,
    #[serde(default)]
    pub delay: f64,
    #[serde(default = "default_min_voltage")]
    pub min_voltage: f64,
}

fn default_rocof_window() -> f64 {
    0.5
}

// @block Distance settings
// The distance function (21): mho or quadrilateral zones with their reach,
// angle, delay and direction, residual compensation `k0`, the minimum loop
// current, and the optional extras around it: power swing blocking (68),
// POTT or PUTT teleprotection with the relay at the other line end, and a
// load-encroachment blinder that keeps heavy load out of the zones.

/// Distance characteristic.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Characteristic {
    /// Mho, polarized with the positive-sequence pre-fault voltage for 100 ms
    /// after a collapse (memory), then with the present voltage; with no
    /// voltage left the decision is latched while fault current flows.
    #[default]
    Mho,
    Quadrilateral,
}

/// One distance zone. Reaches in primary ohms (phase to ground loop:
/// positive-sequence ohms with residual compensation).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Zone {
    pub name: String,
    /// Reach along `angle`: the mho diameter, or the quadrilateral's
    /// reactance reach divided by `sin(angle)`.
    pub reach: f64,
    /// Characteristic angle, degrees; default the line angle.
    #[serde(default)]
    pub angle: Option<f64>,
    /// Quadrilateral resistive reach (ohms); default `reach / 2`.
    #[serde(default)]
    pub resistive_reach: Option<f64>,
    pub delay: f64,
    #[serde(default)]
    pub direction: Direction,
    /// A trip of this zone starts the auto-recloser.
    #[serde(default)]
    pub autoreclose: bool,
}

/// Teleprotection scheme for distance relays at both ends of a line.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SchemeKind {
    /// Permissive overreaching transfer trip: trip when the local and the
    /// remote overreaching zones both see a forward fault.
    Pott,
    /// Permissive underreaching transfer trip: the remote underreaching zone
    /// (Z1) sends permission to the local overreaching zone.
    Putt,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Teleprotection {
    pub scheme: SchemeKind,
    /// The overreaching zone, default `Z2`.
    #[serde(default = "default_overreach_zone")]
    pub zone: String,
    /// Channel time, s.
    #[serde(default = "default_channel_time")]
    pub channel_time: f64,
}

fn default_overreach_zone() -> String {
    "Z2".into()
}

fn default_channel_time() -> f64 {
    0.01
}

/// Power swing blocking (68): an impedance that takes longer than `delta_t`
/// to travel from the outer to the inner boundary is a swing; the zones stay
/// blocked while it lasts (at most `max_block`). The boundaries are
/// concentric with the largest forward zone (IEEE PSRC WG D6): the mho of its
/// reach along the line angle, and that mho scaled by `outer`. An
/// impedance that keeps moving inside the outer boundary for `delta_t` is a
/// swing too (continuous impedance calculation): a fault cleared into a
/// swing lands inside it at once.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PowerSwingBlocking {
    /// Outer boundary as a factor on the largest forward reach.
    #[serde(default = "default_psb_outer")]
    pub outer: f64,
    #[serde(default = "default_psb_dt")]
    pub delta_t: f64,
    #[serde(default = "default_psb_max")]
    pub max_block: f64,
}

fn default_psb_outer() -> f64 {
    1.3
}
fn default_psb_dt() -> f64 {
    0.03
}
fn default_psb_max() -> f64 {
    2.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Distance {
    #[serde(default)]
    pub characteristic: Characteristic,
    pub zones: Vec<Zone>,
    /// Line (positive-sequence) impedance angle, degrees.
    pub line_angle: f64,
    /// Residual compensation `k0 = (Z0 − Z1)/(3 Z1)` as `[re, im]`.
    #[serde(default)]
    pub k0: [f64; 2],
    /// Minimum loop current, multiples of the rated current.
    #[serde(default = "default_min_current")]
    pub min_current: f64,
    #[serde(default)]
    pub power_swing_blocking: Option<PowerSwingBlocking>,
    #[serde(default)]
    pub teleprotection: Option<Teleprotection>,
    /// Load encroachment: impedances beyond `r` ohms within `angle`
    /// degrees of the R axis (either direction) are load, not faults.
    #[serde(default)]
    pub load_encroachment: Option<LoadEncroachment>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LoadEncroachment {
    pub r: f64,
    pub angle: f64,
}

fn default_min_current() -> f64 {
    0.2
}

// @block Differential settings
// The biased differential (87T, 87U, 87L, 87B): the operate current against
// the restraint current on a two-slope characteristic, an unrestrained
// stage, optional second-harmonic blocking, and the channel time of a line
// differential.

/// Biased differential (IEC 60255-187-1): operate when
/// `I_d > pickup` and `I_d > slope1 · I_r` (`I_r ≤ knee`) or
/// `I_d > slope1 · knee + slope2 · (I_r − knee)`; at once above
/// `unrestrained`. Currents in multiples of the rated current;
/// `I_d = |Σ I|`, `I_r = Σ |I| / 2`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Differential {
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default = "default_diff_pickup")]
    pub pickup: f64,
    #[serde(default = "default_slope1")]
    pub slope1: f64,
    #[serde(default = "default_slope2")]
    pub slope2: f64,
    #[serde(default = "default_knee")]
    pub knee: f64,
    #[serde(default = "default_unrestrained")]
    pub unrestrained: f64,
    /// Second-harmonic blocking ratio (transformers; needs an engine that measures harmonics).
    #[serde(default)]
    pub harmonic2: Option<f64>,
    /// Communication time between the ends (line differential), s.
    #[serde(default)]
    pub channel_time: f64,
    #[serde(default)]
    pub delay: f64,
}

fn default_diff_pickup() -> f64 {
    0.3
}
fn default_slope1() -> f64 {
    0.3
}
fn default_slope2() -> f64 {
    0.7
}
fn default_knee() -> f64 {
    2.0
}
fn default_unrestrained() -> f64 {
    10.0
}

// @block Unit and thermal settings
// The functions of a generating unit and the thermal image: reverse power
// (32), loss of field by two offset mho circles (40), out of step by
// blinders and slip counting (78), negative-sequence heating `I2² t` (46)
// and the thermal overload of IEC 60255-149 (49), which transformers use
// too.

/// Reverse power: trips when the unit absorbs more than `pickup` (fraction
/// of its MVA base) for `delay`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReversePower {
    #[serde(default)]
    pub label: Option<String>,
    pub pickup: f64,
    pub delay: f64,
}

/// Loss of field: two offset mho circles below the R axis, on the machine
/// base: centre at `−j (offset + d/2)`, diameter `d`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LossOfField {
    #[serde(default)]
    pub label: Option<String>,
    pub offset: f64,
    pub diameter1: f64,
    pub delay1: f64,
    pub diameter2: f64,
    pub delay2: f64,
}

/// Out of step: blinders at `R = ±blinder` (machine base) between
/// `X = −reverse_reach` and `X = forward_reach`; a slip is the impedance
/// crossing from one side to the other, spending at least `min_traverse` s
/// between the blinders.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OutOfStep {
    #[serde(default)]
    pub label: Option<String>,
    pub blinder: f64,
    pub forward_reach: f64,
    pub reverse_reach: f64,
    #[serde(default = "default_traverse")]
    pub min_traverse: f64,
    #[serde(default = "one_usize")]
    pub slips: usize,
}

fn default_traverse() -> f64 {
    0.025
}
fn one_usize() -> usize {
    1
}

/// Negative-sequence current (IEEE C37.102, C50.13 `I2² t = K`): trips when
/// `∫ (I2² − I2c²) dt ≥ k` (multiples of the rated current). Subtracting the
/// continuous capability `I2c` and cooling at that rate below it are this
/// module's choice; with `continuous = 0` it is the plain `I2² t = K`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NegativeSequence {
    #[serde(default)]
    pub label: Option<String>,
    pub continuous: f64,
    pub k: f64,
}

/// Thermal overload (IEC 60255-149): `τ dθ/dt = (I/(k I_n))² − θ`, trip at
/// `θ = 1`. Starts at the thermal state of the initial current.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Thermal {
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default = "default_thermal_k")]
    pub k: f64,
    pub tau: f64,
}

fn default_thermal_k() -> f64 {
    1.1
}

// @block Ride-through, breaker failure and reclosing
// The voltage-against-time envelope an inverter-based unit must ride
// through, the breaker-failure function (50BF) that trips the neighbouring
// breakers when current still flows after a trip, and three-pole
// auto-reclose (79) with its dead time, reclaim time, shots and
// synchronism check.

/// Fault ride-through: below `start` (pu) the clock starts; the unit trips
/// when the voltage is below the profile `points` (`[t, v]`, linear between
/// points, the last value afterwards). The clock resets once the voltage is
/// back above `start` for `reset` s.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FaultRideThrough {
    #[serde(default)]
    pub label: Option<String>,
    pub points: Vec<[f64; 2]>,
    #[serde(default = "default_frt_start")]
    pub start: f64,
    #[serde(default = "default_frt_reset")]
    pub reset: f64,
}

fn default_frt_start() -> f64 {
    0.85
}
fn default_frt_reset() -> f64 {
    0.5
}

/// Breaker failure (50BF): when current still flows `delay` s after the
/// trip, the other breakers at the relay's bus are tripped.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BreakerFailure {
    #[serde(default = "default_bf_delay")]
    pub delay: f64,
    /// Current threshold, multiples of the rated current.
    #[serde(default = "default_bf_current")]
    pub current: f64,
}

fn default_bf_delay() -> f64 {
    0.15
}
fn default_bf_current() -> f64 {
    0.1
}

/// Three-pole auto-reclose (79) after trips of initiating functions.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AutoReclose {
    #[serde(default = "default_dead_time")]
    pub dead_time: f64,
    #[serde(default = "default_reclaim")]
    pub reclaim: f64,
    #[serde(default = "one_usize")]
    pub shots: usize,
    /// Synchronism check (25): the largest angle across the line, degrees,
    /// at which the follower end closes.
    #[serde(default = "default_sync_angle")]
    pub sync_angle: f64,
}

fn default_sync_angle() -> f64 {
    30.0
}

fn default_dead_time() -> f64 {
    0.5
}
fn default_reclaim() -> f64 {
    5.0
}

// ---------------------------------------------------------------------------
// Results
// ---------------------------------------------------------------------------

// @block Relay record
// What the relays report from a run: every operation in time order
// (pickups, resets, trips, breaker operations, reclosing, blocking, series
// capacitor bypass), the first trip of each relay, and notes such as the
// functions that stay inactive in RMS.

/// What happened in a relay.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationKind {
    Pickup,
    Reset,
    Trip,
    BreakerOpen,
    BreakerStuck,
    BreakerFailure,
    Reclose,
    Lockout,
    Blocked,
    Unblocked,
    /// A series capacitor bank bypassed by its MOV (not a trip).
    Bypassed,
    Reinserted,
    /// Reclosing held by the synchronism check.
    SyncHeld,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Operation {
    pub time: f64,
    pub relay: String,
    pub function: String,
    pub kind: OperationKind,
    pub detail: String,
}

/// The relays' record of a run.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProtectionReport {
    pub operations: Vec<Operation>,
    /// Relay ids that tripped, with the first trip time and function.
    pub trips: Vec<TripRecord>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TripRecord {
    pub relay: String,
    pub function: String,
    pub time: f64,
}

// ---------------------------------------------------------------------------
// Binding to a simulation
// ---------------------------------------------------------------------------

pub use crate::rms::{End, TopoBranch, TopoUnit, Topology, UnitKind};

// @block Engine interface
// What the relays need from an engine and give back to it. An engine
// implements `Measure` (bus voltages, branch end, unit and feeder currents,
// breaker and unit states at one instant), and applies the `Action`s the
// relays return: opening and closing branch ends, tripping units, bypassing
// and reinserting series capacitors.

/// Phasor measurements at one instant (phase phasors in system per unit,
/// phase-to-ground voltages scaled so a balanced set has `|V| = 1` pu).
pub trait Measure: Sync {
    fn time(&self) -> f64;
    /// Speed of the phasor reference frame, pu.
    fn frame_speed(&self) -> f64;
    fn bus_voltage(&self, b: usize) -> [C64; 3];
    /// Current into the branch at its end.
    fn branch_current(&self, e: usize, end: End) -> [C64; 3];
    /// Current out of the unit into its bus (zero when tripped).
    fn unit_current(&self, u: usize) -> [C64; 3];
    /// Current drawn by the loads and fixed shunts at the bus.
    fn feeder_current(&self, b: usize) -> [C64; 3];
    /// Voltage at a branch end's bus.
    fn branch_end_voltage(&self, e: usize, end: End) -> Option<[C64; 3]>;
    fn end_closed(&self, e: usize, end: End) -> bool;
    fn unit_in_service(&self, u: usize) -> bool;
    /// Second-harmonic magnitude of a branch end current per phase, when the
    /// engine measures it (not RMS).
    fn branch_current_h2(&self, _e: usize, _end: End) -> Option<[f64; 3]> {
        None
    }
    /// Whether unbalance is measured; RMS phasors are balanced.
    fn unbalanced(&self) -> bool {
        false
    }
    /// Whether branch currents are the series currents only, without the
    /// line charging (an engine that lumps the charging at the buses).
    fn series_branch_current(&self) -> bool {
        false
    }
}

/// An action for the engine.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Action {
    OpenEnd(usize, End),
    CloseEnd(usize, End),
    TripUnit(usize),
    /// A series capacitor bank bypassed by its protection.
    Bypass(usize),
    /// A series capacitor bank put back in.
    Reinsert(usize),
}

// @block Relay sites
// A relay resolved against the network: the bus whose voltage it measures,
// the current transformer it reads, the current transformers of its
// differential zone, the breakers it trips (and those its breaker-failure
// function trips), its rated current and impedance base; and the breaker
// operations it has scheduled.

/// A series capacitor bank during a run.
#[derive(Clone, Debug)]
struct Bank {
    id: String,
    e: usize,
    rated: f64,
    level: f64,
    reinsert: f64,
    bypassed: bool,
    below_since: Option<f64>,
}

/// A current transformer of a differential zone: its current times `factor`
/// flows into the zone.
#[derive(Clone, Copy, Debug)]
pub(crate) enum CtSource {
    Branch(usize, End),
    Unit(usize),
    Feeder(usize),
}

#[derive(Clone, Debug)]
pub(crate) struct Ct {
    pub source: CtSource,
    pub factor: C64,
}

/// A relay resolved against a topology.
#[derive(Clone, Debug)]
pub(crate) struct Site {
    /// Dense bus of the voltage measurement.
    pub bus: usize,
    /// The measured current (into the protected object).
    pub current: Option<CtSource>,
    /// Differential zone.
    pub zone: Vec<Ct>,
    /// Charging current compensation of a line differential:
    /// `(branch, y_fr, y_to)`.
    pub charging: Option<(usize, C64, C64)>,
    /// Breakers this relay trips.
    pub trips: Vec<Target>,
    /// Extra breakers the differential trips (unit step-up transformer).
    pub zone_trips: Vec<Target>,
    /// Breakers its breaker-failure function trips.
    pub backup: Vec<Target>,
    /// The relay's own branch end (auto-reclose, teleprotection partner).
    pub line: Option<(usize, End)>,
    /// Rated current, system pu.
    pub i_nom: f64,
    /// Machine base over system base (generator functions).
    pub scale: f64,
    pub z_base: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Target {
    End(usize, End),
    Unit(usize),
}

/// A scheduled breaker or reclose action.
#[derive(Clone, Debug)]
struct Scheduled {
    time: f64,
    relay: usize,
    kind: ScheduledKind,
}

#[derive(Clone, Debug)]
enum ScheduledKind {
    Open(Target),
    /// Reclose an end; under the synchronism check until the time given.
    Close(usize, End, f64),
    /// Breaker-failure check of the relay's trip at `trip_time`.
    FailureCheck,
}

// @block Relay runtime
// The relays during one run. The engine asks for the next deadline before
// each step, takes the breaker actions due, lets the series capacitor
// banks act, and has every relay evaluated on its measurements; the
// runtime keeps the schedule of breakers and reclosing, the distance
// relays' impedance channels and the record.

/// The live state of the relays during one run.
pub struct ProtectionRuntime {
    relays: Vec<functions::RelayState>,
    /// Per distance relay, its apparent impedance at each output sample
    /// (`z_r:` and `z_x:` channels), named only when taken.
    traces: Vec<(usize, Vec<f64>, Vec<f64>)>,
    /// Series capacitor banks and their MOV state.
    banks: Vec<Bank>,
    stuck: Vec<Target>,
    schedule: Vec<Scheduled>,
    report: ProtectionReport,
    measuring_time: f64,
    f0: f64,
    last_t: Option<f64>,
    /// [`Self::next_deadline`], kept until a `&mut self` method runs (the
    /// engines ask before every step; it changes only when the relays do).
    deadline: std::cell::Cell<Option<Option<f64>>>,
    /// The earliest relay timer expiring, kept from the last evaluation
    /// (`None`: to be computed); breaker actions that change relays clear it.
    relay_deadline: std::cell::Cell<Option<Option<f64>>>,
    /// The least time from an evaluation to anything it can make due that
    /// the evaluation before did not announce, an inverse-time pickup aside:
    /// a timer's measuring time, or the breaker (or breaker-failure check)
    /// of an element that decides at once.
    lead: f64,
    /// The shortest operate time of any inverse-time element (infinite
    /// without one).
    inverse_min_time: f64,
    /// After the last evaluation, an inverse-time element picked up or
    /// still integrating.
    inverse_running: bool,
}

impl ProtectionRuntime {
    /// Resolve the scheme against a simulation's topology.
    ///
    /// # Errors
    /// When a relay names a missing element or carries a function its
    /// location cannot measure.
    // @block Bind a scheme
    pub fn bind(scheme: &ProtectionScheme, topo: &Topology, rms: bool) -> Result<Self, String> {
        // @block Resolve the relays
        // Each enabled relay is resolved to its site; in RMS a function that
        // needs unbalance (46, 50N/51N, 59N) is noted as inactive.
        let mut relays = Vec::new();
        let mut notes = Vec::new();
        for r in scheme.relays.iter().filter(|r| r.enabled) {
            let site = resolve(r, topo).map_err(|e| format!("relay {}: {e}", r.id))?;
            if rms {
                for f in &r.functions {
                    if matches!(
                        f,
                        ProtectionFunction::EarthFault(_)
                            | ProtectionFunction::NeutralOvervoltage(_)
                            | ProtectionFunction::NegativeSequence(_)
                    ) {
                        notes.push(format!(
                            "relay {}: {} needs unbalance and stays inactive in RMS (balanced phasors)",
                            r.id,
                            f.label()
                        ));
                    }
                }
            }
            relays.push(functions::RelayState::new(r.clone(), site));
        }
        // @block Teleprotection partners
        // Teleprotection partners: the distance relay at the other end of
        // the same branch.
        let ends: Vec<Option<(usize, End)>> = relays.iter().map(|r| r.site.line).collect();
        for (i, r) in relays.iter_mut().enumerate() {
            if let Some((e, end)) = ends[i] {
                r.partner = ends
                    .iter()
                    .enumerate()
                    .find(|(j, o)| {
                        *j != i && matches!(o, Some((e2, end2)) if *e2 == e && *end2 != end)
                    })
                    .map(|(j, _)| j);
            }
        }
        // @block Stuck breakers and capacitor banks
        // The breakers a study makes fail are resolved to branch ends or
        // units; each series capacitor bank gets its protective level and
        // reinsertion time. The measuring time is the scheme's in RMS, and a
        // fixed short pickup time on an engine that samples waveforms.
        let mut stuck = Vec::new();
        for b in &scheme.stuck_breakers {
            let target = if let Some(id) = &b.machine_id {
                Target::Unit(topo.unit(b.at, id)?)
            } else {
                let e = topo.branch(b.from, b.to, &b.circuit)?;
                let end = if topo.branches[e].from_number == b.at {
                    End::From
                } else if topo.branches[e].to_number == b.at {
                    End::To
                } else {
                    return Err(format!(
                        "stuck breaker: bus {} is not an end of {}-{}",
                        b.at, b.from, b.to
                    ));
                };
                Target::End(e, end)
            };
            stuck.push(target);
        }
        let mut banks = Vec::new();
        for c in &scheme.series_capacitors {
            let e = topo.branch(c.from, c.to, &c.circuit)?;
            banks.push(Bank {
                id: if c.circuit.trim() == "1" {
                    format!("SC{}-{}", c.from, c.to)
                } else {
                    format!("SC{}-{}#{}", c.from, c.to, c.circuit.trim())
                },
                e,
                rated: c.rated.max(1e-9),
                level: c.protective_level * c.rated.max(1e-9),
                reinsert: c.reinsert.max(0.0),
                bypassed: false,
                below_since: None,
            });
        }
        let traces = relays
            .iter()
            .enumerate()
            .filter(|(_, r)| r.has_distance())
            .map(|(k, _)| (k, Vec::new(), Vec::new()))
            .collect();
        let inverse_min_time = relays
            .iter()
            .filter_map(functions::RelayState::inverse_min_time)
            .fold(f64::INFINITY, f64::min);
        let mut rt = Self {
            lead: 0.0,
            inverse_min_time,
            inverse_running: false,
            traces,
            relays,
            stuck,
            banks,
            schedule: Vec::new(),
            report: ProtectionReport {
                notes,
                ..ProtectionReport::default()
            },
            // Relays on sampled waveforms filter them themselves; a condition
            // must still hold for 5 ms (several samples) before it operates, as
            // numerical relays count consecutive samples against switching
            // transients in the filter window.
            measuring_time: if rms {
                scheme.measuring_time.max(0.0)
            } else {
                WAVEFORM_PICKUP_TIME
            },
            f0: topo.f0,
            last_t: None,
            deadline: std::cell::Cell::new(None),
            relay_deadline: std::cell::Cell::new(None),
        };
        // A trip on a timer comes at a deadline the engine knows from the
        // evaluation before (and waits on); only the elements that decide
        // at once trip unannounced, a breaker time or a breaker-failure
        // delay after the evaluation.
        let unannounced = rt
            .relays
            .iter()
            .filter(|r| r.decides_at_once())
            .flat_map(|r| {
                let bf = r.spec.breaker_failure.as_ref().map(|b| b.delay);
                std::iter::once(r.spec.breaker_time).chain(bf)
            })
            .fold(f64::INFINITY, f64::min);
        rt.lead = rt.measuring_time.min(unannounced.max(0.0));
        Ok(rt)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.relays.is_empty()
    }

    /// The next instant something is due: a breaker, a reclose, or a relay
    /// timer expiring.
    // @block Next deadline
    #[must_use]
    pub fn next_deadline(&self) -> Option<f64> {
        if let Some(d) = self.deadline.get() {
            return d;
        }
        let r = match self.relay_deadline.get() {
            Some(r) => r,
            None => {
                let r = self.relays_deadline();
                self.relay_deadline.set(Some(r));
                r
            }
        };
        let d = self
            .schedule
            .iter()
            .map(|s| s.time)
            .chain(r)
            .min_by(f64::total_cmp);
        self.deadline.set(Some(d));
        d
    }

    /// The earliest timer expiring over all relays (after a breaker
    /// operation; an evaluation keeps it as it goes).
    fn relays_deadline(&self) -> Option<f64> {
        let mt = self.measuring_time;
        self.relays
            .iter()
            .filter_map(|r| r.deadline(mt))
            .min_by(f64::total_cmp)
    }

    /// The next breaker operation due (an open, a reclose or a
    /// breaker-failure check): the only instants at which [`Self::take_due`]
    /// acts. [`Self::next_deadline`] also names relay timers expiring, which
    /// act at the next evaluation instead.
    #[must_use]
    pub fn next_due(&self) -> Option<f64> {
        self.schedule.iter().map(|s| s.time).min_by(f64::total_cmp)
    }

    /// The shortest time from an evaluation to anything it schedules: a
    /// breaker opening or a breaker-failure check. An engine may run this
    /// far ahead of evaluations it has not seen the results of.
    #[must_use]
    pub fn min_latency(&self) -> f64 {
        self.relays
            .iter()
            .flat_map(|r| {
                let bf = r.spec.breaker_failure.as_ref().map(|b| b.delay);
                std::iter::once(r.spec.breaker_time).chain(bf)
            })
            .fold(f64::INFINITY, f64::min)
            .max(0.0)
    }

    /// Whether the evaluation at `t` may run beside the integration step to
    /// `t_next` (steps at most `h` long) and still act exactly as if it had
    /// run first. It may when no breaker operation is due at `t` (the
    /// engine then leaves the relays alone until `t_next`), no known
    /// deadline falls before `t_next` (one the evaluation could still move),
    /// and nothing the evaluation starts can fall due before `t_next`: a
    /// timer runs at least the measuring time, a breaker opens at least its
    /// breaker time after the trip, an inverse-time curve at least its
    /// shortest operate time after the pickup (less the step it integrated
    /// over) as long as no inverse-time element is already running.
    #[must_use]
    pub fn can_run_beside(&self, t: f64, t_next: f64, h: f64) -> bool {
        let tol = 1e-9;
        let span = t_next - t;
        if self.inverse_running
            || self.lead <= span + tol
            || self.inverse_min_time - h <= span + tol
            || self.next_due().is_some_and(|d| d <= t + tol)
        {
            return false;
        }
        self.next_deadline().is_none_or(|d| d >= t_next)
    }

    /// Actions due at or before `t` (breakers opening, reclosing).
    // @block Breaker actions due
    pub fn take_due(&mut self, t: f64, m: &dyn Measure) -> Vec<Action> {
        self.deadline.set(None);
        let tol = 1e-9;
        let mut due: Vec<Scheduled> = Vec::new();
        self.schedule.retain(|s| {
            if s.time <= t + tol {
                due.push(s.clone());
                false
            } else {
                true
            }
        });
        due.sort_by(|a, b| a.time.total_cmp(&b.time));
        let mut actions = Vec::new();
        for s in due {
            let id = self.relays[s.relay].spec.id.clone();
            match s.kind {
                // @block Open a breaker
                // A breaker made to fail stays closed and is logged as stuck;
                // otherwise a closed end opens or a unit in service trips, and
                // the relay learns when its own line end opened.
                ScheduledKind::Open(target) => {
                    if self.stuck.contains(&target) {
                        self.log(
                            s.time,
                            &id,
                            "CB",
                            OperationKind::BreakerStuck,
                            "breaker failed to open".into(),
                        );
                        continue;
                    }
                    let open = match target {
                        Target::End(e, end) => {
                            m.end_closed(e, end).then_some(Action::OpenEnd(e, end))
                        }
                        Target::Unit(u) => m.unit_in_service(u).then_some(Action::TripUnit(u)),
                    };
                    if let Some(a) = open {
                        self.log(
                            s.time,
                            &id,
                            "CB",
                            OperationKind::BreakerOpen,
                            describe_action(&a),
                        );
                        if let Some(rs) = self.relays.get_mut(s.relay) {
                            rs.breaker_opened(s.time, &a);
                            self.relay_deadline.set(None);
                        }
                        actions.push(a);
                    }
                }
                // @block Reclose under the synchronism check
                // An open line end closes when the line is dead from the other
                // side or the angle across it is within the limit (25);
                // otherwise it tries again every 20 ms until its deadline, then
                // stays open and logs that the check held it.
                ScheduledKind::Close(e, end, until) => {
                    if !m.end_closed(e, end) {
                        // Synchronism check (25): the line live from its other
                        // end, this end closes only within the angle limit.
                        let other = if end == End::From { End::To } else { End::From };
                        let angle = if m.end_closed(e, other) {
                            match (m.branch_end_voltage(e, end), m.branch_end_voltage(e, other)) {
                                (Some(a), Some(b)) if a[0].norm() > 1e-6 && b[0].norm() > 1e-6 => {
                                    Some((a[0] / b[0]).arg().to_degrees().abs())
                                }
                                _ => None,
                            }
                        } else {
                            None
                        };
                        let limit = self.relays[s.relay]
                            .spec
                            .autoreclose
                            .as_ref()
                            .map_or(30.0, |ar| ar.sync_angle);
                        if let Some(d) = angle.filter(|&d| d > limit) {
                            if s.time + 0.02 <= until {
                                self.schedule.push(Scheduled {
                                    time: s.time + 0.02,
                                    relay: s.relay,
                                    kind: ScheduledKind::Close(e, end, until),
                                });
                            } else {
                                self.log(
                                    s.time,
                                    &id,
                                    "25",
                                    OperationKind::SyncHeld,
                                    format!("angle across the line {d:.0}° above {limit:.0}°: not reclosed"),
                                );
                            }
                            continue;
                        }
                        let a = Action::CloseEnd(e, end);
                        self.log(
                            s.time,
                            &id,
                            "79",
                            OperationKind::Reclose,
                            describe_action(&a),
                        );
                        self.relays[s.relay].reclosed(s.time);
                        self.relay_deadline.set(None);
                        actions.push(a);
                    }
                }
                // @block Breaker-failure check
                // 50BF: when current still flows through the relay's current
                // transformer after its delay, the other breakers at the
                // relay's bus are scheduled to open.
                ScheduledKind::FailureCheck => {
                    let r = &self.relays[s.relay];
                    if r.current_flowing(m) {
                        let backup = r.site.backup.clone();
                        self.log(
                            s.time,
                            &id,
                            "50BF",
                            OperationKind::BreakerFailure,
                            format!(
                                "current still flows; tripping {} adjacent breaker(s)",
                                backup.len()
                            ),
                        );
                        for target in backup {
                            self.schedule.push(Scheduled {
                                time: s.time + self.relays[s.relay].spec.breaker_time,
                                relay: s.relay,
                                kind: ScheduledKind::Open(target),
                            });
                        }
                    }
                }
            }
        }
        actions
    }

    /// Evaluate every relay on the measurements at `m.time()`.
    // @block Evaluate the relays
    pub fn evaluate(&mut self, m: &dyn Measure) {
        self.deadline.set(None);
        self.relay_deadline.set(None);
        let t = m.time();
        let dt = self.last_t.map_or(0.0, |t0| (t - t0).max(0.0));
        self.last_t = Some(t);
        // @block Measure
        // Pass 1: measure and run each relay's elements. The relays are
        // independent here, so a large scheme runs in chunks on the thread
        // pool; the operations join in relay order, as inline. Each chunk
        // also names its busy relays (see `RelayState::busy`), in relay
        // order.
        let f0 = self.f0;
        let measure = |c: usize, part: &mut [functions::RelayState]| {
            let mut o = Vec::new();
            let mut b = Vec::new();
            let mut inverse = false;
            for (k, r) in part.iter_mut().enumerate() {
                r.measure(m, dt, f0, &mut o);
                if r.busy() {
                    b.push(c * RELAY_CHUNK + k);
                }
                inverse |= r.inverse_running();
            }
            (o, b, inverse)
        };
        #[cfg(not(target_arch = "wasm32"))]
        let parts: Vec<(Vec<Operation>, Vec<usize>, bool)> = {
            use rayon::prelude::*;
            self.relays
                .par_chunks_mut(RELAY_CHUNK)
                .enumerate()
                .map(|(c, part)| measure(c, part))
                .collect()
        };
        #[cfg(target_arch = "wasm32")]
        let parts: Vec<(Vec<Operation>, Vec<usize>, bool)> = self
            .relays
            .chunks_mut(RELAY_CHUNK)
            .enumerate()
            .map(|(c, part)| measure(c, part))
            .collect();
        let mut ops: Vec<Operation> = Vec::new();
        let mut busy: Vec<usize> = Vec::new();
        self.inverse_running = false;
        for (o, b, inverse) in parts {
            ops.extend(o);
            busy.extend(b);
            self.inverse_running |= inverse;
        }
        // @block Teleprotection permission
        // Teleprotection permission from the partners' zones, for the relays
        // that decide below.
        for &i in &busy {
            let remote = self.relays[i]
                .partner
                .map(|p| (self.relays[p].overreach_fwd, self.relays[p].underreach_fwd));
            self.relays[i].remote = remote;
        }
        // @block Decide
        // Pass 2: decide. Each relay names the function whose timer ran out
        // first, if any; a relay trips once and then stays latched. Only
        // the busy relays are visited (an idle one gives nothing and keeps
        // its state), in relay order; the earliest timer of the relays as
        // they stand is kept on the way.
        let mut trips: Vec<(usize, String, bool, bool)> = Vec::new();
        let mut earliest: Option<f64> = None;
        for i in busy {
            let r = &mut self.relays[i];
            if let Some((label, ar, zone_trip)) = r.decide(t, self.measuring_time, &mut ops) {
                trips.push((i, label, ar, zone_trip));
            }
            if let Some(d) = r.deadline(self.measuring_time) {
                earliest = Some(earliest.map_or(d, |e: f64| e.min(d)));
            }
        }
        self.report.operations.extend(ops);
        // @block Schedule the trips
        // A trip is recorded, its breakers are scheduled after the breaker
        // time (a unit after its breakers), the breaker-failure check after
        // its delay, and auto-reclose is armed or locked out.
        for (i, label, ar, zone_trip) in trips {
            let r = &mut self.relays[i];
            let id = r.spec.id.clone();
            if !self.report.trips.iter().any(|tr| tr.relay == id) {
                self.report.trips.push(TripRecord {
                    relay: id.clone(),
                    function: label.clone(),
                    time: t,
                });
            }
            self.report.operations.push(Operation {
                time: t,
                relay: id,
                function: label,
                kind: OperationKind::Trip,
                detail: String::new(),
            });
            r.trip_time = Some(t);
            r.ar_pending = ar && r.ar_available(t);
            let tb = t + r.spec.breaker_time;
            let mut targets = r.site.trips.clone();
            if zone_trip {
                targets.extend(r.site.zone_trips.iter().copied());
            }
            // A unit whose breakers also open (unit differential) is
            // tripped (field and turbine) after them, as the generator
            // breaker clears at its current zero first.
            let with_ends = targets.iter().any(|t| matches!(t, Target::End(..)));
            for target in targets {
                let at = if with_ends && matches!(target, Target::Unit(_)) {
                    tb + 0.03
                } else {
                    tb
                };
                self.schedule.push(Scheduled {
                    time: at,
                    relay: i,
                    kind: ScheduledKind::Open(target),
                });
            }
            if let Some(bf) = &r.spec.breaker_failure {
                self.schedule.push(Scheduled {
                    time: t + bf.delay,
                    relay: i,
                    kind: ScheduledKind::FailureCheck,
                });
            }
            if !r.ar_pending && r.spec.autoreclose.is_some() && r.reclosed_at.is_some() {
                self.report.operations.push(Operation {
                    time: t,
                    relay: r.spec.id.clone(),
                    function: "79".into(),
                    kind: OperationKind::Lockout,
                    detail: "trip within the reclaim time or shots used up".into(),
                });
            }
        }
        self.relay_deadline.set(Some(earliest));
    }

    /// Called by the engine once the breakers of a relay's line opened, to
    /// schedule its reclose.
    // @block Schedule the reclose
    fn schedule_reclose(&mut self) {
        self.deadline.set(None);
        for (i, r) in self.relays.iter_mut().enumerate() {
            if let (true, Some(t_open), Some((e, end)), Some(ar)) = (
                r.ar_pending,
                r.opened_at,
                r.site.line,
                r.spec.autoreclose.clone(),
            ) {
                r.ar_pending = false;
                r.shots_used += 1;
                // Leader and follower (IEEE C37.104): the from end recloses
                // onto the dead line; the to end follows 0.1 s later under
                // the synchronism check, waiting up to 1 s for the angle.
                let at = t_open + ar.dead_time + if end == End::To { 0.1 } else { 0.0 };
                self.schedule.push(Scheduled {
                    time: at,
                    relay: i,
                    kind: ScheduledKind::Close(e, end, at + 1.0),
                });
            }
        }
    }

    /// The MOV of each series capacitor bank: it conducts (the bank bypassed)
    /// while the bank current exceeds the protective level, and the bank is
    /// back once the current has stayed below its rating for `reinsert`. The
    /// engine applies these at once, before the relays measure: a MOV acts
    /// within the cycle a fault starts.
    // @block Series capacitor MOV
    pub fn bank_actions(&mut self, m: &dyn Measure) -> Vec<Action> {
        let t = m.time();
        let mut actions = Vec::new();
        let mut ops = Vec::new();
        for b in &mut self.banks {
            let i = m.branch_current(b.e, End::From)[0].norm();
            if b.bypassed {
                if i < b.rated {
                    let since = *b.below_since.get_or_insert(t);
                    if t - since >= b.reinsert {
                        b.bypassed = false;
                        b.below_since = None;
                        actions.push(Action::Reinsert(b.e));
                        ops.push((
                            b.id.clone(),
                            OperationKind::Reinserted,
                            format!("{:.2} × rated", i / b.rated),
                        ));
                    }
                } else {
                    b.below_since = None;
                }
            } else if i > b.level {
                b.bypassed = true;
                b.below_since = None;
                actions.push(Action::Bypass(b.e));
                ops.push((
                    b.id.clone(),
                    OperationKind::Bypassed,
                    format!("MOV conducts at {:.2} × rated", i / b.rated),
                ));
            }
        }
        for (id, kind, detail) in ops {
            self.log(t, &id, "SC", kind, detail);
        }
        actions
    }

    /// After the engine applied `actions`.
    pub fn applied(&mut self) {
        self.deadline.set(None);
        self.schedule_reclose();
    }

    /// Apparent impedance channels of the distance relays (primary ohms) for
    /// plotting R–X trajectories: `z_r:{relay}`, `z_x:{relay}`.
    pub fn channels(&mut self, m: &dyn Measure) {
        let relays = &self.relays;
        for (k, re, im) in &mut self.traces {
            let z = relays[*k]
                .apparent_impedance(m)
                .unwrap_or(C64::new(f64::NAN, f64::NAN));
            re.push(z.re);
            im.push(z.im);
        }
    }

    /// Move the recorded relay channels into `out`, keyed `z_r:<relay>` and
    /// `z_x:<relay>`.
    pub fn take_channels(&mut self, out: &mut BTreeMap<String, Vec<f64>>) {
        for (k, re, im) in self.traces.drain(..) {
            let id = &self.relays[k].spec.id;
            out.insert(format!("z_r:{id}"), re);
            out.insert(format!("z_x:{id}"), im);
        }
    }

    fn log(&mut self, time: f64, relay: &str, function: &str, kind: OperationKind, detail: String) {
        self.deadline.set(None);
        self.report.operations.push(Operation {
            time,
            relay: relay.to_owned(),
            function: function.to_owned(),
            kind,
            detail,
        });
    }

    /// What each relay measures now (current through it, its bus voltage,
    /// the apparent impedance of distance relays); `ka(bus, i)` converts a
    /// current to kA at a bus.
    // @block What each relay sees
    pub fn seen(&self, m: &dyn Measure, ka: impl Fn(usize, C64) -> f64) -> Vec<statics::RelaySeen> {
        self.relays
            .iter()
            .map(|r| {
                let i = r.site.current.map_or(C64::new(0.0, 0.0), |s| {
                    let c = match s {
                        CtSource::Branch(e, end) => m.branch_current(e, end),
                        CtSource::Unit(u) => m.unit_current(u),
                        CtSource::Feeder(b) => m.feeder_current(b),
                    };
                    sequences(c)[1]
                });
                statics::RelaySeen {
                    relay: r.spec.id.clone(),
                    current_ka: ka(r.site.bus, i),
                    voltage_pu: sequences(m.bus_voltage(r.site.bus))[1].norm(),
                    impedance: r.apparent_impedance(m).map(|z| [z.re, z.im]),
                }
            })
            .collect()
    }

    /// The record so far.
    #[must_use]
    pub fn report(&self) -> &ProtectionReport {
        &self.report
    }

    #[must_use]
    pub fn into_report(self) -> ProtectionReport {
        self.report
    }
}

fn describe_action(a: &Action) -> String {
    match a {
        Action::OpenEnd(e, end) => format!("branch #{e} {end:?} end opened"),
        Action::CloseEnd(e, end) => format!("branch #{e} {end:?} end closed"),
        Action::TripUnit(u) => format!("unit #{u} tripped"),
        Action::Bypass(e) => format!("series capacitor #{e} bypassed"),
        Action::Reinsert(e) => format!("series capacitor #{e} reinserted"),
    }
}

// @block Resolving a relay
// From a relay's location in bus numbers to its site in the topology: which
// voltage and current it measures, which currents make up its differential
// zone, which breakers it and its breaker-failure function trip, and its
// rated current. A function the location cannot measure is an error.

/// Resolve a relay's location.
fn resolve(r: &Relay, topo: &Topology) -> Result<Site, String> {
    let one = C64::new(1.0, 0.0);
    let ends_at = |b: usize| -> Vec<(usize, End)> {
        topo.branches
            .iter()
            .enumerate()
            .flat_map(|(e, br)| {
                let mut v = Vec::new();
                if br.from == b {
                    v.push((e, End::From));
                }
                if br.to == b {
                    v.push((e, End::To));
                }
                v
            })
            .collect()
    };
    let units_at = |b: usize| -> Vec<usize> {
        topo.units
            .iter()
            .enumerate()
            .filter(|(_, u)| u.bus == b)
            .map(|(k, _)| k)
            .collect()
    };
    let rated = |e: usize| -> f64 {
        let rate = topo.branches[e].rate;
        if rate > 0.0 && rate < 1e6 {
            rate
        } else {
            1.0
        }
    };
    // All breakers at a bus except `skip`: the local busbar trip of 50BF.
    let local = |b: usize, skip: Option<Target>| -> Vec<Target> {
        let mut v: Vec<Target> = ends_at(b)
            .into_iter()
            .map(|(e, end)| Target::End(e, end))
            .collect();
        v.extend(units_at(b).into_iter().map(Target::Unit));
        v.retain(|t| Some(*t) != skip);
        v
    };
    let site = match &r.location {
        // @block Line end
        // A line relay measures its own end's current and bus voltage and
        // trips that end; the zone of a line differential is both ends, with
        // the line charging compensated; 50BF trips the rest of its bus.
        RelayLocation::Line {
            from,
            to,
            circuit,
            at,
        } => {
            let e = topo.branch(*from, *to, circuit)?;
            let br = &topo.branches[e];
            let end = if br.from_number == *at {
                End::From
            } else if br.to_number == *at {
                End::To
            } else {
                return Err(format!("bus {at} is not an end of {from}-{to}"));
            };
            let bus = if end == End::From { br.from } else { br.to };
            let other = if end == End::From { End::To } else { End::From };
            Site {
                bus,
                current: Some(CtSource::Branch(e, end)),
                zone: vec![
                    Ct {
                        source: CtSource::Branch(e, end),
                        factor: one,
                    },
                    Ct {
                        source: CtSource::Branch(e, other),
                        factor: one,
                    },
                ],
                charging: Some((e, br.y_fr / (br.tap.norm_sqr()), br.y_to)),
                trips: vec![Target::End(e, end)],
                zone_trips: Vec::new(),
                backup: local(bus, Some(Target::End(e, end))),
                line: Some((e, end)),
                i_nom: r.i_nom.unwrap_or_else(|| rated(e)),
                scale: 1.0,
                z_base: topo.z_base(bus),
            }
        }
        // @block Transformer
        // A transformer relay measures at the from end and trips both ends;
        // its differential zone refers the from-side current through the
        // tap ratio.
        RelayLocation::Transformer { from, to, circuit } => {
            let e = topo.branch(*from, *to, circuit)?;
            let br = &topo.branches[e];
            // Measured at the branch's from end (HV in PSS/E data).
            Site {
                bus: br.from,
                current: Some(CtSource::Branch(e, End::From)),
                zone: vec![
                    Ct {
                        source: CtSource::Branch(e, End::From),
                        factor: br.tap.conj(),
                    },
                    Ct {
                        source: CtSource::Branch(e, End::To),
                        factor: one,
                    },
                ],
                charging: None,
                trips: vec![Target::End(e, End::From), Target::End(e, End::To)],
                zone_trips: Vec::new(),
                backup: local(br.from, Some(Target::End(e, End::From))),
                line: None,
                i_nom: r.i_nom.unwrap_or_else(|| rated(e)),
                scale: 1.0,
                z_base: topo.z_base(br.from),
            }
        }
        // @block Busbar
        // A busbar zone sums every branch end, unit and load at the bus and
        // trips all their breakers; it measures no single current, and its
        // rated current is the largest feeder rating.
        RelayLocation::Busbar { bus } => {
            let b = topo.bus(*bus)?;
            let mut zone: Vec<Ct> = ends_at(b)
                .into_iter()
                .map(|(e, end)| Ct {
                    source: CtSource::Branch(e, end),
                    factor: -one,
                })
                .collect();
            zone.extend(units_at(b).into_iter().map(|u| Ct {
                source: CtSource::Unit(u),
                factor: one,
            }));
            zone.push(Ct {
                source: CtSource::Feeder(b),
                factor: -one,
            });
            let i_nom = r.i_nom.unwrap_or_else(|| {
                ends_at(b)
                    .iter()
                    .map(|(e, _)| rated(*e))
                    .fold(0.0, f64::max)
                    .max(1.0)
            });
            Site {
                bus: b,
                current: None,
                zone,
                charging: None,
                trips: local(b, None),
                zone_trips: Vec::new(),
                backup: Vec::new(),
                line: None,
                i_nom,
                scale: 1.0,
                z_base: topo.z_base(b),
            }
        }
        // @block Generating unit
        // A unit relay measures the unit's current (its generator functions
        // work on the machine base) and trips the unit; a unit alone on its step-up transformer gets a
        // unit differential zone across the transformer, which trips its
        // ends too.
        RelayLocation::Generator { bus, machine_id }
        | RelayLocation::Inverter { bus, machine_id } => {
            let u = topo.unit(*bus, machine_id)?;
            let unit = &topo.units[u];
            let b = unit.bus;
            let scale = unit.mbase / topo.sbase;
            // Unit differential: generator, terminal bus and step-up
            // transformer when the unit's bus has only that branch.
            let mut zone = vec![Ct {
                source: CtSource::Unit(u),
                factor: one,
            }];
            let mut zone_trips = Vec::new();
            let branches = ends_at(b);
            if branches.len() == 1 && topo.branches[branches[0].0].transformer {
                let (e, end) = branches[0];
                let br = &topo.branches[e];
                let (hv_end, factor) = if end == End::From {
                    (End::To, one / br.tap.conj())
                } else {
                    (End::From, br.tap.conj())
                };
                zone.push(Ct {
                    source: CtSource::Branch(e, hv_end),
                    factor,
                });
                zone.push(Ct {
                    source: CtSource::Feeder(b),
                    factor: -one,
                });
                zone_trips.push(Target::End(e, hv_end));
                zone_trips.push(Target::End(e, end));
            }
            Site {
                bus: b,
                current: Some(CtSource::Unit(u)),
                zone,
                charging: None,
                trips: vec![Target::Unit(u)],
                zone_trips,
                backup: local(b, Some(Target::Unit(u))),
                line: None,
                i_nom: r.i_nom.unwrap_or(scale),
                scale,
                z_base: topo.z_base(b),
            }
        }
    };
    // @block Check the functions
    // A function that needs a current cannot sit on a busbar relay, and a
    // unit differential needs the zone across a step-up transformer.
    for f in &r.functions {
        let needs_current = !matches!(
            f,
            ProtectionFunction::Undervoltage(_)
                | ProtectionFunction::Overvoltage(_)
                | ProtectionFunction::NeutralOvervoltage(_)
                | ProtectionFunction::Underfrequency(_)
                | ProtectionFunction::Overfrequency(_)
                | ProtectionFunction::Rocof(_)
                | ProtectionFunction::Overexcitation(_)
                | ProtectionFunction::FaultRideThrough(_)
                | ProtectionFunction::Differential(_)
        );
        if needs_current && site.current.is_none() {
            return Err(format!(
                "{} needs a current, which a busbar relay does not measure",
                f.label()
            ));
        }
        if matches!(f, ProtectionFunction::Differential(_))
            && matches!(
                r.location,
                RelayLocation::Generator { .. } | RelayLocation::Inverter { .. }
            )
            && site.zone.len() < 2
        {
            return Err(
                "unit differential needs a unit whose bus has only its step-up transformer".into(),
            );
        }
    }
    Ok(site)
}
// @end

/// Relays per task of an evaluation on the thread pool: enough work per task
/// to amortize its scheduling, small enough to balance the threads.
const RELAY_CHUNK: usize = 64;

