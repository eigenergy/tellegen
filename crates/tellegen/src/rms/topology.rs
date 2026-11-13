//! The elements of a prepared RMS system, numbered as the engine numbers
//! them, and a view of the electrical quantities of a state.

use powerio::BalancedNetwork;
use serde::{Deserialize, Serialize};

use super::models::Machine;
use super::system::{DynSystem, GenOut, C64};

/// Which end of a branch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum End {
    From,
    To,
}

/// `C64` as `[re, im]`.
mod c64_pair {
    use super::C64;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(z: &C64, s: S) -> Result<S::Ok, S::Error> {
        [z.re, z.im].serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<C64, D::Error> {
        let [re, im] = <[f64; 2]>::deserialize(d)?;
        Ok(C64::new(re, im))
    }
}

/// A branch of the simulated network (system per unit).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TopoBranch {
    pub from: usize,
    pub to: usize,
    pub from_number: usize,
    pub to_number: usize,
    pub circuit: String,
    pub transformer: bool,
    /// Rating, system per unit (0 when unrated).
    pub rate: f64,
    /// Series impedance.
    #[serde(with = "c64_pair")]
    pub z: C64,
    #[serde(with = "c64_pair")]
    pub y_fr: C64,
    #[serde(with = "c64_pair")]
    pub y_to: C64,
    /// Complex tap at the from end.
    #[serde(with = "c64_pair")]
    pub tap: C64,
}

/// What kind of generating unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnitKind {
    Machine,
    Source,
    Inverter,
}

/// A generating unit of the simulated network.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TopoUnit {
    pub bus: usize,
    pub bus_number: usize,
    pub machine_id: String,
    /// MVA base.
    pub mbase: f64,
    pub kind: UnitKind,
    /// Synchronous, transient and subtransient d-axis reactance on the
    /// machine base (synchronous machines).
    pub xd: Option<f64>,
    pub xd_p: Option<f64>,
    pub xd_pp: Option<f64>,
    /// Active power at the operating point, system pu.
    pub p0: f64,
}

/// The simulation's elements, dense indices as the engine numbers them.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Topology {
    pub sbase: f64,
    pub f0: f64,
    pub bus_numbers: Vec<usize>,
    pub base_kv: Vec<f64>,
    pub branches: Vec<TopoBranch>,
    pub units: Vec<TopoUnit>,
}

impl Topology {
    /// Dense index of a bus.
    ///
    /// # Errors
    /// When the case has no such bus.
    pub fn bus(&self, number: usize) -> Result<usize, String> {
        self.bus_numbers
            .iter()
            .position(|&b| b == number)
            .ok_or_else(|| format!("no bus {number}"))
    }

    /// Dense index of a branch.
    ///
    /// # Errors
    /// When the case has no such branch.
    pub fn branch(&self, from: usize, to: usize, circuit: &str) -> Result<usize, String> {
        self.branches
            .iter()
            .position(|b| {
                ((b.from_number == from && b.to_number == to)
                    || (b.from_number == to && b.to_number == from))
                    && b.circuit.trim() == circuit.trim()
            })
            .ok_or_else(|| format!("no branch {from}-{to} circuit {circuit}"))
    }

    /// Dense index of a generating unit.
    ///
    /// # Errors
    /// When the case has no such unit.
    pub fn unit(&self, bus: usize, id: &str) -> Result<usize, String> {
        self.units
            .iter()
            .position(|u| u.bus_number == bus && u.machine_id.trim() == id.trim())
            .ok_or_else(|| format!("no generating unit {bus} '{id}'"))
    }

    /// Impedance base at a dense bus, ohms.
    #[must_use]
    pub fn z_base(&self, b: usize) -> f64 {
        let kv = self.base_kv.get(b).copied().unwrap_or(1.0).max(1e-9);
        kv * kv / self.sbase
    }
}

