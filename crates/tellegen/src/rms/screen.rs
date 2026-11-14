//! Screening of a finished run: the centre-of-inertia (COI) frequency and
//! rotor angles, bus frequencies estimated from the bus angles, and a
//! verdict against stated, versioned criteria.
//!
//! The criteria are tellegen's own screening defaults
//! ([`ScreeningCriteria::default`], version 1), not a planning standard
//! (NERC TPL-001, WECC): no protection, under-frequency load shedding or
//! motor loads are modelled. Every number is computed from the run's own
//! samples, never from a decimated display copy.
//!
//! The COI of an island is the inertia-weighted mean of its synchronous
//! machines in service, each weighted by `H · MBASE` (MW·s); a machine leaves
//! the COI from its trip on, and each island formed by the run's events is
//! screened on its own COI. An island holding a voltage source (an infinite
//! bus, `ZSORCE`) is referred to the source: its COI turns at the synchronous
//! speed with the source's angle.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{apply_event, topology, unit_key, RmsEvent, RmsRequest, RmsResult, RmsSimulation};

/// The screening criteria. Every field has a default (`version` 1); a
/// request may override any of them.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScreeningCriteria {
    /// The version of the defaults, `1`.
    pub version: u32,
    /// A machine whose rotor angle relative to its island's COI passes
    /// this many degrees has slipped a pole.
    pub pole_slip_deg: f64,
    /// The span at the end of the run over which the run must have settled, s.
    pub settle_window_s: f64,
    /// Settled: the COI frequency stays within this of its final value over
    /// the window, Hz.
    pub settle_frequency_hz: f64,
    /// Settled: the largest rotor angle separation stays within this of its
    /// final value over the window, degrees.
    pub settle_angle_deg: f64,
    /// Every live bus must be at or above this voltage, pu ...
    pub voltage_recovery_pu: f64,
    /// ... within this long after the last event, s.
    pub voltage_recovery_s: f64,
    /// The sliding window of the rate of change of the COI frequency, s.
    pub rocof_window_s: f64,
    /// An island whose COI speed ends further than this from 1 pu runs
    /// away, pu.
    pub runaway_pu: f64,
    /// The time constant of the filter on the bus angle derivative of the
    /// bus frequency estimate, s (the PSS/E `FRQ` convention).
    pub bus_frequency_filter_s: f64,
    /// Below this voltage a bus angle is not used for its frequency (the
    /// estimate holds its last value), pu.
    pub bus_frequency_min_pu: f64,
}

impl Default for ScreeningCriteria {
    fn default() -> Self {
        Self {
            version: 1,
            pole_slip_deg: 180.0,
            settle_window_s: 2.0,
            settle_frequency_hz: 0.01,
            settle_angle_deg: 1.0,
            voltage_recovery_pu: 0.8,
            voltage_recovery_s: 1.0,
            rocof_window_s: 0.5,
            runaway_pu: 0.05,
            bus_frequency_filter_s: 0.02,
            bus_frequency_min_pu: 0.2,
        }
    }
}

impl ScreeningCriteria {
    /// The name of these criteria, `tellegen-rms-screening-<version>`.
    #[must_use]
    pub fn name(&self) -> String {
        format!("tellegen-rms-screening-{}", self.version)
    }

    fn check(&self) -> Result<(), String> {
        let positive = [
            ("pole_slip_deg", self.pole_slip_deg),
            ("settle_window_s", self.settle_window_s),
            ("settle_frequency_hz", self.settle_frequency_hz),
            ("settle_angle_deg", self.settle_angle_deg),
            ("voltage_recovery_s", self.voltage_recovery_s),
            ("rocof_window_s", self.rocof_window_s),
            ("runaway_pu", self.runaway_pu),
            ("bus_frequency_filter_s", self.bus_frequency_filter_s),
        ];
        for (name, v) in positive {
            if !(v > 0.0 && v.is_finite()) {
                return Err(format!(
                    "screening criterion {name} = {v} must be finite and > 0"
                ));
            }
        }
        for (name, v) in [
            ("voltage_recovery_pu", self.voltage_recovery_pu),
            ("bus_frequency_min_pu", self.bus_frequency_min_pu),
        ] {
            if !(v >= 0.0 && v.is_finite()) {
                return Err(format!(
                    "screening criterion {name} = {v} must be finite and >= 0"
                ));
            }
        }
        Ok(())
    }
}

/// The outcome of a screening.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScreeningVerdict {
    /// No pole slip, no runaway island, settled by the end of the run.
    Stable,
    /// A machine slipped a pole.
    LossOfSynchronism,
    /// An island's COI speed ran away.
    IslandRunaway,
    /// Not settled by the end of the run: extend it.
    NotSettled,
}

/// The trend of the rotor angle separation after the last event, read from
/// the peaks of its distance to its final value. Only peaks above
/// [`TREND_FLOOR_DEG`] and above [`TREND_FLOOR_FRACTION`] of the largest
/// distance count: smaller wiggles are numerical, not swings.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SwingTrend {
    /// The last three counted peaks each shrink by more than 2 %.
    Decaying,
    /// The last three counted peaks each grow by more than 2 %.
    Growing,
    /// The swings fell below the floor, and within the settling criterion's
    /// angle band, before the end of the run (or never rose above them).
    DampedOut,
    /// Three or more counted peaks, the last one still counted, with no
    /// monotone trend in the last three: the swings neither grow nor decay.
    Sustained,
    /// Too few counted peaks to tell (such as one large swing cut off by
    /// the end of the run), or a last peak below the floor but outside the
    /// settling band.
    Undetermined,
}

