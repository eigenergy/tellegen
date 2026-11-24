//! The measuring elements and their timers.
//!
//! Each relay keeps one element per protection function. At every
//! evaluation the relay measures: it takes its voltage and current phasors,
//! keeps a memory voltage for polarization and a frequency estimate, and
//! updates each element's pickup and timer (or its integral, for the
//! inverse-time, thermal and negative-sequence functions). It then decides:
//! the first function whose time has run out trips the relay.
//!
//! @block Measuring elements

use std::collections::VecDeque;
use std::f64::consts::PI;

use serde::{Deserialize, Serialize};

use super::{
    op_a, sequences, Action, Characteristic, CtSource, Direction, Distance, End, Measure,
    Operation, OperationKind, ProtectionFunction, Relay, Site, Zone, C64,
};

// @block Inverse-time curves
// The time–current curves of the overcurrent stages: the IEC and IEEE
// constants, the operating time at a multiple of the setting (flat above 20
// times), and the timed reset of the IEEE curves.

/// Time–current curve of an overcurrent stage: IEC 60255-151:2009 Annex A,
/// `t = TMS (k / (G^α − 1) + c)`, with the IEEE curves of IEEE C37.112,
/// `t = TD (A / (G^p − 1) + B)` and reset `TD tr / (1 − G²)`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Curve {
    #[default]
    Definite,
    /// IEC standard inverse: k 0.14, α 0.02.
    IecStandardInverse,
    /// IEC very inverse: 13.5, 1.
    IecVeryInverse,
    /// IEC extremely inverse: 80, 2.
    IecExtremelyInverse,
    /// Long-time inverse: 120, 1 (the older IEC 60255-3 curve; not one of
    /// IEC 60255-151's six).
    IecLongTimeInverse,
    /// IEEE moderately inverse: A 0.0515, p 0.02, B 0.114.
    IeeeModeratelyInverse,
    /// IEEE very inverse: 19.61, 2, 0.491.
    IeeeVeryInverse,
    /// IEEE extremely inverse: 28.2, 2, 0.1217.
    IeeeExtremelyInverse,
}

impl Curve {
    /// `(k, α, c, reset tr)`; `tr = 0` resets at once (the IEC curves).
    fn constants(self) -> Option<(f64, f64, f64, f64)> {
        match self {
            Self::Definite => None,
            Self::IecStandardInverse => Some((0.14, 0.02, 0.0, 0.0)),
            Self::IecVeryInverse => Some((13.5, 1.0, 0.0, 0.0)),
            Self::IecExtremelyInverse => Some((80.0, 2.0, 0.0, 0.0)),
            Self::IecLongTimeInverse => Some((120.0, 1.0, 0.0, 0.0)),
            Self::IeeeModeratelyInverse => Some((0.0515, 0.02, 0.114, 4.85)),
            Self::IeeeVeryInverse => Some((19.61, 2.0, 0.491, 21.6)),
            Self::IeeeExtremelyInverse => Some((28.2, 2.0, 0.1217, 29.1)),
        }
    }
}

/// Operating time of an inverse curve at `g` times the setting (`None` for
/// a definite-time stage or below the setting). Above 20 times the setting
/// the time stays at its value there (IEC 60255-151's `G_D`, at least 20
/// times the setting).
#[must_use]
pub fn curve_time(curve: Curve, tms: f64, g: f64) -> Option<f64> {
    let (k, alpha, c, _) = curve.constants()?;
    if g <= 1.0 {
        return None;
    }
    let g = g.min(20.0);
    Some(tms * (k / (g.powf(alpha) - 1.0) + c))
}

/// Reset time of an IEEE curve below the setting (`None` for an instant
/// reset).
fn reset_time(curve: Curve, tms: f64, g: f64) -> Option<f64> {
    let (_, _, _, tr) = curve.constants()?;
    (tr > 0.0 && g < 1.0).then(|| tms * tr / (1.0 - g * g))
}

// @block Timers
// A pickup timer: it starts when its condition picks up, resets when it
// drops, logs both (the first twenty per function, as relays evaluate every
// sample), and says when its delay has run out.

/// A pickup with its definite-time delay.
#[derive(Clone, Debug, Default)]
struct Timer {
    since: Option<f64>,
    logged: usize,
}

impl Timer {
    /// Update with this evaluation's pickup; logs transitions (`label` and
    /// `value` are formatted only for a log entry: relays evaluate every
    /// sample).
    fn update(
        &mut self,
        picked: bool,
        t: f64,
        ops: &mut Vec<Operation>,
        relay: &str,
        label: impl std::fmt::Display,
        value: impl FnOnce() -> String,
    ) {
        match (picked, self.since) {
            (true, None) => {
                self.since = Some(t);
                if self.logged < 20 {
                    self.logged += 1;
                    ops.push(op(
                        t,
                        relay,
                        &label.to_string(),
                        OperationKind::Pickup,
                        value(),
                    ));
                }
            }
            (false, Some(_)) => {
                self.since = None;
                if self.logged < 20 {
                    self.logged += 1;
                    ops.push(op(
                        t,
                        relay,
                        &label.to_string(),
                        OperationKind::Reset,
                        String::new(),
                    ));
                }
            }
            _ => {}
        }
    }

    fn expired(&self, t: f64, delay: f64) -> bool {
        self.since.is_some_and(|s| t - s >= delay - 1e-9)
    }

    fn deadline(&self, delay: f64) -> Option<f64> {
        self.since.map(|s| s + delay)
    }
}

fn op(time: f64, relay: &str, function: &str, kind: OperationKind, detail: String) -> Operation {
    Operation {
        time,
        relay: relay.to_owned(),
        function: function.to_owned(),
        kind,
        detail,
    }
}

// @block Element state
// What each function remembers between evaluations: its timers, the
// integral of an inverse-time or negative-sequence function, the thermal
// state, the side of the out-of-step blinders, the ride-through clock, and
// for distance the power swing tracker; per relay, the frequency estimate
// and its history.

/// Region of the out-of-step blinders.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Side {
    Left,
    Middle,
    Right,
    Outside,
}

/// The rate (largest forward reaches per second, across the line angle) above
/// which an impedance inside the outer power swing boundary is moving as a
/// swing does; a
/// fault's apparent impedance drifts far slower (this module's choice: on
/// WECC 240 a remote fault seen through changing infeed drifts at up to about
/// 0.33 in the first tens of milliseconds, a swing after a fault is cleared
/// moves at about 1).
const SWING_RATE: f64 = 0.5;

/// The state of one function.
#[derive(Clone, Debug)]
enum Element {
    /// Definite-time elements (and the differential, voltage, frequency, ...).
    Timer {
        timer: Timer,
        delay: f64,
    },
    /// An inverse-time overcurrent: the integral of `dt / t(G)`.
    Inverse {
        timer: Timer,
        integral: f64,
        eta: Option<f64>,
    },
    Distance {
        zones: Vec<Timer>,
        scheme: Timer,
        swing: Swing,
    },
    LossOfField {
        z1: Timer,
        z2: Timer,
    },
    OutOfStep {
        side: Side,
        entered: Option<(Side, f64)>,
        slips: usize,
        /// Experimental: minimum of the current dip and its time, the
        /// voltage-time integral and duration of the oscillation, and the time
        /// the voltage exceeded its average after the last beat.
        beat: [f64; 4],
        beat_ready: Option<f64>,
    },
    Integral {
        acc: f64,
    },
    Thermal {
        theta: Option<f64>,
    },
    Frt {
        t0: Option<f64>,
        above_since: Option<f64>,
        timer: Timer,
        /// The voltage was at or above the start value (a healthy pre-fault
        /// state): only then does a fall below start the ride-through clock.
        armed: bool,
    },
}

#[derive(Clone, Debug, Default)]
struct Swing {
    outer_since: Option<f64>,
    blocked_since: Option<f64>,
    last: Option<C64>,
    /// `(t, z)` inside the outer boundary over the last `delta_t`.
    trail: VecDeque<(f64, C64)>,
    /// A fault entered the boundary with the voltage collapsing (and is
    /// still inside): its impedance moving is no swing.
    fault: bool,
    last_v: Option<f64>,
}

/// Frequency from the angle of the positive-sequence voltage.
#[derive(Clone, Debug, Default)]
struct FrequencyEstimate {
    last: Option<(f64, f64)>,
    f: Option<f64>,
    /// `(t, f)` for the rate of change of frequency, kept only by a relay
    /// with that function (which also trims it to its window).
    history: VecDeque<(f64, f64)>,
    keep_history: bool,
    /// The last time the voltage was below the RoCoF function's minimum.
    low_at: Option<f64>,
}

