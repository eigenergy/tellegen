//! Synchronous generator components: machine, shaft, exciter, governor and
//! stabilizer equations.
//!
//! The equations are translated from PowerSimulationsDynamics.jl
//! (`src/models/generator_models/{machine,shaft,avr,tg,pss}_models.jl`, v0.16.2;
//! BSD-3-Clause, Copyright (c) 2020, 2023 Alliance for Sustainable Energy, LLC
//! and The Regents of the University of California; see
//! `licenses/PowerSimulationsDynamics.jl-LICENSE`), which in turn follow
//! Milano, *Power System Modelling and Scripting*, Sauer & Pai, *Power System
//! Dynamics and Stability*, and IEEE Std 1110 / 421.5. The PSS/E parameter
//! mapping follows PowerSystems.jl `src/parsers/psse_dynamic_mapping.yaml`.
//!
//! Conventions: machine quantities are on the machine base `MBASE`; the
//! network is in system per unit, rectangular coordinates `V = Vr + j Vi`; the
//! dq frame is Kundur's (`ri_dq(δ) = [sin δ  −cos δ; cos δ  sin δ]`). Each
//! component writes the right-hand side of `M ẋ = f(x)`, and `M` holds the
//! time constants that PowerSimulationsDynamics.jl keeps in its mass matrix,
//! so a zero time constant turns the state algebraic.
//!
//! @block Synchronous generator models

use std::f64::consts::PI;

use powerio::DynamicModel;

use super::blocks::*;
#[cfg(feature = "rms-experimental")]
use super::graph::CompiledGraph;

// @block Frames and saturation
// Rotations between the network's `r/i` frame and a machine's `d/q`
// frame, and the saturation curves of the machines.

/// `√(a² + b²)`: the magnitude of per-unit phasors and flux linkages in the
/// equations evaluated in every Newton iteration. Plain arithmetic instead
/// of `hypot`, whose overflow guard these values do not need
/// and which costs a library call per use (4 % of a large run in the
/// browser); it differs from `hypot` by at most a rounding.
#[inline]
pub(crate) fn mag(a: f64, b: f64) -> f64 {
    (a * a + b * b).sqrt()
}

/// `[Vd, Vq] = ri_dq(δ) [Vr, Vi]`.
#[inline]
pub(crate) fn ri_dq(delta: f64, vr: f64, vi: f64) -> (f64, f64) {
    ri_dq_sc(delta.sin_cos(), vr, vi)
}

/// `[Ir, Ii] = dq_ri(δ) [Id, Iq]`.
#[inline]
pub(crate) fn dq_ri(delta: f64, id: f64, iq: f64) -> (f64, f64) {
    dq_ri_sc(delta.sin_cos(), id, iq)
}

/// [`ri_dq`] with `(sin δ, cos δ)` given.
#[inline]
pub(crate) fn ri_dq_sc((s, c): (f64, f64), vr: f64, vi: f64) -> (f64, f64) {
    (s * vr - c * vi, c * vr + s * vi)
}

/// [`dq_ri`] with `(sin δ, cos δ)` given.
#[inline]
pub(crate) fn dq_ri_sc((s, c): (f64, f64), id: f64, iq: f64) -> (f64, f64) {
    (s * id + c * iq, -c * id + s * iq)
}

/// The quadratic saturation function of PSS/E, `Se(x) = B (x − A)² / x` for
/// `x > A` and `0` for `x ≤ A` (no saturation below the curve's knee; the
/// formula alone rises again below `A` and diverges as `x → 0`, which a deep
/// fault reaches). The divisor is floored at [`SAT_X_MIN`], which only data
/// with `A` below it can reach (`Se` there is at most `B · SAT_X_MIN`).
/// PowerSimulationsDynamics.jl evaluates the formula without the knee (the
/// same above `A`); `knee: false` does so too, for comparisons with it.
#[inline]
pub(crate) fn quadratic_se(a: f64, b: f64, x: f64, knee: bool) -> f64 {
    if b == 0.0 || (knee && super::not_gt(x, a)) {
        0.0
    } else {
        b * (x - a).powi(2) / x.max(SAT_X_MIN)
    }
}

/// The smallest divisor of [`quadratic_se`].
pub(crate) const SAT_X_MIN: f64 = 0.05;

/// The exciter voltage `VE` an `ESAC1A` divides the field current by is
/// floored here: the rectifier's normalized current `IN = KC Ifd / VE`
/// stays finite as `VE → 0` (the rectifier then delivers nothing,
/// `FEX(IN ≥ 1) = 0`, so `Efd = VE · FEX` falls to zero with `VE`).
pub(crate) const VE_MIN: f64 = 1e-3;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Saturation {
    /// `Se(x) = B (x − A)² / x` above `A` ([`quadratic_se`]).
    Quadratic { a: f64, b: f64, knee: bool },
    /// `Se(x) = B x^A`.
    Exponential { a: f64, b: f64 },
}

impl Saturation {
    /// Evaluate a quadratic curve below `A` as PowerSimulationsDynamics.jl
    /// does (no knee, [`quadratic_se`]).
    pub(crate) fn without_knee(self) -> Self {
        match self {
            Self::Quadratic { a, b, .. } => Self::Quadratic { a, b, knee: false },
            other => other,
        }
    }

    pub(crate) fn eval(self, x: f64) -> f64 {
        match self {
            Self::Quadratic { a, b, knee } => quadratic_se(a, b, x, knee),
            Self::Exponential { a, b } => {
                if b == 0.0 || super::not_gt(x, 0.0) {
                    0.0
                } else {
                    b * x.powf(a)
                }
            }
        }
    }
}

/// Round rotor (`GENROU`, `GENROE`): states `[e'q, e'd, ψkd, ψkq]`.
#[derive(Clone, Debug)]
pub(crate) struct RoundRotor {
    pub r: f64,
    pub td0_p: f64,
    pub td0_pp: f64,
    pub tq0_p: f64,
    pub tq0_pp: f64,
    pub xd: f64,
    pub xq: f64,
    pub xd_p: f64,
    pub xq_p: f64,
    pub xd_pp: f64,
    pub xl: f64,
    pub sat: Saturation,
}

/// Salient pole (`GENSAL`, `GENSAE`): states `[e'q, ψkd, ψ''q]`.
#[derive(Clone, Debug)]
pub(crate) struct SalientPole {
    pub r: f64,
    pub td0_p: f64,
    pub td0_pp: f64,
    pub tq0_pp: f64,
    pub xd: f64,
    pub xq: f64,
    pub xd_p: f64,
    pub xd_pp: f64,
    pub xl: f64,
    pub sat: Saturation,
    /// `GENSAE`: exponential saturation and the PSS/E `GENSAE` flux sign convention.
    pub exponential: bool,
}

/// The machine models: classical, round rotor (`GENROU`, `GENROE`) and
/// salient pole (`GENSAL`, `GENSAE`), with their stator algebra and
/// rotor flux dynamics.
// @block Machines
#[derive(Clone, Debug)]
pub(crate) enum Machine {
    /// `GENCLS`: constant `e'q` behind `R + j X'd`, no states.
    Classic {
        r: f64,
        xd_p: f64,
        eq_p: f64,
    },
    RoundRotor(RoundRotor),
    SalientPole(SalientPole),
}

/// Machine quantities that do not depend on the field voltage, so the exciter
/// can read `Xad·Ifd` before the machine derivatives are formed.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct MachineAlgebraic {
    pub id: f64,
    pub iq: f64,
    pub te: f64,
    pub xad_ifd: f64,
}

impl Machine {
    pub(crate) fn n_states(&self) -> usize {
        match self {
            Self::Classic { .. } => 0,
            Self::RoundRotor(_) => 4,
            Self::SalientPole(_) => 3,
        }
    }