/// The absolute floor of a counted swing peak, degrees.
pub const TREND_FLOOR_DEG: f64 = 0.5;
/// The relative floor of a counted swing peak: this fraction of the largest
/// distance of the separation from its final value after the last event.
pub const TREND_FLOOR_FRACTION: f64 = 0.1;

/// One reason for the verdict: a stable code and a plain sentence.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Reason {
    pub code: String,
    pub text: String,
}

/// A machine that slipped a pole: the first time its angle relative to its
/// island's COI passed the criterion.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PoleSlip {
    pub unit: String,
    pub t: f64,
}

/// Frequency figures, Hz (the COI of the island with the most inertia).
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FrequencyScreen {
    pub nominal_hz: f64,
    pub coi_nadir_hz: f64,
    pub nadir_t: f64,
    pub coi_peak_hz: f64,
    pub peak_t: f64,
    /// Largest `|Δf_COI| / window` over the sliding window, Hz/s.
    pub max_rocof_hz_s: f64,
    pub rocof_t: f64,
    pub final_hz: f64,
    pub settled: bool,
    /// The lowest estimated bus frequency and where.
    pub lowest_bus_hz: f64,
    pub lowest_bus: Option<usize>,
    pub lowest_bus_t: f64,
    /// The lowest unit speed (in service), Hz, and which unit.
    pub lowest_unit_hz: f64,
    pub lowest_unit: Option<String>,
    pub lowest_unit_t: f64,
}

/// Rotor angle figures, degrees.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AngleScreen {
    /// The largest `max δ − min δ` among the machines in service of one
    /// island from the first event on (over the whole run without events),
    /// the pair and when.
    pub max_separation_deg: f64,
    pub pair: Vec<String>,
    pub separation_t: f64,
    /// The separation before the first event (at the start without
    /// events): the steady-state spread the event starts from.
    #[serde(default)]
    pub initial_separation_deg: f64,
    /// The largest `|δ − δ_COI|` over the run, which unit and when.
    pub max_coi_angle_deg: f64,
    pub max_coi_angle_unit: Option<String>,
    pub max_coi_angle_t: f64,
    /// The largest first swing: per machine the largest distance of
    /// `δ − δ_COI` from its value before the first event, up to its first
    /// extremum after the last event. Compare `max_coi_angle_deg`, not
    /// this, with the pole-slip criterion.
    pub first_swing_deg: f64,
    pub first_swing_unit: Option<String>,
    pub pole_slips: Vec<PoleSlip>,
    pub trend: Option<SwingTrend>,
    pub settled: bool,
}

/// Bus voltage figures, pu.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct VoltageScreen {
    pub min_pu: f64,
    pub min_bus: Option<usize>,
    pub min_t: f64,
    /// The time of the last event, s (0 without events).
    pub last_event_t: f64,
    /// How long after the last event every live bus is back at or above
    /// the criterion for good, s; `None`: not by the end of the run.
    pub recovered_s: Option<f64>,
    pub recovered: bool,
    pub final_min_pu: f64,
    pub final_max_pu: f64,
}

/// One island with a unit in service at the end of the run.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct IslandScreen {
    pub buses: usize,
    pub machines: usize,
    /// `Σ H · MBASE` of its machines in service, MW·s.
    pub inertia_mws: f64,
    /// Its COI frequency at the end (`None` without a synchronous machine).
    pub final_hz: Option<f64>,
    pub runaway: bool,
}

/// The screening of a run.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RmsScreening {
    /// The criteria's name, `tellegen-rms-screening-1`.
    pub criteria_name: String,
    pub criteria: ScreeningCriteria,
    pub verdict: ScreeningVerdict,
    pub reasons: Vec<Reason>,
    pub frequency: FrequencyScreen,
    pub angle: AngleScreen,
    pub voltage: VoltageScreen,
    pub islands: Vec<IslandScreen>,
    /// Units held at their power flow output (not simulated).
    pub held_units: usize,
    /// The unit whose speed turns the reference frame, if any.
    pub reference_unit: Option<String>,
}

/// A topology between two event times.
struct Segment {
    /// Applies to samples after this time.
    start: f64,
    /// Islands with a synchronous machine or a source: the dense buses,
    /// the machines (device indices) in service, a source's angle.
    islands: Vec<SegIsland>,
    /// Per device, its island in `islands` (none: tripped or no island).
    island_of_device: Vec<Option<usize>>,
    /// Per dense bus, whether it is dead.
    dead: Vec<bool>,
    /// The island with the most inertia (a source's island first).
    main: Option<usize>,
}

struct SegIsland {
    buses: Vec<usize>,
    machines: Vec<usize>,
    weight: f64,
    source_angle: Option<f64>,
    live_units: usize,
}