/// The elements of a prepared system: devices, then sources, then held
/// (inverter) units.
pub(crate) fn topology(network: &BalancedNetwork, sys: &DynSystem, x0: &[f64]) -> Topology {
    let base_kv: Vec<f64> = sys
        .bus_numbers
        .iter()
        .map(|&n| {
            network
                .buses()
                .iter()
                .find(|b| b.id.0 == n)
                .map_or(1.0, |b| b.base_kv)
        })
        .collect();
    let ac = &sys.ac;
    let branches = (0..ac.m)
        .map(|e| {
            let (f, t) = (ac.br_from[e], ac.br_to[e]);
            let key = &sys.branch_keys[e];
            let y = C64::new(ac.g[e], ac.b[e]);
            let tap = C64::from_polar(ac.tap[e], ac.shift[e]);
            let rate = ac.rate_a[e];
            TopoBranch {
                from: f,
                to: t,
                from_number: key.0,
                to_number: key.1,
                circuit: key.2.clone(),
                transformer: (ac.tap[e] - 1.0).abs() > 1e-9
                    || ac.shift[e].abs() > 1e-9
                    || (base_kv[f] - base_kv[t]).abs() > 1e-6,
                rate: if rate.is_finite() && rate < 1e3 {
                    rate
                } else {
                    0.0
                },
                z: C64::new(1.0, 0.0) / y,
                y_fr: C64::new(ac.g_fr[e], ac.b_fr[e]),
                y_to: C64::new(ac.g_to[e], ac.b_to[e]),
                tap,
            }
        })
        .collect();
    let m0 = RmsMeasure::new(sys, x0);
    let p0 = |u: usize| {
        let b = m0.sys.devices.get(u).map(|d| d.bus);
        let v = b.map_or(C64::new(1.0, 0.0), |b| m0.bus_voltage(b));
        (v * m0.unit_current(u).conj()).re
    };
    let mut units: Vec<TopoUnit> = sys
        .devices
        .iter()
        .enumerate()
        .map(|(k, d)| {
            let (xd, xd_p, xd_pp) = match &d.machine {
                Machine::Classic { xd_p, .. } => (None, Some(*xd_p), Some(*xd_p)),
                Machine::RoundRotor(m) => (Some(m.xd), Some(m.xd_p), Some(m.xd_pp)),
                Machine::SalientPole(m) => (Some(m.xd), Some(m.xd_p), Some(m.xd_pp)),
            };
            TopoUnit {
                bus: d.bus,
                bus_number: d.bus_number,
                machine_id: d.machine_id.clone(),
                mbase: d.scale * sys.sbase,
                kind: UnitKind::Machine,
                xd,
                xd_p,
                xd_pp,
                p0: p0(k),
            }
        })
        .collect();
    units.extend(sys.sources.iter().map(|s| TopoUnit {
        bus: s.bus,
        bus_number: s.bus_number,
        machine_id: s.machine_id.clone(),
        mbase: sys.sbase,
        kind: UnitKind::Source,
        xd: None,
        xd_p: None,
        xd_pp: Some((C64::new(1.0, 0.0) / s.y).im.abs()),
        p0: {
            let v = C64::new(x0[s.bus], x0[sys.n + s.bus]);
            (v * (s.y * (s.e - v)).conj()).re
        },
    }));
    units.extend(sys.held.iter().map(|h| TopoUnit {
        bus: h.bus,
        bus_number: h.bus_number,
        machine_id: h.machine_id.clone(),
        mbase: h.mbase,
        kind: UnitKind::Inverter,
        xd: None,
        xd_p: None,
        xd_pp: None,
        p0: h.s.re,
    }));
    Topology {
        sbase: sys.sbase,
        f0: sys.f0,
        bus_numbers: sys.bus_numbers.clone(),
        base_kv,
        branches,
        units,
    }
}

/// Bus voltages and unit currents of the state `x` at `t` (system per unit,
/// units in [`Topology`] order).
pub(crate) struct RmsMeasure<'a> {
    sys: &'a DynSystem,
    x: &'a [f64],
    outs: Vec<GenOut>,
}

impl<'a> RmsMeasure<'a> {
    pub(crate) fn new(sys: &'a DynSystem, x: &'a [f64]) -> Self {
        let mut f = vec![0.0; sys.nx];
        let mut outs = Vec::new();
        sys.residual(x, &mut f, Some(&mut outs));
        Self { sys, x, outs }
    }

    /// Voltage at a dense bus.
    pub(crate) fn bus_voltage(&self, b: usize) -> C64 {
        C64::new(self.x[b], self.x[self.sys.n + b])
    }

    /// Current out of the unit into its bus (zero when tripped).
    pub(crate) fn unit_current(&self, u: usize) -> C64 {
        let sys = self.sys;
        let nd = sys.devices.len();
        let ns = sys.sources.len();
        if u < nd {
            let d = &sys.devices[u];
            if d.tripped.is_some() {
                return C64::new(0.0, 0.0);
            }
            let o = self.outs[u];
            C64::new(o.ir, o.ii)
        } else if u < nd + ns {
            let s = &sys.sources[u - nd];
            s.y * (s.e - self.bus_voltage(s.bus))
        } else {
            let h = &sys.held[u - nd - ns];
            h.current_at(self.x, self.bus_voltage(h.bus))
        }
    }

    /// Whether the unit is connected.
    pub(crate) fn unit_in_service(&self, u: usize) -> bool {
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

/// Take a held unit off its bus.
pub(crate) fn trip_held(sys: &mut DynSystem, k: usize) {
    let h = &mut sys.held[k];
    if h.tripped {
        return;
    }
    h.tripped = true;
    if h.plant.is_some() {
        // The plant's states hold (its mass rows change).
        sys.rebuild_mass();
        return;
    }
    let (b, s) = (h.bus, h.s);
    sys.loads[b].p_i += s.re;
    sys.loads[b].q_i += s.im;
}

/// Transient faults inside branches whose ends are both open die out.
pub(crate) fn extinguish(sys: &mut DynSystem, t: f64) -> Vec<String> {
    let mut out = Vec::new();
    for e in 0..sys.ac.m {
        if let Some(f) = sys.branch_faults[e] {
            if !f.permanent && sys.branch_isolated(e) {
                sys.branch_faults[e] = None;
                let (a, b, c) = &sys.branch_keys[e];
                out.push(format!(
                    "t = {t:.4} s: transient fault inside {a}-{b} circuit {c} extinguished (line dead)"
                ));
            }
        }
    }
    out
}