    pub(crate) fn state_names(&self) -> &'static [&'static str] {
        match self {
            Self::Classic { .. } => &[],
            Self::RoundRotor(_) => &["eq_p", "ed_p", "psi_kd", "psi_kq"],
            Self::SalientPole(_) => &["eq_p", "psi_kd", "psiq_pp"],
        }
    }

    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::Classic { .. } => "GENCLS",
            Self::RoundRotor(m) => match m.sat {
                Saturation::Quadratic { .. } => "GENROU",
                Saturation::Exponential { .. } => "GENROE",
            },
            Self::SalientPole(m) => {
                if m.exponential {
                    "GENSAE"
                } else {
                    "GENSAL"
                }
            }
        }
    }

    /// Stator currents (machine base), electrical torque and `Xad·Ifd` from
    /// the machine states, the rotor angle and the terminal voltage.
    pub(crate) fn algebraic(&self, x: &[f64], delta: f64, vr: f64, vi: f64) -> MachineAlgebraic {
        self.algebraic_sc(x, delta.sin_cos(), vr, vi)
    }

    /// [`Self::algebraic`] with `(sin δ, cos δ)` of the rotor angle given.
    pub(crate) fn algebraic_sc(
        &self,
        x: &[f64],
        sc: (f64, f64),
        vr: f64,
        vi: f64,
    ) -> MachineAlgebraic {
        let (vd, vq) = ri_dq_sc(sc, vr, vi);
        match self {
            Self::Classic { r, xd_p, eq_p } => {
                // machine_models.jl, BaseMachine (Milano 15.36, 15.35).
                let den = r * r + xd_p * xd_p;
                let id = (xd_p * (eq_p - vq) - r * vd) / den;
                let iq = (xd_p * vd + r * (eq_p - vq)) / den;
                let pe = (vd + r * id) * id + (vq + r * iq) * iq;
                MachineAlgebraic {
                    id,
                    iq,
                    te: pe,
                    xad_ifd: 0.0,
                }
            }
            Self::RoundRotor(m) => {
                // machine_models.jl, RoundRotorQuadratic / RoundRotorExponential.
                let (eq_p, ed_p, psi_kd, psi_kq) = (x[0], x[1], x[2], x[3]);
                let (g_d1, g_q1, g_d2, g_q2, g_qd) = m.gammas();
                let xq_pp = m.xd_pp;
                let psiq_pp = g_q1 * ed_p + psi_kq * (1.0 - g_q1);
                let psid_pp = g_d1 * eq_p + g_d2 * (m.xd_p - m.xl) * psi_kd;
                let psi_pp = mag(psid_pp, psiq_pp);
                let den = m.r * m.r + xq_pp * m.xd_pp;
                let id = (-m.r * (vd - psiq_pp) + xq_pp * (-vq + psid_pp)) / den;
                let iq = (m.xd_pp * (vd - psiq_pp) + m.r * (-vq + psid_pp)) / den;
                let se = m.sat.eval(psi_pp);
                let xad_ifd = eq_p
                    + (m.xd - m.xd_p) * (g_d1 * id - g_d2 * psi_kd + g_d2 * eq_p)
                    + se * psid_pp;
                let _ = g_q2;
                let _ = g_qd;
                let te = id * (vd + id * m.r) + iq * (vq + iq * m.r);
                MachineAlgebraic {
                    id,
                    iq,
                    te,
                    xad_ifd,
                }
            }
            Self::SalientPole(m) => {
                let (eq_p, psi_kd, psiq_pp) = (x[0], x[1], x[2]);
                let (g_d1, g_q1, g_d2) = m.gammas();
                let xq_pp = m.xd_pp;
                let psid_pp = g_d1 * eq_p + g_q1 * psi_kd;
                let den = m.r * m.r + m.xd_pp * m.xd_pp;
                let (id, iq, xad_ifd);
                if m.exponential {
                    // machine_models.jl, SalientPoleExponential.
                    let psi_pp = mag(psid_pp, psiq_pp);
                    id = (-m.r * (vd - psiq_pp) + xq_pp * (-vq + psid_pp)) / den;
                    iq = (m.xd_pp * (vd - psiq_pp) + m.r * (-vq + psid_pp)) / den;
                    let se = m.sat.eval(psi_pp);
                    xad_ifd = eq_p
                        + se * psid_pp
                        + (m.xd - m.xd_p) * (id + g_d2 * (eq_p - psi_kd - (m.xd_p - m.xl) * id));
                } else {
                    // machine_models.jl, SalientPoleQuadratic.
                    id = (-m.r * (vd + psiq_pp) + m.xd_pp * (psid_pp - vq)) / den;
                    iq = (m.xd_pp * (vd + psiq_pp) + m.r * (psid_pp - vq)) / den;
                    let se = m.sat.eval(eq_p);
                    xad_ifd = eq_p
                        + se * eq_p
                        + (m.xd - m.xd_p) * (id + g_d2 * (eq_p - psi_kd - (m.xd_p - m.xl) * id));
                }
                let te = id * (vd + id * m.r) + iq * (vq + iq * m.r);
                MachineAlgebraic {
                    id,
                    iq,
                    te,
                    xad_ifd,
                }
            }
        }
    }

    /// Machine state derivatives (all with unit mass).
    pub(crate) fn ode(&self, x: &[f64], alg: &MachineAlgebraic, vf: f64, f: &mut [f64]) {
        match self {
            Self::Classic { .. } => {}
            Self::RoundRotor(m) => {
                let (eq_p, ed_p, psi_kd, psi_kq) = (x[0], x[1], x[2], x[3]);
                let (_, g_q1, g_d2, g_q2, g_qd) = m.gammas();
                let (g_d1, ..) = m.gammas();
                let psiq_pp = g_q1 * ed_p + psi_kq * (1.0 - g_q1);
                let psid_pp = g_d1 * eq_p + g_d2 * (m.xd_p - m.xl) * psi_kd;
                let se = m.sat.eval(mag(psid_pp, psiq_pp));
                let xaq_i1q = ed_p
                    + (m.xq - m.xq_p) * (g_q2 * ed_p - g_q2 * psi_kq - g_q1 * alg.iq)
                    + se * psiq_pp * g_qd;
                f[0] = (vf - alg.xad_ifd) / m.td0_p;
                f[1] = -xaq_i1q / m.tq0_p;
                f[2] = (-psi_kd + eq_p - (m.xd_p - m.xl) * alg.id) / m.td0_pp;
                f[3] = (-psi_kq + ed_p + (m.xq_p - m.xl) * alg.iq) / m.tq0_pp;
            }
            Self::SalientPole(m) => {
                let (eq_p, psi_kd, psiq_pp) = (x[0], x[1], x[2]);
                let xq_pp = m.xd_pp;
                f[0] = (vf - alg.xad_ifd) / m.td0_p;
                f[1] = (-psi_kd + eq_p - (m.xd_p - m.xl) * alg.id) / m.td0_pp;
                if m.exponential {
                    let (g_d1, g_q1, _) = m.gammas();
                    let psid_pp = g_d1 * eq_p + g_q1 * psi_kd;
                    let se = m.sat.eval(mag(psid_pp, psiq_pp));
                    let g_qd = (m.xq - m.xl) / (m.xd - m.xl);
                    f[2] = (-psiq_pp + (m.xq - xq_pp) * alg.iq - se * g_qd * psiq_pp) / m.tq0_pp;
                } else {
                    f[2] = (-psiq_pp - (m.xq - xq_pp) * alg.iq) / m.tq0_pp;
                }
            }
        }
    }
}

impl RoundRotor {
    /// `(γd1, γq1, γd2, γq2, γqd)`, PowerSystems.jl `RoundRotorMachine` defaults.
    fn gammas(&self) -> (f64, f64, f64, f64, f64) {
        let g_d1 = (self.xd_pp - self.xl) / (self.xd_p - self.xl);
        let g_q1 = (self.xd_pp - self.xl) / (self.xq_p - self.xl);
        let g_d2 = (self.xd_p - self.xd_pp) / (self.xd_p - self.xl).powi(2);
        let g_q2 = (self.xq_p - self.xd_pp) / (self.xq_p - self.xl).powi(2);
        let g_qd = (self.xq - self.xl) / (self.xd - self.xl);
        (g_d1, g_q1, g_d2, g_q2, g_qd)
    }
}