/// The inertia-weighted mean of `values` (`Σ w v / Σ w`), `None` when the
/// weights sum to zero.
pub(crate) fn coi(weights: &[f64], values: &[f64]) -> Option<f64> {
    let total: f64 = weights.iter().sum();
    if !(total > 0.0) {
        return None;
    }
    Some(weights.iter().zip(values).map(|(w, v)| w * v).sum::<f64>() / total)
}

/// Wrap an angle difference to `(−π, π]`.
fn wrap(a: f64) -> f64 {
    let tau = std::f64::consts::TAU;
    let r = a.rem_euclid(tau);
    if r > std::f64::consts::PI {
        r - tau
    } else {
        r
    }
}

/// The frequency of one bus, Hz, estimated from its angle `va` (rad, in
/// the frame turning at `omega_ref` pu): `f0 (ω_ref + (1/ω0) dva/dt)` with
/// the derivative through a first-order filter of time constant `tf`. A
/// sample flagged in `hold` (an event between it and the one before, or a
/// voltage too low for an angle) keeps the filter's last value. The filter
/// starts at rest (the run starts in steady state). `NaN` where `dead`.
pub(crate) fn bus_frequency(
    time: &[f64],
    va: &[f64],
    omega_ref: &[f64],
    f0: f64,
    tf: f64,
    hold: &dyn Fn(usize) -> bool,
) -> Vec<f64> {
    let tau = std::f64::consts::TAU;
    let mut out = Vec::with_capacity(time.len());
    let mut y = 0.0; // filtered dva/dt, rad/s
    for k in 0..time.len() {
        if k > 0 && !hold(k) {
            let dt = time[k] - time[k - 1];
            if dt > 0.0 {
                let d = wrap(va[k] - va[k - 1]) / dt;
                // Backward Euler on y' = (d − y) / tf: exact for a ramp once settled.
                y += dt / (tf + dt) * (d - y);
            }
        }
        out.push(f0 * omega_ref[k] + y / tau);
    }
    out
}

/// The local maxima of `v` above `floor`, as `(index, value)`.
fn peaks(v: &[f64], floor: f64) -> Vec<(usize, f64)> {
    (1..v.len().saturating_sub(1))
        .filter(|&k| v[k] >= v[k - 1] && v[k] > v[k + 1] && v[k] > floor)
        .map(|k| (k, v[k]))
        .collect()
}

/// The trend of `dev`, the distance of the separation from its final value
/// after the last event, degrees (see [`SwingTrend`]). `settle_deg` is the
/// settling criterion's angle band: a swing larger than it has not damped out.
fn swing_trend(dev: &[f64], settle_deg: f64) -> SwingTrend {
    let largest = dev.iter().copied().fold(0.0_f64, f64::max);
    let floor = TREND_FLOOR_DEG.max(TREND_FLOOR_FRACTION * largest);
    let all = peaks(dev, 0.0);
    let counted: Vec<f64> = all.iter().filter(|p| p.1 > floor).map(|p| p.1).collect();
    if counted.len() >= 3 {
        let a = &counted[counted.len() - 3..];
        if a[1] < 0.98 * a[0] && a[2] < 0.98 * a[1] {
            return SwingTrend::Decaying;
        }
        if a[1] > 1.02 * a[0] && a[2] > 1.02 * a[1] {
            return SwingTrend::Growing;
        }
    }
    // Damped out: the last peak is at or below the floor and within the
    // settling band (or there is no peak at all: `dev` ends at 0 by
    // construction).
    let last = all.last().map_or(0.0, |p| p.1);
    if last <= floor && last <= settle_deg {
        return SwingTrend::DampedOut;
    }
    if last > floor && counted.len() >= 3 {
        return SwingTrend::Sustained;
    }
    SwingTrend::Undetermined
}

/// Everything a screening and the derived series read.
struct Analysis {
    screening: RmsScreening,
    series: BTreeMap<String, Vec<f64>>,
}

impl RmsSimulation {
    /// The topologies of a run: one per distinct event time, built by
    /// applying the request's events in order to a copy of the system.
    fn segments(&self, events: &[RmsEvent], tol: f64) -> Result<Vec<Segment>, String> {
        let mut order: Vec<&RmsEvent> = events.iter().collect();
        order.sort_by(|a, b| a.time().total_cmp(&b.time()));
        let mut sys = self.sys.clone();
        let mut x = self.x0.clone();
        let mut out = vec![self.segment(&sys, f64::NEG_INFINITY)];
        let mut k = 0;
        while k < order.len() {
            let t = order[k].time();
            while k < order.len() && order[k].time() <= t + tol {
                apply_event(&mut sys, &mut x, order[k])
                    .map_err(|e| format!("event at t = {} s: {e}", order[k].time()))?;
                k += 1;
            }
            topology::extinguish(&mut sys, t);
            out.push(self.segment(&sys, t));
        }
        Ok(out)
    }

