//! The differential-algebraic system `M ẋ = f(x)` of a network with dynamic
//! generators, Thevenin sources and static loads.
//!
//! The formulation follows PowerSimulationsDynamics.jl (`src/models/system.jl`,
//! `network_model.jl`, `load_models.jl`, `source_models.jl`; BSD-3-Clause, see
//! `licenses/PowerSimulationsDynamics.jl-LICENSE`): the state vector is
//! `[Vr(1..n); Vi(1..n); device states]`, the bus rows are the algebraic
//! current balance `Σ I_injected − Y V = 0` in system per unit, and each
//! generator adds its stator current to its bus rows.

use num_complex::Complex;

use super::models::{dq_ri_sc, mag, Avr, Governor, Machine, Shaft, Stabilizer};
use crate::model::AcNetwork;

pub(crate) type C64 = Complex<f64>;

/// One synchronous generator with its controls.
#[derive(Clone, Debug)]
pub(crate) struct GenDevice {
    pub bus_number: usize,
    pub machine_id: String,
    /// Dense bus index.
    pub bus: usize,
    /// `MBASE / Sbase`: machine-base current to system-base current.
    pub scale: f64,
    pub f0: f64,
    pub machine: Machine,
    pub shaft: Shaft,
    pub avr: Avr,
    pub gov: Governor,
    pub pss: Stabilizer,
    pub v_ref: f64,
    pub p_ref: f64,
    /// First global state index.
    pub offset: usize,
    /// States frozen by a generator trip.
    pub tripped: Option<Vec<f64>>,
}

/// What a generator evaluation reports besides its derivatives.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct GenOut {
    /// Injected current, system base.
    pub ir: f64,
    pub ii: f64,
    pub tm: f64,
    pub vf: f64,
    pub vs: f64,
}

impl GenDevice {
    pub(crate) fn n_states(&self) -> usize {
        self.machine.n_states()
            + 2
            + self.avr.n_states()
            + self.gov.n_states()
            + self.pss.n_states()
    }

    /// Local offsets of `(machine, shaft, avr, governor, stabilizer)`.
    pub(crate) fn layout(&self) -> [usize; 5] {
        let m = 0;
        let s = m + self.machine.n_states();
        let a = s + 2;
        let g = a + self.avr.n_states();
        let p = g + self.gov.n_states();
        [m, s, a, g, p]
    }

    pub(crate) fn state_names(&self) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        names.extend(self.machine.state_names().iter().map(|s| (*s).to_owned()));
        names.push("delta".into());
        names.push("omega".into());
        names.extend(self.avr.state_names().iter().map(|s| (*s).to_owned()));
        names.extend(self.gov.state_names().iter().map(|s| (*s).to_owned()));
        names.extend(self.pss.state_names().iter().map(|s| (*s).to_owned()));
        names
    }

    pub(crate) fn name(&self) -> String {
        format!("{}-{}", self.bus_number, self.machine_id)
    }

    /// Local mass diagonal.
    pub(crate) fn mass(&self) -> Vec<f64> {
        let mut m = vec![1.0; self.n_states()];
        let [_, _, a, g, p] = self.layout();
        self.avr.mass(&mut m[a..g]);
        self.gov.mass(&mut m[g..p]);
        self.pss.mass(&mut m[p..]);
        if self.tripped.is_some() {
            // A frozen state is algebraic: `0 = x_trip − x`.
            m.iter_mut().for_each(|v| *v = 0.0);
        }
        m
    }

    /// Outputs of every block of the device's graph controllers: none
    /// without the block graphs of `rms-experimental`.
    #[cfg(not(feature = "rms-experimental"))]
    #[allow(clippy::unused_self)]
    pub(crate) fn graph_signals(&self, _x: &[f64], _vr: f64, _vi: f64) -> Vec<(String, f64)> {
        Vec::new()
    }

    /// Outputs of every block of the device's graph controllers, named
    /// `{slot}.{node}` (empty without graphs).
    #[cfg(feature = "rms-experimental")]
    pub(crate) fn graph_signals(&self, x: &[f64], vr: f64, vi: f64) -> Vec<(String, f64)> {
        use super::models::{Avr, Governor, Stabilizer};
        let graphs = (
            matches!(self.avr, Avr::Graph(_)),
            matches!(self.gov, Governor::Graph(_)),
            matches!(self.pss, Stabilizer::Graph(_)),
        );
        if graphs == (false, false, false) {
            return Vec::new();
        }
        let [m, s, a, g, p] = self.layout();
        let n = self.n_states();
        let omega = x[s + 1];
        let alg = self.machine.algebraic(&x[m..s], x[s], vr, vi);
        let vt = mag(vr, vi);
        let pe = alg.te * self.scale;
        let mut scratch = vec![0.0; n];
        let vs = self
            .pss
            .eval(&x[p..n], omega, pe, vt, self.scale, &mut scratch[p..n]);
        let mut out = Vec::new();
        let mut push = |slot: &str, gr: &super::graph::CompiledGraph, xs: &[f64], ports: &[f64]| {
            for (id, v) in gr.node_ids().zip(gr.signals(xs, ports)) {
                out.push((format!("{slot}.{id}"), v));
            }
        };
        if let Avr::Graph(gr) = &self.avr {
            push("exciter", gr, &x[a..g], &[vt, vs, alg.xad_ifd, self.v_ref]);
        }
        if let Governor::Graph(gr) = &self.gov {
            push("governor", gr, &x[g..p], &[omega, self.p_ref]);
        }
        if let Stabilizer::Graph(gr) = &self.pss {
            push(
                "stabilizer",
                gr,
                &x[p..n],
                &[omega, pe, vt, pe / self.scale],
            );
        }
        out
    }

    /// Evaluate the device right-hand side `f` (local) and its outputs.
    pub(crate) fn eval(
        &self,
        x: &[f64],
        vr: f64,
        vi: f64,
        omega_sys: f64,
        f: &mut [f64],
    ) -> GenOut {
        if let Some(frozen) = &self.tripped {
            for i in 0..f.len() {
                f[i] = frozen[i] - x[i];
            }
            return GenOut::default();
        }
        let [m, s, a, g, p] = self.layout();
        let n = self.n_states();
        // One sine and cosine of the rotor angle for both frame changes.
        let sc = x[s].sin_cos();
        let omega = x[s + 1];
        let alg = self.machine.algebraic_sc(&x[m..s], sc, vr, vi);
        let vt = mag(vr, vi);
        let (fm, rest) = f.split_at_mut(s);
        let (fs, rest) = rest.split_at_mut(2);
        let (fa, rest) = rest.split_at_mut(g - a);
        let (fg, fp) = rest.split_at_mut(p - g);
        let vs = self
            .pss
            .eval(&x[p..n], omega, alg.te * self.scale, vt, self.scale, fp);
        let (ir, ii) = dq_ri_sc(sc, alg.id, alg.iq);
        let vf = self.avr.eval(
            &x[a..g],
            super::models::AvrIn {
                vt,
                vs,
                xad_ifd: alg.xad_ifd,
                v_ref: self.v_ref,
            },
            fa,
        );
        let tm = self.gov.eval(&x[g..p], omega, self.p_ref, fg);
        self.machine.ode(&x[m..s], &alg, vf, fm);
        let (fd, fw) = self.shaft.ode(omega, omega_sys, alg.te, tm, self.f0);
        fs[0] = fd;
        fs[1] = fw;
        GenOut {
            ir: ir * self.scale,
            ii: ii * self.scale,
            tm,
            vf,
            vs,
        }
    }
}