impl SalientPole {
    /// `(γd1, γq1, γd2)`, PowerSystems.jl `SalientPoleMachine` defaults.
    fn gammas(&self) -> (f64, f64, f64) {
        let g_d1 = (self.xd_pp - self.xl) / (self.xd_p - self.xl);
        let g_q1 = (self.xd_p - self.xd_pp) / (self.xd_p - self.xl);
        let g_d2 = (self.xd_p - self.xd_pp) / (self.xd_p - self.xl).powi(2);
        (g_d1, g_q1, g_d2)
    }
}

/// Single-mass shaft: states `[δ, ω]`.
// @block Shaft
#[derive(Clone, Copy, Debug)]
pub(crate) struct Shaft {
    pub h: f64,
    pub d: f64,
}

impl Shaft {
    /// shaft_models.jl, SingleMass (Milano 15.5).
    pub(crate) fn ode(&self, omega: f64, omega_sys: f64, te: f64, tm: f64, f0: f64) -> (f64, f64) {
        (
            2.0 * PI * f0 * (omega - omega_sys),
            (tm - te - self.d * (omega - 1.0) / omega) / (2.0 * self.h),
        )
    }
}

/// Experimental: `TELLEGEN_FREEZE=1` freezes the non-windup limits of the SEXS field
/// voltage during a step (see [`LimitMode`]); unset, the limits act within the step as before.
pub(crate) fn freeze_limits() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("TELLEGEN_FREEZE").is_ok_and(|v| v == "1"))
}

/// Experimental: `TELLEGEN_FREEZE_HYST=<pu>` (default 1e-3): how far the free rate
/// `K u − y` must pull back from a limit before the limiter lets go.
fn freeze_hysteresis() -> f64 {
    static H: std::sync::OnceLock<f64> = std::sync::OnceLock::new();
    *H.get_or_init(|| std::env::var("TELLEGEN_FREEZE_HYST").ok().and_then(|v| v.parse().ok()).unwrap_or(1e-3))
}

/// Experimental: a non-windup limit's mode, frozen while Newton solves a step and
/// updated once the step is taken, from the state and free rate of the last evaluation (the
/// accepted point). A limit binds when the state reaches it while the free rate pushes on; it lets
/// go only once the free rate pulls back by more than the hysteresis, so it cannot switch on and
/// off from one step to the next. Before the first update the mode is unset and the limit acts as
/// without freezing.
#[derive(Debug)]
pub(crate) struct LimitMode {
    mode: std::sync::atomic::AtomicU8,
    y: std::sync::atomic::AtomicU64,
    rate: std::sync::atomic::AtomicU64,
}

impl LimitMode {
    pub(crate) const FREE: u8 = 0;
    pub(crate) const AT_MAX: u8 = 1;
    pub(crate) const AT_MIN: u8 = 2;
    const UNSET: u8 = 255;

    pub(crate) fn mode(&self) -> u8 {
        self.mode.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub(crate) fn record(&self, y: f64, rate: f64) {
        self.y.store(y.to_bits(), std::sync::atomic::Ordering::Relaxed);
        self.rate.store(rate.to_bits(), std::sync::atomic::Ordering::Relaxed);
    }

    /// The mode for the next step, from the recorded state and free rate. Returns whether it changed.
    pub(crate) fn update(&self, y_min: f64, y_max: f64) -> bool {
        let y = f64::from_bits(self.y.load(std::sync::atomic::Ordering::Relaxed));
        let rate = f64::from_bits(self.rate.load(std::sync::atomic::Ordering::Relaxed));
        if !(y.is_finite() && rate.is_finite()) {
            return false;
        }
        let hyst = freeze_hysteresis();
        let old = self.mode();
        let new = match old {
            Self::AT_MAX if rate > -hyst => Self::AT_MAX,
            Self::AT_MIN if rate < hyst => Self::AT_MIN,
            _ if y >= y_max && rate > 0.0 => Self::AT_MAX,
            _ if y <= y_min && rate < 0.0 => Self::AT_MIN,
            _ => Self::FREE,
        };
        self.mode.store(new, std::sync::atomic::Ordering::Relaxed);
        new != old && old != Self::UNSET
    }
}

impl Default for LimitMode {
    fn default() -> Self {
        Self {
            mode: std::sync::atomic::AtomicU8::new(Self::UNSET),
            y: std::sync::atomic::AtomicU64::new(f64::NAN.to_bits()),
            rate: std::sync::atomic::AtomicU64::new(f64::NAN.to_bits()),
        }
    }
}

impl Clone for LimitMode {
    fn clone(&self) -> Self {
        let r = std::sync::atomic::Ordering::Relaxed;
        Self {
            mode: std::sync::atomic::AtomicU8::new(self.mode.load(r)),
            y: std::sync::atomic::AtomicU64::new(self.y.load(r)),
            rate: std::sync::atomic::AtomicU64::new(self.rate.load(r)),
        }
    }
}

impl Governor {
    /// Experimental: update a governor graph's frozen limit modes after a taken step.
    pub(crate) fn update_limit_modes(&self) -> usize {
        match self {
            #[cfg(feature = "rms-experimental")]
            Self::Graph(g) => g.update_limit_modes(),
            #[allow(unreachable_patterns)]
            _ => 0,
        }
    }
}

impl Stabilizer {
    /// Experimental: update a stabilizer graph's frozen limit modes after a taken step.
    pub(crate) fn update_limit_modes(&self) -> usize {
        match self {
            #[cfg(feature = "rms-experimental")]
            Self::Graph(g) => g.update_limit_modes(),
            #[allow(unreachable_patterns)]
            _ => 0,
        }
    }
}

impl Avr {
    /// Experimental: update the frozen limit modes after a taken step; returns the
    /// number that switched.
    pub(crate) fn update_limit_modes(&self) -> usize {
        match self {
            Self::Sexs { v_min, v_max, lim, .. } => usize::from(lim.update(*v_min, *v_max)),
            #[cfg(feature = "rms-experimental")]
            Self::Graph(g) => g.update_limit_modes(),
            _ => 0,
        }
    }
}

/// Exciters. `Fixed` holds `Vf = V_ref`.
// @block Exciters
#[derive(Clone, Debug)]
pub(crate) enum Avr {
    Fixed,
    /// A block graph defined at run time.
    #[cfg(feature = "rms-experimental")]
    Graph(Box<CompiledGraph>),
    Sexs {
        ta_tb: f64,
        tb: f64,
        k: f64,
        te: f64,
        v_min: f64,
        v_max: f64,
        /// The field voltage's clamp: `[EMIN, EMAX]`, widened to a state
        /// that rests past them at initialization ([`Avr::rest_at`]).
        out: (f64, f64),
        /// Experimental: the field voltage limiter's mode, frozen during a step
        /// (`TELLEGEN_FREEZE=1`).
        lim: LimitMode,
    },
    Esac1a {
        tr: f64,
        tb: f64,
        tc: f64,
        ka: f64,
        ta: f64,
        va_min: f64,
        va_max: f64,
        /// `VA`'s clamp: `[VAMIN, VAMAX]`, widened to a state that rests
        /// past them at initialization ([`Avr::rest_at`]).
        va_out: (f64, f64),
        te: f64,
        kf: f64,
        tf: f64,
        kc: f64,
        kd: f64,
        ke: f64,
        sat: Saturation,
        vr_min: f64,
        vr_max: f64,
    },
    Exst1 {
        tr: f64,
        vi_min: f64,
        vi_max: f64,
        tc: f64,
        tb: f64,
        ka: f64,
        ta: f64,
        vr_min: f64,
        vr_max: f64,
        kc: f64,
        kf: f64,
        tf: f64,
    },
}

/// Exciter inputs.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AvrIn {
    pub vt: f64,
    pub vs: f64,
    pub xad_ifd: f64,
    pub v_ref: f64,
    /// Experimental: active and reactive power on the machine base.
    pub p: f64,
    pub q: f64,
}

impl Avr {
    pub(crate) fn n_states(&self) -> usize {
        match self {
            Self::Fixed => 0,
            #[cfg(feature = "rms-experimental")]
            Self::Graph(g) => g.n_states(),
            Self::Sexs { .. } => 2,
            Self::Esac1a { .. } => 5,
            Self::Exst1 { .. } => 4,
        }
    }