// @block Relay state
// One relay during a run: its settings and site, an element per function,
// the memory voltage, the teleprotection permission it sends and receives,
// and its trip, breaker and reclose state. The runtime asks it for its next
// deadline, has it measure and decide, and tells it when its breaker opened
// or reclosed.

/// A relay during a run.
#[derive(Clone, Debug)]
pub(crate) struct RelayState {
    pub spec: Relay,
    pub site: Site,
    elements: Vec<Element>,
    freq: FrequencyEstimate,
    /// A function reads the frequency (under-/overfrequency, rate of change,
    /// V/Hz): only then is it estimated.
    uses_frequency: bool,
    /// Memory voltage and when it was taken.
    vmem: Option<(C64, f64)>,
    pub partner: Option<usize>,
    /// The partner's `(overreach, underreach)` forward pickups.
    pub remote: Option<(bool, bool)>,
    pub overreach_fwd: bool,
    pub underreach_fwd: bool,
    pub trip_time: Option<f64>,
    latched: bool,
    pub ar_pending: bool,
    pub opened_at: Option<f64>,
    pub reclosed_at: Option<f64>,
    pub shots_used: usize,
    /// Results of this evaluation for `decide`.
    picks: Vec<Pick>,
    /// Each function's label, formatted once.
    labels: Vec<String>,
}

#[derive(Clone, Debug, Default)]
struct Pick {
    /// Function index → whether its trip condition holds now, with the
    /// label and whether a trip starts the auto-recloser.
    ready: Option<(String, bool, bool)>,
}

impl RelayState {
    /// A relay with one element per function, its timers reset, and a
    /// frequency history kept only when it carries a RoCoF function.
    // @block Elements from the settings
    pub(crate) fn new(spec: Relay, site: Site) -> Self {
        let elements =
            spec.functions
                .iter()
                .map(|f| match f {
                    ProtectionFunction::Overcurrent(o) | ProtectionFunction::EarthFault(o) => {
                        if o.curve == Curve::Definite {
                            Element::Timer {
                                timer: Timer::default(),
                                delay: o.delay,
                            }
                        } else {
                            Element::Inverse {
                                timer: Timer::default(),
                                integral: 0.0,
                                eta: None,
                            }
                        }
                    }
                    ProtectionFunction::Distance(d) => Element::Distance {
                        zones: vec![Timer::default(); d.zones.len()],
                        scheme: Timer::default(),
                        swing: Swing::default(),
                    },
                    ProtectionFunction::Differential(d) => Element::Timer {
                        timer: Timer::default(),
                        delay: d.delay + d.channel_time,
                    },
                    ProtectionFunction::Undervoltage(s)
                    | ProtectionFunction::Overvoltage(s)
                    | ProtectionFunction::NeutralOvervoltage(s)
                    | ProtectionFunction::Overexcitation(s) => Element::Timer {
                        timer: Timer::default(),
                        delay: s.delay,
                    },
                    ProtectionFunction::Underfrequency(s)
                    | ProtectionFunction::Overfrequency(s) => Element::Timer {
                        timer: Timer::default(),
                        delay: s.delay,
                    },
                    ProtectionFunction::Rocof(s) => Element::Timer {
                        timer: Timer::default(),
                        delay: s.delay,
                    },
                    ProtectionFunction::ReversePower(s) => Element::Timer {
                        timer: Timer::default(),
                        delay: s.delay,
                    },
                    ProtectionFunction::LossOfField(_) => Element::LossOfField {
                        z1: Timer::default(),
                        z2: Timer::default(),
                    },
                    ProtectionFunction::OutOfStep(_) => Element::OutOfStep {
                        side: Side::Outside,
                        entered: None,
                        slips: 0,
                        beat: [0.0; 4],
                        beat_ready: None,
                    },
                    ProtectionFunction::NegativeSequence(_) => Element::Integral { acc: 0.0 },
                    ProtectionFunction::Thermal(_) => Element::Thermal { theta: None },
                    ProtectionFunction::FaultRideThrough(_) => Element::Frt {
                        t0: None,
                        above_since: None,
                        timer: Timer::default(),
                        armed: false,
                    },
                })
                .collect();
        let n = spec.functions.len();
        let keep_history = spec
            .functions
            .iter()
            .any(|f| matches!(f, ProtectionFunction::Rocof(_)));
        let uses_frequency = spec.functions.iter().any(|f| {
            matches!(
                f,
                ProtectionFunction::Underfrequency(_)
                    | ProtectionFunction::Overfrequency(_)
                    | ProtectionFunction::Rocof(_)
                    | ProtectionFunction::Overexcitation(_)
            )
        });
        let labels = spec
            .functions
            .iter()
            .map(ProtectionFunction::label)
            .collect();
        Self {
            spec,
            site,
            elements,
            freq: FrequencyEstimate {
                keep_history,
                ..Default::default()
            },
            uses_frequency,
            vmem: None,
            partner: None,
            remote: None,
            overreach_fwd: false,
            underreach_fwd: false,
            trip_time: None,
            latched: false,
            ar_pending: false,
            opened_at: None,
            reclosed_at: None,
            shots_used: 0,
            picks: vec![Pick::default(); n],
            labels,
        }
    }

    fn ct(m: &dyn Measure, s: CtSource) -> [C64; 3] {
        match s {
            CtSource::Branch(e, end) => m.branch_current(e, end),
            CtSource::Unit(u) => m.unit_current(u),
            CtSource::Feeder(b) => m.feeder_current(b),
        }
    }

    fn current(&self, m: &dyn Measure) -> Option<[C64; 3]> {
        self.site.current.map(|s| Self::ct(m, s))
    }

    /// Whether current still flows through the relay's CT (50BF).
    pub(crate) fn current_flowing(&self, m: &dyn Measure) -> bool {
        let threshold = self
            .spec
            .breaker_failure
            .as_ref()
            .map_or(0.1, |b| b.current)
            * self.site.i_nom;
        match self.site.current {
            Some(CtSource::Unit(u)) => m.unit_in_service(u),
            Some(s) => Self::ct(m, s).iter().any(|i| i.norm() > threshold),
            None => false,
        }
    }

    pub(crate) fn breaker_opened(&mut self, t: f64, a: &Action) {
        if let (Some((e, end)), Action::OpenEnd(e2, end2)) = (self.site.line, a) {
            if e == *e2 && end == *end2 {
                self.opened_at = Some(t);
            }
        }
    }

    pub(crate) fn reclosed(&mut self, t: f64) {
        self.reclosed_at = Some(t);
        self.latched = false;
        self.trip_time = None;
        self.opened_at = None;
    }

    pub(crate) fn ar_available(&mut self, t: f64) -> bool {
        let Some(ar) = &self.spec.autoreclose else {
            return false;
        };
        if let Some(rt) = self.reclosed_at {
            if t - rt <= ar.reclaim {
                return false;
            }
            self.shots_used = 0;
        }
        self.shots_used < ar.shots
    }

    /// Apparent positive-sequence impedance in primary ohms, for distance
    /// relays carrying current.
    pub(crate) fn has_distance(&self) -> bool {
        self.spec
            .functions
            .iter()
            .any(|f| matches!(f, ProtectionFunction::Distance(_)))
    }

    pub(crate) fn apparent_impedance(&self, m: &dyn Measure) -> Option<C64> {
        if !self.has_distance() {
            return None;
        }
        let (i, v) = (self.current(m)?, m.bus_voltage(self.site.bus));
        let (i1, v1) = if m.unbalanced() {
            (sequences(i)[1], sequences(v)[1])
        } else {
            (i[0], v[0])
        };
        (i1.norm() > 0.05 * self.site.i_nom).then(|| v1 / i1 * self.site.z_base)
    }

    /// Whether an element trips the moment it decides (out-of-step,
    /// negative-sequence and thermal: no timer whose deadline the engine
    /// knows beforehand).
    pub(crate) fn decides_at_once(&self) -> bool {
        self.elements.iter().any(|el| {
            matches!(
                el,
                Element::OutOfStep { .. } | Element::Integral { .. } | Element::Thermal { .. }
            )
        })
    }

    /// An inverse-time element picked up or still integrating (its integral
    /// decays after a drop-off): its operate estimate can fall due at any
    /// moment of the next evaluations.
    pub(crate) fn inverse_running(&self) -> bool {
        self.elements.iter().any(|el| {
            matches!(el, Element::Inverse { integral, eta, .. } if *integral > 0.0 || eta.is_some())
        })
    }