    fn segment(&self, sys: &super::system::DynSystem, start: f64) -> Segment {
        let dead = sys.dead_buses();
        let mut island_of_bus = vec![usize::MAX; sys.n];
        let mut islands = Vec::new();
        for buses in sys.islands() {
            if buses.iter().all(|&b| dead[b]) {
                continue;
            }
            let k = islands.len();
            for &b in &buses {
                island_of_bus[b] = k;
            }
            let source_angle = sys
                .sources
                .iter()
                .find(|s| s.y != super::system::C64::new(0.0, 0.0) && island_of_bus[s.bus] == k)
                .map(|s| s.e.arg());
            islands.push(SegIsland {
                buses,
                machines: Vec::new(),
                weight: 0.0,
                source_angle,
                live_units: 0,
            });
        }
        let mut island_of_device = vec![None; sys.devices.len()];
        for (j, d) in sys.devices.iter().enumerate() {
            let k = island_of_bus[d.bus];
            if d.tripped.is_some() || k == usize::MAX {
                continue;
            }
            islands[k].live_units += 1;
            let w = d.shaft.h * d.scale * sys.sbase;
            if w > 0.0 {
                islands[k].machines.push(j);
                islands[k].weight += w;
                island_of_device[j] = Some(k);
            }
        }
        for h in sys.held.iter().filter(|h| !h.tripped) {
            if let Some(isl) = islands.get_mut(island_of_bus[h.bus]) {
                isl.live_units += 1;
            }
        }
        let main = (0..islands.len())
            .filter(|&k| islands[k].source_angle.is_some() || islands[k].weight > 0.0)
            .max_by(|&a, &b| {
                let key = |k: usize| (islands[k].source_angle.is_some(), islands[k].weight);
                let (sa, wa) = key(a);
                let (sb, wb) = key(b);
                sa.cmp(&sb).then(wa.total_cmp(&wb))
            });
        Segment {
            start,
            islands,
            island_of_device,
            dead,
            main,
        }
    }

    /// Screen a finished run of this simulation against `criteria`.
    ///
    /// # Errors
    /// When the run does not record the channels a screening needs
    /// (`omega:`, `delta:`, `vm:`, `va:`), naming the missing ones, or an
    /// event names an element the system lacks.
    pub fn screen(
        &self,
        request: &RmsRequest,
        result: &RmsResult,
        criteria: &ScreeningCriteria,
    ) -> Result<RmsScreening, String> {
        Ok(self.analyse(request, result, criteria)?.screening)
    }

    /// The series a screening derives from a run: `coi:omega` (pu),
    /// `coi:delta` (rad, in the frame of `delta:`), `coi:f` (Hz) of the
    /// island with the most inertia, and `f:<bus>`, every bus's frequency
    /// estimated from its angle (Hz, `NaN` while the bus is dead).
    ///
    /// # Errors
    /// As [`Self::screen`].
    pub fn derived_series(
        &self,
        request: &RmsRequest,
        result: &RmsResult,
        criteria: &ScreeningCriteria,
    ) -> Result<BTreeMap<String, Vec<f64>>, String> {
        Ok(self.analyse(request, result, criteria)?.series)
    }

    /// [`Self::screen`] and [`Self::derived_series`] from one pass.
    ///
    /// # Errors
    /// As [`Self::screen`].
    pub fn screen_with_series(
        &self,
        request: &RmsRequest,
        result: &RmsResult,
        criteria: &ScreeningCriteria,
    ) -> Result<(RmsScreening, BTreeMap<String, Vec<f64>>), String> {
        let a = self.analyse(request, result, criteria)?;
        Ok((a.screening, a.series))
    }

    #[allow(clippy::too_many_lines)]
    fn analyse(
        &self,
        request: &RmsRequest,
        result: &RmsResult,
        criteria: &ScreeningCriteria,
    ) -> Result<Analysis, String> {
        criteria.check()?;
        let time = &result.time;
        let ns = time.len();
        if ns == 0 {
            return Err("the run recorded no samples".into());
        }
        let sys = &self.sys;
        let f0 = sys.f0;
        let deg = 180.0 / std::f64::consts::PI;
        // The channels, every one checked before any is read.
        let keys: Vec<String> = sys
            .devices
            .iter()
            .map(|d| unit_key(d.bus_number, &d.machine_id))
            .collect();
        let mut missing: Vec<&str> = Vec::new();
        let mut get = |prefix: &'static str, key: &str| -> Option<&Vec<f64>> {
            let ch = result.channels.get(&format!("{prefix}{key}"));
            if ch.is_none_or(|c| c.len() != ns) && !missing.contains(&prefix) {
                missing.push(prefix);
            }
            ch
        };
        let omega: Vec<Option<&Vec<f64>>> = keys.iter().map(|k| get("omega:", k)).collect();
        let delta: Vec<Option<&Vec<f64>>> = keys.iter().map(|k| get("delta:", k)).collect();
        let bus_keys: Vec<String> = sys.bus_numbers.iter().map(ToString::to_string).collect();
        let vm: Vec<Option<&Vec<f64>>> = bus_keys.iter().map(|k| get("vm:", k)).collect();
        let va: Vec<Option<&Vec<f64>>> = bus_keys.iter().map(|k| get("va:", k)).collect();
        if !missing.is_empty() {
            return Err(format!(
                "screening needs the channels {} recorded for every unit and bus (RecordSpec::display records them)",
                missing.join(", ")
            ));
        }
        let omega: Vec<&Vec<f64>> = omega.into_iter().map(Option::unwrap).collect();
        let delta: Vec<&Vec<f64>> = delta.into_iter().map(Option::unwrap).collect();
        let vm: Vec<&Vec<f64>> = vm.into_iter().map(Option::unwrap).collect();
        let va: Vec<&Vec<f64>> = va.into_iter().map(Option::unwrap).collect();