    pub(crate) fn name(&self) -> String {
        match self {
            Self::Fixed => "fixed",
            #[cfg(feature = "rms-experimental")]
            Self::Graph(g) => return graph_name(g),
            Self::Sexs { .. } => "SEXS",
            Self::Esac1a { .. } => "ESAC1A",
            Self::Exst1 { .. } => "EXST1",
        }
        .into()
    }

    pub(crate) fn state_names(&self) -> Vec<String> {
        let names: &[&str] = match self {
            Self::Fixed => &[],
            #[cfg(feature = "rms-experimental")]
            Self::Graph(g) => return g.state_names().to_vec(),
            Self::Sexs { .. } => &["Vf", "Vr"],
            Self::Esac1a { .. } => &["Vm", "Vr1", "Vr2", "Ve", "Vr3"],
            Self::Exst1 { .. } => &["Vm", "Vrll", "Vr", "Vfb"],
        };
        names.iter().map(|s| (*s).to_owned()).collect()
    }

    /// Mass diagonal entries (avr_models.jl `mass_matrix_avr_entries!`).
    pub(crate) fn mass(&self, m: &mut [f64]) {
        match self {
            #[cfg(feature = "rms-experimental")]
            Self::Graph(g) => g.mass(m),
            Self::Sexs { tb, te, .. } => {
                m[0] = *te;
                m[1] = *tb;
            }
            Self::Exst1 { tr, tb, ta, .. } => {
                m[0] = *tr;
                m[1] = *tb;
                m[2] = *ta;
            }
            // The voltage transducer: `TR = 0` makes it algebraic (PSS/E),
            // where PowerSimulationsDynamics.jl substitutes 1 s.
            // `TB` as the lead-lag's mass, as SEXS and EXST1: `TB = TC = 0`
            // (the lead-lag bypassed, common in PSS/E data) makes it
            // algebraic instead of dividing by zero.
            Self::Esac1a { tr, tb, ta, .. } => {
                m[0] = *tr;
                m[1] = *tb;
                m[2] = *ta;
            }
            _ => {}
        }
    }

    /// The exciter with its output and internal limits removed, for solving
    /// the steady state a dispatch needs before checking it against them.
    pub(crate) fn unlimited(&self) -> Self {
        let mut free = self.clone();
        match &mut free {
            #[cfg(feature = "rms-experimental")]
            Self::Graph(g) => **g = g.without_output_clamps(),
            Self::Sexs {
                v_min, v_max, out, ..
            } => {
                (*v_min, *v_max) = (-NO_LIMIT, NO_LIMIT);
                *out = (-NO_LIMIT, NO_LIMIT);
            }
            Self::Esac1a {
                va_min,
                va_max,
                va_out,
                vr_min,
                vr_max,
                ..
            } => {
                (*va_min, *va_max) = (-NO_LIMIT, NO_LIMIT);
                *va_out = (-NO_LIMIT, NO_LIMIT);
                (*vr_min, *vr_max) = (-NO_LIMIT, NO_LIMIT);
            }
            Self::Exst1 {
                vi_min,
                vi_max,
                vr_min,
                vr_max,
                ..
            } => {
                (*vi_min, *vi_max) = (-NO_LIMIT, NO_LIMIT);
                (*vr_min, *vr_max) = (-NO_LIMIT, NO_LIMIT);
            }
            _ => {}
        }
        free
    }

    /// Sets the output clamps from the steady state `x`: SEXS's field voltage
    /// and ESAC1A's `VA` are their lags' states clamped to `[EMIN, EMAX]`
    /// (`[VAMIN, VAMAX]`), as PSS/E has it, so a step that carries the state
    /// past a limit does not pass it on. A state that rests past its limit
    /// at the dispatch (see [`Avr::limit_breaches`]) widens the clamp to
    /// itself, so the unit starts at rest there as
    /// PowerSimulationsDynamics.jl, which applies the state unclamped, has
    /// it; with `rel > 0` to slightly beyond itself ([`rest_clamp`]), as a
    /// linearization at the start takes it.
    pub(crate) fn rest_at(&mut self, x: &[f64], rel: f64) {
        match self {
            #[cfg(feature = "rms-experimental")]
            Self::Graph(g) => g.rest_at(x, rel),
            Self::Sexs {
                v_min, v_max, out, ..
            } => *out = rest_clamp(*v_min, *v_max, x[0], rel),
            Self::Esac1a {
                va_min,
                va_max,
                va_out,
                ..
            } => *va_out = rest_clamp(*va_min, *va_max, x[2], rel),
            _ => {}
        }
    }

    /// The limits a steady state `x` at the inputs `u` stands beyond
    /// (PowerSimulationsDynamics.jl init_avr.jl reports the same). SEXS's
    /// field voltage past `EMAX` and ESAC1A's `VA` past `VAMAX` rest there
    /// anyway (the non-windup lag freezes only while it is pushed further)
    /// unless the lag's time constant is zero; the others leave no steady
    /// state. A block graph's limited lags are checked as SEXS's field
    /// voltage is.
    pub(crate) fn limit_breaches(&self, x: &[f64], u: AvrIn) -> Vec<LimitBreach> {
        let mut out = Vec::new();
        match self {
            #[cfg(feature = "rms-experimental")]
            Self::Graph(g) => out = g.limit_breaches(x),
            // With `TE = 0` (`TA = 0` for ESAC1A) the limited lag is
            // algebraic (`0 = clamp(K·u) − y`): no rest past the limit.
            Self::Sexs {
                v_min, v_max, te, ..
            } => breach(
                &mut out,
                *te >= f64::EPSILON,
                "SEXS field voltage EFD",
                x[0],
                ("EMIN", *v_min),
                ("EMAX", *v_max),
            ),
            Self::Esac1a {
                va_min,
                va_max,
                vr_min,
                vr_max,
                ta,
                ..
            } => {
                // At rest `V_R = V_FE = VA` (the regulator output is VA clamped).
                breach(
                    &mut out,
                    *ta >= f64::EPSILON,
                    "ESAC1A regulator voltage VA",
                    x[2],
                    ("VAMIN", *va_min),
                    ("VAMAX", *va_max),
                );
                breach(
                    &mut out,
                    false,
                    "ESAC1A regulator output VR",
                    x[2],
                    ("VRMIN", *vr_min),
                    ("VRMAX", *vr_max),
                );
            }
            Self::Exst1 {
                vi_min,
                vi_max,
                vr_min,
                vr_max,
                kc,
                kf,
                tf,
                ..
            } => {
                let (vm, vr, vfb) = (x[0], x[2], x[3]);
                let (y_hp, _) = high_pass(vr, vfb, *kf, *tf);
                breach(
                    &mut out,
                    false,
                    "EXST1 voltage error VI",
                    u.v_ref + u.vs - vm - y_hp,
                    ("VIMIN", *vi_min),
                    ("VIMAX", *vi_max),
                );
                breach(
                    &mut out,
                    false,
                    "EXST1 field voltage EFD",
                    vr,
                    ("VT·VRMIN − KC·Ifd", u.vt * vr_min - kc * u.xad_ifd),
                    ("VT·VRMAX − KC·Ifd", u.vt * vr_max - kc * u.xad_ifd),
                );
            }
            _ => {}
        }
        out
    }