/// A Thevenin source `E` behind `Z`, as a Norton equivalent: `y` enters the
/// admittance matrix and `y E` is a constant injection.
#[derive(Clone, Debug)]
pub(crate) struct Source {
    pub bus_number: usize,
    pub machine_id: String,
    pub bus: usize,
    pub y: C64,
    pub e: C64,
}

/// The static loads at one bus (PowerSimulationsDynamics.jl
/// `mdl_zip_load!`, generalized to any voltage exponent), each part stated
/// as its power at the power flow voltage `v0` (system per unit, drawn
/// positive). A part with exponent `γ` draws
///
/// ```text
/// P(V) = P0 (V / v0)^γ          (the same for Q)
/// ```
///
/// so at `v0` every part draws exactly what the power flow solved it with.
/// Constant impedance (`γ = 2`) enters the admittance matrix; constant
/// current (`γ = 1`), constant power (`γ = 0`) and the other exponents are
/// the nonlinear current `I = (P − jQ) V / |V|²` ([`Self::nonlinear_current`]),
/// with their low-voltage behaviour set by [`LowVoltage`].
#[derive(Clone, Debug, Default)]
pub(crate) struct BusLoad {
    pub v0: f64,
    pub p_z: f64,
    pub q_z: f64,
    pub p_i: f64,
    pub q_i: f64,
    pub p_p: f64,
    pub q_p: f64,
    /// Parts with another exponent (an exponential load).
    pub exp: Vec<ExpPart>,
    /// Constant current drawn for the units held at their power flow output
    /// (negative: they inject), at `v0`. Not a load: no low-voltage
    /// conversion, no load scaling, and no `pl`/`ql` channel.
    pub u_p: f64,
    pub u_q: f64,
}

/// A load part `s0 (V / v0)^gamma` with an exponent other than 0, 1 or 2.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ExpPart {
    pub reactive: bool,
    /// Power at the power flow voltage.
    pub s0: f64,
    pub gamma: f64,
}

/// How the parts with an exponent below 2 behave at low voltage (a part
/// with a larger exponent keeps its law: its current falls faster than an
/// admittance's).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum LowVoltage {
    /// PSS/E `PQBRAK`: below the breakpoint (pu, capped at the power flow
    /// voltage so a bus that starts below it starts on its own law) the part
    /// draws the constant admittance of its power at the breakpoint,
    /// `P(V) = P(V_b) (V / V_b)²`: continuous in value, and the current falls
    /// linearly to zero at 0 V.
    Break(f64),
    /// The stated law at any voltage down to 20 % of the power flow voltage,
    /// below that a smooth floor of the magnitude the parts divide by (half
    /// of it at 0 V), so the current keeps a direction at 0 V.
    /// PowerSimulationsDynamics.jl applies the law at every voltage.
    Floor,
}