        let tol = 1e-9 * request.dt.max(1e-9);
        let segments = self.segments(&request.events, tol)?;
        // The segment of each sample: the last that starts before it (a
        // sample at an event's time is the state before the event).
        let seg_of: Vec<usize> = time
            .iter()
            .map(|&t| {
                segments
                    .iter()
                    .rposition(|s| s.start < t - tol)
                    .unwrap_or(0)
            })
            .collect();
        let mut event_times: Vec<f64> = request.events.iter().map(RmsEvent::time).collect();
        event_times.sort_by(f64::total_cmp);
        let last_event = event_times.last().copied().unwrap_or(0.0);

        // The reference frame speed.
        let omega_ref: Vec<f64> = match sys.omega_ref {
            Some(r) => {
                let j = sys
                    .devices
                    .iter()
                    .position(|d| {
                        let [_, s, ..] = d.layout();
                        d.offset + s + 1 == r
                    })
                    .ok_or("the reference frame's speed is not a machine's")?;
                omega[j].clone()
            }
            None => vec![1.0; ns],
        };
        let weight = |j: usize| sys.devices[j].shaft.h * sys.devices[j].scale * sys.sbase;

        // COI per sample and island.
        let island_coi = |s: usize, isl: &SegIsland| -> Option<(f64, f64)> {
            if let Some(a) = isl.source_angle {
                if sys.omega_ref.is_none() {
                    return Some((1.0, a));
                }
            }
            let w: Vec<f64> = isl.machines.iter().map(|&j| weight(j)).collect();
            let o: Vec<f64> = isl.machines.iter().map(|&j| omega[j][s]).collect();
            let d: Vec<f64> = isl.machines.iter().map(|&j| delta[j][s]).collect();
            Some((coi(&w, &o)?, coi(&w, &d)?))
        };
        let mut coi_omega = vec![f64::NAN; ns];
        let mut coi_delta = vec![f64::NAN; ns];
        // Per sample, per island of its segment, its COI.
        let mut cois: Vec<Vec<Option<(f64, f64)>>> = Vec::with_capacity(ns);
        for s in 0..ns {
            let seg = &segments[seg_of[s]];
            let c: Vec<Option<(f64, f64)>> =
                seg.islands.iter().map(|isl| island_coi(s, isl)).collect();
            if let Some((o, d)) = seg.main.and_then(|m| c[m]) {
                coi_omega[s] = o;
                coi_delta[s] = d;
            }
            cois.push(c);
        }
        let coi_f: Vec<f64> = coi_omega.iter().map(|o| f0 * o).collect();

        // Frequency: nadir, peak, RoCoF, final, lowest unit.
        let mut fr = FrequencyScreen {
            nominal_hz: f0,
            coi_nadir_hz: f64::INFINITY,
            coi_peak_hz: f64::NEG_INFINITY,
            lowest_bus_hz: f64::INFINITY,
            lowest_unit_hz: f64::INFINITY,
            ..FrequencyScreen::default()
        };
        for s in 0..ns {
            let f = coi_f[s];
            if f < fr.coi_nadir_hz {
                fr.coi_nadir_hz = f;
                fr.nadir_t = time[s];
            }
            if f > fr.coi_peak_hz {
                fr.coi_peak_hz = f;
                fr.peak_t = time[s];
            }
            let seg = &segments[seg_of[s]];
            for (j, isl) in seg.island_of_device.iter().enumerate() {
                if isl.is_some() && f0 * omega[j][s] < fr.lowest_unit_hz {
                    fr.lowest_unit_hz = f0 * omega[j][s];
                    fr.lowest_unit = Some(keys[j].clone());
                    fr.lowest_unit_t = time[s];
                }
            }
        }
        let mut j = 0;
        for k in 0..ns {
            while j + 1 < k && time[k] - time[j + 1] >= criteria.rocof_window_s - tol {
                j += 1;
            }
            let span = time[k] - time[j];
            if span >= criteria.rocof_window_s - tol {
                let r = (coi_f[k] - coi_f[j]).abs() / span;
                if r > fr.max_rocof_hz_s {
                    fr.max_rocof_hz_s = r;
                    fr.rocof_t = time[k];
                }
            }
        }
        fr.final_hz = coi_f[ns - 1];

        // Bus frequencies.
        let mut series: BTreeMap<String, Vec<f64>> = BTreeMap::new();
        for (b, number) in sys.bus_numbers.iter().enumerate() {
            let hold = |k: usize| {
                let (t0, t1) = (time[k - 1], time[k]);
                vm[b][k].min(vm[b][k - 1]) < criteria.bus_frequency_min_pu
                    || segments[seg_of[k]].dead[b]
                    || event_times
                        .iter()
                        .any(|&te| te >= t0 - tol && te < t1 - tol)
            };
            let mut f = bus_frequency(
                time,
                va[b],
                &omega_ref,
                f0,
                criteria.bus_frequency_filter_s,
                &hold,
            );
            for s in 0..ns {
                if segments[seg_of[s]].dead[b] {
                    f[s] = f64::NAN;
                } else if f[s] < fr.lowest_bus_hz && vm[b][s] >= criteria.bus_frequency_min_pu {
                    fr.lowest_bus_hz = f[s];
                    fr.lowest_bus = Some(*number);
                    fr.lowest_bus_t = time[s];
                }
            }
            series.insert(format!("f:{number}"), f);
        }
        if !fr.lowest_bus_hz.is_finite() {
            fr.lowest_bus_hz = f64::NAN;
        }
        if !fr.lowest_unit_hz.is_finite() {
            fr.lowest_unit_hz = f64::NAN;
        }