    /// Field voltage and exciter derivatives.
    pub(crate) fn eval(&self, x: &[f64], u: AvrIn, f: &mut [f64]) -> f64 {
        match self {
            Self::Fixed => u.v_ref,
            #[cfg(feature = "rms-experimental")]
            Self::Graph(g) => g.eval(x, &[u.vt, u.vs, u.xad_ifd, u.v_ref, u.p, u.q], f),
            Self::Sexs {
                ta_tb,
                tb,
                k,
                te,
                v_min,
                v_max,
                out,
                lim,
            } => {
                // avr_models.jl, SEXS, with the field voltage the state
                // clamped (PSS/E; avr_models.jl applies the state itself,
                // which a step can carry past EMAX, where the non-windup lag
                // then holds it while it is pushed further).
                let (vf, vr) = (x[0], x[1]);
                let v_in = u.v_ref + u.vs - u.vt;
                let (v_ll, dvr) = lead_lag_mm(v_in, vr, 1.0, tb * ta_tb, *tb);
                let dvf = if freeze_limits() && *te >= f64::EPSILON {
                    // Experimental: the non-windup limit in the mode of the step's
                    // start, so the step's equations are smooth; the free rate and the state
                    // are kept for the mode's update after the step.
                    let rate = k * v_ll - vf;
                    lim.record(vf, rate);
                    match lim.mode() {
                        LimitMode::FREE => rate,
                        LimitMode::AT_MAX | LimitMode::AT_MIN => 0.0,
                        _ => low_pass_nonwindup_mm(v_ll, vf, *k, *te, *v_min, *v_max).1,
                    }
                } else {
                    low_pass_nonwindup_mm(v_ll, vf, *k, *te, *v_min, *v_max).1
                };
                f[0] = dvf;
                f[1] = dvr;
                vf.clamp(out.0, out.1)
            }
            Self::Esac1a {
                tb,
                tc,
                ka,
                ta,
                va_min,
                va_max,
                va_out,
                te,
                kf,
                tf,
                kc,
                kd,
                ke,
                sat,
                vr_min,
                vr_max,
                ..
            } => {
                // avr_models.jl, ESAC1A, with VA the state clamped (PSS/E;
                // avr_models.jl passes the state on).
                let (vm, vr1, vr2, ve, vr3) = (x[0], x[1], x[2], x[3], x[4]);
                let se = sat.eval(ve);
                let i_n = kc * u.xad_ifd / ve.max(VE_MIN);
                let v_fe = kd * u.xad_ifd + ke * ve + se * ve;
                let vf = ve * rectifier(i_n);
                let (_, dvm) = low_pass_mm(u.vt, vm, 1.0);
                let (v_f, dvr3) = high_pass(v_fe, vr3, *kf, *tf);
                let v_in = u.v_ref + u.vs - vm - v_f;
                let (y_ll, dvr1) = lead_lag_mm(v_in, vr1, 1.0, *tc, *tb);
                let (_, dvr2) = low_pass_nonwindup_mm(y_ll, vr2, *ka, *ta, *va_min, *va_max);
                let v_r = vr2.clamp(va_out.0, va_out.1).clamp(*vr_min, *vr_max);
                f[0] = dvm;
                f[1] = dvr1;
                f[2] = dvr2;
                f[3] = (v_r - v_fe) / te;
                f[4] = dvr3;
                vf
            }
            Self::Exst1 {
                vi_min,
                vi_max,
                tc,
                tb,
                ka,
                vr_min,
                vr_max,
                kc,
                kf,
                tf,
                ..
            } => {
                // avr_models.jl, EXST1.
                let (vm, vrll, vr, vfb) = (x[0], x[1], x[2], x[3]);
                let v_ref = u.v_ref + u.vs;
                let (_, dvm) = low_pass_mm(u.vt, vm, 1.0);
                let (y_hp, dvfb) = high_pass(vr, vfb, *kf, *tf);
                let (y_ll, dvrll) = lead_lag_mm(
                    (v_ref - vm - y_hp).clamp(*vi_min, *vi_max),
                    vrll,
                    1.0,
                    *tc,
                    *tb,
                );
                let (_, dvr) = low_pass_mm(y_ll, vr, *ka);
                f[0] = dvm;
                f[1] = dvrll;
                f[2] = dvr;
                f[3] = dvfb;
                vr.clamp(
                    u.vt * vr_min - kc * u.xad_ifd,
                    u.vt * vr_max - kc * u.xad_ifd,
                )
            }
        }
    }
}

/// The output clamp `[lo, hi]` of a limited lag whose state rests at `v`:
/// unchanged with `v` inside it; past a side, that side moved to `v`, and
/// `rel · max(|v|, 1)` beyond it. A run takes `rel = 0`: the non-windup lag
/// holds a state pushed further out, so the clamp at the state changes
/// nothing there. A linearization at the start takes [`REST_MARGIN`]: its
/// forward difference steps the state by [`super::system::FD_REL`]
/// `· max(|v|, 1)`, outward, and a clamp at `v` itself would give the
/// output no sensitivity to the state (a mode at the lag's `−1/T` alone,
/// its coupling lost). The run keeps the clamp at the state because a wider
/// one changes the Jacobian its Newton iterations use, and with it the
/// trajectory in the last digits.
pub(crate) fn rest_clamp(lo: f64, hi: f64, v: f64, rel: f64) -> (f64, f64) {
    let margin = rel * v.abs().max(1.0);
    (
        if v < lo { v - margin } else { lo },
        if v > hi { v + margin } else { hi },
    )
}

/// The relative margin a linearization opens beyond a state resting past
/// its limit ([`rest_clamp`]): a hundred forward-difference steps.
#[cfg(feature = "rms-experimental")]
pub(crate) const REST_MARGIN: f64 = 100.0 * super::system::FD_REL;

/// The bound standing in for a removed limit: finite, so a clamp scaled by
/// the terminal voltage (EXST1's output) stays ordered even at `VT = 0`.
const NO_LIMIT: f64 = 1e30;

/// A limit an exciter or governor at rest stands beyond. `holds` is false
/// when the limited model cannot rest there at all (its initialization
/// fails); true when it rests there anyway, past the limit.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LimitBreach {
    pub holds: bool,
    pub text: String,
}

/// `value` against `[lo, hi]` (with a 1e-8 relative slack for the solver):
/// the breach as `"{what} = value pu is above {hi_name} = hi"`, or below.
pub(crate) fn breach(
    out: &mut Vec<LimitBreach>,
    holds: bool,
    what: &str,
    value: f64,
    (lo_name, lo): (&str, f64),
    (hi_name, hi): (&str, f64),
) {
    let slack = |b: f64| 1e-8 * (1.0 + b.abs());
    let side = if value > hi + slack(hi) {
        Some(("above", hi_name, hi))
    } else if value < lo - slack(lo) {
        Some(("below", lo_name, lo))
    } else {
        None
    };
    if let Some((side, name, bound)) = side {
        out.push(LimitBreach {
            holds,
            text: format!("{what} = {value:.4} pu is {side} {name} = {bound:.4}"),
        });
    }
}

/// Turbine-governors. `Fixed` holds `τm = P_ref`.
// @block Governors
#[derive(Clone, Debug)]
pub(crate) enum Governor {
    Fixed,
    /// A block graph defined at run time; its output is the torque.
    #[cfg(feature = "rms-experimental")]
    Graph(Box<CompiledGraph>),
    Tgov1 {
        r: f64,
        t1: f64,
        v_min: f64,
        v_max: f64,
        t2: f64,
        t3: f64,
        d_t: f64,
    },
    Gast {
        r: f64,
        t1: f64,
        t2: f64,
        t3: f64,
        at: f64,
        kt: f64,
        v_min: f64,
        v_max: f64,
        d_turb: f64,
    },
    Hygov {
        r: f64,
        r_temp: f64,
        tr: f64,
        tf: f64,
        tg: f64,
        velm: f64,
        g_min: f64,
        g_max: f64,
        tw: f64,
        at: f64,
        d_t: f64,
        q_nl: f64,
    },
}

impl Governor {
    pub(crate) fn n_states(&self) -> usize {
        match self {
            Self::Fixed => 0,
            #[cfg(feature = "rms-experimental")]
            Self::Graph(g) => g.n_states(),
            Self::Tgov1 { .. } => 2,
            Self::Gast { .. } => 3,
            Self::Hygov { .. } => 4,
        }
    }

    pub(crate) fn name(&self) -> String {
        match self {
            Self::Fixed => "fixed",
            #[cfg(feature = "rms-experimental")]
            Self::Graph(g) => return graph_name(g),
            Self::Tgov1 { .. } => "TGOV1",
            Self::Gast { .. } => "GAST",
            Self::Hygov { .. } => "HYGOV",
        }
        .into()
    }

    pub(crate) fn state_names(&self) -> Vec<String> {
        let names: &[&str] = match self {
            Self::Fixed => &[],
            #[cfg(feature = "rms-experimental")]
            Self::Graph(g) => return g.state_names().to_vec(),
            Self::Tgov1 { .. } => &["x_g1", "x_g2"],
            Self::Gast { .. } => &["x_g1", "x_g2", "x_g3"],
            Self::Hygov { .. } => &["x_g1", "x_g2", "x_g3", "x_g4"],
        };
        names.iter().map(|s| (*s).to_owned()).collect()
    }