/// `w(V) = (u / v0)^γ / u²` and `(dw/dV) / V` for a part with exponent
/// `gamma`: the part's current is `P0 w(V) V` (rotated by `−jQ0`). `u` is the
/// magnitude the law is evaluated at ([`LowVoltage`]).
#[inline]
fn part_weight(gamma: f64, vm: f64, v0: f64, lv: LowVoltage) -> (f64, f64) {
    // `u`, and `(du/dV) / V`. A part with `γ > 2` falls faster than an
    // admittance and needs no conversion; its law holds down to 0.1 % of
    // `V0`, where `u` stops so `u^(γ − 2)` keeps a finite slope.
    let (u, du) = match lv {
        _ if gamma > 2.0 => {
            let e = 1e-3 * v0;
            if vm > e {
                (vm, 1.0 / vm)
            } else {
                (e, 0.0)
            }
        }
        LowVoltage::Break(vb) => {
            let vb = vb.min(v0);
            if vm > vb {
                (vm, 1.0 / vm)
            } else {
                (vb, 0.0)
            }
        }
        LowVoltage::Floor => {
            let e = 0.2 * v0;
            if vm >= e {
                (vm, 1.0 / vm)
            } else {
                (0.5 * e * ((vm / e).powi(2) + 1.0), 1.0 / e)
            }
        }
    };
    let w = if gamma == 0.0 {
        1.0 / (u * u)
    } else if gamma == 1.0 {
        1.0 / (v0 * u)
    } else {
        (u / v0).powf(gamma) / (u * u)
    };
    (w, (gamma - 2.0) * w / u * du)
}

impl BusLoad {
    /// Constant-impedance part as an admittance (enters `Y`).
    pub(crate) fn admittance(&self) -> C64 {
        C64::new(self.p_z, -self.q_z) / (self.v0 * self.v0)
    }

    /// `(G, B, dG/dV / V, dB/dV / V)` of the nonlinear parts: their current
    /// is `(G + jB)^* V`-like, `Ir = G Vr + B Vi`, `Ii = G Vi − B Vr`.
    fn weights(&self, vm: f64, lv: LowVoltage) -> [f64; 4] {
        let mut out = [0.0; 4];
        let mut add = |p: f64, q: f64, (w, dw): (f64, f64)| {
            out[0] += p * w;
            out[1] += q * w;
            out[2] += p * dw;
            out[3] += q * dw;
        };
        if self.p_p != 0.0 || self.q_p != 0.0 {
            add(self.p_p, self.q_p, part_weight(0.0, vm, self.v0, lv));
        }
        if self.p_i != 0.0 || self.q_i != 0.0 {
            add(self.p_i, self.q_i, part_weight(1.0, vm, self.v0, lv));
        }
        for e in &self.exp {
            let w = part_weight(e.gamma, vm, self.v0, lv);
            if e.reactive {
                add(0.0, e.s0, w);
            } else {
                add(e.s0, 0.0, w);
            }
        }
        if self.u_p != 0.0 || self.u_q != 0.0 {
            add(
                self.u_p,
                self.u_q,
                part_weight(1.0, vm, self.v0, LowVoltage::Floor),
            );
        }
        out
    }

    /// Current drawn by every part but the constant impedance.
    pub(crate) fn nonlinear_current(&self, vr: f64, vi: f64, lv: LowVoltage) -> (f64, f64) {
        if !self.is_nonlinear() {
            return (0.0, 0.0);
        }
        let [g, b, ..] = self.weights(mag(vr, vi), lv);
        (g * vr + b * vi, g * vi - b * vr)
    }

    /// `∂(Ir, Ii)/∂(Vr, Vi)` of [`Self::nonlinear_current`], exact:
    /// `[[∂Ir/∂Vr, ∂Ir/∂Vi], [∂Ii/∂Vr, ∂Ii/∂Vi]]`.
    pub(crate) fn nonlinear_jacobian(&self, vr: f64, vi: f64, lv: LowVoltage) -> [[f64; 2]; 2] {
        let [g, b, dg, db] = self.weights(mag(vr, vi), lv);
        // ∂G/∂Vr = (dG/dV / V) Vr, and so on.
        let (ir_g, ir_b) = (vr, vi);
        let (ii_g, ii_b) = (vi, -vr);
        [
            [
                g + (ir_g * dg + ir_b * db) * vr,
                b + (ir_g * dg + ir_b * db) * vi,
            ],
            [
                -b + (ii_g * dg + ii_b * db) * vr,
                g + (ii_g * dg + ii_b * db) * vi,
            ],
        ]
    }

    pub(crate) fn is_nonlinear(&self) -> bool {
        self.p_i != 0.0
            || self.q_i != 0.0
            || self.p_p != 0.0
            || self.q_p != 0.0
            || self.u_p != 0.0
            || self.u_q != 0.0
            || !self.exp.is_empty()
    }

    /// Whether any load part (not a held unit) is stated.
    /// Active power drawn at the power flow voltage (system per unit).
    pub(crate) fn p0(&self) -> f64 {
        self.p_z
            + self.p_i
            + self.p_p
            + self
                .exp
                .iter()
                .filter(|e| !e.reactive)
                .map(|e| e.s0)
                .sum::<f64>()
    }