    /// The shortest operate time of the relay's inverse-time elements (the
    /// curves stop falling at 20 times the setting).
    pub(crate) fn inverse_min_time(&self) -> Option<f64> {
        self.spec
            .functions
            .iter()
            .zip(&self.elements)
            .filter_map(|(f, el)| match (f, el) {
                (
                    ProtectionFunction::Overcurrent(o) | ProtectionFunction::EarthFault(o),
                    Element::Inverse { .. },
                ) => curve_time(o.curve, o.tms, 20.0),
                _ => None,
            })
            .min_by(f64::total_cmp)
    }

    /// Whether [`Self::decide`] or [`Self::deadline`] can act on this relay:
    /// not latched, and a function ready, a timer running, an inverse-time
    /// operate estimate, or an overreaching zone that teleprotection
    /// permission may complete. An idle relay gives `None` to both and
    /// keeps its state there, so the runtime skips it.
    pub(crate) fn busy(&self) -> bool {
        if self.latched {
            return false;
        }
        if self.overreach_fwd || self.picks.iter().any(|p| p.ready.is_some()) {
            return true;
        }
        self.elements.iter().any(|el| match el {
            Element::Timer { timer, .. } | Element::Frt { timer, .. } => timer.since.is_some(),
            Element::Inverse { timer, eta, .. } => timer.since.is_some() || eta.is_some(),
            Element::Distance { zones, scheme, .. } => {
                scheme.since.is_some() || zones.iter().any(|z| z.since.is_some())
            }
            Element::LossOfField { z1, z2 } => z1.since.is_some() || z2.since.is_some(),
            Element::OutOfStep { .. } | Element::Integral { .. } | Element::Thermal { .. } => false,
        })
    }

    /// Earliest instant a running timer expires.
    // @block Relay deadline
    pub(crate) fn deadline(&self, measuring: f64) -> Option<f64> {
        if self.latched {
            return None;
        }
        let mut best: Option<f64> = None;
        let mut take = |d: Option<f64>| {
            if let Some(d) = d {
                best = Some(best.map_or(d, |b: f64| b.min(d)));
            }
        };
        for (f, el) in self.spec.functions.iter().zip(&self.elements) {
            match el {
                Element::Timer { timer, delay } => take(timer.deadline(delay + measuring)),
                Element::Inverse { eta, .. } => take(*eta),
                Element::Distance { zones, scheme, .. } => {
                    if let ProtectionFunction::Distance(d) = f {
                        for (z, timer) in d.zones.iter().zip(zones) {
                            take(timer.deadline(z.delay + measuring));
                        }
                        if let Some(tp) = &d.teleprotection {
                            take(scheme.deadline(tp.channel_time + measuring));
                        }
                    }
                }
                Element::LossOfField { z1, z2 } => {
                    if let ProtectionFunction::LossOfField(l) = f {
                        take(z1.deadline(l.delay1 + measuring));
                        take(z2.deadline(l.delay2 + measuring));
                    }
                }
                Element::Frt { timer, .. } => take(timer.deadline(measuring)),
                _ => {}
            }
        }
        best
    }