    pub(crate) fn mass(&self, m: &mut [f64]) {
        match self {
            #[cfg(feature = "rms-experimental")]
            Self::Graph(g) => g.mass(m),
            Self::Hygov { tf, tg, .. } => {
                m[0] = *tf;
                m[2] = *tg;
            }
            _ => {}
        }
    }

    /// The governor with its valve and gate limits removed (GAST's
    /// temperature limit stays: its initialization does not solve).
    pub(crate) fn unlimited(&self) -> Self {
        let mut free = self.clone();
        match &mut free {
            Self::Tgov1 { v_min, v_max, .. } | Self::Gast { v_min, v_max, .. } => {
                (*v_min, *v_max) = (-NO_LIMIT, NO_LIMIT);
            }
            Self::Hygov { g_min, g_max, .. } => (*g_min, *g_max) = (-NO_LIMIT, NO_LIMIT),
            _ => {}
        }
        free
    }

    /// The limits a steady state `x` with the torque `tm` stands beyond
    /// (PowerSimulationsDynamics.jl init_tg.jl): none of them leaves a
    /// steady state at that torque.
    pub(crate) fn limit_breaches(&self, x: &[f64], tm: f64) -> Vec<LimitBreach> {
        let mut out = Vec::new();
        match self {
            Self::Tgov1 { v_min, v_max, .. } => breach(
                &mut out,
                false,
                "TGOV1 valve position",
                x[0],
                ("VMIN", *v_min),
                ("VMAX", *v_max),
            ),
            Self::Gast {
                v_min, v_max, at, ..
            } => {
                breach(
                    &mut out,
                    false,
                    "GAST valve position",
                    x[0],
                    ("VMIN", *v_min),
                    ("VMAX", *v_max),
                );
                breach(
                    &mut out,
                    false,
                    "GAST turbine power",
                    tm,
                    ("-", f64::NEG_INFINITY),
                    ("AT", *at),
                );
            }
            // At rest the gate `c` is the gate servo's state `x3`.
            Self::Hygov { g_min, g_max, .. } => breach(
                &mut out,
                false,
                "HYGOV gate opening",
                x[2],
                ("GMIN", *g_min),
                ("GMAX", *g_max),
            ),
            _ => {}
        }
        out
    }

    /// Mechanical torque and governor derivatives.
    pub(crate) fn eval(&self, x: &[f64], omega: f64, p_ref: f64, f: &mut [f64]) -> f64 {
        match self {
            Self::Fixed => p_ref,
            #[cfg(feature = "rms-experimental")]
            Self::Graph(g) => g.eval(x, &[omega, p_ref], f),
            Self::Tgov1 {
                r,
                t1,
                v_min,
                v_max,
                t2,
                t3,
                d_t,
            } => {
                // tg_models.jl, SteamTurbineGov1.
                let inv_r = if *r < f64::EPSILON { 0.0 } else { 1.0 / r };
                let ref_in = inv_r * (p_ref - (omega - 1.0));
                let (x1_sat, dx1) = low_pass_nonwindup(ref_in, x[0], 1.0, *t1, *v_min, *v_max);
                let (y_ll, dx2) = lead_lag(x1_sat, x[1], 1.0, *t2, *t3);
                f[0] = dx1;
                f[1] = dx2;
                (y_ll - d_t * (omega - 1.0)) / omega
            }
            Self::Gast {
                r,
                t1,
                t2,
                t3,
                at,
                kt,
                v_min,
                v_max,
                d_turb,
            } => {
                // tg_models.jl, GasTG.
                let inv_r = if *r < f64::EPSILON { 0.0 } else { 1.0 / r };
                // The exact min (tg_models.jl): at rest `x = P_ref`, so any
                // dispatch up to `AT` is a steady state, at `AT` on the kink
                // where both branches agree.
                let reference = p_ref - inv_r * (omega - 1.0);
                let limit = at + kt * (at - x[2]);
                let x_in = reference.min(limit);
                let (x1_sat, dx1) = low_pass_nonwindup(x_in, x[0], 1.0, *t1, *v_min, *v_max);
                let (_, dx2) = low_pass(x1_sat, x[1], 1.0, *t2);
                let (_, dx3) = low_pass(x[1], x[2], 1.0, *t3);
                // Experimental: the fuel valve moves at most `set_gast_rate` pu/s.
                let rate = super::system::gast_rate();
                f[0] = if rate > 0.0 { dx1.clamp(-rate, rate) } else { dx1 };
                f[1] = dx2;
                f[2] = dx3;
                (x[1] - d_turb * (omega - 1.0)) / omega
            }
            Self::Hygov {
                r,
                r_temp,
                tr,
                velm,
                g_min,
                g_max,
                tw,
                at,
                d_t,
                q_nl,
                ..
            } => {
                // tg_models.jl, HydroTurbineGov.
                let (x1, x2, x3, x4) = (x[0], x[1], x[2], x[3]);
                let d_omega = omega - 1.0;
                let (c, dx2) =
                    pi_nonwindup(x1, x2, 1.0 / r_temp, 1.0 / (r_temp * tr), *g_min, *g_max);
                let p_in = p_ref - d_omega - r * c;
                let (_, dx1) = low_pass_mm(p_in, x1, 1.0);
                let (_, dx3) = low_pass_mm(c, x3, 1.0);
                let h = (x4 / x3).powi(2);
                f[0] = dx1;
                f[1] = dx2.clamp(-velm, *velm);
                f[2] = dx3;
                f[3] = (1.0 - h) / tw;
                ((x4 - q_nl) * h * at - d_t * d_omega * x3) / omega
            }
        }
    }
}

/// Stabilizers.
// @block Stabilizers
#[derive(Clone, Debug)]
pub(crate) enum Stabilizer {
    None,
    /// A block graph defined at run time; its output is `Vs`.
    #[cfg(feature = "rms-experimental")]
    Graph(Box<CompiledGraph>),
    Ieeest {
        input_code: i32,
        a: [f64; 6],
        t: [f64; 6],
        ks: f64,
        ls_min: f64,
        ls_max: f64,
        vcu: f64,
        vcl: f64,
    },
}

impl Stabilizer {
    pub(crate) fn n_states(&self) -> usize {
        match self {
            Self::None => 0,
            #[cfg(feature = "rms-experimental")]
            Self::Graph(g) => g.n_states(),
            Self::Ieeest { .. } => 7,
        }
    }

    pub(crate) fn name(&self) -> String {
        match self {
            Self::None => "none",
            #[cfg(feature = "rms-experimental")]
            Self::Graph(g) => return graph_name(g),
            Self::Ieeest { .. } => "IEEEST",
        }
        .into()
    }

    pub(crate) fn state_names(&self) -> Vec<String> {
        let names: &[&str] = match self {
            Self::None => &[],
            #[cfg(feature = "rms-experimental")]
            Self::Graph(g) => return g.state_names().to_vec(),
            Self::Ieeest { .. } => &["x_p1", "x_p2", "x_p3", "x_p4", "x_p5", "x_p6", "x_p7"],
        };
        names.iter().map(|s| (*s).to_owned()).collect()
    }

    pub(crate) fn mass(&self, m: &mut [f64]) {
        #[cfg(feature = "rms-experimental")]
        if let Self::Graph(g) = self {
            g.mass(m);
        }
        if let Self::Ieeest { a, t, .. } = self {
            m[0] = a[3];
            m[2] = a[1];
            m[4] = t[1];
            m[5] = t[3];
            m[6] = t[5];
        }
    }