    pub(crate) fn has_load(&self) -> bool {
        self.p_z != 0.0
            || self.q_z != 0.0
            || self.p_i != 0.0
            || self.q_i != 0.0
            || self.p_p != 0.0
            || self.q_p != 0.0
            || !self.exp.is_empty()
    }

    /// Scale every load part (not the held units) by `(p, q)`.
    pub(crate) fn scale(&mut self, p: f64, q: f64) {
        self.p_z *= p;
        self.p_i *= p;
        self.p_p *= p;
        self.q_z *= q;
        self.q_i *= q;
        self.q_p *= q;
        for e in &mut self.exp {
            e.s0 *= if e.reactive { q } else { p };
        }
    }

    /// Complex power the load parts (not the held units) draw at `v`.
    pub(crate) fn power(&self, v: C64, lv: LowVoltage) -> C64 {
        let held = BusLoad {
            v0: self.v0,
            u_p: self.u_p,
            u_q: self.u_q,
            ..BusLoad::default()
        };
        let (ir, ii) = self.nonlinear_current(v.re, v.im, lv);
        let (hr, hi) = held.nonlinear_current(v.re, v.im, lv);
        let i = C64::new(ir - hr, ii - hi) + self.admittance() * v;
        v * i.conj()
    }

    /// Complex power the load parts draw at the power flow voltage.
    #[cfg(feature = "rms-experimental")]
    pub(crate) fn power_at_v0(&self) -> C64 {
        let (p, q) = self.exp.iter().fold(
            (
                self.p_z + self.p_i + self.p_p,
                self.q_z + self.q_i + self.q_p,
            ),
            |(p, q), e| {
                if e.reactive {
                    (p, q + e.s0)
                } else {
                    (p + e.s0, q)
                }
            },
        );
        C64::new(p, q)
    }
}

/// A fault inside a branch: at `alpha` of the series impedance from the
/// from end, to ground through admittance `y`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct BranchFault {
    pub alpha: f64,
    pub y: C64,
    /// A transient fault dies out once both ends are open.
    pub permanent: bool,
}

/// The voltage magnitude a held unit's constant current divides by: the
/// voltage itself above 20 % of the power flow voltage, below it a smooth
/// floor (half of that at 0 V), so the current keeps a direction at 0 V.
fn soft_vm(vm: f64, v0: f64) -> f64 {
    let e = 0.2 * v0;
    if vm >= e {
        vm
    } else {
        0.5 * e * ((vm / e).powi(2) + 1.0)
    }
}

/// A unit without a simulated model (an inverter chain), held as a
/// constant-current injection `conj(s) V/|V| / v0` inside its bus's load.
#[derive(Clone, Debug)]
pub(crate) struct HeldUnit {
    pub bus_number: usize,
    pub machine_id: String,
    pub bus: usize,
    pub s: C64,
    pub v0: f64,
    pub mbase: f64,
    pub tripped: bool,
    /// A simulated renewable plant (`REGCA1` + `REECB1` + `REPCA1`) with its
    /// states at `offset`; `None`: the unit is held as a constant current
    /// inside its bus load.
    pub plant: Option<Box<super::renewable::Plant>>,
    pub offset: usize,
}

impl HeldUnit {
    /// Number of states (0 without a plant model).
    pub(crate) fn n_states(&self) -> usize {
        self.plant.as_ref().map_or(0, |p| p.n_states())
    }

    /// Current out of a unit held as a constant current, at bus voltage `v`.
    pub(crate) fn current(&self, v: C64) -> C64 {
        if self.tripped {
            return C64::new(0.0, 0.0);
        }
        self.s.conj() * (v / soft_vm(v.norm(), self.v0)) / self.v0
    }

    /// Current out of the unit (system base) at the global state `x` and
    /// bus voltage `v`, for either kind.
    pub(crate) fn current_at(&self, x: &[f64], v: C64) -> C64 {
        match &self.plant {
            _ if self.tripped => C64::new(0.0, 0.0),
            Some(p) => p.output_current(&x[self.offset..self.offset + p.n_states()], v) * p.scale,
            None => self.current(v),
        }
    }

    /// Local mass diagonal of a plant; a tripped plant holds its states
    /// (`ẋ = 0`).
    pub(crate) fn mass(&self) -> Vec<f64> {
        match &self.plant {
            Some(_) if self.tripped => vec![1.0; self.n_states()],
            Some(p) => p.mass(),
            None => Vec::new(),
        }
    }
}

