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

use std::f64::consts::PI;

use powerio::DynamicModel;

use super::blocks::*;
use super::graph::CompiledGraph;

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

#[derive(Clone, Copy, Debug)]
pub(crate) enum Saturation {
    /// `Se(x) = B (x − A)² / x`.
    Quadratic { a: f64, b: f64 },
    /// `Se(x) = B x^A`.
    Exponential { a: f64, b: f64 },
}

impl Saturation {
    fn eval(self, x: f64) -> f64 {
        match self {
            Self::Quadratic { a, b } => {
                if b == 0.0 {
                    0.0
                } else {
                    b * (x - a).powi(2) / x
                }
            }
            Self::Exponential { a, b } => {
                if b == 0.0 {
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

/// Exciters. `Fixed` holds `Vf = V_ref`.
#[derive(Clone, Debug)]
pub(crate) enum Avr {
    Fixed,
    /// A block graph defined at run time.
    Graph(Box<CompiledGraph>),
    Sexs {
        ta_tb: f64,
        tb: f64,
        k: f64,
        te: f64,
        v_min: f64,
        v_max: f64,
    },
    Esac1a {
        tr: f64,
        tb: f64,
        tc: f64,
        ka: f64,
        ta: f64,
        va_min: f64,
        va_max: f64,
        te: f64,
        kf: f64,
        tf: f64,
        kc: f64,
        kd: f64,
        ke: f64,
        sat: (f64, f64),
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
}

impl Avr {
    pub(crate) fn n_states(&self) -> usize {
        match self {
            Self::Fixed => 0,
            Self::Graph(g) => g.n_states(),
            Self::Sexs { .. } => 2,
            Self::Esac1a { .. } => 5,
            Self::Exst1 { .. } => 4,
        }
    }

    pub(crate) fn name(&self) -> String {
        match self {
            Self::Fixed => "fixed",
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
            Self::Esac1a { tr, .. } => m[0] = *tr,
            _ => {}
        }
    }

    /// Field voltage and exciter derivatives.
    pub(crate) fn eval(&self, x: &[f64], u: AvrIn, f: &mut [f64]) -> f64 {
        match self {
            Self::Fixed => u.v_ref,
            Self::Graph(g) => g.eval(x, &[u.vt, u.vs, u.xad_ifd, u.v_ref], f),
            Self::Sexs {
                ta_tb,
                tb,
                k,
                v_min,
                v_max,
                ..
            } => {
                // avr_models.jl, SEXS. The field voltage is the state itself.
                let (vf, vr) = (x[0], x[1]);
                let v_in = u.v_ref + u.vs - u.vt;
                let (v_ll, dvr) = lead_lag_mm(v_in, vr, 1.0, tb * ta_tb, *tb);
                let (_, dvf) = low_pass_nonwindup_mm(v_ll, vf, *k, *v_min, *v_max);
                f[0] = dvf;
                f[1] = dvr;
                vf
            }
            Self::Esac1a {
                tb,
                tc,
                ka,
                ta,
                va_min,
                va_max,
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
                // avr_models.jl, ESAC1A.
                let (vm, vr1, vr2, ve, vr3) = (x[0], x[1], x[2], x[3], x[4]);
                let se = if sat.1 == 0.0 {
                    0.0
                } else {
                    sat.1 * (ve - sat.0).powi(2) / ve
                };
                let i_n = kc * u.xad_ifd / ve;
                let v_fe = kd * u.xad_ifd + ke * ve + se * ve;
                let vf = ve * rectifier(i_n);
                let (_, dvm) = low_pass_mm(u.vt, vm, 1.0);
                let (v_f, dvr3) = high_pass(v_fe, vr3, *kf, *tf);
                let v_in = u.v_ref + u.vs - vm - v_f;
                let (y_ll, dvr1) = lead_lag(v_in, vr1, 1.0, *tc, *tb);
                let (_, dvr2) = low_pass_nonwindup(y_ll, vr2, *ka, *ta, *va_min, *va_max);
                let v_r = vr2.clamp(*vr_min, *vr_max);
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

/// Turbine-governors. `Fixed` holds `τm = P_ref`.
#[derive(Clone, Debug)]
pub(crate) enum Governor {
    Fixed,
    /// A block graph defined at run time; its output is the torque.
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
            Self::Graph(g) => g.n_states(),
            Self::Tgov1 { .. } => 2,
            Self::Gast { .. } => 3,
            Self::Hygov { .. } => 4,
        }
    }

    pub(crate) fn name(&self) -> String {
        match self {
            Self::Fixed => "fixed",
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
            Self::Graph(g) => return g.state_names().to_vec(),
            Self::Tgov1 { .. } => &["x_g1", "x_g2"],
            Self::Gast { .. } => &["x_g1", "x_g2", "x_g3"],
            Self::Hygov { .. } => &["x_g1", "x_g2", "x_g3", "x_g4"],
        };
        names.iter().map(|s| (*s).to_owned()).collect()
    }

    pub(crate) fn mass(&self, m: &mut [f64]) {
        match self {
            Self::Graph(g) => g.mass(m),
            Self::Hygov { tf, tg, .. } => {
                m[0] = *tf;
                m[2] = *tg;
            }
            _ => {}
        }
    }

    /// Mechanical torque and governor derivatives.
    pub(crate) fn eval(&self, x: &[f64], omega: f64, p_ref: f64, f: &mut [f64]) -> f64 {
        match self {
            Self::Fixed => p_ref,
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
                // A unit initialized at its temperature limit sits exactly on
                // the kink of the min; a tie resolves to the limit, the branch
                // PowerSimulationsDynamics.jl's initialization lands on.
                let reference = p_ref - inv_r * (omega - 1.0);
                let limit = at + kt * (at - x[2]);
                let x_in = if reference < limit - 1e-4 {
                    reference
                } else {
                    limit
                };
                let (x1_sat, dx1) = low_pass_nonwindup(x_in, x[0], 1.0, *t1, *v_min, *v_max);
                let (_, dx2) = low_pass(x1_sat, x[1], 1.0, *t2);
                let (_, dx3) = low_pass(x[1], x[2], 1.0, *t3);
                f[0] = dx1;
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
#[derive(Clone, Debug)]
pub(crate) enum Stabilizer {
    None,
    /// A block graph defined at run time; its output is `Vs`.
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
            Self::Graph(g) => g.n_states(),
            Self::Ieeest { .. } => 7,
        }
    }

    pub(crate) fn name(&self) -> String {
        match self {
            Self::None => "none",
            Self::Graph(g) => return graph_name(g),
            Self::Ieeest { .. } => "IEEEST",
        }
        .into()
    }

    pub(crate) fn state_names(&self) -> Vec<String> {
        let names: &[&str] = match self {
            Self::None => &[],
            Self::Graph(g) => return g.state_names().to_vec(),
            Self::Ieeest { .. } => &["x_p1", "x_p2", "x_p3", "x_p4", "x_p5", "x_p6", "x_p7"],
        };
        names.iter().map(|s| (*s).to_owned()).collect()
    }

    pub(crate) fn mass(&self, m: &mut [f64]) {
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
                // pss_models.jl output_pss_limiter: bypassed when a limit is zero.
                if *vcl == 0.0 || *vcu == 0.0 || (*vcl <= vt && vt <= *vcu) {
                    v_ss
                } else {
                    0.0
                }
            }
        }
    }
}

/// A graph controller's name in model lists: its name with a `*` (edited).
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
                        Saturation::Quadratic { a, b }
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
                        Saturation::Quadratic { a, b }
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
    Ok(match model {
        DynamicModel::Sexs(s) => Avr::Sexs {
            ta_tb: s.ta_tb,
            tb: s.tb,
            k: s.k,
            te: s.te,
            v_min: s.emin,
            v_max: s.emax,
        },
        DynamicModel::Esac1a(e) => Avr::Esac1a {
            tr: e.tr,
            tb: e.tb,
            tc: e.tc,
            ka: e.ka,
            ta: e.ta,
            va_min: e.vamin,
            va_max: e.vamax,
            te: e.te,
            kf: e.kf,
            tf: e.tf,
            kc: e.kc,
            kd: e.kd,
            ke: e.ke,
            sat: quadratic_saturation((e.e1, e.e2), (e.se_e1, e.se_e2))?,
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
            if p.remote_bus != 0 {
                return Err("IEEEST with a remote input bus is not supported".into());
            }
            if !matches!(p.input_code, 1 | 3) {
                return Err(format!(
                    "IEEEST input code {} is not supported (1 speed, 3 electrical power)",
                    p.input_code
                ));
            }
            Stabilizer::Ieeest {
                input_code: p.input_code,
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