        // Angles: separation, COI angle, pole slips. `s_pre`: the last
        // sample before the first event (the sample at its time).
        let first_event = event_times.first().copied();
        let s_pre = first_event.map_or(0, |t0| {
            time.partition_point(|&t| t <= t0 + tol)
                .saturating_sub(1)
                .min(ns - 1)
        });
        let mut an = AngleScreen::default();
        let mut sep = vec![0.0_f64; ns];
        let mut slipped: Vec<Option<f64>> = vec![None; sys.devices.len()];
        for s in 0..ns {
            let seg = &segments[seg_of[s]];
            for (k, isl) in seg.islands.iter().enumerate() {
                let Some((_, dc)) = cois[s][k] else { continue };
                let mut lo: Option<(f64, usize)> = None;
                let mut hi: Option<(f64, usize)> = None;
                for &j in &isl.machines {
                    let d = delta[j][s];
                    if lo.is_none_or(|(v, _)| d < v) {
                        lo = Some((d, j));
                    }
                    if hi.is_none_or(|(v, _)| d > v) {
                        hi = Some((d, j));
                    }
                    let rel = (d - dc) * deg;
                    if rel.abs() > an.max_coi_angle_deg {
                        an.max_coi_angle_deg = rel.abs();
                        an.max_coi_angle_unit = Some(keys[j].clone());
                        an.max_coi_angle_t = time[s];
                    }
                    if slipped[j].is_none() && rel.abs() > criteria.pole_slip_deg {
                        slipped[j] = Some(time[s]);
                    }
                }
                if let (Some((l, jl)), Some((h, jh))) = (lo, hi) {
                    let v = (h - l) * deg;
                    sep[s] = sep[s].max(v);
                    if s >= s_pre && v > an.max_separation_deg {
                        an.max_separation_deg = v;
                        an.pair = vec![keys[jh].clone(), keys[jl].clone()];
                        an.separation_t = time[s];
                    }
                }
            }
        }
        an.pole_slips = slipped
            .iter()
            .enumerate()
            .filter_map(|(j, t)| {
                t.map(|t| PoleSlip {
                    unit: keys[j].clone(),
                    t,
                })
            })
            .collect();
        an.pole_slips.sort_by(|a, b| a.t.total_cmp(&b.t));
        an.initial_separation_deg = sep[s_pre];
        // First swing: from the value before the first event to the first
        // extremum after the last event.
        let s0 = time.partition_point(|&t| t <= last_event + tol).min(ns - 1);
        if !request.events.is_empty() {
            for (j, key) in keys.iter().enumerate() {
                let rel = |s: usize| -> Option<f64> {
                    let seg = &segments[seg_of[s]];
                    let k = seg.island_of_device[j]?;
                    let (_, dc) = cois[s][k]?;
                    Some((delta[j][s] - dc) * deg)
                };
                let Some(base) = rel(s_pre) else { continue };
                let mut swing = 0.0_f64;
                for s in s_pre..=s0 {
                    if let Some(v) = rel(s) {
                        swing = swing.max((v - base).abs());
                    }
                }
                let Some(start) = rel(s0) else { continue };
                let mut prev = start;
                let mut dir = 0.0_f64;
                for s in s0 + 1..ns {
                    let Some(v) = rel(s) else { break };
                    let d = v - prev;
                    if dir == 0.0 {
                        if d.abs() > 1e-9 {
                            dir = d.signum();
                        }
                    } else if d * dir < 0.0 {
                        break;
                    }
                    prev = v;
                    swing = swing.max((v - base).abs());
                }
                if swing > an.first_swing_deg {
                    an.first_swing_deg = swing;
                    an.first_swing_unit = Some(key.clone());
                }
            }
        }
        // Trend and settling.
        let final_sep = sep[ns - 1];
        let dev: Vec<f64> = sep[s0..].iter().map(|v| (v - final_sep).abs()).collect();
        an.trend = Some(swing_trend(&dev, criteria.settle_angle_deg));
        let t_end = time[ns - 1];
        let w0 = t_end - criteria.settle_window_s;
        let window_ok = w0 >= last_event - tol && w0 >= time[0] - tol;
        let in_window = |s: usize| time[s] >= w0 - tol;
        fr.settled = window_ok
            && (0..ns)
                .filter(|&s| in_window(s))
                .all(|s| (coi_f[s] - fr.final_hz).abs() <= criteria.settle_frequency_hz);
        an.settled = window_ok
            && (0..ns)
                .filter(|&s| in_window(s))
                .all(|s| (sep[s] - final_sep).abs() <= criteria.settle_angle_deg);