/// The assembled RMS system.
#[derive(Clone, Debug)]
pub(crate) struct DynSystem {
    pub n: usize,
    pub bus_numbers: Vec<usize>,
    pub sbase: f64,
    pub f0: f64,
    pub ac: AcNetwork,
    /// `(from bus, to bus, circuit)` of each dense branch.
    pub branch_keys: Vec<(usize, usize, String)>,
    pub devices: Vec<GenDevice>,
    pub sources: Vec<Source>,
    pub loads: Vec<BusLoad>,
    /// Fault shunt admittance per bus.
    pub fault_y: Vec<C64>,
    pub ybus: Vec<(usize, usize, C64)>,
    pub nx: usize,
    pub mass: Vec<f64>,
    /// Global index of the reference-frame speed, or `None` for a constant
    /// synchronous frame (a source holds the reference bus).
    pub omega_ref: Option<usize>,
    /// Bumped whenever the admittance matrix or the mass matrix changes, so a
    /// cached Jacobian factorization is not reused across an event.
    pub epoch: u64,
    /// Per branch, whether its `[from, to]` end breaker is open.
    pub end_open: Vec<[bool; 2]>,
    /// Per branch, a fault inside it.
    pub branch_faults: Vec<Option<BranchFault>>,
    pub held: Vec<HeldUnit>,
    /// Low-voltage behaviour of the constant-current and constant-power load
    /// parts ([`super::RmsOptions::low_voltage_load`]).
    pub low_voltage: LowVoltage,
    /// Factorize step matrices by blocks where possible
    /// ([`super::RmsOptions::factorize_by_blocks`]).
    pub factorize_by_blocks: bool,
    /// The islands apart from the reference already named in the notes.
    pub noted_islands: Vec<Vec<usize>>,
}

impl DynSystem {
    /// Rebuild the admittance matrix after a topology or load change.
    pub(crate) fn rebuild_ybus(&mut self) {
        self.epoch += 1;
        let mut y = self.ac.ybus();
        // Branches with an open end or an inner fault: swap their standard
        // stamp for the reduced one.
        for e in 0..self.ac.m {
            if !self.is_special(e) {
                continue;
            }
            let (f, t) = (self.ac.br_from[e], self.ac.br_to[e]);
            let (yff, yft, ytf, ytt) = self.ac.branch_admittance(e);
            let r = self.branch_terminal_y(e);
            for (i, j, std, new) in [
                (f, f, yff, r[0][0]),
                (f, t, yft, r[0][1]),
                (t, f, ytf, r[1][0]),
                (t, t, ytt, r[1][1]),
            ] {
                let d = new - std;
                if d != C64::new(0.0, 0.0) {
                    y.push((i, j, d));
                }
            }
        }
        let dead = self.dead_buses();
        for b in 0..self.n {
            let mut extra = self.fault_y[b] + self.loads[b].admittance();
            // A bus in an island without a live unit holds 0 V (a unit
            // shunt keeps its rows regular).
            if dead[b] {
                extra += C64::new(1.0, 0.0);
            }
            for s in self.sources.iter().filter(|s| s.bus == b) {
                extra += s.y;
            }
            if extra != C64::new(0.0, 0.0) {
                y.push((b, b, extra));
            }
        }
        self.ybus = y;
    }

    pub(crate) fn rebuild_mass(&mut self) {
        self.epoch += 1;
        let mut mass = vec![0.0; self.nx];
        for d in &self.devices {
            mass[d.offset..d.offset + d.n_states()].copy_from_slice(&d.mass());
        }
        for h in self.held.iter().filter(|h| h.plant.is_some()) {
            mass[h.offset..h.offset + h.n_states()].copy_from_slice(&h.mass());
        }
        self.mass = mass;
    }

    pub(crate) fn omega_sys(&self, x: &[f64]) -> f64 {
        self.omega_ref.map_or(1.0, |i| x[i])
    }

    /// `f(x)`, and the device outputs when `outs` is given.
    pub(crate) fn residual(&self, x: &[f64], f: &mut [f64], mut outs: Option<&mut Vec<GenOut>>) {
        let n = self.n;
        f.iter_mut().for_each(|v| *v = 0.0);
        let (vr, rest) = x.split_at(n);
        let vi = &rest[..n];
        for &(r, c, y) in &self.ybus {
            f[r] -= y.re * vr[c] - y.im * vi[c];
            f[n + r] -= y.im * vr[c] + y.re * vi[c];
        }
        for s in &self.sources {
            let i = s.y * s.e;
            f[s.bus] += i.re;
            f[n + s.bus] += i.im;
        }
        for (b, load) in self.loads.iter().enumerate() {
            if load.is_nonlinear() {
                let (ir, ii) = load.nonlinear_current(vr[b], vi[b], self.low_voltage);
                f[b] -= ir;
                f[n + b] -= ii;
            }
        }
        let w_sys = self.omega_sys(x);
        if let Some(outs) = outs.as_deref_mut() {
            outs.clear();
        }
        for d in &self.devices {
            let k = d.n_states();
            let out = d.eval(
                &x[d.offset..d.offset + k],
                vr[d.bus],
                vi[d.bus],
                w_sys,
                &mut f[d.offset..d.offset + k],
            );
            f[d.bus] += out.ir;
            f[n + d.bus] += out.ii;
            if let Some(outs) = outs.as_deref_mut() {
                outs.push(out);
            }
        }
        for h in &self.held {
            let Some(p) = &h.plant else { continue };
            let k = p.n_states();
            if h.tripped {
                f[h.offset..h.offset + k].iter_mut().for_each(|v| *v = 0.0);
                continue;
            }
            let i = p.eval(
                &x[h.offset..h.offset + k],
                vr[h.bus],
                vi[h.bus],
                w_sys,
                &mut f[h.offset..h.offset + k],
            );
            f[h.bus] += i.re;
            f[n + h.bus] += i.im;
        }
    }

