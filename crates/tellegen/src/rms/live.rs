//! The experimental RMS product layer around a prepared simulation: live
//! sessions that advance on demand and take events as they run, replay from
//! checkpoints, results packed as flat buffers, and small-signal analysis of
//! a stored module.
//!
//! @block Live sessions

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::system::{self, DynSystem, C64};
use super::*;

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

/// Pack a result or chunk as JSON metadata plus flat `f64` buffers, so the
/// browser reads the samples without parsing them.
// @block Flat buffers
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
// @block Pack results as buffers
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
    let sim = RmsSimulation::with_models(
        &network,
        &dynamics,
        &models,
        request.load_model.unwrap_or_default(),
    )?;
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
    /// The relays' operations since the previous chunk.
    #[cfg(feature = "protection")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relays: Vec<crate::protection::Operation>,
}

/// A running simulation that advances on demand and takes events at its
/// current time: the engine of the live view.
// @block Live session
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
    /// The COI, separation and bus frequencies at each sample, as a batch
    /// run's screening derives them.
    derive: screen::LiveDerive,
    /// Relays acting on the session ([`RmsLive::set_protection`]) and how
    /// many of their operations the chunks have carried.
    #[cfg(feature = "protection")]
    relays: Option<crate::protection::ProtectionRuntime>,
    #[cfg(feature = "protection")]
    relays_sent: usize,
}