        // Voltages.
        let mut vo = VoltageScreen {
            min_pu: f64::INFINITY,
            last_event_t: last_event,
            final_min_pu: f64::INFINITY,
            final_max_pu: f64::NEG_INFINITY,
            ..VoltageScreen::default()
        };
        let mut last_low: Option<f64> = None;
        for s in 0..ns {
            let seg = &segments[seg_of[s]];
            for (b, number) in sys.bus_numbers.iter().enumerate() {
                if seg.dead[b] {
                    continue;
                }
                let v = vm[b][s];
                if v < vo.min_pu {
                    vo.min_pu = v;
                    vo.min_bus = Some(*number);
                    vo.min_t = time[s];
                }
                if v < criteria.voltage_recovery_pu {
                    last_low = Some(time[s]);
                }
                if s == ns - 1 {
                    vo.final_min_pu = vo.final_min_pu.min(v);
                    vo.final_max_pu = vo.final_max_pu.max(v);
                }
            }
        }
        vo.recovered_s = match last_low {
            None => Some(0.0),
            Some(t) if t < t_end - tol => {
                // Back at the next sample after the last low one.
                let s = time.partition_point(|&x| x <= t + tol);
                Some((time[s] - last_event).max(0.0))
            }
            Some(_) => None,
        };
        vo.recovered = vo
            .recovered_s
            .is_some_and(|r| r <= criteria.voltage_recovery_s + tol);

        // Islands at the end.
        let last_seg = &segments[seg_of[ns - 1]];
        let mut islands = Vec::new();
        for (k, isl) in last_seg.islands.iter().enumerate() {
            if isl.live_units == 0 && isl.source_angle.is_none() {
                continue;
            }
            let final_omega = cois[ns - 1][k].map(|(o, _)| o);
            islands.push(IslandScreen {
                buses: isl.buses.len(),
                machines: isl.machines.len(),
                inertia_mws: isl.weight,
                final_hz: final_omega.map(|o| o * f0),
                runaway: final_omega.is_some_and(|o| (o - 1.0).abs() > criteria.runaway_pu),
            });
        }