    /// Every unit with states, `(first state, states, bus)`, in state order:
    /// the generators, then the simulated renewable plants.
    pub(crate) fn state_units(&self) -> Vec<(usize, usize, usize)> {
        let mut u: Vec<(usize, usize, usize)> = self
            .devices
            .iter()
            .map(|d| (d.offset, d.n_states(), d.bus))
            .chain(
                self.held
                    .iter()
                    .filter(|h| h.plant.is_some())
                    .map(|h| (h.offset, h.n_states(), h.bus)),
            )
            .collect();
        u.sort_unstable();
        u
    }

    /// `∂f/∂x` as triplets. The network and load parts are exact; device
    /// blocks are forward differences of their local functions with the
    /// scaled step of [`fd_step`].
    pub(crate) fn jacobian(&self, x: &[f64]) -> Vec<(usize, usize, f64)> {
        let n = self.n;
        let mut t: Vec<(usize, usize, f64)> = Vec::with_capacity(self.ybus.len() * 4 + self.nx * 8);
        for &(r, c, y) in &self.ybus {
            t.push((r, c, -y.re));
            t.push((r, n + c, y.im));
            t.push((n + r, c, -y.im));
            t.push((n + r, n + c, -y.re));
        }
        for (b, load) in self.loads.iter().enumerate() {
            if !load.is_nonlinear() {
                continue;
            }
            let j = load.nonlinear_jacobian(x[b], x[n + b], self.low_voltage);
            t.push((b, b, -j[0][0]));
            t.push((b, n + b, -j[0][1]));
            t.push((n + b, b, -j[1][0]));
            t.push((n + b, n + b, -j[1][1]));
        }
        let w_sys = self.omega_sys(x);
        for d in &self.devices {
            let eval = |xl: &[f64], vr: f64, vi: f64, w: f64, f: &mut [f64]| {
                let o = d.eval(xl, vr, vi, w, f);
                (o.ir, o.ii)
            };
            self.unit_block(x, w_sys, d.offset, d.n_states(), d.bus, eval, &mut t);
        }
        for h in self.held.iter().filter(|h| !h.tripped) {
            let Some(p) = &h.plant else { continue };
            let eval = |xl: &[f64], vr: f64, vi: f64, w: f64, f: &mut [f64]| {
                let i = p.eval(xl, vr, vi, w, f);
                (i.re, i.im)
            };
            self.unit_block(x, w_sys, h.offset, p.n_states(), h.bus, eval, &mut t);
        }
        t
    }

    /// Forward-difference block of one unit with `k` states at `off` on bus
    /// `bus`: its rows against its states, its bus voltage and the frame
    /// speed, and its bus current against the same.
    #[allow(clippy::too_many_arguments)]
    fn unit_block(
        &self,
        x: &[f64],
        w_sys: f64,
        off: usize,
        k: usize,
        bus: usize,
        unit_eval: impl Fn(&[f64], f64, f64, f64, &mut [f64]) -> (f64, f64),
        t: &mut Vec<(usize, usize, f64)>,
    ) {
        let n = self.n;
        let own_ref = self.omega_ref.filter(|&i| i >= off && i < off + k);
        let ext_ref = self.omega_ref.filter(|_| own_ref.is_none());
        let mut xl: Vec<f64> = x[off..off + k].to_vec();
        let (mut vr, mut vi) = (x[bus], x[n + bus]);
        let eval = |xl: &[f64], vr: f64, vi: f64, w: f64, f: &mut [f64]| {
            let w = own_ref.map_or(w, |i| xl[i - off]);
            unit_eval(xl, vr, vi, w, f)
        };
        let mut f0 = vec![0.0; k];
        let (ir0, ii0) = eval(&xl, vr, vi, w_sys, &mut f0);
        let mut f1 = vec![0.0; k];
        let push_col =
            |col: usize, f1: &[f64], ir: f64, ii: f64, h: f64, t: &mut Vec<(usize, usize, f64)>| {
                for r in 0..k {
                    let v = (f1[r] - f0[r]) / h;
                    if v != 0.0 {
                        t.push((off + r, col, v));
                    }
                }
                let dr = (ir - ir0) / h;
                let di = (ii - ii0) / h;
                if dr != 0.0 {
                    t.push((bus, col, dr));
                }
                if di != 0.0 {
                    t.push((n + bus, col, di));
                }
            };
        for j in 0..k {
            let keep = xl[j];
            let h = fd_step(keep);
            xl[j] += h;
            let (ir, ii) = eval(&xl, vr, vi, w_sys, &mut f1);
            xl[j] = keep;
            push_col(off + j, &f1, ir, ii, h, t);
        }
        let keep = vr;
        let h = fd_step(keep);
        vr += h;
        let (ir, ii) = eval(&xl, vr, vi, w_sys, &mut f1);
        vr = keep;
        push_col(bus, &f1, ir, ii, h, t);
        let keep = vi;
        let h = fd_step(keep);
        vi += h;
        let (ir, ii) = eval(&xl, vr, vi, w_sys, &mut f1);
        vi = keep;
        push_col(n + bus, &f1, ir, ii, h, t);
        if let Some(ri) = ext_ref {
            let h = fd_step(w_sys);
            let (ir, ii) = eval(&xl, vr, vi, w_sys + h, &mut f1);
            push_col(ri, &f1, ir, ii, h, t);
        }
    }

