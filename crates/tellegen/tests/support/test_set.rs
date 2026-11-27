// A relay test set for secondary injection: it drives prescribed voltage and
// current phasors (balanced or per phase) into the relays of a scheme, one
// evaluation per injection step, and operates the breakers the relays
// command. No network is solved. Included (inside a module) by the
// protection type-test files.

use num_complex::Complex;
use tellegen::protection::{
    Action, End, Measure, Operation, OperationKind, ProtectionReport, ProtectionRuntime,
    ProtectionScheme, Relay, RelayLocation, TopoBranch, TopoUnit, Topology, UnitKind,
};

pub type C64 = Complex<f64>;

/// The relays' measuring time (one cycle at 50 Hz).
pub const MT: f64 = 0.02;
/// Rated frequency of the rig.
pub const F0: f64 = 50.0;
/// The usual injection step.
pub const DT: f64 = 0.001;

pub fn c(re: f64, im: f64) -> C64 {
    C64::new(re, im)
}

pub fn polar(m: f64, deg: f64) -> C64 {
    C64::from_polar(m, deg.to_radians())
}

/// Balanced phase phasors `[a, b, c]` of a positive-sequence phasor.
pub fn balanced(v: C64) -> [C64; 3] {
    let a = polar(1.0, 120.0);
    [v, v * a * a, v * a]
}

/// Two buses of 10 kV on 100 MVA (so 1 Ω is 1 pu), one branch from bus 1 to
/// bus 2 and one unit of 100 MVA at bus 2.
pub fn rig(transformer: bool, z: C64, kind: UnitKind) -> Topology {
    Topology {
        sbase: 100.0,
        f0: F0,
        bus_numbers: vec![1, 2],
        base_kv: vec![10.0, 10.0],
        branches: vec![TopoBranch {
            from: 0,
            to: 1,
            from_number: 1,
            to_number: 2,
            circuit: "1".into(),
            transformer,
            rate: 1.0,
            z,
            y_fr: c(0.0, 0.0),
            y_to: c(0.0, 0.0),
            tap: c(1.0, 0.0),
        }],
        units: vec![TopoUnit {
            bus: 1,
            bus_number: 2,
            machine_id: "1".into(),
            mbase: 100.0,
            kind,
            xd: None,
            xd_p: None,
            xd_pp: None,
            p0: 0.0,
        }],
    }
}

/// The usual rig: a line of `j1` pu and a synchronous unit.
pub fn line_rig() -> Topology {
    rig(false, c(0.0, 1.0), UnitKind::Machine)
}

/// What the test set injects at one instant: per-phase voltages at both
/// buses, per-phase currents into the branch at either end and out of the
/// unit, the second-harmonic content of the branch currents, and whether the
/// relays are told the phases are unbalanced (otherwise phase a carries the
/// positive sequence, as an RMS engine reports it).
#[derive(Clone, Copy)]
pub struct Shot {
    pub v: [[C64; 3]; 2],
    pub i_from: [C64; 3],
    pub i_to: [C64; 3],
    pub i_unit: [C64; 3],
    pub h2: Option<[f64; 3]>,
    pub unbalanced: bool,
}

impl Shot {
    /// Rated voltage at both buses, no current.
    pub fn at_rest() -> Self {
        let zero = balanced(c(0.0, 0.0));
        Self {
            v: [balanced(c(1.0, 0.0)), balanced(c(1.0, 0.0))],
            i_from: zero,
            i_to: zero,
            i_unit: zero,
            h2: None,
            unbalanced: false,
        }
    }

    /// Positive-sequence voltage `v` at bus 1 and current `i` into the branch
    /// at bus 1.
    pub fn line(v: C64, i: C64) -> Self {
        let mut s = Self::at_rest();
        s.v[0] = balanced(v);
        s.i_from = balanced(i);
        s
    }

    /// Positive-sequence voltage `v` at bus 2 and current `i` out of the unit.
    pub fn unit(v: C64, i: C64) -> Self {
        let mut s = Self::at_rest();
        s.v[1] = balanced(v);
        s.i_unit = balanced(i);
        s
    }
}

/// The breakers as the test set models them.
#[derive(Clone, Copy)]
pub struct Breakers {
    pub end: [bool; 2],
    pub unit: bool,
}

pub struct Injection {
    pub t: f64,
    pub shot: Shot,
    pub breakers: Breakers,
}

impl Measure for Injection {
    fn time(&self) -> f64 {
        self.t
    }
    fn frame_speed(&self) -> f64 {
        1.0
    }
    fn bus_voltage(&self, b: usize) -> [C64; 3] {
        self.shot.v[b]
    }
    fn branch_current(&self, _e: usize, end: End) -> [C64; 3] {
        let k = usize::from(end == End::To);
        if !self.breakers.end[k] {
            return [c(0.0, 0.0); 3];
        }
        if k == 0 {
            self.shot.i_from
        } else {
            self.shot.i_to
        }
    }
    fn unit_current(&self, _u: usize) -> [C64; 3] {
        if self.breakers.unit {
            self.shot.i_unit
        } else {
            [c(0.0, 0.0); 3]
        }
    }
    fn feeder_current(&self, _b: usize) -> [C64; 3] {
        [c(0.0, 0.0); 3]
    }
    fn branch_end_voltage(&self, _e: usize, end: End) -> Option<[C64; 3]> {
        Some(self.shot.v[usize::from(end == End::To)])
    }
    fn end_closed(&self, _e: usize, end: End) -> bool {
        self.breakers.end[usize::from(end == End::To)]
    }
    fn unit_in_service(&self, _u: usize) -> bool {
        self.breakers.unit
    }
    fn branch_current_h2(&self, _e: usize, end: End) -> Option<[f64; 3]> {
        (end == End::From).then_some(self.shot.h2).flatten()
    }
    fn unbalanced(&self) -> bool {
        self.shot.unbalanced
    }
}

