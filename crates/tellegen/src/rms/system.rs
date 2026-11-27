//! The differential-algebraic system `M ẋ = f(x)` of a network with dynamic
//! generators, Thevenin sources and static loads.
//!
//! The formulation follows PowerSimulationsDynamics.jl (`src/models/system.jl`,
//! `network_model.jl`, `load_models.jl`, `source_models.jl`; BSD-3-Clause, see
//! `licenses/PowerSimulationsDynamics.jl-LICENSE`): the state vector is
//! `[Vr(1..n); Vi(1..n); device states]`, the bus rows are the algebraic
//! current balance `Σ I_injected − Y V = 0` in system per unit, and each
//! generator adds its stator current to its bus rows.
//!
//! @block Differential-algebraic system

use std::collections::HashMap;

use num_complex::Complex;

use powerio::electrical::{calc_squared_voltage_admittance, calc_voltage_power_term};
use powerio_dist::kron::calc_kron_reduced_impedance;
use powerio_matrix::{AcBranchData, AcBranchFlows};

use super::models::{dq_ri_sc, mag, Avr, Governor, Machine, Shaft, Stabilizer};
use crate::model::AcNetwork;

pub(crate) type C64 = Complex<f64>;

/// One synchronous generator with its controls.
// @block Generating unit
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
    /// Experimental: external signal at the summing point, added to the
    /// stabilizer output (`InputSignal::Vs`).
    pub vs_ext: f64,
    /// First global state index.
    pub offset: usize,
    /// States frozen by a generator trip.
    pub tripped: Option<Vec<f64>>,
    /// Experimental: under-excitation limiter `[q_lim, gain]`, see [`set_uel`].
    pub uel: Option<[f64; 2]>,
    /// Experimental: load compensator of the AVR input `[rc, xc]`, see [`set_vcomp`].
    pub vcomp: Option<[f64; 3]>,
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
            + self.lag_states()
    }

    /// Experimental: the compensator's lag state (one with a time constant, else none).
    pub(crate) fn lag_states(&self) -> usize {
        usize::from(self.vcomp.is_some_and(|c| c[2] > 0.0))
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
        if self.lag_states() > 0 {
            names.push("vcomp_lag".into());
        }
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
        let pe = p + self.pss.n_states();
        self.pss.mass(&mut m[p..pe]);
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
            .eval(&x[p..p + self.pss.n_states()], omega, pe, vt, self.scale, &mut scratch[p..p + self.pss.n_states()])
            + self.vs_ext;
        let mut out = Vec::new();
        let mut push = |slot: &str, gr: &super::graph::CompiledGraph, xs: &[f64], ports: &[f64]| {
            for (id, v) in gr.node_ids().zip(gr.signals(xs, ports)) {
                out.push((format!("{slot}.{id}"), v));
            }
        };
        if let Avr::Graph(gr) = &self.avr {
            let (ir, ii) = super::models::dq_ri(x[s], alg.id, alg.iq);
            push(
                "exciter",
                gr,
                &x[a..g],
                &[avr_vt(self.vcomp, vr, vi, ir, ii, if self.lag_states() > 0 { x[n - 1] } else { 0.0 }), vs, alg.xad_ifd, self.v_ref, vr * ir + vi * ii, vi * ir - vr * ii],
            );
        }
        if let Governor::Graph(gr) = &self.gov {
            push("governor", gr, &x[g..p], &[omega, self.p_ref]);
        }
        if let Stabilizer::Graph(gr) = &self.pss {
            push(
                "stabilizer",
                gr,
                &x[p..p + self.pss.n_states()],
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
        let (fp, fc) = fp.split_at_mut(self.pss.n_states());
        let vs = self
            .pss
            .eval(&x[p..p + self.pss.n_states()], omega, alg.te * self.scale, vt, self.scale, fp)
            + self.vs_ext;
        let (ir, ii) = dq_ri_sc(sc, alg.id, alg.iq);
        // Experimental: under-excitation limiter of the summing-point type (IEEE Std 421.5,
        // UEL1/UEL2 reduced to its proportional path). The limit is `-q_lim Vt^2` of reactive power on
        // the machine base; absorption beyond it raises the voltage error by `gain` times the excess.
        let vs = match self.uel {
            Some([q_lim, gain]) => {
                let q = vi * ir - vr * ii;
                vs + gain * (-q_lim * vt * vt - q).max(0.0)
            }
            None => vs,
        };
        let vf = self.avr.eval(
            &x[a..g],
            super::models::AvrIn {
                vt: avr_vt(self.vcomp, vr, vi, ir, ii, if self.lag_states() > 0 { x[n - 1] } else { 0.0 }),
                vs,
                xad_ifd: alg.xad_ifd,
                v_ref: self.v_ref,
                p: vr * ir + vi * ii,
                q: vi * ir - vr * ii,
            },
            fa,
        );
        if let (Some([_, _, t]), 1) = (self.vcomp, self.lag_states()) {
            // the compensator's lag of the reactive current (machine base)
            fc[0] = ((vi * ir - vr * ii) / vt.max(1e-3) - x[n - 1]) / t;
        }
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
// @block Thevenin source
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
// @block Static load
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
    /// Experimental (load recovery): the slowly following voltage the
    /// constant-power part refers to; 0 = not started (the power flow voltage).
    pub vf: f64,
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

/// Experimental: `TELLEGEN_PQBRAK_SMOOTH=<d>` (pu) rounds off the PQBRAK breakpoint
/// over `Vb ± d` (see [`part_weight`]); unset or 0, the plain break.
fn pqbrak_smooth() -> f64 {
    static D: std::sync::OnceLock<f64> = std::sync::OnceLock::new();
    *D.get_or_init(|| std::env::var("TELLEGEN_PQBRAK_SMOOTH").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0))
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
            // Experimental: a negative value is relative to the bus's
            // power flow voltage (-0.95: the break at 95 % of it).
            let vb = if vb < 0.0 { -vb * v0 } else { vb.min(v0) };
            let d = pqbrak_smooth();
            // Only where the start lies above the band, so every bus starts on its own law.
            if d > 0.0 && v0 - vb >= d && (vm - vb).abs() < d {
                // Experimental: `u = max(V, Vb)` rounded off over `Vb ± d` by a
                // parabola, so `u` and its slope are continuous (the plain break flips the
                // current's slope at `Vb`); `u` is at most `d / 4` above the plain one, at `Vb`.
                let z = vm - vb + d;
                (vb + z * z / (4.0 * d), z / (2.0 * d) / vm)
            } else if vm > vb {
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
    // The law `(u / v0)^γ` and its slope in `u / v0`: PowerIO's voltage
    // power law term.
    let (law, slope) = calc_voltage_power_term(1.0, gamma, u / v0);
    let w = law / (u * u);
    // dw/du = slope / (v0 u²) − 2 w / u.
    (w, (slope / (v0 * u * u) - 2.0 * w / u) * du)
}

// @block Model switches
// Experimental: model options a native caller sets before it builds a
// system: load saturation, load recovery and the one-sided rise of load
// reactive power, under-excitation limiters and AVR load compensators by bus,
// the voltage exponent of plants' reactive set point, the GAST valve rate
// limit, and HVDC links in AC emulation. Each is off unless set, and the
// browser engine never sets them.

// Experimental: saturation of the transformers behind a load above a knee voltage.
// A load of active power P0 draws the extra reactive power `c P0 ((V - v_knee) / 0.1)^2` for
// `V > v_knee` (pu of nominal). Off (c = 0) unless set.
static LOAD_SAT_C: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static LOAD_SAT_VK: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

// Experimental: load recovery. The constant-power part of a load draws
// `P (V / Vf)^alpha`, where `Vf` follows the bus voltage with the time constant `t`: in a fast
// change the demand follows the voltage with the exponent `alpha`, and returns to its power
// with `t` (tap changers, thermostats, converters). Off (alpha = 0) unless set.
static LOAD_DYN_ALPHA: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static LOAD_DYN_T: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Sets the load recovery (experimental).
pub fn set_load_recovery(alpha: f64, t: f64) {
    LOAD_DYN_ALPHA.store(alpha.to_bits(), std::sync::atomic::Ordering::Relaxed);
    LOAD_DYN_T.store(t.to_bits(), std::sync::atomic::Ordering::Relaxed);
}

// Experimental: under-excitation limiters by bus number, `[q_lim, gain]`: reactive power
// absorption limit in pu of MBASE at 1 pu terminal voltage and the limiter gain (pu voltage error per pu
// of reactive power beyond the limit).
// Experimental: HVDC links in AC emulation, `(from bus, to bus, K in MW/degree, limit in MW)`
// by bus number; see [`AcEmulation`].
static HVDC_EMULATION: std::sync::RwLock<Vec<(usize, usize, f64, f64)>> = std::sync::RwLock::new(Vec::new());

/// Sets the HVDC links in AC emulation (experimental): `(from bus, to bus, K MW/degree,
/// limit MW)`. Call before the system is built.
pub fn set_hvdc_emulation(links: Vec<(usize, usize, f64, f64)>) {
    *HVDC_EMULATION.write().expect("hvdc emulation lock") = links;
}

/// Experimental: the AC-emulation part of an HVDC link between two AC buses. The link's
/// set point (its converters, as the case models them) is left as it is; on top, the link moves
/// `dP = K (dtheta - dtheta0)` from `from` to `to`, `dtheta` the angle of `from` against `to` and
/// `dtheta0` that angle at the start (so the start is unchanged), limited to `+-p_max`. As an AC line
/// of reactance `1/K` would, for the active power. Lossless, no own dynamics (the converters'
/// control is fast against electromechanical swings).
// @block HVDC link in AC emulation
#[derive(Clone, Debug)]
pub(crate) struct AcEmulation {
    pub from: usize,
    pub to: usize,
    /// pu power per radian.
    pub k: f64,
    pub p_max: f64,
    pub dtheta0: f64,
}

impl AcEmulation {
    /// The power moved from `from` to `to` (pu) at the given voltages.
    pub(crate) fn power(&self, vr_f: f64, vi_f: f64, vr_t: f64, vi_t: f64) -> f64 {
        let dtheta = (vi_f * vr_t - vr_f * vi_t).atan2(vr_f * vr_t + vi_f * vi_t);
        (self.k * (dtheta - self.dtheta0)).clamp(-self.p_max, self.p_max)
    }

    /// The currents injected at `from` and `to` (real, imaginary), for active power only.
    pub(crate) fn currents(&self, vr_f: f64, vi_f: f64, vr_t: f64, vi_t: f64) -> [f64; 4] {
        let p = self.power(vr_f, vi_f, vr_t, vi_t);
        let (mf, mt) = ((vr_f * vr_f + vi_f * vi_f).max(1e-6), (vr_t * vr_t + vi_t * vi_t).max(1e-6));
        [-p * vr_f / mf, -p * vi_f / mf, p * vr_t / mt, p * vi_t / mt]
    }
}

static UEL: std::sync::RwLock<Option<std::collections::HashMap<usize, [f64; 2]>>> = std::sync::RwLock::new(None);

/// Sets the under-excitation limiters (experimental). Call before the system is built.
pub fn set_uel(map: std::collections::HashMap<usize, [f64; 2]>) {
    *UEL.write().expect("uel lock") = Some(map);
}

pub(crate) fn uel_of(bus_number: usize) -> Option<[f64; 2]> {
    UEL.read().expect("uel lock").as_ref().and_then(|m| m.get(&bus_number).copied())
}

// Experimental: load compensators of the AVR input by bus number, `[rc, xc]` in pu of MBASE
// (IEEE Std 421.5, Vc = |Vt + (Rc + j Xc) It|): positive values give reactive droop.
static VCOMP: std::sync::RwLock<Option<std::collections::HashMap<usize, [f64; 3]>>> = std::sync::RwLock::new(None);

/// Sets the AVR load compensators (experimental). Call before the system is built.
pub fn set_vcomp(map: std::collections::HashMap<usize, [f64; 3]>) {
    *VCOMP.write().expect("vcomp lock") = Some(map);
}

pub(crate) fn vcomp_of(bus_number: usize) -> Option<[f64; 3]> {
    VCOMP.read().expect("vcomp lock").as_ref().and_then(|m| m.get(&bus_number).copied())
}

/// The voltage the regulator sees: the terminal voltage, or the compensated one.
pub(crate) fn avr_vt(vcomp: Option<[f64; 3]>, vr: f64, vi: f64, ir: f64, ii: f64, z: f64) -> f64 {
    match vcomp {
        // with a time constant: the droop through the lag state z (reactive current, Q / Vt)
        Some([rc, xc, t]) if t > 0.0 => mag(vr + rc * ir, vi + rc * ii) + xc * z,
        Some([rc, xc, _]) => mag(vr + rc * ir - xc * ii, vi + rc * ii + xc * ir),
        None => mag(vr, vi),
    }
}

// Experimental: reactive power of a load above its own power flow voltage `v0`:
// the extra `c P0 ((V - v0) / 0.1)^2` for `V > v0`. With a mild law below, this gives the steep
// rise seen for rising voltage without the load supporting the voltage in a dip.
static LOAD_RISE_C: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Sets the one-sided rise of the loads' reactive power (experimental).
pub fn set_load_rise(c: f64) {
    LOAD_RISE_C.store(c.to_bits(), std::sync::atomic::Ordering::Relaxed);
}

// Experimental: voltage exponent of the reactive power set point of renewable plants in
// reactive power control, by bus number: Q_ref (V / V0)^n with the start voltage V0.
static PLANT_QV: std::sync::RwLock<Option<std::collections::HashMap<usize, f64>>> = std::sync::RwLock::new(None);

/// Sets the voltage exponent of the plants' reactive power set point (experimental). Call before the system is built.
pub fn set_plant_q_exponent(map: std::collections::HashMap<usize, f64>) {
    *PLANT_QV.write().expect("plant qv lock") = Some(map);
}

pub(crate) fn plant_qv_of(bus_number: usize) -> f64 {
    PLANT_QV.read().expect("plant qv lock").as_ref().and_then(|m| m.get(&bus_number).copied()).unwrap_or(0.0)
}

// Experimental: opening and closing rate of the GAST fuel valve, pu of the turbine rating per
// second; 0 leaves the plain lag of the model.
static GAST_RATE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Sets the rate limit of the GAST fuel valve (experimental). Call before the system is built.
pub fn set_gast_rate(rate: f64) {
    GAST_RATE.store(rate.to_bits(), std::sync::atomic::Ordering::Relaxed);
}

pub(crate) fn gast_rate() -> f64 {
    f64::from_bits(GAST_RATE.load(std::sync::atomic::Ordering::Relaxed))
}

/// Sets the load saturation (experimental).
pub fn set_load_saturation(c: f64, v_knee: f64) {
    LOAD_SAT_C.store(c.to_bits(), std::sync::atomic::Ordering::Relaxed);
    LOAD_SAT_VK.store(v_knee.to_bits(), std::sync::atomic::Ordering::Relaxed);
}
// @end

impl BusLoad {
    /// Constant-impedance part as an admittance (enters `Y`).
    pub(crate) fn admittance(&self) -> C64 {
        calc_squared_voltage_admittance(C64::new(self.p_z, -self.q_z), self.v0 * self.v0)
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
            let alpha = f64::from_bits(LOAD_DYN_ALPHA.load(std::sync::atomic::Ordering::Relaxed));
            if alpha > 0.0 {
                let vf = if self.vf > 0.0 { self.vf } else { self.v0 };
                add(self.p_p, 0.0, part_weight(alpha, vm, vf, lv));
                add(0.0, self.q_p, part_weight(0.0, vm, self.v0, lv));
            } else {
                add(self.p_p, self.q_p, part_weight(0.0, vm, self.v0, lv));
            }
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
        let c = f64::from_bits(LOAD_SAT_C.load(std::sync::atomic::Ordering::Relaxed));
        if c > 0.0 {
            // never below the bus's own power flow voltage: no extra demand at the start
            let vk = f64::from_bits(LOAD_SAT_VK.load(std::sync::atomic::Ordering::Relaxed)).max(self.v0);
            let p0 = self.p0();
            if vm > vk && p0 > 0.0 {
                let x = (vm - vk) / 0.1;
                let q = c * p0 * x * x;
                let dq = c * p0 * 2.0 * x / 0.1;
                out[1] += q / (vm * vm);
                out[3] += (dq / (vm * vm) - 2.0 * q / (vm * vm * vm)) / vm;
            }
        }
        let c2 = f64::from_bits(LOAD_RISE_C.load(std::sync::atomic::Ordering::Relaxed));
        if c2 > 0.0 {
            let p0 = self.p0();
            if vm > self.v0 && p0 > 0.0 {
                let x = (vm - self.v0) / 0.1;
                let q = c2 * p0 * x * x;
                let dq = c2 * p0 * 2.0 * x / 0.1;
                out[1] += q / (vm * vm);
                out[3] += (dq / (vm * vm) - 2.0 * q / (vm * vm * vm)) / vm;
            }
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
// @block Held unit
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

/// Series reactance of a bypassed series capacitor, system pu: a near short
/// (0.25 Ω at 500 kV on 100 MVA, a percent of a typical bank).
#[cfg(feature = "protection")]
const BYPASS_X: f64 = 1e-4;
#[cfg(not(feature = "protection"))]
const BYPASS_X: f64 = 1.0;

/// The assembled RMS system.
// @block The system
#[derive(Clone, Debug)]
pub(crate) struct DynSystem {
    /// Hold non-windup limits during a step ([`super::RmsOptions::freeze_limits`]).
    pub freeze_limits: bool,
    /// Experimental: HVDC links in AC emulation ([`AcEmulation`]).
    pub hvdc_emulation: Vec<AcEmulation>,
    pub n: usize,
    pub bus_numbers: Vec<usize>,
    /// Dense index of each bus number in `bus_numbers` ([`bus_index`]).
    pub bus_index: HashMap<usize, usize>,
    pub sbase: f64,
    pub f0: f64,
    /// The static network. Its PowerIO branch columns (`ac.branches`) are the
    /// case's branches, with any tap a live session set; [`Self::branches`]
    /// is what the events made of them. RMS never reads `ac.ybus()`.
    pub ac: AcNetwork,
    /// Per branch, whether it is in service (a branch trip opens it).
    pub closed: Vec<bool>,
    /// The PowerIO branch columns as switched now ([`Self::rebuild_ybus`]):
    /// `ac.branches` with a tripped branch's admittances zero and the open
    /// ends and inner fault of a branch folded into its pi columns. The
    /// admittance matrix and the branch flows are PowerIO's over these.
    pub branches: AcBranchData,
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
    /// Per branch, whether a series capacitor's protection bypasses it (its
    /// MOV conducting during a fault): the branch is then a near short.
    #[cfg(feature = "protection")]
    pub bypassed: Vec<bool>,
    /// Per branch, a fault inside it.
    pub branch_faults: Vec<Option<BranchFault>>,
    pub held: Vec<HeldUnit>,
    /// Low-voltage behaviour of the constant-current and constant-power load
    /// parts ([`super::RmsOptions::low_voltage_load`]).
    pub low_voltage: LowVoltage,
    /// Factorize step matrices by blocks where possible
    /// ([`super::RmsOptions::factorize_by_blocks`]).
    pub factorize_by_blocks: bool,
    /// [`super::RmsOptions::refactor_values`].
    pub refactor_values: bool,
    /// [`super::RmsOptions::keep_factorization`].
    pub keep_factorization: bool,
    /// [`super::RmsOptions::threads`].
    pub threads: bool,
    /// The islands apart from the reference already named in the notes.
    pub noted_islands: Vec<Vec<usize>>,
}

impl DynSystem {
    // @block Layout and matrices
    // The state vector's names, the bus admittance matrix with every switched
    // branch, fault, shunt and source, and the mass matrix.

    /// Experimental: moves the reference voltage of every load towards its bus
    /// voltage over `dt` (see `set_load_recovery`).
    pub(crate) fn relax_loads(&mut self, x: &[f64], dt: f64) {
        let alpha = f64::from_bits(LOAD_DYN_ALPHA.load(std::sync::atomic::Ordering::Relaxed));
        let tc = f64::from_bits(LOAD_DYN_T.load(std::sync::atomic::Ordering::Relaxed));
        if alpha <= 0.0 || tc <= 0.0 {
            return;
        }
        let n = self.loads.len();
        let a = (dt / tc).min(1.0);
        for (b, l) in self.loads.iter_mut().enumerate() {
            if l.p_p == 0.0 || b >= n || n + b >= x.len() {
                continue;
            }
            let vm = (x[b] * x[b] + x[n + b] * x[n + b]).sqrt();
            if l.vf <= 0.0 {
                l.vf = l.v0;
            }
            if vm > 0.3 {
                l.vf += a * (vm - l.vf);
            }
        }
    }
    /// Names of every global state, `bus:quantity` for buses and
    /// `G<bus>[ 'id'].<state>` for devices.
    pub(crate) fn variable_names(&self) -> Vec<String> {
        let mut names = vec![String::new(); self.nx];
        for (b, number) in self.bus_numbers.iter().enumerate() {
            names[b] = format!("bus {number} Vr");
            names[self.n + b] = format!("bus {number} Vi");
        }
        for d in &self.devices {
            let tag = if d.machine_id == "1" {
                format!("G{}", d.bus_number)
            } else {
                format!("G{} '{}'", d.bus_number, d.machine_id)
            };
            for (k, s) in d.state_names().into_iter().enumerate() {
                names[d.offset + k] = format!("{tag} {s}");
            }
        }
        for h in &self.held {
            let Some(p) = &h.plant else { continue };
            let tag = if h.machine_id == "1" {
                format!("R{}", h.bus_number)
            } else {
                format!("R{} '{}'", h.bus_number, h.machine_id)
            };
            for (k, s) in p.state_names().into_iter().enumerate() {
                names[h.offset + k] = format!("{tag} {s}");
            }
        }
        names
    }

    /// Rebuild the admittance matrix after a topology or load change:
    /// PowerIO's `AcBranchData::calc_admittance_matrix` over the switched
    /// branch columns, with the bus shunts of the case plus every fault,
    /// constant-impedance load part, Thevenin source and dead-bus ground.
    pub(crate) fn rebuild_ybus(&mut self) {
        self.epoch += 1;
        self.branches = self.switched_branches();
        let dead = self.dead_buses();
        let mut gs = self.ac.gs.clone();
        let mut bs = self.ac.bs.clone();
        let mut extra: Vec<C64> = (0..self.n)
            .map(|b| {
                let y = self.fault_y[b] + self.loads[b].admittance();
                // A bus in an island without a live unit holds 0 V (a unit
                // shunt keeps its rows regular).
                if dead[b] {
                    y + C64::new(1.0, 0.0)
                } else {
                    y
                }
            })
            .collect();
        // One pass in source order, so each bus adds its sources in the
        // order a per-bus search of the list would.
        for s in &self.sources {
            if let Some(y) = extra.get_mut(s.bus) {
                *y += s.y;
            }
        }
        for (b, y) in extra.iter().enumerate() {
            gs[b] += y.re;
            bs[b] += y.im;
        }
        let y = self.branches.calc_admittance_matrix(self.n, &gs, &bs);
        let mut entries: std::collections::BTreeMap<(usize, usize), C64> =
            std::collections::BTreeMap::new();
        for (&g, (r, c)) in y.g.iter() {
            entries.entry((r, c)).or_default().re = g;
        }
        for (&b, (r, c)) in y.b.iter() {
            entries.entry((r, c)).or_default().im = b;
        }
        self.ybus = entries.into_iter().map(|((r, c), v)| (r, c, v)).collect();
    }

    /// `ac.branches` as the events left them: a tripped branch's admittances
    /// zero (its block stamps nothing), and a closed branch with an open end
    /// or an inner fault carrying its terminal equivalent
    /// ([`Self::terminal_equivalent`]) in its series and charging columns.
    /// The tap and shift columns stay the branch's own.
    /// Whether branch `e` (a series capacitor) is bypassed by its protection.
    fn is_bypassed(&self, e: usize) -> bool {
        #[cfg(feature = "protection")]
        {
            self.bypassed.get(e).copied().unwrap_or(false)
        }
        #[cfg(not(feature = "protection"))]
        {
            let _ = e;
            false
        }
    }

    fn switched_branches(&self) -> AcBranchData {
        let mut br = self.ac.branches.clone();
        for e in 0..self.ac.m {
            let (series, y_fr, y_to) = if !self.closed[e] {
                let zero = C64::new(0.0, 0.0);
                (zero, zero, zero)
            } else if self.is_bypassed(e) {
                let zero = C64::new(0.0, 0.0);
                (C64::new(0.0, -1.0 / BYPASS_X), zero, zero)
            } else if self.is_special(e) {
                self.terminal_equivalent(e)
            } else {
                continue;
            };
            br.g[e] = series.re;
            br.b[e] = series.im;
            br.g_fr[e] = y_fr.re;
            br.b_fr[e] = y_fr.im;
            br.g_to[e] = y_to.re;
            br.b_to[e] = y_to.im;
        }
        br
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

    /// Experimental: the configured HVDC links in AC emulation, with their start angle
    /// from the initial state `x0` (so `f(x0)` is unchanged).
    pub(crate) fn set_hvdc_emulation_start(&mut self, x0: &[f64]) {
        let cfg = HVDC_EMULATION.read().expect("hvdc emulation lock").clone();
        let index: std::collections::HashMap<usize, usize> =
            self.bus_numbers.iter().enumerate().map(|(i, &b)| (b, i)).collect();
        let n = self.n;
        self.hvdc_emulation = cfg
            .iter()
            .filter_map(|&(bf, bt, k_mw_deg, p_max_mw)| {
                let (f, t) = (*index.get(&bf)?, *index.get(&bt)?);
                let dtheta0 = (x0[n + f] * x0[t] - x0[f] * x0[n + t]).atan2(x0[f] * x0[t] + x0[n + f] * x0[n + t]);
                Some(AcEmulation {
                    from: f,
                    to: t,
                    k: k_mw_deg * 180.0 / std::f64::consts::PI / self.sbase,
                    p_max: if p_max_mw > 0.0 { p_max_mw / self.sbase } else { f64::INFINITY },
                    dtheta0,
                })
            })
            .collect();
    }

    /// Experimental: after a taken step, the frozen limit modes from the last
    /// evaluation (`TELLEGEN_FREEZE=1`); returns how many switched.
    pub(crate) fn update_limit_modes(&self) -> usize {
        if !super::models::freeze_limits() {
            return 0;
        }
        self.devices.iter().map(|d| d.avr.update_limit_modes() + d.gov.update_limit_modes() + d.pss.update_limit_modes()).sum()
    }

    /// Experimental: with frozen limits, set the exciter states a step carried past
    /// their clamps back onto them ([`super::models::Avr::project_limits`]); returns how many.
    pub(crate) fn project_limits(&self, x: &mut [f64]) -> usize {
        if !super::models::freeze_limits() {
            return 0;
        }
        self.devices
            .iter()
            .filter(|d| d.tripped.is_none())
            .map(|d| {
                let [_, _, a, g, p] = d.layout();
                let o = d.offset;
                let np = d.pss.n_states();
                usize::from(d.avr.project_limits(&mut x[o + a..o + g]))
                    + usize::from(d.gov.project_limits(&mut x[o + g..o + p]))
                    + usize::from(d.pss.project_limits(&mut x[o + p..o + p + np]))
            })
            .sum()
    }

    /// `f(x)`, and the device outputs when `outs` is given. Without outputs and with
    /// regions (`TELLEGEN_REGIONS`) it is evaluated by regions (`Self::residual_by_regions`),
    /// with the same result bit for bit.
    // @block Residual f(x)
    pub(crate) fn residual(&self, x: &[f64], f: &mut [f64], mut outs: Option<&mut Vec<GenOut>>) {
        if outs.is_none() {
            if let Some(ri) = super::regions::region_index(self) {
                self.residual_by_regions(x, f, &ri);
                return;
            }
        }
        let n = self.n;
        f.iter_mut().for_each(|v| *v = 0.0);
        let (vr, rest) = x.split_at(n);
        let vi = &rest[..n];
        // @block Network current balance
        // The bus rows start as `−Y V`: the current the network draws.
        for &(r, c, y) in &self.ybus {
            f[r] -= y.re * vr[c] - y.im * vi[c];
            f[n + r] -= y.im * vr[c] + y.re * vi[c];
        }
        // @block Thevenin sources
        // Each source injects its Norton current.
        for s in &self.sources {
            let i = s.y * s.e;
            f[s.bus] += i.re;
            f[n + s.bus] += i.im;
        }
        // @block Voltage-dependent loads
        // The constant-power and constant-current parts draw their current at the
        // bus voltage (the impedance part is in the admittance matrix).
        for (b, load) in self.loads.iter().enumerate() {
            if load.is_nonlinear() {
                let (ir, ii) = load.nonlinear_current(vr[b], vi[b], self.low_voltage);
                f[b] -= ir;
                f[n + b] -= ii;
            }
        }
        // @block HVDC links in AC emulation
        // Each emulated link moves `K (Δθ − Δθ0)` (limited) from one end bus to the
        // other ([`AcEmulation`]).
        for e in &self.hvdc_emulation {
            let c = e.currents(vr[e.from], vi[e.from], vr[e.to], vi[e.to]);
            f[e.from] += c[0];
            f[n + e.from] += c[1];
            f[e.to] += c[2];
            f[n + e.to] += c[3];
        }
        let w_sys = self.omega_sys(x);
        if let Some(outs) = outs.as_deref_mut() {
            outs.clear();
        }
        // @block Generating units
        // Each unit evaluates its machine and controllers at its bus voltage and
        // the frame speed, writes its state rows and adds its stator current to
        // its bus.
        #[cfg(not(all(target_arch = "wasm32", not(target_feature = "atomics"))))]
        if self.devices.len() >= PAR_MIN_UNITS && self.threads && super::spin_pool::SpinPool::global().threads() > 0 {
            // Experimental: the units in chunks on the spin pool, each writing
            // only its own state rows and output; the bus currents are added
            // below in unit order, as serially.
            let nd = self.devices.len();
            let mut res = vec![GenOut::default(); nd];
            let fp = SendPtr(f.as_mut_ptr());
            let rp = SendPtr(res.as_mut_ptr().cast::<f64>());
            let chunks = nd.div_ceil(PAR_CHUNK);
            super::spin_pool::SpinPool::global().run(chunks, &|c| {
                for u in c * PAR_CHUNK..((c + 1) * PAR_CHUNK).min(nd) {
                    let d = &self.devices[u];
                    let k = d.n_states();
                    // SAFETY: each unit writes its own state rows and its own
                    // output slot, no two alike.
                    let fs = unsafe { std::slice::from_raw_parts_mut(fp.get().add(d.offset), k) };
                    let out = d.eval(&x[d.offset..d.offset + k], vr[d.bus], vi[d.bus], w_sys, fs);
                    unsafe { *rp.get().cast::<GenOut>().add(u) = out };
                }
            });
            for (d, out) in self.devices.iter().zip(res) {
                f[d.bus] += out.ir;
                f[n + d.bus] += out.ii;
                if let Some(outs) = outs.as_deref_mut() {
                    outs.push(out);
                }
            }
        } else {
            self.residual_units_serial(x, f, w_sys, outs);
        }
        #[cfg(all(target_arch = "wasm32", not(target_feature = "atomics")))]
        self.residual_units_serial(x, f, w_sys, outs);
        // @block Simulated plants
        // Each renewable plant's states and its current into its bus; a tripped
        // plant's rows hold its states.
        #[cfg(not(all(target_arch = "wasm32", not(target_feature = "atomics"))))]
        if self.held.len() >= PAR_MIN_UNITS && self.threads && super::spin_pool::SpinPool::global().threads() > 0 {
            let nh = self.held.len();
            let mut res: Vec<Option<C64>> = vec![None; nh];
            let fp = SendPtr(f.as_mut_ptr());
            let rp = SendPtr(res.as_mut_ptr().cast::<f64>());
            let chunks = nh.div_ceil(PAR_CHUNK);
            super::spin_pool::SpinPool::global().run(chunks, &|c| {
                for u in c * PAR_CHUNK..((c + 1) * PAR_CHUNK).min(nh) {
                    let h = &self.held[u];
                    let Some(p) = h.plant.as_ref() else { continue };
                    let k = p.n_states();
                    // SAFETY: as for the units above.
                    let fs = unsafe { std::slice::from_raw_parts_mut(fp.get().add(h.offset), k) };
                    let out = if h.tripped {
                        fs.iter_mut().for_each(|v| *v = 0.0);
                        None
                    } else {
                        Some(p.eval(&x[h.offset..h.offset + k], vr[h.bus], vi[h.bus], w_sys, fs))
                    };
                    unsafe { *rp.get().cast::<Option<C64>>().add(u) = out };
                }
            });
            for (h, i) in self.held.iter().zip(res) {
                if let Some(i) = i {
                    f[h.bus] += i.re;
                    f[n + h.bus] += i.im;
                }
            }
            return;
        }
        self.residual_plants_serial(x, f, w_sys);
    }

    /// Experimental: `f(x)` by regions, each region one task. A region writes
    /// only its buses' rows and its units' states, and adds to each bus in the
    /// order of [`Self::residual`] (network, sources, loads, units, plants), so
    /// `f` is bit for bit the serial one.
    // @block Residual by regions
    fn residual_by_regions(&self, x: &[f64], f: &mut [f64], ri: &super::regions::RegionIndex) {
        use super::regions::Shared;
        let n = self.n;
        let w_sys = self.omega_sys(x);
        let fp = Shared(f.as_mut_ptr());
        let ybus = &self.ybus;
        let t_call = crate::rms::clock::Instant::now();
        super::regions::par_for(ri.nr, &|r| {
            let t_task = crate::rms::clock::Instant::now();
            let _rt = super::regions::RegionTimer(r, t_task);
            // The region's bus rows are summed here, in the order of the serial
            // residual, and written once at the end: buses of different regions
            // share cache lines in `f`.
            let (vr, vi) = (&x[..n], &x[n..2 * n]);
            let buses = &ri.buses[r];
            let mut acc = vec![0.0_f64; 2 * buses.len()];
            for (l, &b) in buses.iter().enumerate() {
                let (mut ar, mut ai) = (0.0_f64, 0.0_f64);
                let lo = ybus.partition_point(|e| e.0 < b);
                let mut k = lo;
                while k < ybus.len() && ybus[k].0 == b {
                    let (_, c, y) = ybus[k];
                    ar -= y.re * vr[c] - y.im * vi[c];
                    ai -= y.im * vr[c] + y.re * vi[c];
                    k += 1;
                }
                acc[2 * l] = ar;
                acc[2 * l + 1] = ai;
            }
            let li = |b: usize| 2 * ri.local[b] as usize;
            for &k in &ri.sources[r] {
                let s = &self.sources[k];
                let i = s.y * s.e;
                acc[li(s.bus)] += i.re;
                acc[li(s.bus) + 1] += i.im;
            }
            for (l, &b) in buses.iter().enumerate() {
                let load = &self.loads[b];
                if load.is_nonlinear() {
                    let (ir, ii) = load.nonlinear_current(vr[b], vi[b], self.low_voltage);
                    acc[2 * l] -= ir;
                    acc[2 * l + 1] -= ii;
                }
                for e in &self.hvdc_emulation {
                    if e.from == b || e.to == b {
                        let c = e.currents(vr[e.from], vi[e.from], vr[e.to], vi[e.to]);
                        let k = if e.from == b { 0 } else { 2 };
                        acc[2 * l] += c[k];
                        acc[2 * l + 1] += c[k + 1];
                    }
                }
            }
            for &u in &ri.devices[r] {
                let d = &self.devices[u];
                let k = d.n_states();
                // SAFETY: the unit's own state rows.
                let fs = unsafe { fp.slice(d.offset, k) };
                fs.fill(0.0);
                let out = d.eval(&x[d.offset..d.offset + k], vr[d.bus], vi[d.bus], w_sys, fs);
                acc[li(d.bus)] += out.ir;
                acc[li(d.bus) + 1] += out.ii;
            }
            for &u in &ri.held[r] {
                let h = &self.held[u];
                let Some(p) = &h.plant else { continue };
                let k = p.n_states();
                // SAFETY: the plant's own state rows.
                let fs = unsafe { fp.slice(h.offset, k) };
                fs.fill(0.0);
                if h.tripped {
                    continue;
                }
                let i = p.eval(&x[h.offset..h.offset + k], vr[h.bus], vi[h.bus], w_sys, fs);
                acc[li(h.bus)] += i.re;
                acc[li(h.bus) + 1] += i.im;
            }
            for (l, &b) in buses.iter().enumerate() {
                // SAFETY: this region's bus rows.
                unsafe {
                    *fp.at(b) = acc[2 * l];
                    *fp.at(n + b) = acc[2 * l + 1];
                }
            }
        });
        super::regions::CALL_US.fetch_add(t_call.elapsed().as_micros() as u64, std::sync::atomic::Ordering::Relaxed);
        super::regions::CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    fn residual_units_serial(&self, x: &[f64], f: &mut [f64], w_sys: f64, mut outs: Option<&mut Vec<GenOut>>) {
        let n = self.n;
        let (vr, rest) = x.split_at(n);
        let vi = &rest[..n];
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
    }

    fn residual_plants_serial(&self, x: &[f64], f: &mut [f64], w_sys: f64) {
        let n = self.n;
        let (vr, rest) = x.split_at(n);
        let vi = &rest[..n];
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

    // @block Jacobian
    // `∂f/∂x`: exact for the network and loads, forward differences for each
    // unit's own block (and for HVDC links in AC emulation). The units' blocks
    // are built in chunks on the spin pool and joined in unit order.

    /// `∂f/∂x` as triplets. The network and load parts are exact; device
    /// blocks are forward differences of their local functions with the
    /// scaled step of [`fd_step`].
    pub(crate) fn jacobian(&self, x: &[f64]) -> Vec<(usize, usize, f64)> {
        self.jacobian_units(x).0
    }

    /// Experimental: [`Self::jacobian`], and where each unit's entries lie in it:
    /// `(first state, start, end)` per device and per held plant with a model, in the order
    /// they are built; the entries before the first unit's are the network's and the loads'.
    #[allow(clippy::type_complexity)]
    pub(crate) fn jacobian_units(&self, x: &[f64]) -> (Vec<(usize, usize, f64)>, Vec<(usize, usize, usize)>) {
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
        // Experimental: HVDC links in AC emulation, forward differences, every entry
        // entered (zero or not) so the pattern stays.
        for e in &self.hvdc_emulation {
            let cols = [e.from, n + e.from, e.to, n + e.to];
            let v = |xs: &[f64; 4]| e.currents(xs[0], xs[1], xs[2], xs[3]);
            let v0 = [x[e.from], x[n + e.from], x[e.to], x[n + e.to]];
            let c0 = v(&v0);
            for (j, &col) in cols.iter().enumerate() {
                let mut vp = v0;
                let h = fd_step(vp[j]);
                vp[j] += h;
                let c1 = v(&vp);
                for (i, &row) in cols.iter().enumerate() {
                    t.push((row, col, (c1[i] - c0[i]) / h));
                }
            }
        }
        let w_sys = self.omega_sys(x);
        let dev_block = |d: &GenDevice, t: &mut Vec<(usize, usize, f64)>| {
            let eval = |xl: &[f64], vr: f64, vi: f64, w: f64, f: &mut [f64]| {
                let o = d.eval(xl, vr, vi, w, f);
                (o.ir, o.ii)
            };
            self.unit_block(x, w_sys, d.offset, d.n_states(), d.bus, eval, t);
        };
        let plant_block = |h: &HeldUnit, t: &mut Vec<(usize, usize, f64)>| {
            if h.tripped {
                return;
            }
            let Some(p) = &h.plant else { return };
            let eval = |xl: &[f64], vr: f64, vi: f64, w: f64, f: &mut [f64]| {
                let i = p.eval(xl, vr, vi, w, f);
                (i.re, i.im)
            };
            self.unit_block(x, w_sys, h.offset, p.n_states(), h.bus, eval, t);
        };
        #[cfg(not(all(target_arch = "wasm32", not(target_feature = "atomics"))))]
        if self.devices.len() + self.held.len() >= PAR_MIN_UNITS
            && self.threads && super::spin_pool::SpinPool::global().threads() > 0
        {
            // Experimental: the units' blocks in chunks on the spin pool,
            // joined in unit order (the same triplets in the same order).
            let nd = self.devices.len();
            let total = nd + self.held.len();
            let chunks = total.div_ceil(PAR_CHUNK);
            #[allow(clippy::type_complexity)]
            let parts: Vec<std::sync::Mutex<(Vec<(usize, usize, f64)>, Vec<(usize, usize, usize)>)>> =
                (0..chunks).map(|_| std::sync::Mutex::new((Vec::new(), Vec::new()))).collect();
            super::spin_pool::SpinPool::global().run(chunks, &|c| {
                let mut v = Vec::new();
                let mut r = Vec::new();
                for u in c * PAR_CHUNK..((c + 1) * PAR_CHUNK).min(total) {
                    let s0 = v.len();
                    if u < nd {
                        dev_block(&self.devices[u], &mut v);
                        r.push((self.devices[u].offset, s0, v.len()));
                    } else {
                        let h = &self.held[u - nd];
                        plant_block(h, &mut v);
                        if h.plant.is_some() {
                            r.push((h.offset, s0, v.len()));
                        }
                    }
                }
                *parts[c].lock().expect("jacobian part") = (v, r);
            });
            let mut ranges = Vec::with_capacity(total);
            for p in parts {
                let (v, r) = p.into_inner().expect("jacobian part");
                let base = t.len();
                ranges.extend(r.into_iter().map(|(o, a, b)| (o, a + base, b + base)));
                t.extend(v);
            }
            return (t, ranges);
        }
        let mut ranges = Vec::with_capacity(self.devices.len() + self.held.len());
        for d in &self.devices {
            let s0 = t.len();
            dev_block(d, &mut t);
            ranges.push((d.offset, s0, t.len()));
        }
        for h in &self.held {
            let s0 = t.len();
            plant_block(h, &mut t);
            if h.plant.is_some() {
                ranges.push((h.offset, s0, t.len()));
            }
        }
        (t, ranges)
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

    // @block Branch flows
    // The flows at both ends of every branch, open ends and inner faults
    // included.

    /// The terminal flows of every branch at the state `x`, system base:
    /// PowerIO's `AcBranchData::calc_branch_flows` over the switched branch
    /// columns, so an open end carries no flow and a faulted branch the flow
    /// into its fault.
    pub(crate) fn branch_flows(&self, x: &[f64]) -> AcBranchFlows {
        let (vr, rest) = x.split_at(self.n);
        let vi = &rest[..self.n];
        let vm: Vec<f64> = vr.iter().zip(vi).map(|(r, i)| r.hypot(*i)).collect();
        let va: Vec<f64> = vr.iter().zip(vi).map(|(r, i)| i.atan2(*r)).collect();
        self.branches.calc_branch_flows(&vm, &va)
    }

    fn is_special(&self, e: usize) -> bool {
        self.closed[e]
            && (self.end_open.get(e).is_some_and(|o| o[0] || o[1])
                || self.branch_faults.get(e).is_some_and(Option::is_some))
    }

    /// The series admittance and terminal charging `(y, y_fr, y_to)` of the
    /// pi model whose block is branch `e`'s terminal admittance with its open
    /// ends and inner fault, before its tap: the series admittance split at
    /// the fault point into PowerIO pi sections (`y / α` with the from
    /// charging, `y / (1 − α)` with the to charging), the fault admittance to
    /// ground there, and the inner nodes and open ends eliminated (PowerIO's
    /// Kron reduction). The reduction of these reciprocal sections is
    /// symmetric, so it is a pi model: series `−Y_ft`, charging
    /// `Y_ff + Y_ft` and `Y_tt + Y_tf`. The branch's own tap
    /// and shift then refer it to the from bus when PowerIO stamps it.
    fn terminal_equivalent(&self, e: usize) -> (C64, C64, C64) {
        let zero = C64::new(0.0, 0.0);
        let br = &self.ac.branches;
        let y = C64::new(br.g[e], br.b[e]);
        let y_fr = C64::new(br.g_fr[e], br.b_fr[e]);
        let y_to = C64::new(br.g_to[e], br.b_to[e]);
        let (alpha, yf) =
            self.branch_faults[e].map_or((0.5, zero), |f| (f.alpha.clamp(1e-4, 1.0 - 1e-4), f.y));
        // Local nodes: 0 from (behind the tap), 1 to, 2 the fault point.
        let [a_ff, a_fk, a_kf, a_kk] =
            powerio_tx::PiModel::new(y / alpha, y_fr, zero, 1.0, 0.0).calc_block();
        let [b_kk, b_kt, b_tk, b_tt] =
            powerio_tx::PiModel::new(y / (1.0 - alpha), zero, y_to, 1.0, 0.0).calc_block();
        let full = [
            [a_ff, zero, a_fk],
            [zero, b_tt, b_tk],
            [a_kf, b_kt, a_kk + b_kk + yf],
        ];
        let open = self.end_open[e];
        let ext: Vec<usize> = (0..2).filter(|&k| !open[k]).collect();
        let int: Vec<usize> = (0..3).filter(|k| !ext.contains(k)).collect();
        // PowerIO's Kron reduction `M_kk − M_ke M_ee⁻¹ M_ek` (named for the
        // conductor impedance it was written for; the algebra is the same
        // for an admittance). An exactly singular `Y_II` (inner nodes that
        // float) gives `None`: nothing reaches the terminals.
        let full: Vec<Vec<C64>> = full.iter().map(|row| row.to_vec()).collect();
        let red = calc_kron_reduced_impedance(&full, &ext, &int, 0.0)
            .unwrap_or_else(|| vec![vec![zero; ext.len()]; ext.len()]);
        let mut out = [[zero; 2]; 2];
        for (a, &i) in ext.iter().enumerate() {
            for (b, &j) in ext.iter().enumerate() {
                out[i][j] = red[a][b];
            }
        }
        let series = -out[0][1];
        (series, out[0][0] - series, out[1][1] + out[1][0])
    }

    // @block Islands
    // Which buses hang together over closed branches, which islands have no
    // unit left, and how a split network is named in messages.

    /// The electrical islands over branches closed at both ends, each its
    /// dense buses in ascending order, in order of each island's first bus:
    /// PowerIO's dense island labels over those branches.
    pub(crate) fn islands(&self) -> Vec<Vec<usize>> {
        let (from, to): (Vec<usize>, Vec<usize>) = (0..self.ac.m)
            .filter(|&e| self.closed[e] && !self.end_open[e][0] && !self.end_open[e][1])
            .map(|e| (self.ac.branches.from_bus[e], self.ac.branches.to_bus[e]))
            .unzip();
        let labels = powerio_tx::calc_dense_island_labels(self.n, &from, &to);
        let count = labels.iter().max().map_or(0, |&last| last + 1);
        let mut islands = vec![Vec::new(); count];
        for (b, &label) in labels.iter().enumerate() {
            islands[label].push(b);
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
        self.unit_islands()
            .into_iter()
            .filter(|(island, _)| reference.is_none_or(|r| island.binary_search(&r).is_err()))
            .collect()
    }

    /// Every island with a unit in service, the reference bus's among them,
    /// with the text [`Self::detached_islands`] gives.
    pub(crate) fn unit_islands(&self) -> Vec<(Vec<usize>, String)> {
        self.islands()
            .into_iter()
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
        !self.closed[e] || self.end_open[e].iter().all(|&o| o)
    }

    pub(crate) fn dense_bus(&self, number: usize) -> Option<usize> {
        self.bus_index.get(&number).copied()
    }
}

/// Dense index of each bus number in `numbers`; the first position wins for
/// a repeated number, as a linear search from the front would find it.
pub(crate) fn bus_index(numbers: &[usize]) -> HashMap<usize, usize> {
    let mut index = HashMap::with_capacity(numbers.len());
    for (k, &number) in numbers.iter().enumerate() {
        index.entry(number).or_insert(k);
    }
    index
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

/// Experimental: below this many units the residual and the Jacobian stay serial.
#[cfg(not(all(target_arch = "wasm32", not(target_feature = "atomics"))))]
const PAR_MIN_UNITS: usize = 64;
/// Experimental: units per task on the spin pool.
#[cfg(not(all(target_arch = "wasm32", not(target_feature = "atomics"))))]
const PAR_CHUNK: usize = 48;

/// A raw pointer that may cross threads: each thread writes a disjoint part.
#[cfg(not(all(target_arch = "wasm32", not(target_feature = "atomics"))))]
#[derive(Clone, Copy)]
struct SendPtr(*mut f64);
#[cfg(not(all(target_arch = "wasm32", not(target_feature = "atomics"))))]
unsafe impl Send for SendPtr {}
#[cfg(not(all(target_arch = "wasm32", not(target_feature = "atomics"))))]
unsafe impl Sync for SendPtr {}
#[cfg(not(all(target_arch = "wasm32", not(target_feature = "atomics"))))]
impl SendPtr {
    fn get(self) -> *mut f64 {
        self.0
    }
}