    /// Complex power flows `(S_from, S_to)` of dense branch `e`, system base.
    pub(crate) fn branch_flow(&self, x: &[f64], e: usize) -> (C64, C64) {
        let (vf, vt, i_f, i_t) = self.branch_currents(x, e);
        (vf * i_f.conj(), vt * i_t.conj())
    }

    /// End voltages and the currents into the branch at its ends.
    pub(crate) fn branch_currents(&self, x: &[f64], e: usize) -> (C64, C64, C64, C64) {
        let y = self.branch_terminal_y(e);
        let f = self.ac.br_from[e];
        let t = self.ac.br_to[e];
        let vf = C64::new(x[f], x[self.n + f]);
        let vt = C64::new(x[t], x[self.n + t]);
        (
            vf,
            vt,
            y[0][0] * vf + y[0][1] * vt,
            y[1][0] * vf + y[1][1] * vt,
        )
    }

    fn is_special(&self, e: usize) -> bool {
        self.ac.sw[e] != 0.0
            && (self.end_open.get(e).is_some_and(|o| o[0] || o[1])
                || self.branch_faults.get(e).is_some_and(Option::is_some))
    }

    /// The branch's terminal admittance `[[Yff, Yft], [Ytf, Ytt]]` with its
    /// open ends and inner fault: the series impedance split at the fault
    /// point (`α z`, `(1 − α) z`, the fault admittance to ground there),
    /// the inner nodes and open ends eliminated (Kron reduction), then
    /// referred through the from-end tap.
    pub(crate) fn branch_terminal_y(&self, e: usize) -> [[C64; 2]; 2] {
        let zero = C64::new(0.0, 0.0);
        if !self.is_special(e) {
            let (yff, yft, ytf, ytt) = self.ac.branch_admittance(e);
            return [[yff, yft], [ytf, ytt]];
        }
        let ac = &self.ac;
        let y = C64::new(ac.g[e], ac.b[e]);
        let tap = C64::from_polar(ac.tap[e], ac.shift[e]);
        let y_fr = C64::new(ac.g_fr[e], ac.b_fr[e]);
        let y_to = C64::new(ac.g_to[e], ac.b_to[e]);
        let (alpha, yf) =
            self.branch_faults[e].map_or((0.5, zero), |f| (f.alpha.clamp(1e-4, 1.0 - 1e-4), f.y));
        let y1 = y / alpha;
        let y2 = y / (1.0 - alpha);
        // Local nodes: 0 from (behind the tap), 1 to, 2 the fault point.
        let full = [
            [y1 + y_fr, zero, -y1],
            [zero, y2 + y_to, -y2],
            [-y1, -y2, y1 + y2 + yf],
        ];
        let open = self.end_open[e];
        let ext: Vec<usize> = (0..2).filter(|&k| !open[k]).collect();
        let int: Vec<usize> = (0..3).filter(|k| !ext.contains(k)).collect();
        let red = kron(&full, &ext, &int);
        let mut out = [[zero; 2]; 2];
        for (a, &i) in ext.iter().enumerate() {
            for (b, &j) in ext.iter().enumerate() {
                out[i][j] = red[a][b];
            }
        }
        let tap2 = tap.norm_sqr();
        [
            [out[0][0] / tap2, out[0][1] / tap.conj()],
            [out[1][0] / tap, out[1][1]],
        ]
    }

    /// The electrical islands over branches closed at both ends, each its
    /// dense buses in ascending order.
    pub(crate) fn islands(&self) -> Vec<Vec<usize>> {
        let n = self.n;
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
        for e in 0..self.ac.m {
            if self.ac.sw[e] != 0.0 && !self.end_open[e][0] && !self.end_open[e][1] {
                let (f, t) = (self.ac.br_from[e], self.ac.br_to[e]);
                adj[f].push(t);
                adj[t].push(f);
            }
        }
        let mut seen = vec![false; n];
        let mut islands = Vec::new();
        for start in 0..n {
            if seen[start] {
                continue;
            }
            let mut stack = vec![start];
            let mut members = Vec::new();
            seen[start] = true;
            while let Some(b) = stack.pop() {
                members.push(b);
                for &o in &adj[b] {
                    if !seen[o] {
                        seen[o] = true;
                        stack.push(o);
                    }
                }
            }
            members.sort_unstable();
            islands.push(members);
        }
        islands
    }

    /// Buses in islands (over branches closed at both ends) without a
    /// unit in service.
    pub(crate) fn dead_buses(&self) -> Vec<bool> {
        let n = self.n;
        let mut live = vec![false; n];
        for d in self.devices.iter().filter(|d| d.tripped.is_none()) {
            live[d.bus] = true;
        }
        for s in self.sources.iter().filter(|s| s.y != C64::new(0.0, 0.0)) {
            live[s.bus] = true;
        }
        for h in self.held.iter().filter(|h| !h.tripped) {
            live[h.bus] = true;
        }
        let mut dead = vec![false; n];
        for island in self.islands() {
            if !island.iter().any(|&b| live[b]) {
                for b in island {
                    dead[b] = true;
                }
            }
        }
        dead
    }