impl RmsSimulation {
    /// Start a live session at the initial operating point.
    // @block Start a live session
    #[must_use]
    pub fn live(&self, dt: f64, output_step: f64) -> RmsLive {
        let mut f = vec![0.0; self.sys.nx];
        self.sys.residual(&self.x0, &mut f, None);
        // A step below the engine's smallest (`MIN_DT`) is raised to it.
        let dt = if dt.is_finite() {
            dt.max(MIN_DT)
        } else {
            MIN_DT
        };
        let output_step = output_step.max(dt);
        let substeps = substeps(output_step, dt);
        let mut rec = Recorder::new(&self.sys, None);
        let mut derive = screen::LiveDerive::new(&self.sys);
        rec.with_derived(derive.keys().to_vec());
        rec.set_derived(derive.sample(&self.sys, 0.0, &self.x0));
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
            derive,
            #[cfg(feature = "protection")]
            relays: None,
            #[cfg(feature = "protection")]
            relays_sent: 0,
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
    /// When `dt` is not a finite number of at least 1e-6 s.
    pub fn set_step(&mut self, dt: f64) -> Result<(), String> {
        if !(dt.is_finite() && dt >= MIN_DT) {
            return Err(format!("the step must be at least {MIN_DT} s, not {dt}"));
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
    // @block Take an event while running
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
        // A series capacitor's MOV acts at once, before the relays measure.
        #[cfg(feature = "protection")]
        if let Some(r) = &mut self.relays {
            let mut notes = Vec::new();
            super::protect::settle_banks(
                r,
                &mut sys,
                &mut x,
                &mut f,
                &mut self.stepper,
                now,
                &mut notes,
            )?;
            for note in notes {
                text.push('\n');
                text.push_str(&note);
            }
        }
        self.sys = sys;
        self.x = x;
        self.f = f;
        self.derive.topology_changed(&self.sys, now);
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
    // @block Synchronize a unit
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
            let (local, _) = init::init_device(d, u, C64::new(0.0, 0.0))?;
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
                sys.loads[b].u_p -= s.re;
                sys.loads[b].u_q -= s.im;
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
        self.derive.topology_changed(&self.sys, self.t);
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
            let mut scaled = self
                .base_loads
                .get(b)
                .ok_or_else(|| format!("no bus index {b}"))?
                .clone();
            scaled.scale(p, q);
            // The held units' currents are not load: they stay as they are.
            let l = &mut sys.loads[b];
            (scaled.u_p, scaled.u_q) = (l.u_p, l.u_q);
            *l = scaled;
            self.load_scales[b] = (p, q);
        }
        self.resolve_network(sys)
    }

    // @block Read the present state
    // What the live view shows between steps: loads, unit powers, the
    // frequency, unit speeds, references, breakers, faults and taps.

    /// Operating point load per bus (dense), system per unit at `v0`.
    #[must_use]
    pub fn base_loads(&self) -> Vec<C64> {
        self.base_loads
            .iter()
            .map(system::BusLoad::power_at_v0)
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
            .map(|(b, l)| l.power(C64::new(self.x[b], self.x[n + b]), self.sys.low_voltage))
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

    // @block Change setpoints
    // Operator actions on a running session (unit and voltage references,
    // breakers' state, taps) with the reads that go with them; a change
    // re-solves the network.

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
        self.sys.closed.get(e).is_some_and(|&c| c)
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
        self.sys.ac.branches.tap.get(e).copied().unwrap_or(1.0)
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
        // The PowerIO tap column; the re-solve stamps the matrix from it.
        sys.ac.branches.tap[e] = tap;
        self.resolve_network(sys)
    }

    /// Integrate `duration` seconds and return the samples recorded on the
    /// way.
    ///
    /// # Errors
    /// When the integration fails; the session stays at the last good point.
    // @block Advance
    pub fn advance(&mut self, duration: f64) -> Result<RmsChunk, String> {
        self.integrate(duration)?;
        let (time, channels, frames) = self.rec.take();
        Ok(RmsChunk {
            time,
            channels,
            events: std::mem::take(&mut self.applied),
            t: self.t,
            frames,
            #[cfg(feature = "protection")]
            relays: self.new_relay_operations(),
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
        #[cfg(feature = "protection")]
        {
            meta["relays"] = serde_json::json!(self.new_relay_operations());
        }
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
    // @block Integrate a stretch
    fn integrate(&mut self, duration: f64) -> Result<(), String> {
        let end = self.t + duration.max(0.0);
        let mut euler = 0usize;
        while self.t < end - 1e-12 {
            // Breakers and reclosers due now.
            #[cfg(feature = "protection")]
            self.relay_actions()?;
            #[cfg_attr(not(feature = "protection"), allow(unused_mut))]
            let mut h = self
                .h
                .min(self.output_step - self.since_output)
                .max(1e-9)
                .min(end - self.t);
            #[cfg(feature = "protection")]
            if let Some(d) = self.relays.as_ref().and_then(|r| r.next_deadline()) {
                if d > self.t + 1e-9 {
                    h = h.min(d - self.t);
                }
            }
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
            #[cfg(feature = "protection")]
            if let Some(r) = &mut self.relays {
                let t = self.t;
                super::protect::settle_banks(
                    r,
                    &mut self.sys,
                    &mut self.x,
                    &mut self.f,
                    &mut self.stepper,
                    t,
                    &mut self.applied,
                )?;
                r.evaluate(&super::protect::RelayMeasure::new(&self.sys, &self.x, t));
            }
            if self.since_output >= self.output_step - 1e-12 {
                self.since_output = 0.0;
                self.t = (self.t / self.output_step).round() * self.output_step;
                self.rec
                    .set_derived(self.derive.sample(&self.sys, self.t, &self.x));
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

#[cfg(feature = "protection")]
impl RmsLive {
    /// Let relays act from now on (`None` switches them off). Their
    /// operations come with the chunks.
    ///
    /// # Errors
    /// When a relay names a missing element.
    pub fn set_protection(
        &mut self,
        scheme: Option<&crate::protection::ProtectionScheme>,
    ) -> Result<(), String> {
        self.relays = match scheme {
            Some(s) if !s.relays.is_empty() || !s.series_capacitors.is_empty() => {
                let mut r = crate::protection::ProtectionRuntime::bind(s, &self.topology, true)?;
                r.evaluate(&super::protect::RelayMeasure::new(
                    &self.sys, &self.x, self.t,
                ));
                Some(r)
            }
            _ => None,
        };
        self.relays_sent = 0;
        Ok(())
    }

    /// The relay operations since the last call.
    fn new_relay_operations(&mut self) -> Vec<crate::protection::Operation> {
        match &self.relays {
            Some(r) => {
                let ops = &r.report().operations;
                let new = ops[self.relays_sent.min(ops.len())..].to_vec();
                self.relays_sent = ops.len();
                new
            }
            None => Vec::new(),
        }
    }

    /// Apply the breaker and recloser actions due now and re-solve.
    fn relay_actions(&mut self) -> Result<(), String> {
        let t = self.t;
        let Some(r) = &mut self.relays else {
            return Ok(());
        };
        if !r.next_deadline().is_some_and(|d| d <= t + 1e-9) {
            return Ok(());
        }
        let actions = r.take_due(t, &super::protect::RelayMeasure::new(&self.sys, &self.x, t));
        r.applied();
        if actions.is_empty() {
            return Ok(());
        }
        for a in &actions {
            self.applied
                .push(super::protect::apply_action(&mut self.sys, &self.x, a, t));
        }
        self.applied.extend(topology::extinguish(&mut self.sys, t));
        self.sys.rebuild_ybus();
        self.sys.rebuild_mass();
        let mut f = vec![0.0; self.sys.nx];
        self.sys.residual(&self.x, &mut f, None);
        self.stepper
            .reinit(&self.sys, &mut self.x, &mut f)
            .map_err(|e| format!("re-solving the network at t = {t:.4} s: {e}"))?;
        self.f = f;
        super::protect::settle_banks(
            r,
            &mut self.sys,
            &mut self.x,
            &mut self.f,
            &mut self.stepper,
            t,
            &mut self.applied,
        )?;
        Ok(())
    }
}
