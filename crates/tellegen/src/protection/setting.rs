//! Protection settings from the network: a short-circuit calculation, the
//! setting rules practitioners apply, and an audit of a scheme.
//!
//! Short circuits follow the IEC 60909 method in its simplest form:
//! the equivalent voltage source `c U_n` at every synchronous unit behind
//! its subtransient reactance, loads and line charging neglected,
//! inverter-based units contributing no current (the conservative choice
//! for minimum fault levels).
//!
//! The rules (each relay carries its basis in [`AutoSetting::notes`]):
//!
//! - Distance (IEC 60255-121 functions; the reach rules of common published
//!   practice, as in IEEE C37.113), on what the relay measures for a fault at the
//!   far end of its line (the short circuit, so line charging and shunt
//!   reactors count): Z1 85 % of the shorter of that and the line,
//!   instantaneous; Z2 the largest of 120 % of the longer, and the line plus
//!   half the shortest adjacent branch, 0.4 s; Z3 the line plus 120 % of the
//!   longest adjacent branch (at least 120 % of Z2), 1.0 s; a
//!   Z2 that sees past an adjacent line's Z1 (also through a series
//!   capacitor) graded 0.25 s above that line's Z2, to a fixed point; a
//!   load-encroachment blinder when the zones reach the load impedance (0.9
//!   pu voltage, rated power, ±30°); a reverse Z4 of 25 % of the line, 1.5 s; POTT on
//!   Z2; power swing blocking; `k0 = 0`, matching the network model's
//!   uncoupled phases (`Z0 = Z1`); a wrong `k0` makes the earth loops over-
//!   or underreach.
//!   A series capacitor (negative series reactance) gets no line relay; the
//!   lines next to it keep Z1 short of a fault beyond it, and their zones
//!   are polarized with the pre-fault voltage, so the capacitor's voltage
//!   inversion does not turn them (IEEE C37.116).//! - Line differential 87L above three times the charging current;
//!   backup 67 (directional, IEC standard inverse, 1.5 In) graded 0.3 s above Z3 at the
//!   remote-bus fault current, at least 0.3 s above Z2 at any current; 67N (needs unbalance, not RMS); 50BF 150 ms; three-pole
//!   auto-reclose 0.5 s dead time on Z1, POTT and 87L.
//! - Transformer: 87T (0.3 In, slopes 0.3/0.7, second-harmonic blocking
//!   15 %, unrestrained above 1.25 × the largest through-fault current),
//!   51 graded to 0.8 s at the largest through-fault current, 49 (τ 30
//!   min), 24 at 1.1 pu V/Hz.
//! - Busbar: 87B (0.8 × the largest feeder rating, slope 0.5) on buses with
//!   three or more feeders.
//! - Synchronous units (IEEE C37.102 practice): 87U across a step-up
//!   transformer the unit has alone; 40 two-zone offset mho (offset X'd/2, diameters 1.0 pu
//!   and Xd, 0.1/0.5 s); 32 at 2 % for units that generate; 78 blinders at
//!   `(X'd + Xs)/2 · tan 30°` (120° critical angle); 81U/81O at 95 % /
//!   103 % of the nominal frequency (47.5/51.5 Hz at 50 Hz), 1 s; 24 at
//!   1.18 pu, 2 s; 27/59; 46 (I2 8 %, K 10); 49.
//! - A branch without a rating (none in the case, or one above 9000 MVA,
//!   a placeholder) is taken as rated for the typical thermal rating of
//!   its voltage level ([`typical_rating`]), at least 125 % of its flow at
//!   the operating point ([`estimate_ratings`]): the relays set on it carry
//!   that rated current ([`carry_ratings`]). Without one, 1 pu on the system
//!   base would stand in, far below the load of a transmission line, and
//!   its overcurrent and thermal stages would operate at rest.
//! - Inverter interface protection: the type D power park module ride-through
//!   profile of Commission Regulation (EU) 2016/631 (RfG), Art. 16(3)(a),
//!   Table 7.2
//!   (0 pu for 150 ms, then linear to 0.85 pu at 1.5 s); 27 0.85 pu 3 s;
//!   59 1.1 pu 60 s and 1.2 pu 0.1 s; 81U/81O at 95 %/103 % of the nominal
//!   frequency, 0.1 s; RoCoF 2 Hz/s over 500 ms (ENTSO-E's RoCoF
//!   withstand guidance; not an RfG value), blocked below 0.8 pu and its
//!   window restarted after a dip, so a phase jump does not read as a rate
//!   (this module's choice). These are indicative
//!   values from the published ranges; the grid operator sets the actual
//!   ones.

use serde::{Deserialize, Serialize};

use super::{
    AutoReclose, BreakerFailure, Characteristic, Curve, Differential, Direction, Distance, End,
    FaultRideThrough, FrequencyStage, LoadEncroachment, LossOfField, NegativeSequence, OutOfStep,
    Overcurrent, PowerSwingBlocking, ProtectionFunction, ProtectionScheme, Relay, RelayLocation,
    ReversePower, Rocof, SchemeKind, SeriesCapacitor, Teleprotection, Thermal, Topology, UnitKind,
    VoltageStage, Zone, C64,
};

// ---------------------------------------------------------------------------
// Short circuit
// ---------------------------------------------------------------------------

/// Where a short circuit sits.
#[derive(Clone, Copy, Debug)]
pub enum FaultPoint {
    /// Dense bus.
    Bus(usize),
    /// Inside branch `e` at `alpha` of its series impedance from its from
    /// end.
    Branch(usize, f64),
}

/// Voltages and branch end currents during a balanced fault.
#[derive(Clone, Debug)]
pub struct ShortCircuit {
    pub v: Vec<C64>,
    /// Currents into each branch at `(from, to)`.
    pub branch: Vec<(C64, C64)>,
    pub fault_current: C64,
}

/// Balanced short circuits on a topology (IEC 60909 equivalent source).
pub struct FaultCalc<'a> {
    topo: &'a Topology,
    /// Voltage factor `c` (1.1 for maximum, 1.0 for minimum currents).
    pub c: f64,
    /// The intact network factored once, with its impedance columns as
    /// faults ask for them: a fault on the intact network is then
    /// superposition, O(n²) instead of a new O(n³) solve.
    base: std::sync::Mutex<Option<Base>>,
}

struct Base {
    c: f64,
    lu: NetLu,
    v0: Vec<C64>,
    cols: std::collections::HashMap<usize, Vec<C64>>,
    /// Per branch the coefficients of its end currents,
    /// `I_from = a V_from − b V_to`, `I_to = c V_to − d V_from`: every
    /// fault evaluates them for every branch.
    ends: Vec<[C64; 4]>,
}

/// The entries of a network matrix (repeated entries add up).
type Entries = Vec<(usize, usize, C64)>;

/// A sparse complex LU of a network matrix: a fault calculation of a
/// continental network solves hundreds of right-hand sides, each a few
/// thousand times cheaper here than through dense factors.
struct NetLu {
    lu: faer::sparse::linalg::solvers::Lu<usize, C64>,
}

impl NetLu {
    fn factor(n: usize, entries: &[(usize, usize, C64)]) -> Result<Self, String> {
        let singular = || "singular network matrix (a part without a source?)".to_owned();
        let trip: Vec<faer::sparse::Triplet<usize, usize, C64>> = entries
            .iter()
            .map(|&(r, c, v)| faer::sparse::Triplet::new(r, c, v))
            .collect();
        let mat = faer::sparse::SparseColMat::<usize, C64>::try_new_from_triplets(n, n, &trip)
            .map_err(|e| format!("network matrix: {e:?}"))?;
        Ok(Self {
            lu: mat.sp_lu().map_err(|_| singular())?,
        })
    }

    fn solve(&self, b: &[C64]) -> Result<Vec<C64>, String> {
        use faer::linalg::solvers::Solve;
        let mut x = faer::Mat::<C64>::from_fn(b.len(), 1, |i, _| b[i]);
        self.lu.solve_in_place(x.as_mut());
        let x: Vec<C64> = (0..b.len()).map(|i| x[(i, 0)]).collect();
        if x.iter().all(|v| v.re.is_finite() && v.im.is_finite()) {
            Ok(x)
        } else {
            Err("singular network matrix (a part without a source?)".into())
        }
    }
}

impl<'a> FaultCalc<'a> {
    #[must_use]
    pub fn new(topo: &'a Topology) -> Self {
        Self {
            topo,
            c: 1.1,
            base: std::sync::Mutex::new(None),
        }
    }

