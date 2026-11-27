//! The elements of a prepared RMS system, numbered as the engine numbers
//! them, and a view of the electrical quantities of a state.
//!
//! @block Topology and measurements

use powerio::BalancedNetwork;
use serde::{Deserialize, Serialize};

use super::models::Machine;
use super::system::{DynSystem, C64};

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
// @block Topology
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

    /// Impedance base at a dense bus, ohms: PowerIO's `kV² / MVA` (`NaN`
    /// when the system base is not positive and finite).
    #[must_use]
    pub fn z_base(&self, b: usize) -> f64 {
        let kv = self.base_kv.get(b).copied().unwrap_or(1.0).max(1e-9);
        powerio::electrical::calc_impedance_base(kv, self.sbase).unwrap_or(f64::NAN)
    }
}

/// The elements of a prepared system: devices, then sources, then held
/// (inverter) units.
// @block Number the elements
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
    let br = &ac.branches;
    let branches = (0..ac.m)
        .map(|e| {
            let (f, t) = (br.from_bus[e], br.to_bus[e]);
            let key = &sys.branch_keys[e];
            let y = C64::new(br.g[e], br.b[e]);
            let tap = C64::from_polar(br.tap[e], br.shift[e]);
            let rate = ac.rate_a[e];
            TopoBranch {
                from: f,
                to: t,
                from_number: key.0,
                to_number: key.1,
                circuit: key.2.clone(),
                // PowerIO's transformer test on the branch the column came
                // from (a stated tap or shift), the one `case_edit` reads.
                transformer: network
                    .branches()
                    .get(ac.branch_ids[e] - 1)
                    .is_some_and(powerio::network::Branch::is_transformer),
                rate: if rate.is_finite() && rate < 1e3 {
                    rate
                } else {
                    0.0
                },
                z: C64::new(1.0, 0.0) / y,
                y_fr: C64::new(br.g_fr[e], br.b_fr[e]),
                y_to: C64::new(br.g_to[e], br.b_to[e]),
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
// @block Measurements
pub(crate) struct RmsMeasure<'a> {
    sys: &'a DynSystem,
    x: &'a [f64],
}

impl<'a> RmsMeasure<'a> {
    pub(crate) fn new(sys: &'a DynSystem, x: &'a [f64]) -> Self {
        Self { sys, x }
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
            // This device alone, as the residual evaluates it (its state
            // derivatives go to scratch).
            let k = d.n_states();
            let mut scratch = [0.0; 64];
            let mut heap = Vec::new();
            let df: &mut [f64] = if k <= scratch.len() {
                &mut scratch[..k]
            } else {
                heap.resize(k, 0.0);
                &mut heap
            };
            let o = d.eval(
                &self.x[d.offset..d.offset + k],
                self.x[d.bus],
                self.x[self.sys.n + d.bus],
                self.sys.omega_sys(self.x),
                df,
            );
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
    #[cfg(feature = "rms-experimental")]
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
    sys.loads[b].u_p += s.re;
    sys.loads[b].u_q += s.im;
}

/// After an event: transient faults inside branches whose ends are both
/// open die out; an island whose only units in service are inverters or
/// held units (nothing that holds a voltage and a frequency) loses them, so
/// its buses go dead; and each island newly apart from the reference bus is
/// named once (it runs at its own frequency, which renewable plants do not
/// measure: they measure the reference's). A dead island (no unit in service)
/// also loses its load: its constant-power and constant-current parts have
/// no solution at 0 V.
// @block Switch off dead islands
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
    // Every island, the reference bus's too: one left without a synchronous
    // unit has nothing to hold its voltage.
    let reference = sys.ac.slacks.first().copied();
    for (island, text) in sys.unit_islands() {
        let inside = |b: usize| island.binary_search(&b).is_ok();
        let synchronous = sys
            .devices
            .iter()
            .any(|d| d.tripped.is_none() && inside(d.bus))
            || sys
                .sources
                .iter()
                .any(|s| s.y != C64::new(0.0, 0.0) && inside(s.bus));
        if !synchronous {
            let held: Vec<usize> = (0..sys.held.len())
                .filter(|&k| !sys.held[k].tripped && inside(sys.held[k].bus))
                .collect();
            for k in held {
                trip_held(sys, k);
            }
            out.push(format!(
                "t = {t:.4} s: island of {text} has no synchronous unit to hold its voltage; its units trip and its buses go dead"
            ));
        } else if reference.is_none_or(|r| island.binary_search(&r).is_err())
            && !sys.noted_islands.contains(&island)
        {
            out.push(format!(
                "t = {t:.4} s: island of {text}, apart from the reference bus: it runs at its own frequency, which its renewable plants do not measure"
            ));
            sys.noted_islands.push(island);
        }
    }
    // Experimental: a dead island (no unit in service) cannot feed its
    // loads. A constant-power or constant-current part has no solution at 0 V,
    // and the run would stop while re-solving the network; the island's demand
    // is lost with it.
    let dead = sys.dead_buses();
    let mut lost = 0.0;
    let mut buses = 0usize;
    for (b, _) in dead.iter().enumerate().take(sys.n).filter(|(_, &d)| d) {
        {
            let p = sys.loads[b].p0();
            if p != 0.0 || sys.loads[b].u_p != 0.0 || sys.loads[b].u_q != 0.0 {
                lost += p;
                buses += 1;
                sys.loads[b].scale(0.0, 0.0);
                sys.loads[b].u_p = 0.0;
                sys.loads[b].u_q = 0.0;
            }
        }
    }
    if buses > 0 {
        out.push(format!(
            "t = {t:.4} s: {buses} dead bus(es) lose their load ({:.1} MW at the power flow voltage)",
            lost * sys.sbase
        ));
    }
    out
}

/// Experimental: relief when the network cannot be solved in a
/// collapse. Level 0: every pocket of fewer than 40 buses apart from the largest island loses its units (its
/// load then goes with [`extinguish`]). Level 1: the bus the solver names
/// (`hint`, a bus number) loses its load and its units, as load that stalls
/// or drops out and units that trip on undervoltage. Level 2: that bus and
/// its neighbours. Level 3: every bus below 0.3 pu.
/// Returns what was done (empty: nothing to relieve).
// @block Relieve a collapse
pub(crate) fn relieve(
    sys: &mut DynSystem,
    x: &[f64],
    t: f64,
    level: usize,
    hint: Option<usize>,
) -> Vec<String> {
    let n = sys.n;
    let mut out = Vec::new();
    let islands = sys.islands();
    let main = islands
        .iter()
        .enumerate()
        .max_by_key(|(_, i)| i.len())
        .map(|(k, _)| k);
    let mut in_main = vec![false; n];
    if let Some(k) = main {
        for &b in &islands[k] {
            in_main[b] = true;
        }
    }
    let vm = |b: usize| (x[b] * x[b] + x[n + b] * x[n + b]).sqrt();
    let mut pick = vec![false; n];
    match level {
        0 => {
            // small pockets only (under 40 buses): a large island apart from the
            // largest one is a system
            for island in &islands {
                if island.len() < 40 && !island.iter().any(|&q| in_main[q]) {
                    for &q in island {
                        pick[q] = true;
                    }
                }
            }
        }
        1 | 2 => {
            let Some(hb) = hint.and_then(|num| sys.bus_numbers.iter().position(|&q| q == num))
            else {
                return out;
            };
            pick[hb] = true;
            if level == 2 {
                for e in 0..sys.ac.m {
                    let (a, b) = (sys.ac.branches.from_bus[e], sys.ac.branches.to_bus[e]);
                    if sys.closed[e] && a == hb {
                        pick[b] = true;
                    }
                    if sys.closed[e] && b == hb {
                        pick[a] = true;
                    }
                }
            }
        }
        _ => {
            for (b, p) in pick.iter_mut().enumerate().take(n) {
                *p = vm(b) < 0.3;
            }
        }
    }
    let mut units = 0usize;
    for d in sys.devices.iter_mut() {
        if d.tripped.is_none() && pick[d.bus] {
            d.tripped = Some(x[d.offset..d.offset + d.n_states()].to_vec());
            units += 1;
        }
    }
    for s in sys.sources.iter_mut() {
        if s.y != C64::new(0.0, 0.0) && pick[s.bus] {
            s.y = C64::new(0.0, 0.0);
            s.e = C64::new(0.0, 0.0);
            units += 1;
        }
    }
    let held: Vec<usize> = (0..sys.held.len())
        .filter(|&k| !sys.held[k].tripped && pick[sys.held[k].bus])
        .collect();
    units += held.len();
    for k in held {
        trip_held(sys, k);
    }
    let mut lost = 0.0;
    let mut buses = 0usize;
    let mut lowest = f64::INFINITY;
    if level > 0 {
        for (b, _) in pick.iter().enumerate().take(n).filter(|(_, &p)| p) {
            {
                let p = sys.loads[b].p0();
                if p != 0.0 || sys.loads[b].u_p != 0.0 || sys.loads[b].u_q != 0.0 {
                    lost += p;
                    buses += 1;
                    lowest = lowest.min(vm(b));
                    sys.loads[b].scale(0.0, 0.0);
                    sys.loads[b].u_p = 0.0;
                    sys.loads[b].u_q = 0.0;
                }
            }
        }
    }
    if units > 0 || buses > 0 {
        out.push(if level == 0 {
            format!("t = {t:.4} s: the network could not be solved; {units} unit(s) in small pockets apart from the largest island trip")
        } else {
            format!(
                "t = {t:.4} s: the network could not be solved (relief level {level}, bus {}); {units} unit(s) trip and {buses} bus(es) lose their load ({:.1} MW at the power flow voltage, lowest voltage {lowest:.2} pu)",
                hint.map_or_else(|| "-".to_owned(), |h| h.to_string()),
                lost * sys.sbase
            )
        });
    }
    out
}

/// The bus number a solver message names as carrying its largest residual
/// (`largest in bus 123 Vr`, or a unit's state `largest in G123 ...`).
// @block Bus named by a solver failure
pub(crate) fn hint_bus(message: &str) -> Option<usize> {
    for key in ["largest in bus ", "largest in G", "largest in R"] {
        if let Some(k) = message.find(key) {
            let digits: String = message[k + key.len()..]
                .chars()
                .take_while(char::is_ascii_digit)
                .collect();
            if let Ok(v) = digits.parse() {
                return Some(v);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{extinguish, hint_bus, relieve};
    use crate::rms::system::{DynSystem, C64};
    use crate::rms::{LoadModel, RmsSimulation};

    /// Five buses on 100 MVA: the infinite bus 1 feeds the load bus 3
    /// (50 MW), which feeds 4 (30 MW), which feeds 5 (10 MW and a unit);
    /// a unit at 2 feeds 3. Lines 1-3, 2-3, 3-4, 4-5.
    const CHAIN_RAW: &str = "0,   100.00,  32, 0, 1, 60.00     / chain


1,'B1', 230.0000,3,   1,   1,   1,1.00000,   0.0000
2,'B2', 230.0000,2,   1,   1,   1,1.00000,   0.0000
3,'B3', 230.0000,1,   1,   1,   1,1.00000,   0.0000
4,'B4', 230.0000,1,   1,   1,   1,1.00000,   0.0000
5,'B5', 230.0000,2,   1,   1,   1,1.00000,   0.0000
 0 /End of Bus data, Begin Load data
3,'1 ',1,   1,   1, 50.000, 10.000, 0.000, 0.000, 0.000, 0.000,   1,1
4,'1 ',1,   1,   1, 30.000, 5.000, 0.000, 0.000, 0.000, 0.000,   1,1
5,'1 ',1,   1,   1, 10.000, 2.000, 0.000, 0.000, 0.000, 0.000,   1,1
 0 /End of Load data, Begin Fixed shunt data
 0 /End of Fixed shunt data, Begin Generator data
1,'1 ', 0.000, 0.000, 9999.000, -9999.000,1.00000, 0, 100.000, 0.00000E+0, 1.00000E-5, 0.00000E+0, 0.00000E+0,1.00000,1,  100.0, 9999.000, -9999.000,   1,1.0000
2,'1 ', 30.000, 0.000, 9999.000, -9999.000,1.00000, 0, 100.000, 0.00000E+0, 3.00000E-1, 0.00000E+0, 0.00000E+0,1.00000,1,  100.0, 9999.000, -9999.000,   1,1.0000
5,'1 ', 20.000, 0.000, 9999.000, -9999.000,1.00000, 0, 100.000, 0.00000E+0, 3.00000E-1, 0.00000E+0, 0.00000E+0,1.00000,1,  100.0, 9999.000, -9999.000,   1,1.0000
 0 /End of Generator data, Begin Branch data
1, 3,'1 ', 0.00000E+0, 5.00000E-2, 0.00000, 9999.00, 9999.00, 9999.00, 0.00000, 0.00000, 0.00000, 0.00000,1,1, 0.00, 1,1.0000
2, 3,'1 ', 0.00000E+0, 5.00000E-2, 0.00000, 9999.00, 9999.00, 9999.00, 0.00000, 0.00000, 0.00000, 0.00000,1,1, 0.00, 1,1.0000
3, 4,'1 ', 0.00000E+0, 5.00000E-2, 0.00000, 9999.00, 9999.00, 9999.00, 0.00000, 0.00000, 0.00000, 0.00000,1,1, 0.00, 1,1.0000
4, 5,'1 ', 0.00000E+0, 5.00000E-2, 0.00000, 9999.00, 9999.00, 9999.00, 0.00000, 0.00000, 0.00000, 0.00000,1,1, 0.00, 1,1.0000
 0 /End of Branch data, Begin Transformer data
 0 /End of Transformer data, Begin Area interchange data
 0 /End of Area interchange data, Begin Two-terminal dc line data
 0 /End of Two-terminal dc line data, Begin VSC dc line data
 0 /End of VSC dc line data, Begin Impedance correction table data
 0 /End of Impedance correction table data, Begin Multi-terminal dc line data
 0 /End of Multi-terminal dc line data, Begin Multi-section line data
 0 /End of Multi-section line data, Begin Zone data
 0 /End of Zone data, Begin Inter-area transfer data
 0 /End of Inter-area transfer data, Begin Owner data
 0 /End of Owner data, Begin FACTS device data
 0 /End of FACTS device data, Begin Switched shunt data
 0 /End of Switched shunt data, Begin GNE device data
 0 /End of GNE device data
Q
";

    /// The chain's system (classical machines at 2 and 5, the infinite bus a
    /// source) and its start state.
    fn chain() -> (DynSystem, Vec<f64>) {
        let src = powerio::Source::from_memory("case.raw", CHAIN_RAW.as_bytes().to_vec()).unwrap();
        let module = powerio::parse(src).unwrap();
        let powerio::PioValue::BalancedNetwork(net) = module.value() else {
            panic!("not a balanced network")
        };
        let dyr = powerio::parse_dyr(
            "1 'GENCLS' 1 0.0 0.0 /\n2 'GENCLS' 1 3.0 0.0 /\n5 'GENCLS' 1 3.0 0.0 /\n",
        )
        .unwrap();
        let sim = RmsSimulation::new(net, &dyr, LoadModel::ConstantPower).unwrap();
        (sim.sys.clone(), sim.x0.clone())
    }

    /// Opens the branch `from`-`to` (bus numbers).
    fn open(sys: &mut DynSystem, from: usize, to: usize) {
        let e = sys
            .branch_keys
            .iter()
            .position(|(a, b, _)| (*a, *b) == (from, to))
            .unwrap();
        sys.closed[e] = false;
        sys.rebuild_ybus();
    }

    fn load_mw(sys: &DynSystem, bus: usize) -> f64 {
        sys.loads[sys.dense_bus(bus).unwrap()].p0() * sys.sbase
    }

    fn unit_tripped(sys: &DynSystem, bus: usize) -> bool {
        sys.devices
            .iter()
            .find(|d| d.bus_number == bus)
            .unwrap()
            .tripped
            .is_some()
    }

    /// The load (MW) at buses 3, 4, 5 and whether the units at 2 and 5 and
    /// the source at 1 are out.
    fn state(sys: &DynSystem) -> ([f64; 3], [bool; 3]) {
        (
            [load_mw(sys, 3), load_mw(sys, 4), load_mw(sys, 5)],
            [
                sys.sources[0].y == C64::new(0.0, 0.0),
                unit_tripped(sys, 2),
                unit_tripped(sys, 5),
            ],
        )
    }

    const INTACT: ([f64; 3], [bool; 3]) = ([50.0, 30.0, 10.0], [false, false, false]);

    fn assert_state(sys: &DynSystem, expected: ([f64; 3], [bool; 3]), what: &str) {
        let (loads, out) = state(sys);
        for (l, e) in loads.iter().zip(expected.0) {
            assert!(
                (l - e).abs() < 1e-9,
                "{what}: loads {loads:?}, expected {:?}",
                expected.0
            );
        }
        assert_eq!(
            out, expected.1,
            "{what}: units out (source 1, unit 2, unit 5)"
        );
    }

    /// The bus a solver failure names: its largest residual in a bus row
    /// (`bus 123 Vr`), a unit's state (`G123 …`) or a plant's (`R123 …`);
    /// nothing when no number follows.
    #[test]
    fn hint_bus_reads_the_bus_of_the_largest_residual() {
        let fail = |what: &str| {
            format!("the Newton step does not lower the residual 1.2e-3 (largest in {what})")
        };
        assert_eq!(hint_bus(&fail("bus 123 Vr")), Some(123));
        assert_eq!(hint_bus(&fail("bus 7 Vi")), Some(7));
        assert_eq!(hint_bus(&fail("G4567 'x' omega")), Some(4567));
        assert_eq!(hint_bus(&fail("G88 psi_d")), Some(88));
        assert_eq!(hint_bus(&fail("R31 inv.Iq")), Some(31));
        assert_eq!(hint_bus(&fail("bus Vr")), None);
        assert_eq!(hint_bus(&fail("Gx")), None);
        assert_eq!(
            hint_bus("the Newton step is not finite (singular Jacobian?)"),
            None
        );
        assert_eq!(hint_bus(""), None);
        // A bus row takes precedence over a unit named later in the text.
        assert_eq!(
            hint_bus("largest in G5 delta; largest in bus 9 Vr"),
            Some(9)
        );
    }

    /// Level 0 trips the units of small pockets apart from the largest
    /// island and nothing else (their load goes with `extinguish`, which
    /// then names the dead buses); with one island it does nothing.
    #[test]
    fn relief_level_0_trips_small_pockets() {
        let (mut sys, x) = chain();
        assert!(relieve(&mut sys, &x, 1.0, 0, None).is_empty());
        assert_state(&sys, INTACT, "one island");
        open(&mut sys, 4, 5);
        let notes = relieve(&mut sys, &x, 1.0, 0, Some(3));
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert!(notes[0].contains("1 unit(s) in small pockets"), "{notes:?}");
        assert_state(&sys, ([50.0, 30.0, 10.0], [false, false, true]), "level 0");
        let notes = extinguish(&mut sys, 1.0);
        assert!(
            notes
                .iter()
                .any(|n| n.contains("1 dead bus(es) lose their load (10.0 MW")),
            "{notes:?}"
        );
        assert_state(
            &sys,
            ([50.0, 30.0, 0.0], [false, false, true]),
            "after extinguish",
        );
    }

    /// Level 1 takes the named bus: its load and its units; a hint that is
    /// missing or names no bus of the case does nothing.
    #[test]
    fn relief_level_1_takes_the_named_bus() {
        let (sys0, x) = chain();
        let mut sys = sys0.clone();
        assert!(relieve(&mut sys, &x, 1.0, 1, None).is_empty());
        assert!(relieve(&mut sys, &x, 1.0, 1, Some(99)).is_empty());
        assert_state(&sys, INTACT, "no bus named");
        let notes = relieve(&mut sys, &x, 1.0, 1, Some(4));
        assert!(
            notes[0].contains(
                "(relief level 1, bus 4); 0 unit(s) trip and 1 bus(es) lose their load (30.0 MW"
            ),
            "{notes:?}"
        );
        assert_state(&sys, ([50.0, 0.0, 10.0], [false, false, false]), "bus 4");
        let mut sys = sys0.clone();
        let notes = relieve(&mut sys, &x, 1.0, 1, Some(5));
        assert!(
            notes[0].contains("1 unit(s) trip and 1 bus(es)"),
            "{notes:?}"
        );
        assert_state(&sys, ([50.0, 30.0, 0.0], [false, false, true]), "bus 5");
        let mut sys = sys0;
        let notes = relieve(&mut sys, &x, 1.0, 1, Some(2));
        assert!(
            notes[0].contains("1 unit(s) trip and 0 bus(es)"),
            "{notes:?}"
        );
        assert_state(&sys, ([50.0, 30.0, 10.0], [false, true, false]), "bus 2");
    }

    /// Level 2 takes the named bus and its neighbours over closed branches.
    #[test]
    fn relief_level_2_takes_the_neighbours_too() {
        let (sys0, x) = chain();
        let mut sys = sys0.clone();
        let notes = relieve(&mut sys, &x, 1.0, 2, Some(4));
        assert!(
            notes[0].contains("1 unit(s) trip and 3 bus(es)"),
            "{notes:?}"
        );
        assert_state(
            &sys,
            ([0.0, 0.0, 0.0], [false, false, true]),
            "bus 4 and neighbours",
        );
        let mut sys = sys0.clone();
        open(&mut sys, 4, 5);
        relieve(&mut sys, &x, 1.0, 2, Some(4));
        assert_state(&sys, ([0.0, 0.0, 10.0], [false, false, false]), "4-5 open");
        let mut sys = sys0;
        relieve(&mut sys, &x, 1.0, 2, Some(3));
        assert_state(
            &sys,
            ([0.0, 0.0, 10.0], [true, true, false]),
            "bus 3 and neighbours",
        );
    }

    /// Level 3 takes every bus below 0.3 pu, whatever the hint; at normal
    /// voltages it does nothing.
    #[test]
    fn relief_level_3_takes_the_buses_below_0_3_pu() {
        let (mut sys, x) = chain();
        assert!(relieve(&mut sys, &x, 1.0, 3, Some(4)).is_empty());
        assert_state(&sys, INTACT, "normal voltages");
        let mut low = x.clone();
        let (n, b) = (sys.n, sys.dense_bus(4).unwrap());
        low[b] *= 0.25;
        low[n + b] *= 0.25;
        let notes = relieve(&mut sys, &low, 1.0, 3, Some(3));
        assert!(notes[0].contains("relief level 3"), "{notes:?}");
        assert_state(
            &sys,
            ([50.0, 0.0, 10.0], [false, false, false]),
            "bus 4 at 0.25 pu",
        );
    }

    /// An island left without a unit in service loses its load (constant
    /// power has no solution at 0 V), once; an island with a unit keeps it
    /// and is named once as apart from the reference bus.
    #[test]
    fn extinguish_takes_the_load_of_a_dead_island() {
        let (mut sys, _) = chain();
        assert!(extinguish(&mut sys, 1.0).is_empty());
        open(&mut sys, 3, 4);
        let notes = extinguish(&mut sys, 1.0);
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert!(
            notes[0].contains("apart from the reference bus"),
            "{notes:?}"
        );
        assert!(extinguish(&mut sys, 1.0).is_empty(), "named once");
        assert_state(&sys, INTACT, "a live island");
        let d = sys.devices.iter().position(|d| d.bus_number == 5).unwrap();
        sys.devices[d].tripped = Some(vec![0.0; sys.devices[d].n_states()]);
        let notes = extinguish(&mut sys, 1.0);
        assert!(
            notes
                .iter()
                .any(|n| n.contains("2 dead bus(es) lose their load (40.0 MW")),
            "{notes:?}"
        );
        assert_state(
            &sys,
            ([50.0, 0.0, 0.0], [false, false, true]),
            "a dead island",
        );
        assert!(
            extinguish(&mut sys, 1.0).is_empty(),
            "a dead island loses its load once"
        );
    }
}
