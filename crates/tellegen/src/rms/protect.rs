//! The RMS engine's side of the protection relays: what they measure and
//! what their breakers do.

use super::system::{BusLoad, DynSystem, C64};
use super::topology::{trip_held, RmsMeasure};
use crate::protection::{balanced, Action, End, Measure};

/// The relays' view of the state `x` at `t`: the engine's positive-sequence
/// phasors as balanced phase sets.
pub(crate) struct RelayMeasure<'a> {
    sys: &'a DynSystem,
    x: &'a [f64],
    t: f64,
    probe: RmsMeasure<'a>,
}

impl<'a> RelayMeasure<'a> {
    pub(crate) fn new(sys: &'a DynSystem, x: &'a [f64], t: f64) -> Self {
        Self {
            sys,
            x,
            t,
            probe: RmsMeasure::new(sys, x),
        }
    }

    /// Currents into branch `e` at its from and to ends: the switched pi
    /// model (open ends and inner fault folded in), its tap at the from end.
    fn branch_currents(&self, e: usize) -> (C64, C64) {
        let br = &self.sys.branches;
        let (f, t) = (br.from_bus[e], br.to_bus[e]);
        let vf = self.probe.bus_voltage(f);
        let vt = self.probe.bus_voltage(t);
        let y = C64::new(br.g[e], br.b[e]);
        let y_fr = C64::new(br.g_fr[e], br.b_fr[e]);
        let y_to = C64::new(br.g_to[e], br.b_to[e]);
        let tap = C64::from_polar(br.tap[e], br.shift[e]);
        let i_f = (y + y_fr) / tap.norm_sqr() * vf - y / tap.conj() * vt;
        let i_t = (y + y_to) * vt - y / tap * vf;
        (i_f, i_t)
    }
}

impl Measure for RelayMeasure<'_> {
    fn time(&self) -> f64 {
        self.t
    }

    fn frame_speed(&self) -> f64 {
        self.sys.omega_sys(self.x)
    }

    fn bus_voltage(&self, b: usize) -> [C64; 3] {
        balanced(self.probe.bus_voltage(b))
    }

    fn branch_current(&self, e: usize, end: End) -> [C64; 3] {
        let (i_f, i_t) = self.branch_currents(e);
        balanced(if end == End::From { i_f } else { i_t })
    }

    fn unit_current(&self, u: usize) -> [C64; 3] {
        balanced(self.probe.unit_current(u))
    }

    /// Current into the bus's loads and shunts. The held units sit in the
    /// load's constant-current columns (`u_p`, `u_q`); they are units, not
    /// feeders, so their part is left out.
    fn feeder_current(&self, b: usize) -> [C64; 3] {
        let sys = self.sys;
        let v = self.probe.bus_voltage(b);
        let load = &sys.loads[b];
        let held = BusLoad {
            v0: load.v0,
            u_p: load.u_p,
            u_q: load.u_q,
            ..BusLoad::default()
        };
        let (ir, ii) = load.nonlinear_current(v.re, v.im, sys.low_voltage);
        let (hr, hi) = held.nonlinear_current(v.re, v.im, sys.low_voltage);
        let shunt = C64::new(sys.ac.gs[b], sys.ac.bs[b]);
        balanced((load.admittance() + shunt) * v + C64::new(ir - hr, ii - hi))
    }

    fn branch_end_voltage(&self, e: usize, end: End) -> Option<[C64; 3]> {
        let br = &self.sys.ac.branches;
        let b = if end == End::From {
            br.from_bus[e]
        } else {
            br.to_bus[e]
        };
        Some(balanced(self.probe.bus_voltage(b)))
    }

    fn end_closed(&self, e: usize, end: End) -> bool {
        self.sys.closed[e] && !self.sys.end_open[e][usize::from(end == End::To)]
    }

    fn unit_in_service(&self, u: usize) -> bool {
        let sys = self.sys;
        let nd = sys.devices.len();
        let ns = sys.sources.len();
        if u < nd {
            sys.devices[u].tripped.is_none()
        } else if u < nd + ns {
            sys.sources[u - nd].y != C64::new(0.0, 0.0)
        } else {
            !sys.held[u - nd - ns].tripped
        }
    }
}

/// Apply a relay's breaker action; returns its log text.
pub(crate) fn apply_action(sys: &mut DynSystem, x: &[f64], a: &Action, t: f64) -> String {
    match *a {
        Action::OpenEnd(e, end) | Action::CloseEnd(e, end) => {
            let k = usize::from(end == End::To);
            let open = matches!(a, Action::OpenEnd(..));
            sys.end_open[e][k] = open;
            let (f, to, c) = &sys.branch_keys[e];
            let at = if k == 0 { f } else { to };
            format!(
                "t = {t:.4} s: protection {} breaker of {f}-{to} circuit {c} at bus {at}",
                if open { "opened" } else { "reclosed" }
            )
        }
        Action::Bypass(e) | Action::Reinsert(e) => {
            let bypass = matches!(a, Action::Bypass(_));
            sys.bypassed[e] = bypass;
            let (f, to, c) = &sys.branch_keys[e];
            format!(
                "t = {t:.4} s: series capacitor {f}-{to} circuit {c} {}",
                if bypass {
                    "bypassed by its MOV"
                } else {
                    "reinserted"
                }
            )
        }
        Action::TripUnit(u) => {
            let nd = sys.devices.len();
            let ns = sys.sources.len();
            let (bus, id) = if u < nd {
                let d = &mut sys.devices[u];
                if d.tripped.is_none() {
                    d.tripped = Some(x[d.offset..d.offset + d.n_states()].to_vec());
                }
                (d.bus_number, d.machine_id.clone())
            } else if u < nd + ns {
                let s = &mut sys.sources[u - nd];
                s.y = C64::new(0.0, 0.0);
                s.e = C64::new(0.0, 0.0);
                (s.bus_number, s.machine_id.clone())
            } else {
                let k = u - nd - ns;
                trip_held(sys, k);
                (sys.held[k].bus_number, sys.held[k].machine_id.clone())
            };
            format!("t = {t:.4} s: protection tripped unit {bus} '{id}'")
        }
    }
}

/// Apply the series capacitor banks' bypass and reinsertion due in the state
/// `x` at `t` and re-solve the network, until none changes (at most three
/// rounds): the MOV acts before the relays measure. Returns the Newton
/// iterations.
pub(crate) fn settle_banks(
    r: &mut crate::protection::ProtectionRuntime,
    sys: &mut DynSystem,
    x: &mut Vec<f64>,
    f: &mut Vec<f64>,
    stepper: &mut super::integrate::Stepper,
    t: f64,
    applied: &mut Vec<String>,
) -> Result<usize, String> {
    let mut iterations = 0;
    for _ in 0..3 {
        let actions = r.bank_actions(&RelayMeasure::new(sys, x, t));
        if actions.is_empty() {
            break;
        }
        for a in &actions {
            applied.push(apply_action(sys, x, a, t));
        }
        sys.rebuild_ybus();
        sys.rebuild_mass();
        sys.residual(x, f, None);
        iterations += stepper
            .reinit(sys, x, f)
            .map_err(|e| format!("re-solving the network at t = {t:.4} s: {e}"))?;
    }
    Ok(iterations)
}