        // The verdict and its reasons.
        let mut reasons = Vec::new();
        let reason = |code: &str, text: String| Reason {
            code: code.into(),
            text,
        };
        let runaway = islands.iter().find(|i| i.runaway);
        let runaway_text = |i: &IslandScreen| {
            format!(
                "an island of {} buses ends at {:.3} Hz, more than {} pu from nominal",
                i.buses,
                i.final_hz.unwrap_or(f64::NAN),
                criteria.runaway_pu
            )
        };
        let verdict = if let Some(p) = an.pole_slips.first() {
            reasons.push(reason(
                "pole_slip",
                format!(
                    "unit {} passed {:.0}° from its centre of inertia at t = {:.3} s ({} unit(s) in all)",
                    p.unit,
                    criteria.pole_slip_deg,
                    p.t,
                    an.pole_slips.len()
                ),
            ));
            // A runaway island besides is a reason too.
            if let Some(i) = runaway {
                reasons.push(reason("island_runaway", runaway_text(i)));
            }
            ScreeningVerdict::LossOfSynchronism
        } else if let Some(i) = runaway {
            reasons.push(reason("island_runaway", runaway_text(i)));
            ScreeningVerdict::IslandRunaway
        } else if !(fr.settled && an.settled) {
            reasons.push(reason(
                "not_settled",
                if window_ok {
                    format!(
                        "not settled over the last {} s (COI frequency within {} Hz: {}, angle separation within {}°: {}); extend the run",
                        criteria.settle_window_s,
                        criteria.settle_frequency_hz,
                        if fr.settled { "yes" } else { "no" },
                        criteria.settle_angle_deg,
                        if an.settled { "yes" } else { "no" }
                    )
                } else {
                    format!(
                        "the run ends less than {} s after the last event; extend the run",
                        criteria.settle_window_s
                    )
                },
            ));
            ScreeningVerdict::NotSettled
        } else {
            reasons.push(reason("no_pole_slip", "no pole slip".into()));
            reasons.push(reason(
                "settled",
                format!(
                    "settled over the last {} s at {:.3} Hz",
                    criteria.settle_window_s, fr.final_hz
                ),
            ));
            ScreeningVerdict::Stable
        };
        if !vo.recovered {
            reasons.push(reason(
                "voltage_not_recovered",
                match vo.recovered_s {
                    Some(r) => format!(
                        "every bus back above {} pu only {:.3} s after the last event (criterion {} s)",
                        criteria.voltage_recovery_pu, r, criteria.voltage_recovery_s
                    ),
                    None => format!(
                        "a bus is below {} pu at the end of the run",
                        criteria.voltage_recovery_pu
                    ),
                },
            ));
        }
        if islands.len() > 1 {
            reasons.push(reason(
                "islanded",
                format!(
                    "the network ends in {} islands, each screened on its own COI",
                    islands.len()
                ),
            ));
        }
        series.insert("coi:omega".into(), coi_omega);
        series.insert("coi:delta".into(), coi_delta);
        series.insert("coi:f".into(), coi_f);
        Ok(Analysis {
            screening: RmsScreening {
                criteria_name: criteria.name(),
                criteria: criteria.clone(),
                verdict,
                reasons,
                frequency: fr,
                angle: an,
                voltage: vo,
                islands,
                held_units: self.held_units().len(),
                reference_unit: self.reference_unit(),
            },
            series,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coi_weights_are_the_given_weights() {
        // H · MBASE: 5 s · 900 MVA and 3 s · 100 MVA.
        let w = [5.0 * 900.0, 3.0 * 100.0];
        let c = coi(&w, &[1.01, 0.99]).unwrap();
        assert!((c - (4500.0 * 1.01 + 300.0 * 0.99) / 4800.0).abs() < 1e-15);
        assert_eq!(coi(&[0.0, 0.0], &[1.0, 1.0]), None);
    }

    #[test]
    fn a_bus_angle_ramp_gives_its_frequency() {
        // 60 Hz system, the frame at 1.0 pu, the bus 0.05 Hz below it.
        let f0 = 60.0;
        let df = -0.05;
        let time: Vec<f64> = (0..=200).map(|k| f64::from(k) * 0.01).collect();
        let va: Vec<f64> = time
            .iter()
            .map(|t| wrap(std::f64::consts::TAU * df * t + 3.0))
            .collect();
        let f = bus_frequency(&time, &va, &vec![1.0; time.len()], f0, 0.02, &|_| false);
        assert!(
            (f[time.len() - 1] - (f0 + df)).abs() < 1e-6,
            "{}",
            f[time.len() - 1]
        );
        // In a frame turning at 1.001 pu the same angle ramp is 60.06 − 0.05 Hz.
        let f = bus_frequency(&time, &va, &vec![1.001; time.len()], f0, 0.02, &|_| false);
        assert!((f[time.len() - 1] - (f0 * 1.001 + df)).abs() < 1e-6);
    }

    #[test]
    fn an_angle_jump_at_an_event_is_masked() {
        let time: Vec<f64> = (0..=100).map(|k| f64::from(k) * 0.01).collect();
        // A 0.3 rad jump between samples 50 and 51 (the event at t = 0.5 s).
        let va: Vec<f64> = (0..=100).map(|k| if k > 50 { 0.3 } else { 0.0 }).collect();
        let omega = vec![1.0; time.len()];
        let masked = bus_frequency(&time, &va, &omega, 50.0, 0.02, &|k| k == 51);
        assert!(masked.iter().all(|f| (f - 50.0).abs() < 1e-12));
        let unmasked = bus_frequency(&time, &va, &omega, 50.0, 0.02, &|_| false);
        assert!(unmasked.iter().any(|f| (f - 50.0).abs() > 0.5));
    }

    #[test]
    fn numerical_wiggles_do_not_make_a_trend() {
        // A 0.73° first excursion, then peaks of 0.061°, 0.083° and 0.097°
        // (fault 1032 through j0.05 on WECC 240): damped out, not growing.
        let mut dev = vec![0.0, 0.73, 0.2, 0.0];
        for p in [0.061, 0.083, 0.097] {
            dev.extend([p, 0.0]);
        }
        assert_eq!(swing_trend(&dev, 1.0), SwingTrend::DampedOut);
        // Real swings: 10°, 8°, 6.4° → decaying; 4°, 5°, 6° → growing.
        assert_eq!(
            swing_trend(&[0.0, 10.0, 0.0, 8.0, 0.0, 6.4, 0.0, 2.0, 1.0], 1.0),
            SwingTrend::Decaying
        );
        assert_eq!(
            swing_trend(&[0.0, 4.0, 0.0, 5.0, 0.0, 6.0, 0.0], 1.0),
            SwingTrend::Growing
        );
        // One big swing, cut off by the end of the run: undetermined.
        assert_eq!(
            swing_trend(&[0.0, 30.0, 60.0, 40.0, 0.0], 1.0),
            SwingTrend::Undetermined
        );
        // Peaks of 5°, 6°, 5°, 6°: sustained, neither growing nor decaying.
        assert_eq!(
            swing_trend(&[0.0, 5.0, 0.0, 6.0, 0.0, 5.0, 0.0, 6.0, 0.0], 1.0),
            SwingTrend::Sustained
        );
    }

    #[test]
    fn a_swing_outside_the_settling_band_has_not_damped_out() {
        // A 60° first swing puts the floor at 6°; a last peak of 3° is below
        // it but three times the 1° settling band: not damped out.
        let dev = [0.0, 60.0, 0.0, 3.0, 0.0];
        assert_eq!(swing_trend(&dev, 1.0), SwingTrend::Undetermined);
        // The same peak within a 5° band has damped out.
        assert_eq!(swing_trend(&dev, 5.0), SwingTrend::DampedOut);
        // A last peak of 0.8° within the 1° band has too.
        assert_eq!(
            swing_trend(&[0.0, 60.0, 0.0, 0.8, 0.0], 1.0),
            SwingTrend::DampedOut
        );
    }

    #[test]
    fn peaks_are_local_maxima() {
        let v = [0.0, 1.0, 0.5, 2.0, 2.0, 1.0, 0.01, 0.02, 0.0];
        assert_eq!(peaks(&v, 0.05), vec![(1, 1.0), (4, 2.0)]);
    }
}