    /// Stabilizer output `Vs` and derivatives. `pe_sys` is the electrical
    /// power on the system base, `scale` the machine's `MBASE / Sbase`.
    pub(crate) fn eval(
        &self,
        x: &[f64],
        omega: f64,
        pe_sys: f64,
        vt: f64,
        scale: f64,
        f: &mut [f64],
    ) -> f64 {
        match self {
            Self::None => 0.0,
            #[cfg(feature = "rms-experimental")]
            Self::Graph(g) => g.eval(x, &[omega, pe_sys, vt, pe_sys / scale], f),
            Self::Ieeest {
                input_code,
                a,
                t,
                ks,
                ls_min,
                ls_max,
                vcu,
                vcl,
            } => {
                // pss_models.jl, IEEEST. The PSS/E input code 3 is the
                // electrical power on the machine base (`MBASE`).
                let u = match input_code {
                    3 => pe_sys / scale,
                    _ => omega - 1.0,
                };
                let (_, dx1, dx2) = low_pass_2nd_mm(u, x[0], x[1], 1.0, a[2]);
                let (y_f, dx3, dx4) = lead_lag_2nd_mm(x[1], x[2], x[3], a[0], a[1], a[4], a[5]);
                let (y_ll1, dx5) = lead_lag_mm(y_f, x[4], 1.0, t[0], t[1]);
                let (y_ll2, dx6) = lead_lag_mm(y_ll1, x[5], 1.0, t[2], t[3]);
                let (y_out, dx7) = high_pass_mm(y_ll2, x[6], ks * t[4], t[5]);
                f[0] = dx1;
                f[1] = dx2;
                f[2] = dx3;
                f[3] = dx4;
                f[4] = dx5;
                f[5] = dx6;
                f[6] = dx7;
                let v_ss = y_out.clamp(*ls_min, *ls_max);
                terminal_cutoff(v_ss, vt, *vcl, *vcu)
            }
        }
    }
}

/// The IEEEST output cut-off (PSS/E): `Vs = 0` while the terminal voltage
/// is outside `[VCL, VCU]`, each bound ignored when it is zero.
/// PowerSimulationsDynamics.jl 0.16.2 (`output_pss_limiter`) ignores both
/// when either is zero.
pub(crate) fn terminal_cutoff(v_ss: f64, vt: f64, vcl: f64, vcu: f64) -> f64 {
    let above = vcu != 0.0 && vt > vcu;
    let below = vcl != 0.0 && vt < vcl;
    if above || below {
        0.0
    } else {
        v_ss
    }
}

/// A graph controller's name in model lists: its name with a `*` (edited).
#[cfg(feature = "rms-experimental")]
fn graph_name(g: &CompiledGraph) -> String {
    let n = g.graph.name.trim();
    format!("{}*", if n.is_empty() { "graph" } else { n })
}

/// Map a PSS/E machine model to the machine and shaft, PowerSystems.jl style:
/// `R = ZR`, the classical `X'd = ZX`.
pub(crate) fn machine_from_psse(
    model: &DynamicModel,
    zsource: (f64, f64),
) -> Result<(Machine, Shaft), String> {
    Ok(match model {
        DynamicModel::Gencls(g) => (
            Machine::Classic {
                r: zsource.0,
                xd_p: zsource.1,
                eq_p: 1.0,
            },
            Shaft { h: g.h, d: g.d },
        ),
        DynamicModel::Genrou(g) | DynamicModel::Genroe(g) => {
            let quadratic = matches!(model, DynamicModel::Genrou(_));
            let (a, b) = if quadratic {
                quadratic_saturation((1.0, 1.2), (g.s1, g.s12))?
            } else {
                exponential_saturation((g.s1, g.s12))?
            };
            (
                Machine::RoundRotor(RoundRotor {
                    r: zsource.0,
                    td0_p: g.td0_p,
                    td0_pp: g.td0_pp,
                    tq0_p: g.tq0_p,
                    tq0_pp: g.tq0_pp,
                    xd: g.xd,
                    xq: g.xq,
                    xd_p: g.xd_p,
                    xq_p: g.xq_p,
                    xd_pp: g.xd_pp,
                    xl: g.xl,
                    sat: if quadratic {
                        Saturation::Quadratic { a, b, knee: true }
                    } else {
                        Saturation::Exponential { a, b }
                    },
                }),
                Shaft { h: g.h, d: g.d },
            )
        }
        DynamicModel::Gensal(g) | DynamicModel::Gensae(g) => {
            let exponential = matches!(model, DynamicModel::Gensae(_));
            let (a, b) = if exponential {
                exponential_saturation((g.s1, g.s12))?
            } else {
                quadratic_saturation((1.0, 1.2), (g.s1, g.s12))?
            };
            (
                Machine::SalientPole(SalientPole {
                    r: zsource.0,
                    td0_p: g.td0_p,
                    td0_pp: g.td0_pp,
                    tq0_pp: g.tq0_pp,
                    xd: g.xd,
                    xq: g.xq,
                    xd_p: g.xd_p,
                    xd_pp: g.xd_pp,
                    xl: g.xl,
                    sat: if exponential {
                        Saturation::Exponential { a, b }
                    } else {
                        Saturation::Quadratic { a, b, knee: true }
                    },
                    exponential,
                }),
                Shaft { h: g.h, d: g.d },
            )
        }
        other => return Err(format!("{} is not a machine model", other.name())),
    })
}

pub(crate) fn avr_from_psse(model: &DynamicModel) -> Result<Avr, String> {
    match model {
        DynamicModel::Sexs(s) => ordered("SEXS", ("EMIN", s.emin), ("EMAX", s.emax))?,
        DynamicModel::Esac1a(e) => {
            ordered("ESAC1A", ("VAMIN", e.vamin), ("VAMAX", e.vamax))?;
            ordered("ESAC1A", ("VRMIN", e.vrmin), ("VRMAX", e.vrmax))?;
        }
        DynamicModel::Exst1(e) => {
            ordered("EXST1", ("VIMIN", e.vimin), ("VIMAX", e.vimax))?;
            ordered("EXST1", ("VRMIN", e.vrmin), ("VRMAX", e.vrmax))?;
        }
        _ => {}
    }
    Ok(match model {
        DynamicModel::Sexs(s) => Avr::Sexs {
            ta_tb: s.ta_tb,
            tb: s.tb,
            k: s.k,
            te: s.te,
            v_min: s.emin,
            v_max: s.emax,
            out: (s.emin, s.emax),
            lim: LimitMode::default(),
        },
        DynamicModel::Esac1a(e) => Avr::Esac1a {
            tr: e.tr,
            tb: e.tb,
            tc: e.tc,
            ka: e.ka,
            ta: e.ta,
            va_min: e.vamin,
            va_max: e.vamax,
            va_out: (e.vamin, e.vamax),
            te: e.te,
            kf: e.kf,
            tf: e.tf,
            kc: e.kc,
            kd: e.kd,
            ke: e.ke,
            sat: {
                let (a, b) = quadratic_saturation((e.e1, e.e2), (e.se_e1, e.se_e2))?;
                Saturation::Quadratic { a, b, knee: true }
            },
            vr_min: e.vrmin,
            vr_max: e.vrmax,
        },
        DynamicModel::Exst1(e) => Avr::Exst1 {
            tr: e.tr,
            vi_min: e.vimin,
            vi_max: e.vimax,
            tc: e.tc,
            tb: e.tb,
            ka: e.ka,
            ta: e.ta,
            vr_min: e.vrmin,
            vr_max: e.vrmax,
            kc: e.kc,
            kf: e.kf,
            tf: e.tf,
        },
        other => return Err(format!("{} is not an exciter model", other.name())),
    })
}

pub(crate) fn governor_from_psse(model: &DynamicModel) -> Result<Governor, String> {
    match model {
        DynamicModel::Tgov1(g) => ordered("TGOV1", ("VMIN", g.vmin), ("VMAX", g.vmax))?,
        DynamicModel::Gast(g) => ordered("GAST", ("VMIN", g.vmin), ("VMAX", g.vmax))?,
        DynamicModel::Hygov(g) => {
            ordered("HYGOV", ("GMIN", g.gmin), ("GMAX", g.gmax))?;
            ordered("HYGOV", ("-VELM", -g.velm), ("VELM", g.velm))?;
        }
        _ => {}
    }
    Ok(match model {
        DynamicModel::Tgov1(g) => Governor::Tgov1 {
            r: g.r,
            t1: g.t1,
            v_min: g.vmin,
            v_max: g.vmax,
            t2: g.t2,
            t3: g.t3,
            d_t: g.dt,
        },
        DynamicModel::Gast(g) => Governor::Gast {
            r: g.r,
            t1: g.t1,
            t2: g.t2,
            t3: g.t3,
            at: g.at,
            kt: g.kt,
            v_min: g.vmin,
            v_max: g.vmax,
            d_turb: g.dturb,
        },
        DynamicModel::Hygov(g) => Governor::Hygov {
            r: g.r,
            r_temp: g.r_temp,
            tr: g.tr,
            tf: g.tf,
            tg: g.tg,
            velm: g.velm,
            g_min: g.gmin,
            g_max: g.gmax,
            tw: g.tw,
            at: g.at,
            d_t: g.dturb,
            q_nl: g.qnl,
        },
        other => return Err(format!("{} is not a governor model", other.name())),
    })
}