    /// Measure and update the elements.
    // @block Measure
    pub(crate) fn measure(&mut self, m: &dyn Measure, dt: f64, f0: f64, ops: &mut Vec<Operation>) {
        let t = m.time();
        let id = self.spec.id.as_str();
        let v = m.bus_voltage(self.site.bus);
        // A balanced measurement is a positive-sequence phasor in phase a
        // (the other phases rotated): its components are read off directly,
        // and one phase stands for all three.
        let bal = !m.unbalanced();
        let np = if bal { 1 } else { 3 };
        let pos = |p: [C64; 3]| if bal { [C64::new(0.0, 0.0), p[0], C64::new(0.0, 0.0)] } else { sequences(p) };
        let seq_v = pos(v);
        let v1 = seq_v[1];
        // Its magnitude, read by several elements.
        let v1n = v1.norm();
        let i = self.current(m);
        let seq_i = i.map(pos);
        let i_nom = self.site.i_nom.max(1e-9);
        // @block Memory voltage
        // Positive-sequence memory voltage (polarization): the pre-fault
        // voltage is held for 100 ms after a collapse, then the relay
        // polarizes with the present voltage (blocked without one).
        let vmem = match self.vmem {
            Some((vm, since)) if v1n < 0.9 * vm.norm() => {
                if t - since <= 0.1 {
                    self.vmem = Some((vm, since));
                    vm
                } else {
                    v1
                }
            }
            _ => {
                self.vmem = Some((v1, t));
                v1
            }
        };
        // @block Frequency estimate
        // Frequency: the angle rate of V1 in the reference frame, filtered
        // over 40 ms (for the relays that read it). Below 5 % voltage the
        // last estimate is kept.
        let freq = if !self.uses_frequency {
            f0
        } else {
            let theta = v1.arg();
            let mut f_now = None;
            if let Some((t_prev, th_prev)) = self.freq.last {
                let h = t - t_prev;
                if h > 1e-6 {
                    let mut d = theta - th_prev;
                    while d > PI {
                        d -= 2.0 * PI;
                    }
                    while d < -PI {
                        d += 2.0 * PI;
                    }
                    let inst = f0 * m.frame_speed() + d / (2.0 * PI * h);
                    let prev = self.freq.f.unwrap_or(inst);
                    let a = (h / 0.04).min(1.0);
                    f_now = Some(prev + a * (inst - prev));
                }
            }
            if v1n < 0.05 {
                f_now = self.freq.f;
            }
            if let Some(f) = f_now.or(self.freq.f).or(Some(f0 * m.frame_speed())) {
                self.freq.f = Some(f);
                if self.freq.keep_history {
                    self.freq.history.push_back((t, f));
                }
            }
            self.freq.last = Some((t, theta));
            self.freq.f.unwrap_or(f0)
        };
        // @block Functions
        // Each function updates its element from the measurements; one that
        // trips without a timer (78, 46, 49) marks itself ready for the
        // decision.
        let mut over = false;
        let mut under = false;
        for (k, f) in self.spec.functions.iter().enumerate() {
            let label = &self.labels[k];
            let mut ready: Option<(String, bool, bool)> = None;
            let el = &mut self.elements[k];
            match (f, el) {
                // @block Overcurrent
                // 50/51/67 on the largest phase current, 50N/51N/67N on the
                // residual current (unbalanced engines only). The direction
                // compares the current with the polarizing voltage. A
                // definite-time stage times its pickup; an inverse stage
                // integrates `dt / t(G)` and trips at 1, resetting at once
                // (IEC) or along the reset curve (IEEE).
                (ProtectionFunction::Overcurrent(o) | ProtectionFunction::EarthFault(o), el) => {
                    let earth = matches!(f, ProtectionFunction::EarthFault(_));
                    let Some(ip) = i else { continue };
                    let mag = if earth {
                        if !m.unbalanced() {
                            continue;
                        }
                        (ip[0] + ip[1] + ip[2]).norm()
                    } else {
                        ip[..np].iter().map(|x| x.norm()).fold(0.0, f64::max)
                    };
                    let g = mag / (o.pickup * i_nom).max(1e-12);
                    let directed = match o.direction {
                        Direction::NonDirectional => true,
                        d => {
                            let s =
                                seq_i.map_or(
                                    C64::new(0.0, 0.0),
                                    |s| if earth { s[0] } else { s[1] },
                                );
                            let pol = if earth { -seq_v[0] } else { vmem };
                            let fwd = (s * rotate_60() * pol.conj())
                                .re
                                > 0.0;
                            (d == Direction::Forward) == fwd
                        }
                    };
                    match el {
                        Element::Timer { timer, delay } => {
                            let held = timer.since.is_some();
                            let picked = directed && (g >= 1.0 || (held && g >= 0.95));
                            timer.update(picked, t, ops, id, label, || format!("{:.2} × I>", g));
                            let _ = delay;
                        }
                        Element::Inverse {
                            timer,
                            integral,
                            eta,
                        } => {
                            let held = timer.since.is_some();
                            let picked = directed && (g >= 1.1 || (held && g >= 0.95));
                            timer.update(picked, t, ops, id, label, || format!("{:.2} × I>", g));
                            if picked {
                                if let Some(top) = curve_time(o.curve, o.tms, g) {
                                    *integral += dt / top;
                                    *eta = Some(t + (1.0 - *integral).max(0.0) * top);
                                }
                            } else {
                                *eta = None;
                                *integral = match reset_time(o.curve, o.tms, g) {
                                    Some(tr) => (*integral - dt / tr).max(0.0),
                                    None => 0.0,
                                };
                            }
                        }
                        _ => {}
                    }
                }
                // @block Distance
                // 21: the measuring loops are checked against every zone, the
                // power swing detection may block the zones, and each zone's
                // timer runs on what is left. The forward Z1 and overreaching
                // zone pickups are the permission teleprotection sends.
                (
                    ProtectionFunction::Distance(d),
                    Element::Distance {
                        zones,
                        scheme,
                        swing,
                    },
                ) => {
                    let Some(ip) = i else { continue };
                    // Balanced: the positive sequence the relay computed.
                    let seq1 = bal.then(|| (v1, seq_i.map_or(C64::new(0.0, 0.0), |s| s[1])));
                    let res = distance(d, &v, &ip, seq1, vmem, &self.site, i_nom);
                    // @block Power swing blocking
                    // Power swing detection on the positive-sequence impedance.
                    // 68 watches two boundaries concentric with the largest
                    // forward zone. An impedance that takes longer than
                    // `delta_t` from the outer to the inner one, or keeps moving
                    // across the characteristic inside the outer one, is a swing
                    // and blocks the zones; a jump with the voltage falling is a
                    // fault and does not. The block ends when the impedance
                    // leaves the outer boundary or after the maximum blocking
                    // time.
                    let mut blocked = false;
                    if let (Some(psb), Some(si)) = (&d.power_swing_blocking, seq_i) {
                        if si[1].norm() > d.min_current * i_nom {
                            let z = v1 / si[1] * self.site.z_base;
                            let reach = d
                                .zones
                                .iter()
                                .filter(|z| z.direction == Direction::Forward)
                                .map(|z| z.reach)
                                .fold(0.0, f64::max);
                            // Characteristics concentric with the largest
                            // forward zone (IEEE PSRC WG D6): the mho of that
                            // reach along the line angle, and the same mho
                            // scaled by `outer`. `scale` is the smallest
                            // such mho that holds `z`, so the load, far out
                            // along the R axis, stays outside both.
                            let line = d.line_angle.to_radians();
                            let scale = |z: C64| {
                                let c = (z.arg() - line).cos();
                                if c > 1e-9 && reach > 0.0 {
                                    (z.norm() / (reach * c)).min(1e3)
                                } else {
                                    1e3
                                }
                            };
                            let s = scale(z);
                            let inner = s <= 1.0;
                            let outer = s <= psb.outer;
                            // A jump of half the band in one evaluation is a
                            // fault, not a swing (a load sitting in the band
                            // does not count as entering it).
                            let jump = swing
                                .last
                                .is_some_and(|z0| (s - scale(z0)).abs() > 0.5 * (psb.outer - 1.0));
                            swing.last = Some(z);
                            if jump && swing.blocked_since.is_none() {
                                swing.outer_since = None;
                            }
                            // A jump with the voltage falling is a fault starting; with
                            // it rising, one cleared (into a swing, maybe).
                            let vm = v1n;
                            if jump {
                                swing.fault = swing.last_v.is_some_and(|v0| vm < 0.8 * v0);
                            }
                            if !outer {
                                swing.fault = false;
                            }
                            swing.last_v = Some(vm);
                            // Continuous impedance calculation (IEEE PSRC WG D6): a
                            // fault's impedance stands still, a swing's moves on.
                            // One that keeps moving inside the outer boundary is a
                            // swing even when it never crossed the band (a fault
                            // cleared into a swing lands inside at once).
                            // Steady: over the last `delta_t` it moved at least
                            // `SWING_RATE` reaches per second, no single step
                            // more than half of that (a breaker opening elsewhere
                            // moves it once, then it stands still).
                            if outer && !jump && !swing.fault {
                                swing.trail.push_back((t, z));
                                while swing.trail.len() > 2
                                    && swing
                                        .trail
                                        .get(1)
                                        .is_some_and(|&(t1, _)| t - t1 >= psb.delta_t)
                                {
                                    swing.trail.pop_front();
                                }
                            } else {
                                swing.trail.clear();
                            }
                            if swing.blocked_since.is_none() && reach > 0.0 {
                                if let (Some(&(ta, za)), Some(&(tb, zb))) =
                                    (swing.trail.front(), swing.trail.back())
                                {
                                    let span = tb - ta;
                                    // Across the characteristic (perpendicular to the
                                    // line angle, the continuous ΔR of PSRC WG D6): a
                                    // swing crosses it, a fault seen through changing
                                    // infeed drifts along it.
                                    let across = ((zb - za) * C64::from_polar(1.0, -line)).im.abs();
                                    let moved = (zb - za).norm();
                                    let step = swing
                                        .trail
                                        .iter()
                                        .zip(swing.trail.iter().skip(1))
                                        .map(|(a, b)| (b.1 - a.1).norm())
                                        .fold(0.0, f64::max);
                                    let rate = across / (reach * span.max(1e-9));
                                    if span >= psb.delta_t
                                        && rate > SWING_RATE
                                        && step <= 0.5 * moved
                                    {
                                        swing.blocked_since = Some(t);
                                        ops.push(op(t, id, "68", OperationKind::Blocked, format!("power swing: the impedance moves {rate:.2} reaches/s inside the outer boundary")));
                                    }
                                }
                            }
                            if !outer {
                                if swing.blocked_since.is_some() {
                                    ops.push(op(
                                        t,
                                        id,
                                        "68",
                                        OperationKind::Unblocked,
                                        "impedance left the outer boundary".into(),
                                    ));
                                }
                                swing.outer_since = None;
                                swing.blocked_since = None;
                            } else if !inner {
                                swing.outer_since.get_or_insert(t);
                            } else if swing.blocked_since.is_none() {
                                if let Some(t_out) = swing.outer_since {
                                    if t - t_out >= psb.delta_t {
                                        swing.blocked_since = Some(t);
                                        ops.push(op(t, id, "68", OperationKind::Blocked, format!("power swing: {:.0} ms from the outer to the inner boundary", (t - t_out) * 1e3)));
                                    }
                                }
                                swing.outer_since = None;
                            }
                            if let Some(tb) = swing.blocked_since {
                                if t - tb > psb.max_block {
                                    swing.blocked_since = None;
                                    ops.push(op(
                                        t,
                                        id,
                                        "68",
                                        OperationKind::Unblocked,
                                        "maximum blocking time".into(),
                                    ));
                                } else {
                                    blocked = true;
                                }
                            }
                        } else {
                            swing.outer_since = None;
                            swing.last = None;
                            swing.trail.clear();
                            swing.fault = false;
                            swing.last_v = None;
                        }
                    }
                    // @block Zone timers
                    // A zone that decided with the memory voltage holds its
                    // decision while fault current flows and no voltage is
                    // left to polarize with (directional latch): a sustained
                    // close-in fault still reaches its zone time.
                    let hold = res.unpolarized && !res.zones.as_slice().iter().any(|&p| p);
                    for (zi, (z, timer)) in d.zones.iter().zip(zones.iter_mut()).enumerate() {
                        let decided = if hold {
                            timer.since.is_some()
                        } else {
                            res.zones.as_slice()[zi]
                        };
                        let picked = decided && !blocked;
                        timer.update(picked, t, ops, id, format_args!("21 {}", z.name), || {
                            res.detail()
                        });
                    }
                    let over_name: &str = d
                        .teleprotection
                        .as_ref()
                        .map_or("Z2", |tp| tp.zone.as_str());
                    over = d.zones.iter().zip(res.zones.as_slice()).any(|(z, &p)| {
                        p && !blocked && z.name == over_name && z.direction == Direction::Forward
                    });
                    under = d.zones.iter().zip(res.zones.as_slice()).any(|(z, &p)| {
                        p && !blocked && z.name == "Z1" && z.direction == Direction::Forward
                    });
                    let _ = scheme;
                }
                // @block Differential
                // 87: per phase the operate current `|Σ I|` against the
                // restraint `Σ |I| / 2` over the zone's current transformers,
                // a line's charging current subtracted while both ends are
                // closed. It operates above the biased characteristic or the
                // unrestrained level; second-harmonic content above the
                // setting blocks it below the unrestrained level (inrush).
                (ProtectionFunction::Differential(dset), Element::Timer { timer, .. }) => {
                    let mut id_max: f64 = 0.0;
                    let mut ir_at: f64 = 0.0;
                    let mut sums = [C64::new(0.0, 0.0); 3];
                    let mut mags = [0.0; 3];
                    for ct in &self.site.zone {
                        let c = Self::ct(m, ct.source);
                        for p in 0..np {
                            let x = c[p] * ct.factor;
                            sums[p] += x;
                            mags[p] += x.norm();
                        }
                    }
                    if let Some((e, y_fr, y_to)) = self.site.charging {
                        let topo_ends = (m.end_closed(e, End::From), m.end_closed(e, End::To));
                        if topo_ends.0 && topo_ends.1 {
                            if let Some((vf, vt)) = line_voltages(m, &self.site, e) {
                                for p in 0..np {
                                    sums[p] -= y_fr * vf[p] + y_to * vt[p];
                                }
                            }
                        }
                    }
                    let mut operate = false;
                    for p in 0..np {
                        let idf = sums[p].norm() / i_nom;
                        let ir = mags[p] / 2.0 / i_nom;
                        let restrained = if ir <= dset.knee {
                            dset.slope1 * ir
                        } else {
                            dset.slope1 * dset.knee + dset.slope2 * (ir - dset.knee)
                        };
                        if idf > dset.unrestrained || (idf > dset.pickup && idf > restrained) {
                            operate = true;
                        }
                        if idf > id_max {
                            id_max = idf;
                            ir_at = ir;
                        }
                    }
                    if let (true, Some(limit)) = (operate, dset.harmonic2) {
                        if id_max <= dset.unrestrained {
                            for ct in &self.site.zone {
                                if let CtSource::Branch(e, end) = ct.source {
                                    if let Some(h2) = m.branch_current_h2(e, end) {
                                        let fund = m.branch_current(e, end);
                                        if (0..3).any(|p| {
                                            fund[p].norm() > 1e-6 && h2[p] / fund[p].norm() > limit
                                        }) {
                                            operate = false;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    timer.update(operate, t, ops, id, label, || {
                        format!("Id {id_max:.2} In, Ir {ir_at:.2} In")
                    });
                }
                // @block Voltage and V/Hz
                // 27 on the lowest and 59 on the highest phase-to-ground
                // voltage, each with a small reset hysteresis; 59N on `3 V0`
                // (unbalanced engines only); 24 on the voltage over the
                // frequency, both per unit.
                (ProtectionFunction::Undervoltage(s), Element::Timer { timer, .. }) => {
                    let vmin = phase_ground_of(&v, m.unbalanced(), v1n)
                        .into_iter()
                        .fold(f64::INFINITY, f64::min);
                    let held = timer.since.is_some();
                    let picked = vmin < s.pickup || (held && vmin < 1.02 * s.pickup);
                    timer.update(picked, t, ops, id, label, || format!("V {vmin:.3} pu"));
                }
                (ProtectionFunction::Overvoltage(s), Element::Timer { timer, .. }) => {
                    let vmax = phase_ground_of(&v, m.unbalanced(), v1n)
                        .into_iter()
                        .fold(0.0, f64::max);
                    let held = timer.since.is_some();
                    let picked = vmax > s.pickup || (held && vmax > s.reset_ratio.unwrap_or(0.98) * s.pickup);
                    timer.update(picked, t, ops, id, label, || format!("V {vmax:.3} pu"));
                }
                (ProtectionFunction::NeutralOvervoltage(s), Element::Timer { timer, .. }) => {
                    if !m.unbalanced() {
                        continue;
                    }
                    let v0 = 3.0 * seq_v[0].norm();
                    timer.update(v0 > s.pickup, t, ops, id, label, || {
                        format!("3V0 {v0:.3} pu")
                    });
                }
                (ProtectionFunction::Overexcitation(s), Element::Timer { timer, .. }) => {
                    let vhz = v1n / (freq / f0).max(1e-3);
                    timer.update(vhz > s.pickup, t, ops, id, label, || {
                        format!("V/Hz {vhz:.3} pu")
                    });
                }
                // @block Frequency
                // 81U and 81O on the frequency estimate, blocked below their
                // minimum voltage, with a 20 mHz reset hysteresis.
                (ProtectionFunction::Underfrequency(s), Element::Timer { timer, .. }) => {
                    let held = timer.since.is_some();
                    let ok = v1n >= s.min_voltage;
                    let picked = ok && (freq < s.pickup || (held && freq < s.pickup + 0.02));
                    timer.update(picked, t, ops, id, label, || format!("f {freq:.3} Hz"));
                }
                (ProtectionFunction::Overfrequency(s), Element::Timer { timer, .. }) => {
                    let held = timer.since.is_some();
                    let ok = v1n >= s.min_voltage;
                    let picked = ok && (freq > s.pickup || (held && freq > s.pickup - 0.02));
                    timer.update(picked, t, ops, id, label, || format!("f {freq:.3} Hz"));
                }
                // @block Rate of change of frequency
                // 81R: the least-squares slope of the frequency over a full
                // window, leaving out a voltage dip and the 50 ms after it;
                // blocked below its minimum voltage.
                (ProtectionFunction::Rocof(s), Element::Timer { timer, .. }) => {
                    let ok = v1n >= s.min_voltage;
                    if !ok {
                        self.freq.low_at = Some(t);
                    }
                    // The rate is that of a full window (ENTSO-E's 500 ms).
                    // Samples from a dip, and the settling of the frequency
                    // filter after it, do not count: the angle jumps when the
                    // voltage collapses and returns, which is no change of
                    // frequency.
                    let after = self.freq.low_at.map_or(f64::NEG_INFINITY, |l| l + 0.05);
                    let hist = &mut self.freq.history;
                    while hist.len() > 2 && hist.front().is_some_and(|(th, _)| t - th > s.window) {
                        hist.pop_front();
                    }
                    // The samples after `after`: the history runs in time,
                    // so they are its tail.
                    let start = hist.partition_point(|&(th, _)| th <= after);
                    let window = hist.range(start..);
                    // Least-squares slope over the window: a phase jump's
                    // brief frequency spike does not read as a rate.
                    let rate = match (window.clone().next(), window.clone().next_back()) {
                        (Some(&(ta, _)), Some(&(tb, _))) if tb - ta >= 0.95 * s.window => {
                            let n = window.len() as f64;
                            let (st, sf) = window
                                .clone()
                                .fold((0.0, 0.0), |(a, b), &(t, f)| (a + t, b + f));
                            let (mt, mf) = (st / n, sf / n);
                            let (num, den) = window.fold((0.0, 0.0), |(a, b), &(t, f)| {
                                (a + (t - mt) * (f - mf), b + (t - mt) * (t - mt))
                            });
                            if den > 0.0 {
                                num / den
                            } else {
                                0.0
                            }
                        }
                        _ => 0.0,
                    };
                    timer.update(ok && rate.abs() > s.pickup, t, ops, id, label, || {
                        format!("df/dt {rate:.2} Hz/s")
                    });
                }
                // @block Reverse power and loss of field
                // 32 picks up when the unit's active power, on its machine
                // base, is below minus the setting. 40 places the
                // positive-sequence impedance on the machine base against two
                // offset mho circles below the R axis, each with its delay.
                (ProtectionFunction::ReversePower(s), Element::Timer { timer, .. }) => {
                    let Some(ip) = i else { continue };
                    let p: f64 = if bal {
                        (v[0] * ip[0].conj()).re
                    } else {
                        (0..3).map(|k| (v[k] * ip[k].conj()).re).sum::<f64>() / 3.0
                    };
                    let p_mach = p / self.site.scale;
                    timer.update(p_mach < -s.pickup, t, ops, id, label, || {
                        format!("P {p_mach:.4} pu")
                    });
                }
                (ProtectionFunction::LossOfField(l), Element::LossOfField { z1, z2 }) => {
                    let Some(si) = seq_i else { continue };
                    let (in1, in2) = if si[1].norm() > 0.1 * i_nom {
                        let z = v1 / si[1] * self.site.scale;
                        let inside =
                            |d: f64| (z - C64::new(0.0, -(l.offset + d / 2.0))).norm() <= d / 2.0;
                        (inside(l.diameter1), inside(l.diameter2))
                    } else {
                        (false, false)
                    };
                    z1.update(in1, t, ops, id, format_args!("{label} Z1"), String::new);
                    z2.update(in2, t, ops, id, format_args!("{label} Z2"), String::new);
                }
                // @block Out of step
                // 78: the impedance is placed left of, between or right of
                // the blinders (or outside the reach); a slip is a crossing
                // from one side to the other that spent at least the minimum
                // time between them. The set number of slips trips.
                (
                    ProtectionFunction::OutOfStep(o),
                    Element::OutOfStep {
                        side,
                        entered,
                        slips,
                        beat,
                        beat_ready,
                    },
                ) => {
                    if let Some(vb) = o.beat_voltage {
                        // Experimental: voltage beats. A beat is
                        // counted once the voltage has passed a minimum below `vb` and has
                        // been rising for four half-cycles (40 ms). After the last beat the
                        // relay operates `beat_delay` after the voltage exceeds its average
                        // over the oscillation. `entered` holds the start of the dip, `side`
                        // is `Middle` once the dip has been counted.
                        let vm = v1.norm();
                        if entered.is_some() || *slips > 0 {
                            beat[2] += vm * dt;
                            beat[3] += dt;
                        }
                        if vm < vb {
                            if entered.is_none() {
                                *entered = Some((Side::Middle, t));
                                *side = Side::Outside;
                                beat[0] = vm;
                                beat[1] = t;
                            }
                            if vm < beat[0] {
                                beat[0] = vm;
                                beat[1] = t;
                            }
                        }
                        if entered.is_some() && *side != Side::Middle && t - beat[1] >= 0.04 && vm > beat[0] {
                            *side = Side::Middle;
                            *slips += 1;
                            ops.push(op(
                                t,
                                id,
                                label,
                                OperationKind::Pickup,
                                format!("voltage beat {} (minimum {:.2} pu, below {vb} pu)", *slips, beat[0]),
                            ));
                        }
                        if vm > 1.1 * vb && *side == Side::Middle {
                            *entered = None;
                        }
                        if *slips >= o.slips && beat_ready.is_none() && beat[3] > 0.0 && vm > beat[2] / beat[3] {
                            *beat_ready = Some(t);
                        }
                        if let Some(tr) = *beat_ready {
                            if t - tr >= o.beat_delay {
                                ready = Some((label.clone(), false, false));
                            }
                        }
                    }
                    let si = match seq_i {
                        Some(si) if o.beat_voltage.is_none() => si,
                        _ => {
                            if o.beat_voltage.is_some() {
                                // fall through to the trip handling below
                                [C64::new(0.0, 0.0); 3]
                            } else {
                                continue;
                            }
                        }
                    };
                    let now = if o.beat_voltage.is_some() {
                        *side
                    } else if si[1].norm() > 0.1 * i_nom {
                        let z = v1 / si[1] * self.site.scale;
                        if z.im > o.forward_reach || z.im < -o.reverse_reach {
                            Side::Outside
                        } else if z.re > o.blinder {
                            Side::Right
                        } else if z.re < -o.blinder {
                            Side::Left
                        } else {
                            Side::Middle
                        }
                    } else {
                        Side::Outside
                    };
                    if now != *side {
                        match (*side, now) {
                            (Side::Left | Side::Right, Side::Middle) => *entered = Some((*side, t)),
                            (Side::Middle, Side::Left | Side::Right) => {
                                if let Some((from, t_in)) = *entered {
                                    if from != now && t - t_in >= o.min_traverse {
                                        *slips += 1;
                                        ops.push(op(
                                            t,
                                            id,
                                            label,
                                            OperationKind::Pickup,
                                            format!(
                                                "pole slip {} ({:.0} ms between the blinders)",
                                                *slips,
                                                (t - t_in) * 1e3
                                            ),
                                        ));
                                    }
                                }
                                *entered = None;
                            }
                            _ => *entered = None,
                        }
                        *side = now;
                    }
                    if o.beat_voltage.is_none() && *slips >= o.slips {
                        ready = Some((label.clone(), false, false));
                    }
                }
                // @block Negative sequence and thermal
                // 46 integrates `I2² − I2c²` above the continuous capability
                // and cools at that rate below it, tripping at `K`
                // (unbalanced engines only). 49 follows the first-order
                // thermal image of the largest phase current and trips at
                // `θ = 1`, starting from the state of the first current.
                (ProtectionFunction::NegativeSequence(n), Element::Integral { acc }) => {
                    if !m.unbalanced() {
                        continue;
                    }
                    let Some(si) = seq_i else { continue };
                    let i2 = si[2].norm() / i_nom;
                    if i2 > n.continuous {
                        *acc += (i2 * i2 - n.continuous * n.continuous) * dt;
                    } else {
                        *acc = (*acc - n.continuous * n.continuous * dt).max(0.0);
                    }
                    if *acc >= n.k {
                        ready = Some((label.clone(), false, false));
                    }
                }
                (ProtectionFunction::Thermal(th), Element::Thermal { theta }) => {
                    let Some(ip) = i else { continue };
                    let imax = ip[..np].iter().map(|x| x.norm()).fold(0.0, f64::max);
                    let q = (imax / (th.k * i_nom)).powi(2);
                    let next = match *theta {
                        None => q,
                        Some(old) => old + (dt / th.tau).min(1.0) * (q - old),
                    };
                    *theta = Some(next);
                    if next >= 1.0 {
                        ready = Some((label.clone(), false, false));
                    }
                }
                // @block Ride-through envelope
                // FRT: a fall below the start voltage from a healthy state
                // starts the clock, a recovery that lasts the reset time stops
                // it, and the unit trips while the voltage is below the
                // profile at the clock's time.
                (
                    ProtectionFunction::FaultRideThrough(frt),
                    Element::Frt {
                        t0,
                        above_since,
                        timer,
                        armed,
                    },
                ) => {
                    let vm = if m.unbalanced() {
                        phase_phase(&v).into_iter().fold(f64::INFINITY, f64::min)
                    } else {
                        v1n
                    };
                    // The profile is that of a fault (RfG Art. 14(3) and 16(3):
                    // the voltage during and after it, from a pre-fault state):
                    // a unit that sits below the start value at rest is no
                    // fault ride-through; its undervoltage stage covers it.
                    if vm >= frt.start && t0.is_none() {
                        *armed = true;
                    }
                    if vm < frt.start && (t0.is_some() || *armed) {
                        above_since.take();
                        *armed = false;
                        if t0.is_none() {
                            *t0 = Some(t);
                            ops.push(op(
                                t,
                                id,
                                label,
                                OperationKind::Pickup,
                                format!("ride-through started at {vm:.3} pu"),
                            ));
                        }
                    } else if t0.is_some() {
                        let s = *above_since.get_or_insert(t);
                        if t - s >= frt.reset {
                            *t0 = None;
                            *above_since = None;
                            ops.push(op(t, id, label, OperationKind::Reset, String::new()));
                        }
                    }
                    let below = t0.is_some_and(|s| vm < profile(&frt.points, t - s));
                    timer.update(below, t, ops, id, format_args!("{label} envelope"), || {
                        format!("V {vm:.3} pu")
                    });
                }
                _ => {}
            }
            self.picks[k].ready = ready;
        }
        self.overreach_fwd = over;
        self.underreach_fwd = under;
    }

    /// Whether a function trips now: `(label, starts auto-reclose, unit
    /// zone trip)`.
    // @block Decide
    pub(crate) fn decide(
        &mut self,
        t: f64,
        measuring: f64,
        ops: &mut Vec<Operation>,
    ) -> Option<(String, bool, bool)> {
        if self.latched {
            return None;
        }
        let line = matches!(self.spec.location, super::RelayLocation::Line { .. });
        let mut result: Option<(String, bool, bool)> = None;
        for (k, f) in self.spec.functions.iter().enumerate() {
            if result.is_some() {
                break;
            }
            if let Some(r) = self.picks[k].ready.take() {
                result = Some(r);
                break;
            }
            let label = &self.labels[k];
            match (f, &mut self.elements[k]) {
                (ProtectionFunction::Differential(_), Element::Timer { timer, delay }) => {
                    if timer.expired(t, *delay + measuring) {
                        result = Some((label.clone(), line, true));
                    }
                }
                (_, Element::Timer { timer, delay }) => {
                    if timer.expired(t, *delay + measuring) {
                        result = Some((label.clone(), false, false));
                    }
                }
                (
                    ProtectionFunction::Overcurrent(o) | ProtectionFunction::EarthFault(o),
                    Element::Inverse {
                        timer, integral, ..
                    },
                ) => {
                    if *integral >= 1.0 && timer.expired(t, o.delay + measuring) {
                        result = Some((label.clone(), false, false));
                    }
                }
                (ProtectionFunction::Distance(d), Element::Distance { zones, scheme, .. }) => {
                    for (z, timer) in d.zones.iter().zip(zones.iter()) {
                        if timer.expired(t, z.delay + measuring) {
                            result = Some((format!("21 {}", z.name), z.autoreclose, false));
                            break;
                        }
                    }
                    if result.is_none() {
                        if let Some(tp) = &d.teleprotection {
                            let permission = match tp.scheme {
                                super::SchemeKind::Pott => self.remote.is_some_and(|r| r.0),
                                super::SchemeKind::Putt => self.remote.is_some_and(|r| r.1),
                            };
                            let name = match tp.scheme {
                                super::SchemeKind::Pott => "21 POTT",
                                super::SchemeKind::Putt => "21 PUTT",
                            };
                            scheme.update(
                                self.overreach_fwd && permission,
                                t,
                                ops,
                                &self.spec.id,
                                name,
                                || "permission received".to_owned(),
                            );
                            if scheme.expired(t, tp.channel_time + measuring) {
                                result = Some((name.into(), true, false));
                            }
                        }
                    }
                }
                (ProtectionFunction::LossOfField(l), Element::LossOfField { z1, z2 }) => {
                    if z1.expired(t, l.delay1 + measuring) || z2.expired(t, l.delay2 + measuring) {
                        result = Some((label.clone(), false, false));
                    }
                }
                (ProtectionFunction::FaultRideThrough(_), Element::Frt { timer, .. })
                    if timer.expired(t, measuring) =>
                {
                    result = Some((label.clone(), false, false));
                }
                _ => {}
            }
        }
        if result.is_some() {
            self.latched = true;
        }
        result
    }
}

// @block Voltage helpers
// Phase-to-ground and phase-to-phase voltage magnitudes, the voltages at
// both ends of a line for its charging current, and the ride-through
// profile at a time after the fault.

/// Phase-to-ground voltage magnitudes (pu of the phase-to-ground base); in
/// a balanced measurement the positive sequence.
fn phase_ground(v: &[C64; 3], unbalanced: bool) -> [f64; 3] {
    if unbalanced {
        [v[0].norm(), v[1].norm(), v[2].norm()]
    } else {
        let m = sequences(*v)[1].norm();
        [m; 3]
    }
}

/// [`phase_ground`] where the positive-sequence magnitude `v1n` of `v` is
/// already known (the balanced case reads only it).
fn phase_ground_of(v: &[C64; 3], unbalanced: bool, v1n: f64) -> [f64; 3] {
    if unbalanced {
        phase_ground(v, true)
    } else {
        [v1n; 3]
    }
}

/// The 60° rotation of the directional overcurrent characteristic.
fn rotate_60() -> C64 {
    static R: std::sync::OnceLock<C64> = std::sync::OnceLock::new();
    *R.get_or_init(|| C64::from_polar(1.0, 60f64.to_radians()))
}

/// Phase-to-phase voltages divided by √3.
fn phase_phase(v: &[C64; 3]) -> [f64; 3] {
    let s = 3f64.sqrt();
    [
        (v[0] - v[1]).norm() / s,
        (v[1] - v[2]).norm() / s,
        (v[2] - v[0]).norm() / s,
    ]
}

/// The voltages at both ends of branch `e` (for charging compensation).
fn line_voltages(m: &dyn Measure, _site: &Site, e: usize) -> Option<([C64; 3], [C64; 3])> {
    let vf = m.branch_end_voltage(e, End::From)?;
    let vt = m.branch_end_voltage(e, End::To)?;
    Some((vf, vt))
}

/// Linear interpolation of a ride-through profile.
pub(crate) fn profile(points: &[[f64; 2]], t: f64) -> f64 {
    let Some(first) = points.first() else {
        return 0.0;
    };
    if t <= first[0] {
        return first[1];
    }
    for w in points.windows(2) {
        let ([t0, v0], [t1, v1]) = (w[0], w[1]);
        if t <= t1 {
            if t1 - t0 < 1e-12 {
                return v1;
            }
            return v0 + (v1 - v0) * (t - t0) / (t1 - t0);
        }
    }
    points.last().map_or(0.0, |p| p[1])
}

// @block Distance characteristic
// The distance measurement: the measuring loops of a relay (six loops on
// unbalanced phasors, the positive-sequence loop on balanced ones), the
// load-encroachment region, and whether a loop's impedance lies inside a
// mho or quadrilateral zone.
/// Which zones of a distance function see the fault: inline up to
/// `INLINE_ZONES` (every practical scheme), on the heap beyond. Relays
/// evaluate every step, so the common case allocates nothing.
enum ZoneFlags {
    Inline([bool; INLINE_ZONES], usize),
    Heap(Vec<bool>),
}

const INLINE_ZONES: usize = 8;

impl ZoneFlags {
    fn new(n: usize) -> Self {
        if n <= INLINE_ZONES {
            Self::Inline([false; INLINE_ZONES], n)
        } else {
            Self::Heap(vec![false; n])
        }
    }

    fn as_slice(&self) -> &[bool] {
        match self {
            Self::Inline(a, n) => &a[..*n],
            Self::Heap(v) => v,
        }
    }

    fn as_mut_slice(&mut self) -> &mut [bool] {
        match self {
            Self::Inline(a, n) => &mut a[..*n],
            Self::Heap(v) => v,
        }
    }
}

struct DistanceResult {
    zones: ZoneFlags,
    /// The first loop's apparent impedance (Ω) outside the load region.
    z_ohm: Option<C64>,
    /// A loop carried fault current but had no voltage to polarize with
    /// (memory expired on a sustained close-in fault).
    unpolarized: bool,
}

impl DistanceResult {
    fn detail(&self) -> String {
        self.z_ohm
            .map_or_else(String::new, |z| format!("Z = {:.2} + j{:.2} Ω", z.re, z.im))
    }
}

/// Evaluate the zones of a distance function on the six loops (or the
/// positive-sequence loop of balanced phasors).
// @block Measuring loops
fn distance(
    d: &Distance,
    v: &[C64; 3],
    i: &[C64; 3],
    seq1: Option<(C64, C64)>,
    vmem: C64,
    site: &Site,
    i_nom: f64,
) -> DistanceResult {
    let a = op_a();
    let k0 = C64::new(d.k0[0], d.k0[1]);
    let imin = d.min_current * i_nom;
    // (loop voltage, loop current, polarizing voltage): at most the three
    // phase-to-phase and three phase-to-ground loops, kept inline.
    let zero = C64::new(0.0, 0.0);
    let mut loops = [(zero, zero, zero); 6];
    let mut n_loops = 0;
    let mut push = |l: (C64, C64, C64)| {
        loops[n_loops] = l;
        n_loops += 1;
    };
    if let Some((v1, i1)) = seq1 {
        if i1.norm() > imin {
            push((v1, i1, vmem));
        }
    } else {
        let i_res = i[0] + i[1] + i[2];
        let pol_ph = [vmem, vmem * a * a, vmem * a];
        for p in 0..3 {
            let q = (p + 1) % 3;
            let il = i[p] - i[q];
            if il.norm() > 3f64.sqrt() * imin {
                push((v[p] - v[q], il, pol_ph[p] - pol_ph[q]));
            }
        }
        if i_res.norm() > 0.1 * i_nom {
            for p in 0..3 {
                let il = i[p] + k0 * i_res;
                if i[p].norm() > imin {
                    push((v[p], il, pol_ph[p]));
                }
            }
        }
    }
    let zb = site.z_base;
    let mut flags = ZoneFlags::new(d.zones.len());
    let zones = flags.as_mut_slice();
    let mut first: Option<C64> = None;
    let mut unpolarized = false;
    for (vl, il, pol) in &loops[..n_loops] {
        // Polarized with the pre-fault (memory) voltage for 100 ms after a
        // collapse, then with the present voltage: the memory keeps the
        // direction right for close-in faults and through the voltage
        // inversion of a series capacitor (IEEE C37.116).
        // Below 10 % no direction can be told from the voltage (the
        // close-in limit of the memory above): the decision made with the
        // memory is held.
        if pol.norm() < 0.1 {
            unpolarized = true;
            continue;
        }
        let z_ohm = vl / il * zb;
        if in_load(d, z_ohm) {
            continue;
        }
        first.get_or_insert(z_ohm);
        for (k, zone) in d.zones.iter().enumerate() {
            if zones[k] {
                continue;
            }
            zones[k] = in_zone(d, zone, *vl, *il, *pol, zb);
        }
    }
    DistanceResult {
        zones: flags,
        z_ohm: first,
        unpolarized,
    }
}

/// Whether an apparent impedance (system pu) lies inside a zone, the zone
/// self-polarized (the static characteristic).
pub(crate) fn zone_contains(d: &Distance, z: &Zone, zapp: C64, zb: f64) -> bool {
    !in_load(d, zapp * zb) && in_zone(d, z, zapp, C64::new(1.0, 0.0), zapp, zb)
}

/// Whether an impedance (ohms) lies in the load region of the
/// load-encroachment blinder.
fn in_load(d: &Distance, z: C64) -> bool {
    d.load_encroachment.as_ref().is_some_and(|l| {
        let a = z.arg().to_degrees().abs();
        z.norm() >= l.r && (a <= l.angle || a >= 180.0 - l.angle)
    })
}

/// Whether a loop (voltage `v`, current `i`, polarizing voltage `pol`) lies
/// inside zone `z`: the polarized mho of the zone's reach along its angle,
/// or the quadrilateral of its reactance and resistive reaches after a
/// directional check.
// @block Mho and quadrilateral
fn in_zone(d: &Distance, z: &Zone, v: C64, i: C64, pol: C64, zb: f64) -> bool {
    let angle = z.angle.unwrap_or(d.line_angle).to_radians();
    let sign = if z.direction == Direction::Reverse {
        -1.0
    } else {
        1.0
    };
    match d.characteristic {
        Characteristic::Mho => {
            let zr = C64::from_polar(sign * z.reach / zb, angle);
            ((zr * i - v) * pol.conj()).re >= 0.0
        }
        Characteristic::Quadrilateral => {
            let dir =
                (i * C64::from_polar(1.0, d.line_angle.to_radians()) * pol.conj()).re * sign > 0.0;
            if !dir {
                return false;
            }
            let zo = v / i * zb * sign;
            // Reactance line at the reach, resistive blinders parallel to the
            // line angle; the lower (−0.2 X) and left (−0.25 R) limits are
            // this module's choice (no standard fixes the shape).
            let xr = z.reach * angle.sin();
            let rr = z.resistive_reach.unwrap_or(z.reach / 2.0);
            let cot = 1.0 / d.line_angle.to_radians().tan().max(1e-6);
            zo.im <= xr
                && zo.im >= -0.2 * xr
                && zo.re <= rr + zo.im * cot
                && zo.re >= -0.25 * rr + zo.im * cot
        }
    }
}
// @end

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iec_curves_match_the_standard_points() {
        // IEC 60255-151 Annex A and IEEE C37.112 constants, TMS 1 at
        // G/Gs = 10 (long-time inverse: the older IEC 60255-3 curve).
        let t = |c| curve_time(c, 1.0, 10.0).unwrap();
        assert!((t(Curve::IecStandardInverse) - 2.971).abs() < 1e-3);
        assert!((t(Curve::IecVeryInverse) - 1.5).abs() < 1e-9);
        assert!((t(Curve::IecExtremelyInverse) - 0.8081).abs() < 1e-4);
        assert!((t(Curve::IecLongTimeInverse) - 13.333).abs() < 1e-3);
        assert!((t(Curve::IeeeModeratelyInverse) - 1.2067).abs() < 1e-3);
        // Flat above 20 × and none below the setting.
        assert_eq!(
            curve_time(Curve::IecVeryInverse, 1.0, 40.0),
            curve_time(Curve::IecVeryInverse, 1.0, 20.0)
        );
        assert!(curve_time(Curve::IecVeryInverse, 1.0, 0.9).is_none());
    }

    #[test]
    fn curves_at_published_points() {
        // Each curve at 2 and 5 times the setting, TMS / TD 1, from its
        // equation with the published constants.
        let t = |c, g| curve_time(c, 1.0, g).unwrap();
        let iec = |k: f64, a: f64, g: f64| k / (g.powf(a) - 1.0);
        let ieee = |a: f64, p: f64, b: f64, g: f64| a / (g.powf(p) - 1.0) + b;
        for g in [2.0, 5.0] {
            assert!((t(Curve::IecStandardInverse, g) - iec(0.14, 0.02, g)).abs() < 1e-12);
            assert!((t(Curve::IecVeryInverse, g) - iec(13.5, 1.0, g)).abs() < 1e-12);
            assert!((t(Curve::IecExtremelyInverse, g) - iec(80.0, 2.0, g)).abs() < 1e-12);
            assert!(
                (t(Curve::IeeeModeratelyInverse, g) - ieee(0.0515, 0.02, 0.114, g)).abs() < 1e-12
            );
            assert!((t(Curve::IeeeVeryInverse, g) - ieee(19.61, 2.0, 0.491, g)).abs() < 1e-12);
            assert!((t(Curve::IeeeExtremelyInverse, g) - ieee(28.2, 2.0, 0.1217, g)).abs() < 1e-12);
        }
        // Rounded reference values: IEC SI 10.03 s and VI 13.5 s at 2 ×.
        assert!((t(Curve::IecStandardInverse, 2.0) - 10.03).abs() < 0.01);
        assert!((t(Curve::IecVeryInverse, 2.0) - 13.5).abs() < 1e-12);
        // The time multiplier scales the time.
        assert!((curve_time(Curve::IecVeryInverse, 0.1, 2.0).unwrap() - 1.35).abs() < 1e-12);
    }

    #[test]
    fn ieee_reset_follows_c37_112() {
        // `TD tr / (1 − M²)` below the setting; the IEC curves reset at once.
        let r = |c, m| reset_time(c, 1.0, m);
        assert!((r(Curve::IeeeModeratelyInverse, 0.5).unwrap() - 4.85 / 0.75).abs() < 1e-12);
        assert!((r(Curve::IeeeVeryInverse, 0.5).unwrap() - 21.6 / 0.75).abs() < 1e-12);
        assert!((r(Curve::IeeeExtremelyInverse, 0.0).unwrap() - 29.1).abs() < 1e-12);
        assert!(r(Curve::IecStandardInverse, 0.5).is_none());
    }

    fn one_zone(characteristic: Characteristic) -> (Distance, Zone) {
        let zone = Zone {
            name: "Z1".into(),
            reach: 10.0,
            angle: None,
            resistive_reach: Some(5.0),
            delay: 0.0,
            direction: Direction::Forward,
            autoreclose: false,
        };
        let d = Distance {
            characteristic,
            zones: vec![zone.clone()],
            line_angle: 75.0,
            k0: [0.0, 0.0],
            min_current: 0.2,
            power_swing_blocking: None,
            teleprotection: None,
            load_encroachment: None,
        };
        (d, zone)
    }

    #[test]
    fn mho_is_the_circle_through_the_origin_along_the_line_angle() {
        // IEC 60255-121: a self-polarized mho is the circle with the reach as
        // its diameter, from the origin along the line angle (ohms, z_base 1).
        let (d, z) = one_zone(Characteristic::Mho);
        let at = |r: f64, deg: f64| C64::from_polar(r, deg.to_radians());
        assert!(zone_contains(&d, &z, at(9.99, 75.0), 1.0));
        assert!(!zone_contains(&d, &z, at(10.01, 75.0), 1.0));
        // At 30° off the line angle the boundary is at reach · cos 30°.
        let edge = 10.0 * 30f64.to_radians().cos();
        assert!(zone_contains(&d, &z, at(edge - 0.01, 45.0), 1.0));
        assert!(!zone_contains(&d, &z, at(edge + 0.01, 45.0), 1.0));
        // A reverse fault is outside.
        assert!(!zone_contains(&d, &z, at(1.0, 75.0 + 180.0), 1.0));
    }

    #[test]
    fn quadrilateral_reactance_line_and_blinder() {
        let (d, z) = one_zone(Characteristic::Quadrilateral);
        let x = 10.0 * 75f64.to_radians().sin();
        let cot = 1.0 / 75f64.to_radians().tan();
        let inside = C64::new(0.5 * x * cot, 0.5 * x);
        assert!(zone_contains(&d, &z, inside, 1.0));
        assert!(!zone_contains(
            &d,
            &z,
            C64::new(0.5 * x * cot, 1.01 * x),
            1.0
        ));
        assert!(zone_contains(
            &d,
            &z,
            C64::new(0.5 * x * cot + 4.9, 0.5 * x),
            1.0
        ));
        assert!(!zone_contains(
            &d,
            &z,
            C64::new(0.5 * x * cot + 5.1, 0.5 * x),
            1.0
        ));
    }

    #[test]
    fn ride_through_profile_interpolates() {
        let p = [[0.0, 0.0], [0.15, 0.0], [1.5, 0.85]];
        assert_eq!(profile(&p, 0.1), 0.0);
        assert!((profile(&p, 0.825) - 0.425).abs() < 1e-12);
        assert_eq!(profile(&p, 3.0), 0.85);
    }
}