/// Run the relays of `scheme` on `topo` from 0 to `t_end` in steps of `dt`
/// with the injection `shot(t)`; breakers open and close as the relays
/// command, the actions with their times returned beside the record.
pub fn inject(
    scheme: &ProtectionScheme,
    topo: &Topology,
    t_end: f64,
    dt: f64,
    shot: impl Fn(f64) -> Shot,
) -> (ProtectionReport, Vec<(f64, Action)>) {
    let mut rt = ProtectionRuntime::bind(scheme, topo, true).expect("relays bind");
    rt.set_threads(false);
    let mut breakers = Breakers {
        end: [true, true],
        unit: true,
    };
    let mut actions = Vec::new();
    let steps = (t_end / dt).round() as usize;
    for k in 0..=steps {
        let t = k as f64 * dt;
        let s = shot(t);
        let before = Injection {
            t,
            shot: s,
            breakers,
        };
        let mut due = rt.take_due(t, &before);
        due.extend(rt.bank_actions(&before));
        for a in &due {
            match *a {
                Action::OpenEnd(_, end) => breakers.end[usize::from(end == End::To)] = false,
                Action::CloseEnd(_, end) => breakers.end[usize::from(end == End::To)] = true,
                Action::TripUnit(_) => breakers.unit = false,
                Action::Bypass(_) | Action::Reinsert(_) => {}
            }
            actions.push((t, a.clone()));
        }
        rt.applied();
        rt.evaluate(&Injection {
            t,
            shot: s,
            breakers,
        });
    }
    (rt.into_report(), actions)
}

pub fn scheme(relays: Vec<Relay>) -> ProtectionScheme {
    ProtectionScheme {
        relays,
        stuck_breakers: Vec::new(),
        measuring_time: MT,
        series_capacitors: Vec::new(),
    }
}

/// A relay with rated current 1 pu and a 50 ms breaker.
pub fn relay(
    id: &str,
    location: RelayLocation,
    functions: Vec<tellegen::protection::ProtectionFunction>,
) -> Relay {
    Relay {
        id: id.into(),
        location,
        functions,
        breaker_time: 0.05,
        breaker_failure: None,
        autoreclose: None,
        enabled: true,
        i_nom: Some(1.0),
    }
}

/// The end of the branch at bus `at`.
pub fn line_end(at: usize) -> RelayLocation {
    RelayLocation::Line {
        from: 1,
        to: 2,
        circuit: "1".into(),
        at,
    }
}

/// The unit at bus 2.
pub fn generator() -> RelayLocation {
    RelayLocation::Generator {
        bus: 2,
        machine_id: "1".into(),
    }
}

/// The operations of `kind` by `relay` whose function starts with
/// `function`, in time order.
pub fn ops<'a>(
    report: &'a ProtectionReport,
    relay: &str,
    kind: OperationKind,
    function: &str,
) -> Vec<&'a Operation> {
    report
        .operations
        .iter()
        .filter(|o| o.kind == kind && o.relay == relay && o.function.starts_with(function))
        .collect()
}

/// The first operation of `kind` (see [`ops`]), its time.
pub fn first(
    report: &ProtectionReport,
    relay: &str,
    kind: OperationKind,
    function: &str,
) -> Option<f64> {
    ops(report, relay, kind, function).first().map(|o| o.time)
}

/// The first trip of `relay` by a function starting with `function`.
pub fn trip(report: &ProtectionReport, relay: &str, function: &str) -> Option<f64> {
    first(report, relay, OperationKind::Trip, function)
}

/// Assert `t` lies within `[expected - early, expected + late]`.
pub fn assert_within(what: &str, t: Option<f64>, expected: f64, early: f64, late: f64) {
    let t = t.unwrap_or_else(|| panic!("{what}: did not operate, expected at {expected:.4} s"));
    assert!(
        t >= expected - early - 1e-9 && t <= expected + late + 1e-9,
        "{what}: at {t:.4} s, expected {expected:.4} s (-{early:.4}/+{late:.4})"
    );
}

/// Assert nothing operated.
pub fn assert_none(what: &str, t: Option<f64>) {
    assert!(
        t.is_none(),
        "{what}: operated at {:.4} s",
        t.unwrap_or_default()
    );
}

/// `x0` until `t0`, then changing at `rate` per second.
pub fn ramp(t: f64, t0: f64, x0: f64, rate: f64) -> f64 {
    x0 + rate * (t - t0).max(0.0)
}