    /// The intact network's admittance matrix and source injections.
    fn base_matrix(&self) -> (Entries, Vec<C64>) {
        let topo = self.topo;
        let n = topo.bus_numbers.len();
        let zero = C64::new(0.0, 0.0);
        let mut y: Entries = Vec::with_capacity(4 * topo.branches.len() + 2 * n);
        let mut rhs = vec![zero; n];
        for br in &topo.branches {
            let ys = C64::new(1.0, 0.0) / br.z;
            let t = br.tap;
            let t2 = t.norm_sqr();
            y.push((br.from, br.from, (ys + br.y_fr) / t2));
            y.push((br.from, br.to, -(ys / t.conj())));
            y.push((br.to, br.from, -(ys / t)));
            y.push((br.to, br.to, ys + br.y_to));
        }
        for (u, unit) in topo.units.iter().enumerate() {
            let yu = self.unit_y(u);
            y.push((unit.bus, unit.bus, yu));
            rhs[unit.bus] += yu * self.c;
        }
        for k in 0..n {
            y.push((k, k, C64::new(1e-9, 0.0)));
        }
        (y, rhs)
    }

    /// A fault on the intact network by superposition on the factored
    /// matrix; `None` when the fault point needs the full solve (a fault
    /// inside a transformer).
    fn solve_fast(&self, at: Option<FaultPoint>, zf: C64) -> Result<Option<ShortCircuit>, String> {
        let topo = self.topo;
        if let Some(FaultPoint::Branch(e, _)) = at {
            let t = topo.branches[e].tap;
            if (t - C64::new(1.0, 0.0)).norm() > 1e-12 {
                return Ok(None);
            }
        }
        let mut cell = self
            .base
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if cell.as_ref().is_none_or(|b| b.c != self.c) {
            let (y, rhs) = self.base_matrix();
            let lu = NetLu::factor(rhs.len(), &y)?;
            let v0 = lu.solve(&rhs)?;
            let ends = topo
                .branches
                .iter()
                .map(|br| {
                    let ys = C64::new(1.0, 0.0) / br.z;
                    let t = br.tap;
                    let t2 = t.norm_sqr();
                    [(ys + br.y_fr) / t2, ys / t.conj(), ys + br.y_to, ys / t]
                })
                .collect();
            *cell = Some(Base {
                c: self.c,
                lu,
                v0,
                cols: std::collections::HashMap::new(),
                ends,
            });
        }
        let base = cell.as_mut().expect("set above");
        let n = topo.bus_numbers.len();
        let zero = C64::new(0.0, 0.0);
        let mut col = |k: usize| -> Result<Vec<C64>, String> {
            if !base.cols.contains_key(&k) {
                let mut e = vec![zero; n];
                e[k] = C64::new(1.0, 0.0);
                let z = base.lu.solve(&e)?;
                base.cols.insert(k, z);
            }
            Ok(base.cols[&k].clone())
        };
        let zf = if zf.norm() < 1e-6 {
            C64::new(0.0, 1e-6)
        } else {
            zf
        };
        // v = v0 − z_F I_f, with z_F the network's response to a unit
        // injection at the fault point.
        let (z_f, z_ff, v_f0, inner) = match at {
            None => (vec![zero; n], C64::new(1.0, 0.0), zero, None),
            Some(FaultPoint::Bus(b)) => {
                let z = col(b)?;
                let zbb = z[b];
                (z, zbb, base.v0[b], None)
            }
            Some(FaultPoint::Branch(e, alpha)) => {
                let alpha = alpha.clamp(1e-4, 1.0 - 1e-4);
                let br = &topo.branches[e];
                let (zi, zj) = (col(br.from)?, col(br.to)?);
                // A point inside a series element: the injection splits
                // (1 − α) to the from bus and α to the to bus.
                let z: Vec<C64> = (0..n)
                    .map(|k| zi[k] * (1.0 - alpha) + zj[k] * alpha)
                    .collect();
                let zff = zi[br.from] * (1.0 - alpha).powi(2)
                    + zj[br.to] * alpha.powi(2)
                    + zi[br.to] * (2.0 * alpha * (1.0 - alpha))
                    + br.z * (alpha * (1.0 - alpha));
                let vf0 = base.v0[br.from] * (1.0 - alpha) + base.v0[br.to] * alpha;
                (z, zff, vf0, Some((e, alpha)))
            }
        };
        let i_f = if at.is_some() {
            v_f0 / (z_ff + zf)
        } else {
            zero
        };
        let v: Vec<C64> = (0..n).map(|k| base.v0[k] - z_f[k] * i_f).collect();
        let branch = topo
            .branches
            .iter()
            .enumerate()
            .map(|(e, br)| {
                let (vf, vt) = (v[br.from], v[br.to]);
                if let Some((fe, alpha)) = inner {
                    if fe == e {
                        let ys = C64::new(1.0, 0.0) / br.z;
                        let vp = v_f0 - z_ff * i_f;
                        return (
                            br.y_fr * vf + (vf - vp) * ys / alpha,
                            br.y_to * vt + (vt - vp) * ys / (1.0 - alpha),
                        );
                    }
                }
                let [a, b, c, d] = base.ends[e];
                (a * vf - b * vt, c * vt - d * vf)
            })
            .collect();
        Ok(Some(ShortCircuit {
            v,
            branch,
            fault_current: i_f,
        }))
    }

    #[must_use]
    pub fn topo(&self) -> &'a Topology {
        self.topo
    }

    /// Admittance of a unit to ground, system per unit.
    fn unit_y(&self, u: usize) -> C64 {
        let unit = &self.topo.units[u];
        if unit.kind == UnitKind::Inverter {
            return C64::new(0.0, 0.0);
        }
        let x = unit.xd_pp.unwrap_or(0.2).max(1e-4);
        let scale = unit.mbase / self.topo.sbase;
        C64::new(0.0, -scale / x)
    }

    /// Solve one fault through impedance `zf` with the branches in `out`
    /// open.
    ///
    /// # Errors
    /// When the network matrix is singular (an island without a source).
    pub fn solve(&self, at: FaultPoint, zf: C64, out: &[usize]) -> Result<ShortCircuit, String> {
        let open: Vec<(usize, End)> = out
            .iter()
            .flat_map(|&e| [(e, End::From), (e, End::To)])
            .collect();
        self.solve_with(Some(at), zf, &open, &[])
    }

    /// Solve with breakers open at branch ends and units out of service; no
    /// fault when `at` is `None` (the pre-fault state of the method). A
    /// branch open at one end is dead unless it carries the fault, which the
    /// closed end then still feeds. Parts without a source hold 0 V.
    ///
    /// # Errors
    /// When the network matrix is singular.
    pub fn solve_with(
        &self,
        at: Option<FaultPoint>,
        zf: C64,
        open: &[(usize, End)],
        units_out: &[usize],
    ) -> Result<ShortCircuit, String> {
        if open.is_empty() && units_out.is_empty() {
            if let Some(sc) = self.solve_fast(at, zf)? {
                return Ok(sc);
            }
        }
        let at = at.unwrap_or(FaultPoint::Bus(usize::MAX));
        let is_open = |e: usize, end: End| open.contains(&(e, end));
        let out: Vec<usize> = (0..self.topo.branches.len())
            .filter(|&e| {
                let (f, t) = (is_open(e, End::From), is_open(e, End::To));
                let faulted = matches!(at, FaultPoint::Branch(fe, _) if fe == e);
                (f && t) || ((f || t) && !faulted)
            })
            .collect();
        let topo = self.topo;
        let n = topo.bus_numbers.len();
        let extra = usize::from(matches!(at, FaultPoint::Branch(..)));
        let size = n + extra;
        let zero = C64::new(0.0, 0.0);
        let mut y: Entries = Vec::with_capacity(4 * topo.branches.len() + 2 * size);
        let mut rhs = vec![zero; size];
        for (e, br) in topo.branches.iter().enumerate() {
            if out.contains(&e) {
                continue;
            }
            let ys = C64::new(1.0, 0.0) / br.z;
            let t = br.tap;
            let t2 = t.norm_sqr();
            if let FaultPoint::Branch(fe, alpha) = at {
                if fe == e {
                    let alpha = alpha.clamp(1e-4, 1.0 - 1e-4);
                    let (y1, y2) = (ys / alpha, ys / (1.0 - alpha));
                    let fnode = n;
                    if !is_open(e, End::From) {
                        y.push((br.from, br.from, (y1 + br.y_fr) / t2));
                        y.push((br.from, fnode, -(y1 / t.conj())));
                        y.push((fnode, br.from, -(y1 / t)));
                        y.push((fnode, fnode, y1));
                    }
                    if !is_open(e, End::To) {
                        y.push((fnode, fnode, y2));
                        y.push((fnode, br.to, -(y2)));
                        y.push((br.to, fnode, -(y2)));
                        y.push((br.to, br.to, y2 + br.y_to));
                    }
                    continue;
                }
            }
            y.push((br.from, br.from, (ys + br.y_fr) / t2));
            y.push((br.from, br.to, -(ys / t.conj())));
            y.push((br.to, br.from, -(ys / t)));
            y.push((br.to, br.to, ys + br.y_to));
        }
        for (u, unit) in topo.units.iter().enumerate() {
            if units_out.contains(&u) {
                continue;
            }
            let yu = self.unit_y(u);
            y.push((unit.bus, unit.bus, yu));
            rhs[unit.bus] += yu * self.c;
        }
        // A tiny shunt keeps parts without a source regular (0 V there).
        for k in 0..size {
            y.push((k, k, C64::new(1e-9, 0.0)));
        }
        let fnode = match at {
            FaultPoint::Bus(b) => b,
            FaultPoint::Branch(..) => n,
        };
        let zf = if zf.norm() < 1e-6 {
            C64::new(0.0, 1e-6)
        } else {
            zf
        };
        let yf = C64::new(1.0, 0.0) / zf;
        if fnode < size {
            y.push((fnode, fnode, yf));
        }
        let v = NetLu::factor(size, &y)?.solve(&rhs)?;
        let branch = topo
            .branches
            .iter()
            .enumerate()
            .map(|(e, br)| {
                if out.contains(&e) {
                    return (zero, zero);
                }
                let ys = C64::new(1.0, 0.0) / br.z;
                let (open_f, open_t) = (is_open(e, End::From), is_open(e, End::To));
                let t = br.tap;
                let t2 = t.norm_sqr();
                let (vf, vt) = (v[br.from], v[br.to]);
                if let FaultPoint::Branch(fe, alpha) = at {
                    if fe == e {
                        let alpha = alpha.clamp(1e-4, 1.0 - 1e-4);
                        let (y1, y2) = (ys / alpha, ys / (1.0 - alpha));
                        let vfp = v[n];
                        return (
                            if open_f {
                                zero
                            } else {
                                (y1 + br.y_fr) / t2 * vf - y1 / t.conj() * vfp
                            },
                            if open_t {
                                zero
                            } else {
                                (y2 + br.y_to) * vt - y2 * vfp
                            },
                        );
                    }
                }
                (
                    (ys + br.y_fr) / t2 * vf - ys / t.conj() * vt,
                    (ys + br.y_to) * vt - ys / t * vf,
                )
            })
            .collect();
        Ok(ShortCircuit {
            fault_current: if fnode < size {
                v[fnode] * yf
            } else {
                C64::new(0.0, 0.0)
            },
            v: v[..n].to_vec(),
            branch,
        })
    }

    /// Current out of unit `u` into its bus.
    #[must_use]
    pub fn unit_current(&self, sc: &ShortCircuit, u: usize) -> C64 {
        let b = self.topo.units[u].bus;
        self.unit_y(u) * (C64::new(self.c, 0.0) - sc.v[b])
    }

    /// Apparent impedance (system pu) seen at the end of branch `e`.
    #[must_use]
    pub fn apparent(&self, sc: &ShortCircuit, e: usize, end: End) -> Option<C64> {
        let br = &self.topo.branches[e];
        let (bus, i) = match end {
            End::From => (br.from, sc.branch[e].0),
            End::To => (br.to, sc.branch[e].1),
        };
        (i.norm() > 1e-9).then(|| sc.v[bus] / i)
    }
}