pub(crate) fn stabilizer_from_psse(model: &DynamicModel) -> Result<Stabilizer, String> {
    Ok(match model {
        DynamicModel::Ieeest(p) => {
            if p.remote_bus.is_some() {
                return Err("IEEEST with a remote input bus is not supported".into());
            }
            ordered("IEEEST", ("LSMIN", p.lsmin), ("LSMAX", p.lsmax))?;
            if !matches!(p.input_code, 1 | 3) {
                return Err(format!(
                    "IEEEST input code {} is not supported (1 speed, 3 electrical power)",
                    p.input_code
                ));
            }
            Stabilizer::Ieeest {
                input_code: i32::from(p.input_code),
                a: [p.a1, p.a2, p.a3, p.a4, p.a5, p.a6],
                t: [p.t1, p.t2, p.t3, p.t4, p.t5, p.t6],
                ks: p.ks,
                ls_min: p.lsmin,
                ls_max: p.lsmax,
                vcu: p.vcu,
                vcl: p.vcl,
            }
        }
        other => return Err(format!("{} is not a stabilizer model", other.name())),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// IEEEST's cut-off: each of `VCL`, `VCU` ignored only when it is zero
    /// itself, so `VCU = 1.2`, `VCL = 0` still cuts the output above 1.2.
    #[test]
    fn ieeest_cutoff_bounds_are_independent() {
        assert_eq!(terminal_cutoff(0.05, 1.25, 0.0, 1.2), 0.0);
        assert_eq!(terminal_cutoff(0.05, 1.1, 0.0, 1.2), 0.05);
        assert_eq!(terminal_cutoff(0.05, 0.1, 0.0, 1.2), 0.05);
        assert_eq!(terminal_cutoff(0.05, 0.7, 0.8, 0.0), 0.0);
        assert_eq!(terminal_cutoff(0.05, 1.5, 0.8, 0.0), 0.05);
        assert_eq!(terminal_cutoff(0.05, 1.5, 0.0, 0.0), 0.05);
        assert_eq!(terminal_cutoff(0.05, 1.0, 0.8, 1.2), 0.05);
        assert_eq!(terminal_cutoff(0.05, 1.3, 0.8, 1.2), 0.0);
    }

    /// GAST's governor input is the exact `min(reference, limit)`: a unit
    /// dispatched just under its temperature limit `AT` is at rest (before,
    /// a 1e-4 tie band put it on the limit branch, 2e-4 off rest).
    #[test]
    fn gast_just_under_its_temperature_limit_is_at_rest() {
        let gov = Governor::Gast {
            r: 0.05,
            t1: 0.4,
            t2: 0.1,
            t3: 3.0,
            at: 1.0,
            kt: 2.0,
            v_min: 0.0,
            v_max: 1.2,
            d_turb: 0.0,
        };
        for tm0 in [0.5, 1.0 - 2e-5, 1.0] {
            let mut f = [0.0; 3];
            let tm = gov.eval(&[tm0; 3], 1.0, tm0, &mut f);
            assert!((tm - tm0).abs() < 1e-15, "{tm0}: {tm}");
            assert!(f.iter().all(|d| d.abs() < 1e-15), "{tm0}: {f:?}");
        }
    }

    /// PSS/E's quadratic saturation: zero up to the knee `A`, continuous
    /// there, rising above it, and finite down to (and below) zero, where
    /// the formula alone diverges (`B A² / x`).
    #[test]
    fn quadratic_saturation_has_a_knee_and_stays_finite() {
        let (a, b) = quadratic_saturation((1.0, 1.2), (0.1, 0.4)).unwrap();
        assert!(a > SAT_X_MIN && a < 1.0, "A = {a}");
        let se = |x: f64| quadratic_se(a, b, x, true);
        assert!((se(1.0) - 0.1).abs() < 1e-12 && (se(1.2) - 0.4).abs() < 1e-12);
        for x in [-1.0, 0.0, 1e-300, 1e-6, 0.5 * a, a] {
            assert_eq!(se(x), 0.0, "Se({x})");
        }
        // Continuous at the knee: Se(A + d) = B d² / (A + d).
        for d in [1e-3, 1e-6, 1e-9] {
            assert!(se(a + d) <= b * d * d / a * (1.0 + 1e-12), "{}", se(a + d));
        }
        // Increasing above it.
        let xs: Vec<f64> = (1..200).map(|k| a + 0.01 * f64::from(k)).collect();
        assert!(xs.windows(2).all(|w| se(w[1]) > se(w[0])));
        // The formula without the knee (PowerSimulationsDynamics.jl) rises
        // below A and is floored only where x < SAT_X_MIN.
        let psid = |x: f64| quadratic_se(a, b, x, false);
        assert!(psid(0.5 * a) > 0.0);
        assert!(psid(0.0).is_finite() && psid(0.0) == b * a * a / SAT_X_MIN);
        // Exponential saturation is zero at and below zero flux, not NaN.
        let e = Saturation::Exponential { a: 6.0, b: 0.05 };
        assert_eq!(e.eval(0.0), 0.0);
        assert_eq!(e.eval(-0.1), 0.0);
    }

    /// `ESAC1A` as its exciter voltage `VE` collapses (a deep fault): the
    /// field voltage and every derivative stay finite and the field voltage
    /// falls to zero with `VE` (`IN = KC Ifd / max(VE, VE_MIN)`, `FEX(IN ≥ 1)
    /// = 0`). Before, `IN = KC Ifd / VE` was infinite at `VE = 0` and the
    /// saturation `B (VE − A)² / VE` too.
    #[test]
    fn esac1a_stays_finite_as_its_exciter_voltage_collapses() {
        let (a, b) = quadratic_saturation((3.1, 4.2), (0.03, 0.1)).unwrap();
        let avr = Avr::Esac1a {
            tr: 0.0,
            tb: 0.0,
            tc: 0.0,
            ka: 400.0,
            ta: 0.02,
            va_min: -14.5,
            va_max: 14.5,
            va_out: (-14.5, 14.5),
            te: 0.8,
            kf: 0.03,
            tf: 1.0,
            kc: 0.2,
            kd: 0.38,
            ke: 1.0,
            sat: Saturation::Quadratic { a, b, knee: true },
            vr_min: -5.43,
            vr_max: 6.03,
        };
        let mut f = vec![0.0; avr.n_states()];
        let mut m = vec![1.0; avr.n_states()];
        avr.mass(&mut m);
        assert_eq!(m[..2], [0.0, 0.0], "TR = TB = 0: algebraic");
        let mut last = f64::INFINITY;
        for ve in [2.0, 1.0, 0.1, 1e-2, 1e-3, 1e-6, 0.0, -1e-3] {
            let x = [1.0, 0.0, 2.0, ve, 0.0];
            let u = AvrIn {
                vt: 0.1,
                vs: 0.0,
                xad_ifd: 2.5,
                v_ref: 1.0,
                p: 0.0,
                q: 0.0,
            };
            let vf = avr.eval(&x, u, &mut f);
            assert!(vf.is_finite() && f.iter().all(|v| v.is_finite()), "VE {ve}");
            assert!(vf.abs() <= last.abs() + 1e-12, "VE {ve}: Efd {vf}");
            last = vf;
        }
        assert!(last.abs() < 1e-12, "Efd {last} at VE ≤ 0");
    }
}