    /// The islands apart from the reference (slack) bus that hold a unit in
    /// service, each named: its buses, units and load.
    pub(crate) fn detached_islands(&self) -> Vec<(Vec<usize>, String)> {
        let reference = self.ac.slacks.first().copied();
        self.islands()
            .into_iter()
            .filter(|island| reference.is_none_or(|r| island.binary_search(&r).is_err()))
            .filter_map(|island| {
                let inside = |b: usize| island.binary_search(&b).is_ok();
                let mut units: Vec<String> = Vec::new();
                for d in self.devices.iter().filter(|d| d.tripped.is_none()) {
                    if inside(d.bus) {
                        units.push(format!("{} '{}'", d.bus_number, d.machine_id));
                    }
                }
                for h in self.held.iter().filter(|h| !h.tripped) {
                    if inside(h.bus) {
                        units.push(format!("{} '{}' (inverter)", h.bus_number, h.machine_id));
                    }
                }
                if units.is_empty() {
                    return None;
                }
                let load: f64 = island.iter().map(|&b| self.loads[b].p0()).sum();
                let mut buses: Vec<String> = island
                    .iter()
                    .take(10)
                    .map(|&b| self.bus_numbers[b].to_string())
                    .collect();
                if island.len() > 10 {
                    buses.push(format!("… ({} buses)", island.len()));
                }
                let text = format!(
                    "buses {} with units {} and {:.1} MW of load",
                    buses.join(", "),
                    units.join(", "),
                    load * self.sbase
                );
                Some((island, text))
            })
            .collect()
    }

    /// Whether every path from the network into branch `e` is open.
    pub(crate) fn branch_isolated(&self, e: usize) -> bool {
        self.ac.sw[e] == 0.0 || self.end_open[e].iter().all(|&o| o)
    }

    pub(crate) fn dense_bus(&self, number: usize) -> Option<usize> {
        self.bus_numbers.iter().position(|&b| b == number)
    }
}

/// The forward-difference step for a variable at `x`: [`FD_REL`] `· max(|x|, 1)`,
/// rounded to the step `x + h − x` actually taken, which the difference
/// quotient divides by (dividing by the nominal step instead puts a relative
/// error of up to `ε |x| / h` into every column).
#[inline]
pub(crate) fn fd_step(x: f64) -> f64 {
    let h = FD_REL * x.abs().max(1.0);
    (x + h) - x
}

/// The relative forward-difference step. The optimum balances truncation
/// (`h |f''| / 2`) against rounding (`ε F / h`, `F` the largest term that
/// cancels in `f`): `h = √(2 ε F / |f''|)`, which is `√ε ≈ 1.5e-8` only for
/// `F ≈ |f''|`. Device rows divide by time constants down to milliseconds,
/// so `F` exceeds the curvature by about two orders and the optimum is near
/// `10 √ε`. With `√ε` the block-graph controllers, whose rows are the same
/// functions summed in another order, depart from the compiled models by up
/// to 7.5e-6 (`tests/rms_graph.rs`); with `1e-7` they agree to 1e-9.
pub(crate) const FD_REL: f64 = 1e-7;

/// `Y_EE − Y_EI Y_II⁻¹ Y_IE` of a 3×3 admittance matrix.
fn kron(y: &[[C64; 3]; 3], ext: &[usize], int: &[usize]) -> Vec<Vec<C64>> {
    let zero = C64::new(0.0, 0.0);
    let ni = int.len();
    // Invert Y_II (at most 3×3) by Gauss–Jordan.
    let mut a: Vec<Vec<C64>> = int
        .iter()
        .map(|&i| int.iter().map(|&j| y[i][j]).collect())
        .collect();
    let mut inv: Vec<Vec<C64>> = (0..ni)
        .map(|i| {
            (0..ni)
                .map(|j| if i == j { C64::new(1.0, 0.0) } else { zero })
                .collect()
        })
        .collect();
    for c in 0..ni {
        let p = (c..ni)
            .max_by(|&i, &j| a[i][c].norm().total_cmp(&a[j][c].norm()))
            .unwrap_or(c);
        a.swap(c, p);
        inv.swap(c, p);
        let d = a[c][c];
        if d.norm() < 1e-300 {
            continue;
        }
        for k in 0..ni {
            a[c][k] /= d;
            inv[c][k] /= d;
        }
        for r in 0..ni {
            if r != c {
                let fct = a[r][c];
                if fct != zero {
                    for k in 0..ni {
                        let (ack, ick) = (a[c][k], inv[c][k]);
                        a[r][k] -= fct * ack;
                        inv[r][k] -= fct * ick;
                    }
                }
            }
        }
    }
    ext.iter()
        .map(|&i| {
            ext.iter()
                .map(|&j| {
                    let mut v = y[i][j];
                    for (a_, &p) in int.iter().enumerate() {
                        for (b_, &q) in int.iter().enumerate() {
                            v -= y[i][p] * inv[a_][b_] * y[q][j];
                        }
                    }
                    v
                })
                .collect()
        })
        .collect()
}