// ---------------------------------------------------------------------------
// Automatic settings
// ---------------------------------------------------------------------------

/// What to protect and how.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SettingOptions {
    #[serde(default)]
    pub characteristic: Characteristic,
    #[serde(default = "yes")]
    pub lines: bool,
    #[serde(default = "yes")]
    pub line_differential: bool,
    #[serde(default = "yes")]
    pub teleprotection: bool,
    #[serde(default = "yes")]
    pub autoreclose: bool,
    #[serde(default = "yes")]
    pub transformers: bool,
    #[serde(default = "yes")]
    pub busbars: bool,
    #[serde(default = "yes")]
    pub generators: bool,
    #[serde(default = "yes")]
    pub inverters: bool,
    #[serde(default = "default_breaker")]
    pub breaker_time: f64,
}

fn yes() -> bool {
    true
}

fn default_breaker() -> f64 {
    0.05
}

impl Default for SettingOptions {
    fn default() -> Self {
        Self {
            characteristic: Characteristic::Mho,
            lines: true,
            line_differential: true,
            teleprotection: true,
            autoreclose: true,
            transformers: true,
            busbars: true,
            generators: true,
            inverters: true,
            breaker_time: default_breaker(),
        }
    }
}

/// A scheme set by the rules, with the basis of each setting.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AutoSetting {
    pub scheme: ProtectionScheme,
    pub notes: Vec<String>,
}

/// `from-to`, with `#circuit` for a circuit other than 1 (parallel branches).
#[must_use]
pub fn branch_tag(from: usize, to: usize, circuit: &str) -> String {
    if circuit.trim() == "1" {
        format!("{from}-{to}")
    } else {
        format!("{from}-{to}#{}", circuit.trim())
    }
}

/// Make relay ids unique: a later duplicate gets `~2`, `~3`, ...
pub fn unique_ids(relays: &mut [Relay]) {
    let mut seen = std::collections::HashMap::<String, usize>::new();
    for r in relays {
        let n = seen.entry(r.id.clone()).or_insert(0);
        *n += 1;
        if *n > 1 {
            r.id = format!("{}~{n}", r.id);
        }
    }
}

/// Relay id of a line end.
#[must_use]
pub fn line_relay_id(from: usize, to: usize, circuit: &str, at: usize) -> String {
    let other = if at == from { to } else { from };
    if circuit.trim() == "1" {
        format!("L{at}-{other}")
    } else {
        format!("L{at}-{other}#{}", circuit.trim())
    }
}

fn rated(topo: &Topology, e: usize) -> f64 {
    let r = topo.branches[e].rate;
    if r > 0.0 {
        r
    } else {
        1.0
    }
}

/// Branches at a dense bus (index, the far bus).
fn at_bus(topo: &Topology, b: usize) -> Vec<(usize, usize)> {
    topo.branches
        .iter()
        .enumerate()
        .filter_map(|(e, br)| {
            if br.from == b {
                Some((e, br.to))
            } else if br.to == b {
                Some((e, br.from))
            } else {
                None
            }
        })
        .collect()
}

/// A typical thermal rating (MVA) of an overhead line or transformer at a
/// voltage level, for branches the case leaves unrated: round values of
/// this module's own choosing, not from a standard or a utility table.
#[must_use]
pub fn typical_rating(kv: f64) -> f64 {
    match kv {
        k if k >= 600.0 => 4000.0,
        k if k >= 450.0 => 3000.0,
        k if k >= 300.0 => 1500.0,
        k if k >= 200.0 => 800.0,
        k if k >= 100.0 => 300.0,
        k if k >= 50.0 => 100.0,
        k if k >= 20.0 => 40.0,
        _ => 20.0,
    }
}

/// Give the branches the case leaves unrated (no rating, or 9000 MVA and
/// more: a placeholder) a rating: the typical thermal rating of their
/// voltage level, at least 125 % of `base_flow` (the apparent power at
/// the branch's more loaded end at the operating point, system per unit).
/// Returns the branches rated this way.
pub fn estimate_ratings(topo: &mut Topology, base_flow: &[f64]) -> Vec<usize> {
    let mut estimated = Vec::new();
    for (e, br) in topo.branches.iter_mut().enumerate() {
        if br.rate > 0.0 && br.rate * topo.sbase < 9000.0 {
            continue;
        }
        let kv = topo.base_kv[br.from].max(topo.base_kv[br.to]);
        let flow = base_flow.get(e).copied().unwrap_or(0.0);
        br.rate = (typical_rating(kv) / topo.sbase).max(1.25 * flow);
        estimated.push(e);
    }
    estimated
}

/// Write the rated current of `topo`'s branches `on` into the line and
/// transformer relays at them that have none: a scheme set on estimated
/// ratings ([`estimate_ratings`]) then says so itself, and binds the same
/// to the case's own topology, where those branches are unrated.
pub fn carry_ratings(scheme: &mut ProtectionScheme, topo: &Topology, on: &[usize]) {
    let mut estimated = vec![false; topo.branches.len()];
    for &e in on {
        estimated[e] = true;
    }
    // The first branch between two buses with a circuit id, as
    // `Topology::branch`.
    let mut between: std::collections::HashMap<(usize, usize), Vec<usize>> =
        std::collections::HashMap::with_capacity(topo.branches.len());
    let pair = |a: usize, b: usize| (a.min(b), a.max(b));
    for (e, br) in topo.branches.iter().enumerate() {
        between
            .entry(pair(br.from_number, br.to_number))
            .or_default()
            .push(e);
    }
    for r in &mut scheme.relays {
        let (RelayLocation::Line {
            from, to, circuit, ..
        }
        | RelayLocation::Transformer { from, to, circuit }) = &r.location
        else {
            continue;
        };
        let e = between.get(&pair(*from, *to)).and_then(|list| {
            list.iter()
                .copied()
                .find(|&e| topo.branches[e].circuit.trim() == circuit.trim())
        });
        if let (Some(e), None) = (e, r.i_nom) {
            if estimated[e] {
                r.i_nom = Some(topo.branches[e].rate);
            }
        }
    }
}

/// Set a scheme by the rules in the module documentation.
#[must_use]
pub fn auto_scheme(topo: &Topology, opts: &SettingOptions) -> AutoSetting {
    let mut relays = Vec::new();
    let mut notes = Vec::new();
    let calc = FaultCalc::new(topo);
    let f0 = topo.f0;
    let bf = Some(BreakerFailure {
        delay: 0.15,
        current: 0.1,
    });
    let ar = opts.autoreclose.then_some(AutoReclose {
        dead_time: 0.5,
        reclaim: 5.0,
        shots: 1,
        sync_angle: 30.0,
    });
    let sc_bus: Vec<Option<ShortCircuit>> = (0..topo.bus_numbers.len())
        .map(|b| calc.solve(FaultPoint::Bus(b), C64::new(0.0, 0.0), &[]).ok())
        .collect();
    notes.push(
        "k0 = 0 on every line: the case has no zero-sequence data and the network model's lines are uncoupled phases (Z0 = Z1); with zero-sequence data set k0 = (Z0 − Z1)/(3 Z1)".into(),
    );

    // A branch with a negative series reactance is a series capacitor, not a
    // line: a distance relay does not protect it (the bank has its own
    // protection, IEEE C37.116, outside this model).
    let series_caps: Vec<usize> = (0..topo.branches.len())
        .filter(|&e| !topo.branches[e].transformer && topo.branches[e].z.im < 0.0)
        .collect();
    if opts.lines && !series_caps.is_empty() {
        notes.push(format!(
            "{} series capacitor(s) (negative series reactance) get no line relays: {}; the lines next to them are set on their own impedance, so a zone 1 there can overreach through the capacitor (IEEE C37.116)",
            series_caps.len(),
            series_caps
                .iter()
                .map(|&e| {
                    let b = &topo.branches[e];
                    format!("{}-{} #{}", b.from_number, b.to_number, b.circuit.trim())
                })
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if opts.lines {
        for (e, br) in topo.branches.iter().enumerate() {
            if br.transformer {
                continue;
            }
            if series_caps.binary_search(&e).is_ok() {
                continue;
            }
            let zl = br.z;
            let angle = zl.arg().to_degrees().clamp(30.0, 89.0);
            for (here, there, end) in [(br.from, br.to, End::From), (br.to, br.from, End::To)] {
                let at = topo.bus_numbers[here];
                let zb = topo.z_base(here);
                let adjacent: Vec<f64> = at_bus(topo, there)
                    .into_iter()
                    .filter(|&(k, _)| k != e)
                    .map(|(k, _)| topo.branches[k].z.norm())
                    .collect();
                let zl_m = zl.norm();
                let adj_min = adjacent.iter().copied().fold(f64::INFINITY, f64::min);
                let adj_max = adjacent.iter().copied().fold(0.0, f64::max);
                // What the relay measures for a fault at the far end of its
                // own line: charging and shunt reactors in the measured
                // current move it away from the series impedance.
                let alpha_end = if end == End::From { 0.999 } else { 0.001 };
                let z_end = calc
                    .solve(FaultPoint::Branch(e, alpha_end), C64::new(0.0, 0.0), &[])
                    .ok()
                    .and_then(|sc| calc.apparent(&sc, e, end))
                    .map_or(zl_m, |z| z.norm());
                // A series capacitor at the remote bus makes a fault beyond
                // it look closer than the remote bus: zone 1 is set short of
                // that too, so it does not overreach through the capacitor
                // in service (IEEE C37.116).
                let z_beyond_cap = at_bus(topo, there)
                    .into_iter()
                    .filter(|&(k, _)| k != e && series_caps.binary_search(&k).is_ok())
                    .filter_map(|(_, far)| {
                        let sc = calc
                            .solve(FaultPoint::Bus(far), C64::new(0.0, 0.0), &[])
                            .ok()?;
                        calc.apparent(&sc, e, end).map(|z| z.norm())
                    })
                    .fold(f64::INFINITY, f64::min);
                let cap_note = if z_beyond_cap < zl_m.min(z_end) {
                    format!(
                        "; Z1 short of a fault beyond the series capacitor at the remote bus ({:.2} Ω)",
                        z_beyond_cap * zb
                    )
                } else {
                    String::new()
                };
                let z1 = 0.85 * zl_m.min(z_end).min(z_beyond_cap);
                let base2 = 1.2 * zl_m.max(z_end);
                let z2 = if adj_min.is_finite() {
                    base2.max(zl_m + 0.5 * adj_min)
                } else {
                    base2
                };
                let z3 = if adj_max > 0.0 {
                    (zl_m + 1.2 * adj_max).max(1.2 * z2)
                } else {
                    (1.5 * zl_m).max(1.2 * z2)
                };
                let mut basis = format!(
                    "{}: line {:.2} Ω ∠{angle:.0}°{}, Z1 {:.0} %, Z2 {:.0} %, Z3 {:.0} %",
                    line_relay_id(br.from_number, br.to_number, &br.circuit, at),
                    zl_m * zb,
                    if (z_end / zl_m - 1.0).abs() > 0.03 {
                        format!(" (a fault at the far end measures {:.2} Ω: charging or shunt reactors)", z_end * zb)
                    } else {
                        String::new()
                    },
                    z1 / zl_m * 100.0,
                    z2 / zl_m * 100.0,
                    z3 / zl_m * 100.0
                );
                basis.push_str(&cap_note);
                // Load encroachment: the load impedance at 0.9 pu and the
                // rating stays outside every zone, by a blinder when the
                // zones reach that far.
                let mut load = None;
                if br.rate > 0.0 {
                    let z_load = 0.81 / br.rate;
                    let along = (angle - 30.0).to_radians().cos();
                    if z3 * along > 0.9 * z_load {
                        load = Some(LoadEncroachment {
                            r: 0.9 * z_load * zb,
                            angle: 35.0,
                        });
                        basis.push_str(&format!(
                            "; load blinder at {:.1} Ω, ±35°",
                            0.9 * z_load * zb
                        ));
                    }
                }
                notes.push(basis);
                let ohm = |z: f64| z * zb;
                let mut zones = vec![
                    Zone {
                        name: "Z1".into(),
                        reach: ohm(z1),
                        angle: None,
                        resistive_reach: None,
                        delay: 0.0,
                        direction: Direction::Forward,
                        autoreclose: true,
                    },
                    Zone {
                        name: "Z2".into(),
                        reach: ohm(z2),
                        angle: None,
                        resistive_reach: None,
                        delay: 0.4,
                        direction: Direction::Forward,
                        autoreclose: false,
                    },
                    Zone {
                        name: "Z3".into(),
                        reach: ohm(z3),
                        angle: None,
                        resistive_reach: None,
                        delay: 1.0,
                        direction: Direction::Forward,
                        autoreclose: false,
                    },
                    Zone {
                        name: "Z4".into(),
                        reach: ohm(0.25 * zl_m),
                        angle: None,
                        resistive_reach: None,
                        delay: 1.5,
                        direction: Direction::Reverse,
                        autoreclose: false,
                    },
                ];
                if opts.characteristic == Characteristic::Quadrilateral {
                    for z in &mut zones {
                        z.resistive_reach = Some((z.reach * 0.6).max(ohm(0.02)));
                    }
                }
                // The backup overcurrent never runs ahead of zone 2: its
                // minimum operate time is a coordination margin above it.
                let z2_delay = zones
                    .iter()
                    .find(|z| z.name == "Z2")
                    .map_or(0.4, |z| z.delay);
                let mut functions = vec![ProtectionFunction::Distance(Distance {
                    characteristic: opts.characteristic,
                    zones,
                    line_angle: angle,
                    k0: [0.0, 0.0],
                    min_current: 0.2,
                    power_swing_blocking: Some(PowerSwingBlocking {
                        outer: 1.3,
                        delta_t: 0.03,
                        max_block: 2.0,
                    }),
                    teleprotection: opts.teleprotection.then(|| Teleprotection {
                        scheme: SchemeKind::Pott,
                        zone: "Z2".into(),
                        channel_time: 0.01,
                    }),
                    load_encroachment: load,
                })];
                let i_nom = rated(topo, e);
                if opts.line_differential {
                    let charging = (br.y_fr + br.y_to).norm();
                    functions.push(ProtectionFunction::Differential(Differential {
                        label: Some("87L".into()),
                        pickup: (0.3_f64).max(3.0 * charging / i_nom),
                        slope1: 0.3,
                        slope2: 0.7,
                        knee: 2.0,
                        unrestrained: 1e3,
                        harmonic2: None,
                        channel_time: 0.01,
                        delay: 0.0,
                    }));
                }
                // Backup overcurrent graded 0.3 s above Z3 at the remote-bus
                // fault current through this end, and at least 0.3 s above Z2
                // at any current.
                let i_remote = sc_bus[there].as_ref().map_or(0.0, |sc| match end {
                    End::From => sc.branch[e].0.norm(),
                    End::To => sc.branch[e].1.norm(),
                });
                let g = i_remote / (1.5 * i_nom);
                let tms = if g > 1.1 {
                    (1.3 / super::curve_time(Curve::IecStandardInverse, 1.0, g).unwrap_or(1.0))
                        .clamp(0.05, 1.5)
                } else {
                    0.5
                };
                functions.push(ProtectionFunction::Overcurrent(Overcurrent {
                    label: Some("67".into()),
                    pickup: 1.5,
                    curve: Curve::IecStandardInverse,
                    tms,
                    delay: z2_delay + 0.3,
                    direction: Direction::Forward,
                }));
                functions.push(ProtectionFunction::EarthFault(Overcurrent {
                    label: Some("67N".into()),
                    pickup: 0.2,
                    curve: Curve::IecStandardInverse,
                    tms: 0.3,
                    delay: 0.0,
                    direction: Direction::Forward,
                }));
                let q_charging = (br.y_fr + br.y_to).im.abs();
                if br.rate > 0.0 && q_charging > 0.3 * br.rate {
                    functions.push(ProtectionFunction::Overvoltage(VoltageStage {
                        label: Some("59".into()),
                        pickup: 1.15,
                        delay: 2.0,
                    }));
                }
                relays.push(Relay {
                    id: line_relay_id(br.from_number, br.to_number, &br.circuit, at),
                    location: RelayLocation::Line {
                        from: br.from_number,
                        to: br.to_number,
                        circuit: br.circuit.clone(),
                        at,
                    },
                    functions,
                    breaker_time: opts.breaker_time,
                    breaker_failure: bf.clone(),
                    autoreclose: ar.clone(),
                    enabled: true,
                    i_nom: None,
                });
            }
        }
    }

    // Selectivity: a Z2 that sees past an adjacent line's Z1 (where that
    // line's own Z2 clears) waits a grading step (0.25 s) longer than that
    // Z2. Adjacent also through a series capacitor in service (a fault
    // current below its protective level leaves it in). The delays are
    // graded to a fixed point, so a chain of such overlaps cascades.
    if opts.lines {
        // For each relay, the relays whose Z2 it must wait for.
        let mut waits_for: Vec<Vec<String>> = vec![Vec::new(); relays.len()];
        for (k, r) in relays.iter().enumerate() {
            let RelayLocation::Line {
                from,
                to,
                circuit,
                at,
            } = &r.location
            else {
                continue;
            };
            let Ok(e) = topo.branch(*from, *to, circuit) else {
                continue;
            };
            let br = &topo.branches[e];
            let end = if br.from_number == *at {
                End::From
            } else {
                End::To
            };
            let (here, there) = if end == End::From {
                (br.from, br.to)
            } else {
                (br.to, br.from)
            };
            let zb = topo.z_base(here);
            let Some(ProtectionFunction::Distance(d)) = r.functions.first() else {
                continue;
            };
            let Some(z2) = d.zones.iter().find(|z| z.name == "Z2") else {
                continue;
            };
            // Lines next to `there`, and those beyond a series capacitor at
            // it: (branch, the bus its relay sits at).
            let mut next: Vec<(usize, usize)> = Vec::new();
            for (k2, far) in at_bus(topo, there) {
                if k2 == e {
                    continue;
                }
                if series_caps.binary_search(&k2).is_ok() {
                    for (kb, _) in at_bus(topo, far) {
                        if kb != k2 && series_caps.binary_search(&kb).is_err() {
                            next.push((kb, far));
                        }
                    }
                } else {
                    next.push((k2, there));
                }
            }
            for (kn, at_bus_n) in next {
                let adj = &topo.branches[kn];
                if adj.transformer {
                    continue;
                }
                // Just past that line's Z1, from its relay at `at_bus_n`.
                let alpha = if adj.from == at_bus_n { 0.9 } else { 0.1 };
                let sees = calc
                    .solve(FaultPoint::Branch(kn, alpha), C64::new(0.0, 0.0), &[])
                    .ok()
                    .and_then(|sc| calc.apparent(&sc, e, end))
                    .is_some_and(|za| super::functions::zone_contains(d, z2, za, zb));
                if sees {
                    waits_for[k].push(line_relay_id(
                        adj.from_number,
                        adj.to_number,
                        &adj.circuit,
                        topo.bus_numbers[at_bus_n],
                    ));
                }
            }
        }
        let index: std::collections::HashMap<String, usize> = relays
            .iter()
            .enumerate()
            .map(|(k, r)| (r.id.clone(), k))
            .collect();
        let z2_of = |r: &Relay| match r.functions.first() {
            Some(ProtectionFunction::Distance(d)) => {
                d.zones.iter().find(|z| z.name == "Z2").map(|z| z.delay)
            }
            _ => None,
        };
        let mut delay: Vec<Option<f64>> = relays.iter().map(z2_of).collect();
        let first = delay.clone();
        // A few rounds reach the fixed point of any real chain; a loop of
        // mutual overlaps stops at the cap.
        for _ in 0..6 {
            let mut changed = false;
            for k in 0..relays.len() {
                let Some(mine) = delay[k] else { continue };
                let need = waits_for[k]
                    .iter()
                    .filter_map(|id| index.get(id).and_then(|&j| delay[j]))
                    .map(|d| d + 0.25)
                    .fold(mine, f64::max)
                    .min(1.4);
                if need > mine + 1e-9 {
                    delay[k] = Some(need);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        for k in 0..relays.len() {
            let (Some(old), Some(new)) = (first[k], delay[k]) else {
                continue;
            };
            if new <= old + 1e-9 {
                continue;
            }
            if let Some(ProtectionFunction::Distance(d)) = relays[k].functions.first_mut() {
                for z in d.zones.iter_mut() {
                    if z.name == "Z2" {
                        z.delay = new;
                    }
                    if z.name == "Z3" {
                        z.delay = z.delay.max(new + 0.55);
                    }
                }
            }
            // The backup overcurrent stays a coordination margin above Z2.
            for f in &mut relays[k].functions {
                if let ProtectionFunction::Overcurrent(o) = f {
                    if o.label.as_deref() == Some("67") {
                        o.delay = o.delay.max(new + 0.3);
                    }
                }
            }
            notes.push(format!(
                "{}: Z2 sees past {}'s Z1, graded to {new:.2} s",
                relays[k].id,
                waits_for[k].join(", ")
            ));
        }
    }

    if opts.transformers {
        for (e, br) in topo.branches.iter().enumerate() {
            if !br.transformer {
                continue;
            }
            let i_nom = rated(topo, e);
            let through = [br.from, br.to]
                .iter()
                .filter_map(|&b| sc_bus[b].as_ref())
                .map(|sc| sc.branch[e].0.norm().max(sc.branch[e].1.norm()))
                .fold(0.0, f64::max)
                / i_nom;
            let g = through / 1.3;
            let tms = if g > 1.1 {
                (0.8 / super::curve_time(Curve::IecStandardInverse, 1.0, g).unwrap_or(1.0))
                    .clamp(0.05, 1.5)
            } else {
                0.3
            };
            let id = format!("T{}", branch_tag(br.from_number, br.to_number, &br.circuit));
            notes.push(format!(
                "{id}: largest through-fault current {through:.1} In; 87T unrestrained {:.1} In",
                (1.25 * through).max(8.0)
            ));
            relays.push(Relay {
                id,
                location: RelayLocation::Transformer {
                    from: br.from_number,
                    to: br.to_number,
                    circuit: br.circuit.clone(),
                },
                functions: vec![
                    ProtectionFunction::Differential(Differential {
                        label: Some("87T".into()),
                        pickup: 0.3,
                        slope1: 0.3,
                        slope2: 0.7,
                        knee: 2.0,
                        unrestrained: (1.25 * through).max(8.0),
                        harmonic2: Some(0.15),
                        channel_time: 0.0,
                        delay: 0.0,
                    }),
                    ProtectionFunction::Overcurrent(Overcurrent {
                        label: Some("51".into()),
                        pickup: 1.3,
                        curve: Curve::IecStandardInverse,
                        tms,
                        delay: 0.0,
                        direction: Direction::NonDirectional,
                    }),
                    ProtectionFunction::Thermal(Thermal {
                        label: None,
                        k: 1.1,
                        tau: 1800.0,
                    }),
                    ProtectionFunction::Overexcitation(VoltageStage {
                        label: None,
                        pickup: 1.1,
                        delay: 10.0,
                    }),
                ],
                breaker_time: opts.breaker_time,
                breaker_failure: bf.clone(),
                autoreclose: None,
                enabled: true,
                i_nom: None,
            });
        }
    }

    if opts.busbars {
        for (b, &number) in topo.bus_numbers.iter().enumerate() {
            let feeders = at_bus(topo, b).len() + topo.units.iter().filter(|u| u.bus == b).count();
            if feeders < 3 {
                continue;
            }
            relays.push(Relay {
                id: format!("B{number}"),
                location: RelayLocation::Busbar { bus: number },
                functions: vec![ProtectionFunction::Differential(Differential {
                    label: Some("87B".into()),
                    pickup: 0.8,
                    slope1: 0.5,
                    slope2: 0.5,
                    knee: 1e3,
                    unrestrained: 1e3,
                    harmonic2: None,
                    channel_time: 0.0,
                    delay: 0.0,
                })],
                breaker_time: opts.breaker_time,
                breaker_failure: None,
                autoreclose: None,
                enabled: true,
                i_nom: None,
            });
        }
    }

    for (u, unit) in topo.units.iter().enumerate() {
        let scale = unit.mbase / topo.sbase;
        match unit.kind {
            UnitKind::Machine if opts.generators => {
                let xd_p = unit.xd_p.unwrap_or(0.3);
                let xd = unit.xd.unwrap_or(1.5 * xd_p.max(0.3) + 0.6);
                // System impedance behind the terminal on the machine base.
                let xs = sc_bus[unit.bus].as_ref().map_or(0.3, |sc| {
                    let z_total = calc.c / sc.fault_current;
                    let y_unit = calc.unit_y(u);
                    let z_ex = C64::new(1.0, 0.0) / (C64::new(1.0, 0.0) / z_total - y_unit);
                    (z_ex * scale).norm()
                });
                let mut functions = Vec::new();
                let branches = at_bus(topo, unit.bus);
                // 87U spans a unit alone on its step-up transformer; units
                // sharing one are covered by the transformer's 87T (a zone
                // around one of them would see the others' current).
                let alone = topo.units.iter().filter(|o| o.bus == unit.bus).count() == 1;
                if alone && branches.len() == 1 && topo.branches[branches[0].0].transformer {
                    functions.push(ProtectionFunction::Differential(Differential {
                        label: Some("87U".into()),
                        pickup: 0.2,
                        slope1: 0.3,
                        slope2: 0.7,
                        knee: 2.0,
                        unrestrained: 8.0,
                        harmonic2: None,
                        channel_time: 0.0,
                        delay: 0.0,
                    }));
                }
                functions.push(ProtectionFunction::LossOfField(LossOfField {
                    label: None,
                    offset: xd_p / 2.0,
                    diameter1: 1.0,
                    delay1: 0.1,
                    diameter2: xd,
                    delay2: 0.5,
                }));
                let generating = unit.p0 > 0.01 * scale;
                if generating {
                    functions.push(ProtectionFunction::ReversePower(ReversePower {
                        label: None,
                        pickup: 0.02,
                        delay: 5.0,
                    }));
                }
                functions.push(ProtectionFunction::OutOfStep(OutOfStep {
                    label: None,
                    blinder: 0.5 * (xd_p + xs) * (30f64).to_radians().tan(),
                    forward_reach: 1.5 * xs,
                    reverse_reach: 2.0 * xd_p,
                    min_traverse: 0.025,
                    slips: 1,
                }));
                functions.extend([
                    ProtectionFunction::Underfrequency(FrequencyStage {
                        label: None,
                        pickup: 0.95 * f0,
                        delay: 1.0,
                        min_voltage: 0.3,
                    }),
                    ProtectionFunction::Overfrequency(FrequencyStage {
                        label: None,
                        pickup: 1.03 * f0,
                        delay: 1.0,
                        min_voltage: 0.3,
                    }),
                    ProtectionFunction::Overexcitation(VoltageStage {
                        label: None,
                        pickup: 1.18,
                        delay: 2.0,
                    }),
                    ProtectionFunction::Undervoltage(VoltageStage {
                        label: None,
                        pickup: 0.7,
                        delay: 3.0,
                    }),
                    ProtectionFunction::Overvoltage(VoltageStage {
                        label: None,
                        pickup: 1.15,
                        delay: 2.0,
                    }),
                    ProtectionFunction::NegativeSequence(NegativeSequence {
                        label: None,
                        continuous: 0.08,
                        k: 10.0,
                    }),
                    ProtectionFunction::Thermal(Thermal {
                        label: None,
                        k: 1.05,
                        tau: 600.0,
                    }),
                ]);
                notes.push(format!(
                    "G{}-{}: X'd {xd_p:.3}, Xd {xd:.3}, system {xs:.3} pu (machine base){}",
                    unit.bus_number,
                    unit.machine_id,
                    if generating {
                        ""
                    } else {
                        "; no 32: the unit does not generate (condenser)"
                    }
                ));
                relays.push(Relay {
                    id: format!("G{}-{}", unit.bus_number, unit.machine_id),
                    location: RelayLocation::Generator {
                        bus: unit.bus_number,
                        machine_id: unit.machine_id.clone(),
                    },
                    functions,
                    breaker_time: opts.breaker_time,
                    breaker_failure: bf.clone(),
                    autoreclose: None,
                    enabled: true,
                    i_nom: None,
                });
            }
            UnitKind::Inverter if opts.inverters => {
                relays.push(Relay {
                    id: format!("I{}-{}", unit.bus_number, unit.machine_id),
                    location: RelayLocation::Inverter {
                        bus: unit.bus_number,
                        machine_id: unit.machine_id.clone(),
                    },
                    functions: inverter_interface(f0),
                    breaker_time: opts.breaker_time,
                    breaker_failure: None,
                    autoreclose: None,
                    enabled: true,
                    i_nom: None,
                });
            }
            _ => {}
        }
    }
    if opts.inverters && topo.units.iter().any(|u| u.kind == UnitKind::Inverter) {
        notes.push("inverter interface protection: indicative values from the RfG ranges; the grid operator sets the actual ones".into());
    }
    unique_ids(&mut relays);
    // Each series capacitor bank's own protection: its MOV at 2 times the
    // rated current (IEEE C37.116 puts the protective level at about 2 to
    // 2.5 times the rated capacitor voltage), back in 50 ms after the current
    // is below its rating.
    let series_capacitors: Vec<SeriesCapacitor> = series_caps
        .iter()
        .map(|&e| {
            let b = &topo.branches[e];
            SeriesCapacitor {
                from: b.from_number,
                to: b.to_number,
                circuit: b.circuit.trim().to_owned(),
                rated: rated(topo, e),
                protective_level: 2.0,
                reinsert: 0.05,
            }
        })
        .collect();
    if !series_capacitors.is_empty() {
        notes.push(format!(
            "{} series capacitor bank(s) protected by their MOV at 2 times the rated current: bypassed while a fault drives the current above it (IEEE C37.116)",
            series_capacitors.len()
        ));
    }
    AutoSetting {
        scheme: ProtectionScheme {
            relays,
            stuck_breakers: Vec::new(),
            measuring_time: 0.02,
            series_capacitors,
        },
        notes,
    }
}

/// The interface protection of an inverter-based unit.
#[must_use]
pub fn inverter_interface(f0: f64) -> Vec<ProtectionFunction> {
    vec![
        ProtectionFunction::FaultRideThrough(FaultRideThrough {
            label: None,
            points: vec![[0.0, 0.0], [0.15, 0.0], [1.5, 0.85]],
            start: 0.85,
            reset: 0.5,
        }),
        ProtectionFunction::Undervoltage(VoltageStage {
            label: None,
            pickup: 0.85,
            delay: 3.0,
        }),
        ProtectionFunction::Overvoltage(VoltageStage {
            label: Some("59 U>".into()),
            pickup: 1.1,
            delay: 60.0,
        }),
        ProtectionFunction::Overvoltage(VoltageStage {
            label: Some("59 U>>".into()),
            pickup: 1.2,
            delay: 0.1,
        }),
        ProtectionFunction::Underfrequency(FrequencyStage {
            label: None,
            pickup: 0.95 * f0,
            delay: 0.1,
            min_voltage: 0.2,
        }),
        ProtectionFunction::Overfrequency(FrequencyStage {
            label: None,
            pickup: 1.03 * f0,
            delay: 0.1,
            min_voltage: 0.2,
        }),
        ProtectionFunction::Rocof(Rocof {
            label: None,
            pickup: 2.0,
            window: 0.5,
            delay: 0.0,
            min_voltage: 0.8,
        }),
    ]
}

// ---------------------------------------------------------------------------
// Audit
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Ok,
    Info,
    Warning,
    Error,
}

/// One result of a setting check.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Finding {
    pub severity: Severity,
    pub relay: String,
    pub check: String,
    pub message: String,
}

/// Check a scheme's settings against the network (reaches, coverage,
/// selectivity, timing, load encroachment, stability limits).
#[must_use]
pub fn audit(topo: &Topology, scheme: &ProtectionScheme) -> Vec<Finding> {
    let mut out = Vec::new();
    let calc = FaultCalc::new(topo);
    let mut push = |severity, relay: &str, check: &str, message: String| {
        out.push(Finding {
            severity,
            relay: relay.to_owned(),
            check: check.to_owned(),
            message,
        });
    };
    // Which branch ends carry a main protection (21 or 87L).
    let mut covered: Vec<[bool; 2]> = vec![[false; 2]; topo.branches.len()];
    for r in scheme.relays.iter().filter(|r| r.enabled) {
        let RelayLocation::Line {
            from,
            to,
            circuit,
            at,
        } = &r.location
        else {
            continue;
        };
        let Ok(e) = topo.branch(*from, *to, circuit) else {
            push(
                Severity::Error,
                &r.id,
                "location",
                format!("no branch {from}-{to} circuit {circuit}"),
            );
            continue;
        };
        let br = &topo.branches[e];
        let end = if br.from_number == *at {
            End::From
        } else {
            End::To
        };
        let (here, there) = if end == End::From {
            (br.from, br.to)
        } else {
            (br.to, br.from)
        };
        let zb = topo.z_base(here);
        let zl = br.z.norm() * zb;
        let main = r.functions.iter().any(|f| {
            matches!(
                f,
                ProtectionFunction::Distance(_) | ProtectionFunction::Differential(_)
            )
        });
        if main {
            covered[e][usize::from(end == End::To)] = true;
        }
        for f in &r.functions {
            match f {
                ProtectionFunction::Distance(d) => {
                    let zone = |n: &str| {
                        d.zones
                            .iter()
                            .find(|z| z.name == n && z.direction == Direction::Forward)
                    };
                    let in_zone =
                        |z: &Zone, zapp: C64| super::functions::zone_contains(d, z, zapp, zb);
                    if let Some(z1) = zone("Z1") {
                        let alpha_end = if end == End::From { 0.999 } else { 0.001 };
                        if let Some(za) = calc
                            .solve(FaultPoint::Branch(e, alpha_end), C64::new(0.0, 0.0), &[])
                            .ok()
                            .and_then(|sc| calc.apparent(&sc, e, end))
                        {
                            if in_zone(z1, za / 0.95) {
                                push(
                                    Severity::Error,
                                    &r.id,
                                    "21 Z1 overreach",
                                    format!(
                                        "a fault at the far end of the line measures {:.2} Ω (series {:.2} Ω): Z1 reaches within 5 % of the remote bus",
                                        za.norm() * zb,
                                        zl
                                    ),
                                );
                            }
                        }
                        let ratio = z1.reach / zl;
                        let (sev, msg) = if ratio >= 1.0 {
                            (Severity::Error, format!("Z1 reaches {:.0} % of the line: it overreaches the remote bus and trips instantaneously for faults beyond", ratio * 100.0))
                        } else if !(0.7..=0.9).contains(&ratio) {
                            (
                                Severity::Warning,
                                format!(
                                    "Z1 reaches {:.0} % of the line (practice: 70–90 %)",
                                    ratio * 100.0
                                ),
                            )
                        } else {
                            (
                                Severity::Ok,
                                format!("Z1 reaches {:.0} % of the line", ratio * 100.0),
                            )
                        };
                        push(sev, &r.id, "21 Z1 reach", msg);
                    } else {
                        push(
                            Severity::Error,
                            &r.id,
                            "21 Z1 reach",
                            "no forward Z1".into(),
                        );
                    }
                    let remote = calc.solve(
                        super::setting::FaultPoint::Bus(there),
                        C64::new(0.0, 0.0),
                        &[],
                    );
                    if let Some(z2) = zone("Z2") {
                        let ratio = z2.reach / zl;
                        push(
                            // The reach is set at 1.2 times the line and read back in
                            // ohms: allow the rounding.
                            if ratio < 1.2 * (1.0 - 1e-9) { Severity::Error } else { Severity::Ok },
                            &r.id,
                            "21 Z2 reach",
                            format!("Z2 reaches {:.0} % of the line (needs ≥ 120 % to cover the remote bus with margin)", ratio * 100.0),
                        );
                        if let Ok(sc) = &remote {
                            if let Some(za) = calc.apparent(sc, e, end) {
                                let ok = in_zone(z2, za);
                                push(
                                    if ok { Severity::Ok } else { Severity::Error },
                                    &r.id,
                                    "21 Z2 remote bus fault",
                                    format!(
                                        "a bolted fault at bus {} appears at {:.2} Ω ∠{:.0}°: {}",
                                        topo.bus_numbers[there],
                                        za.norm() * zb,
                                        za.arg().to_degrees(),
                                        if ok {
                                            "inside Z2"
                                        } else {
                                            "outside Z2: the remote bus is not covered"
                                        }
                                    ),
                                );
                            }
                        }
                        // Selectivity: the end of the adjacent lines' Z1.
                        for (k, far) in at_bus(topo, there).into_iter().filter(|&(k, _)| k != e) {
                            let adj = &topo.branches[k];
                            if adj.transformer {
                                continue;
                            }
                            let alpha = if adj.from == there { 0.85 } else { 0.15 };
                            if let Ok(sc) = calc.solve(
                                super::setting::FaultPoint::Branch(k, alpha),
                                C64::new(0.0, 0.0),
                                &[],
                            ) {
                                if let Some(za) = calc.apparent(&sc, e, end) {
                                    if in_zone(z2, za) {
                                        let adj_z2 = scheme
                                            .relays
                                            .iter()
                                            .find(|o| matches!(&o.location, RelayLocation::Line { from, to, at, .. } if *at == topo.bus_numbers[there] && ((topo.bus_numbers[far] == *from) || topo.bus_numbers[far] == *to)))
                                            .and_then(|o| o.functions.iter().find_map(|f| match f {
                                                ProtectionFunction::Distance(dd) => dd.zones.iter().find(|z| z.name == "Z2").map(|z| z.delay),
                                                _ => None,
                                            }));
                                        let graded = adj_z2.is_some_and(|t| z2.delay >= t + 0.25);
                                        push(
                                            if graded {
                                                Severity::Info
                                            } else {
                                                Severity::Warning
                                            },
                                            &r.id,
                                            "21 Z2 selectivity",
                                            format!(
                                                "Z2 sees the end of Z1 of {}-{} ({:.2} Ω): {}",
                                                adj.from_number,
                                                adj.to_number,
                                                za.norm() * zb,
                                                if graded {
                                                    "time-graded above the adjacent Z2"
                                                } else {
                                                    "it overlaps the adjacent Z2 with the same time; lower Z2 or raise its delay"
                                                }
                                            ),
                                        );
                                    }
                                }
                            }
                        }
                    } else {
                        push(
                            Severity::Warning,
                            &r.id,
                            "21 Z2 reach",
                            "no forward Z2 (no remote-bus coverage)".into(),
                        );
                    }
                    if let Some(z3) = zone("Z3") {
                        // Remote backup: the far end of every adjacent line.
                        for (k, _far) in at_bus(topo, there).into_iter().filter(|&(k, _)| k != e) {
                            let adj = &topo.branches[k];
                            let alpha = if adj.from == there { 0.999 } else { 0.001 };
                            if let Ok(sc) = calc.solve(
                                super::setting::FaultPoint::Branch(k, alpha),
                                C64::new(0.0, 0.0),
                                &[],
                            ) {
                                if let Some(za) = calc.apparent(&sc, e, end) {
                                    if !in_zone(z3, za) {
                                        push(
                                            Severity::Info,
                                            &r.id,
                                            "21 Z3 remote backup",
                                            format!(
                                                "the far end of {}-{} appears at {:.2} Ω (infeed): outside Z3",
                                                adj.from_number,
                                                adj.to_number,
                                                za.norm() * zb
                                            ),
                                        );
                                    }
                                }
                            }
                        }
                        // Load encroachment.
                        if br.rate > 0.0 {
                            let z_load = 0.81 / br.rate;
                            let enc = [30.0f64, -30.0, 150.0, 210.0]
                                .iter()
                                .any(|a| in_zone(z3, C64::from_polar(z_load, a.to_radians())));
                            push(
                                if enc { Severity::Error } else { Severity::Ok },
                                &r.id,
                                "21 load encroachment",
                                format!(
                                    "load impedance at 0.9 pu and rating {:.0} MVA is {:.1} Ω: {}",
                                    br.rate * topo.sbase,
                                    z_load * zb,
                                    if enc {
                                        "inside Z3, the relay trips on heavy load"
                                    } else {
                                        "outside every zone"
                                    }
                                ),
                            );
                        }
                        if let Some(z2) = zone("Z2") {
                            if z3.delay < z2.delay + 0.25 {
                                push(
                                    Severity::Warning,
                                    &r.id,
                                    "21 timing",
                                    format!(
                                        "Z3 {:.2} s is less than 0.25 s above Z2 {:.2} s",
                                        z3.delay, z2.delay
                                    ),
                                );
                            }
                        }
                    }
                    if let (Some(z1), Some(z2)) = (zone("Z1"), zone("Z2")) {
                        let cti = z2.delay - z1.delay;
                        push(
                            if cti < 0.25 {
                                Severity::Warning
                            } else {
                                Severity::Ok
                            },
                            &r.id,
                            "21 timing",
                            format!(
                                "Z2 − Z1 = {:.2} s (needs ≥ 0.25 s: breaker, reset and margin)",
                                cti
                            ),
                        );
                    }
                    if let Some(tp) = &d.teleprotection {
                        let partner = scheme.relays.iter().any(|o| {
                            o.id != r.id
                                && matches!(&o.location, RelayLocation::Line { from: f2, to: t2, circuit: c2, at: a2 } if ((f2 == from && t2 == to) || (f2 == to && t2 == from)) && c2.trim() == circuit.trim() && a2 != at)
                                && o.functions.iter().any(|f| matches!(f, ProtectionFunction::Distance(dd) if dd.teleprotection.as_ref().is_some_and(|t| t.scheme == tp.scheme)))
                        });
                        push(
                            if partner {
                                Severity::Ok
                            } else {
                                Severity::Error
                            },
                            &r.id,
                            "21 teleprotection",
                            if partner {
                                format!("{:?} partner at the other end", tp.scheme)
                            } else {
                                format!(
                                    "{:?} without a partner at the other end: never trips",
                                    tp.scheme
                                )
                            },
                        );
                    }
                }
                ProtectionFunction::Differential(dset) => {
                    let charging =
                        (br.y_fr + br.y_to).norm() / r.i_nom.unwrap_or_else(|| rated(topo, e));
                    push(
                        if dset.pickup < 2.0 * charging {
                            Severity::Warning
                        } else {
                            Severity::Ok
                        },
                        &r.id,
                        "87L pickup",
                        format!(
                            "Id> {:.2} In against a charging current of {:.2} In",
                            dset.pickup, charging
                        ),
                    );
                }
                _ => {}
            }
        }
    }
    for (e, br) in topo.branches.iter().enumerate() {
        if br.transformer {
            continue;
        }
        let c = covered[e];
        // A series capacitor carries no line relays: its bank protection
        // (the MOV and its bypass) guards it, and the relays of the lines
        // at its ends clear a fault at it (IEEE C37.116).
        let bank = scheme.series_capacitors.iter().any(|s| {
            s.circuit.trim() == br.circuit.trim()
                && ((s.from == br.from_number && s.to == br.to_number)
                    || (s.from == br.to_number && s.to == br.from_number))
        });
        if bank && !(c[0] && c[1]) {
            push(
                Severity::Ok,
                &format!("{}-{}", br.from_number, br.to_number),
                "coverage",
                format!(
                    "series capacitor {}-{} circuit {} is guarded by its bank protection (MOV bypass); the lines at its ends clear a fault at it",
                    br.from_number, br.to_number, br.circuit
                ),
            );
        } else if !(c[0] && c[1]) {
            push(
                Severity::Warning,
                &format!("{}-{}", br.from_number, br.to_number),
                "coverage",
                format!(
                    "line {}-{} circuit {} lacks main protection at {}",
                    br.from_number,
                    br.to_number,
                    br.circuit,
                    match c {
                        [false, false] => "both ends".to_owned(),
                        [false, true] => format!("bus {}", br.from_number),
                        _ => format!("bus {}", br.to_number),
                    }
                ),
            );
        }
    }
    for r in scheme.relays.iter().filter(|r| r.enabled) {
        match &r.location {
            RelayLocation::Transformer { from, to, circuit } => {
                let Ok(e) = topo.branch(*from, *to, circuit) else {
                    continue;
                };
                let br = &topo.branches[e];
                let i_nom = r.i_nom.unwrap_or_else(|| rated(topo, e));
                let through = [br.from, br.to]
                    .iter()
                    .filter_map(|&b| calc.solve(FaultPoint::Bus(b), C64::new(0.0, 0.0), &[]).ok())
                    .map(|sc| sc.branch[e].0.norm().max(sc.branch[e].1.norm()))
                    .fold(0.0, f64::max)
                    / i_nom;
                for f in &r.functions {
                    if let ProtectionFunction::Differential(d) = f {
                        push(
                            if d.unrestrained < through {
                                Severity::Error
                            } else {
                                Severity::Ok
                            },
                            &r.id,
                            "87T unrestrained",
                            format!(
                                "Id>> {:.1} In against the largest through-fault current {:.1} In",
                                d.unrestrained, through
                            ),
                        );
                        if d.harmonic2.is_none() {
                            push(
                                Severity::Warning,
                                &r.id,
                                "87T inrush",
                                "no second-harmonic blocking: energizing inrush can trip".into(),
                            );
                        }
                    }
                }
            }
            RelayLocation::Generator { .. } => {
                for f in &r.functions {
                    if let ProtectionFunction::Underfrequency(s) = f {
                        let limit = 0.95 * topo.f0;
                        push(
                            if s.pickup > limit + 1e-9 {
                                Severity::Error
                            } else {
                                Severity::Ok
                            },
                            &r.id,
                            "81U ride-through",
                            format!(
                                "81U at {:.2} Hz (RfG: stay connected down to {:.2} Hz)",
                                s.pickup, limit
                            ),
                        );
                    }
                }
            }
            RelayLocation::Inverter { .. } => {
                for f in &r.functions {
                    match f {
                        ProtectionFunction::Underfrequency(s)
                            if s.pickup > 0.95 * topo.f0 + 1e-9 =>
                        {
                            push(
                                Severity::Error,
                                &r.id,
                                "81U ride-through",
                                format!("81U at {:.2} Hz trips inside the RfG range", s.pickup),
                            );
                        }
                        ProtectionFunction::Overfrequency(s)
                            if s.pickup < 1.03 * topo.f0 - 1e-9 =>
                        {
                            push(
                                Severity::Error,
                                &r.id,
                                "81O ride-through",
                                format!("81O at {:.2} Hz trips inside the RfG range", s.pickup),
                            );
                        }
                        ProtectionFunction::FaultRideThrough(frt) => {
                            let reference = [[0.0, 0.0], [0.15, 0.0], [1.5, 0.85]];
                            let strict = (0..=30).any(|k| {
                                let t = k as f64 * 0.1;
                                super::functions::profile(&frt.points, t)
                                    > super::functions::profile(&reference, t) + 1e-9
                            });
                            push(
                                if strict {
                                    Severity::Warning
                                } else {
                                    Severity::Ok
                                },
                                &r.id,
                                "FRT profile",
                                if strict {
                                    "the ride-through profile trips above the RfG type D reference"
                                        .into()
                                } else {
                                    "at least as tolerant as the RfG type D reference".to_owned()
                                },
                            );
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    // Busbars without differential protection.
    for (b, &number) in topo.bus_numbers.iter().enumerate() {
        let feeders = at_bus(topo, b).len();
        if feeders >= 3
            && !scheme.relays.iter().any(|r| {
                r.enabled && matches!(r.location, RelayLocation::Busbar { bus } if bus == number)
            })
        {
            push(
                Severity::Info,
                &format!("B{number}"),
                "87B coverage",
                format!("bus {number} ({feeders} feeders) has no busbar protection: its faults clear in Z2 time"),
            );
        }
    }
    out.sort_by(|a, b| b.severity.cmp(&a.severity).then(a.relay.cmp(&b.relay)));
    out
}
