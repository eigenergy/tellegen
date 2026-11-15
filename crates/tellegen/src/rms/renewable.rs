//! The WECC generic renewable plant in RMS: converter `REGCA1`, electrical
//! control `REECB1` and plant control `REPCA1` (PSS/E Model Library names).
//!
//! # Provenance
//!
//! A translation of PowerSimulationsDynamics.jl v0.16.2 (BSD-3-Clause,
//! Copyright (c) 2020, 2023 Alliance for Sustainable Energy, LLC and The
//! Regents of the University of California; license text in
//! `licenses/PowerSimulationsDynamics.jl-LICENSE`), which represents the plant
//! as a `DynamicInverter` with
//!
//! - `RenewableEnergyConverterTypeA` (REGCA1): `src/models/inverter_models/converter_models.jl`,
//!   `initialization/inverter_components/init_converter.jl`;
//! - `RECurrentControlB` (REECB1): `inner_control_models.jl`, `init_inner.jl`;
//! - `ActiveRenewableControllerAB` and `ReactiveRenewableControllerAB` (the
//!   REPCA1 plant controller with the REECB1 P and Q paths):
//!   `outer_control_models.jl`, `init_outer.jl`;
//! - `RLFilter(0, 0)`, `FixedFrequency`, `FixedDCSource`: the defaults
//!   PowerSystems.jl v5.12 assigns to a DYR inverter (`psse_dynamic_data.jl`);
//!   the zero filter makes the plant a Norton current at its terminal,
//!   `I = I_cnv − V/Z_source` with PowerSystems.jl's `R_source = 0`,
//!   `X_source = 1e5` (`models/device.jl`, `_update_inner_vars!`);
//! - the block helpers of `src/utils/` (`deadband_function`,
//!   `pi_block_nonwindup`, `low_pass_nonwindup_ramp_limits`, `lead_lag`,
//!   `current_limit_logic`, `get_LVPL_gain`).
//!
//! The parameter order is PowerSystems.jl's DYR mapping
//! (`parsers/psse_dynamic_mapping.yaml`). Flag combinations are the ones
//! PowerSimulationsDynamics.jl implements: active control `Freq_Flag` 0 or 1;
//! reactive control `(Ref_Flag, PF_Flag, V_Flag)` (0, 0, 1), (0, 0, 0),
//! (1, 0, 1), (1, 0, 0); inner `Q_Flag` and `PQ_Flag` 0 or 1.
//!
//! Where this differs from PowerSimulationsDynamics.jl 0.16.2, on purpose:
//!
//! - The low-voltage active current gain is the PSS/E one,
//!   `(V − Lvpnt0)/(Lvpnt1 − Lvpnt0)` between the points; 0.16.2 computes
//!   `(V − Lvpnt0)/(Lvpnt0 − Lvpnt1)` there, a negative gain.
//! - The compensation reactance is `X_c`; 0.16.2 reads `R_c` for it.
//! - Every first-order block is written in mass-matrix form (`T ẏ = u − y`),
//!   so a zero time constant makes the block algebraic instead of dividing by
//!   zero. With nonzero time constants the equations are the same.
//! - The non-windup PI ([`super::blocks::pi_nonwindup`]) integrates while its
//!   input points back inside a limit, and its rate is continuous at the
//!   limit; 0.16.2 stops the integrator whenever the output is limited.
//! - LVPL: below the ceiling the rising rate of `Ip` is capped at
//!   `(LVPL − Ip)/τ` ([`super::blocks::LIMIT_TAU`]), so it is continuous in
//!   `Ip` at the ceiling; 0.16.2 switches it to zero there, which leaves an
//!   implicit step without a solution while `Ip` rides a rising ceiling.
//!   Above the ceiling `Ip` holds, as in 0.16.2.
//! - `IREG` (REECB1, REPCA1) must be 0 or the plant's bus, and limits must
//!   be ordered; otherwise the plant is refused (and held).
//! - The lead-lag state starts at its steady state `(1 − T_ft/T_fv) Q`, and
//!   with `Ref_Flag = 1` the voltage reference is the compensated voltage at
//!   the operating point; 0.16.2 starts both at values that are only steady
//!   when `T_ft = 0` and `K_c = 0` (or `R_c = X_c = 0`).
//!
//! The plant measures the system frequency (the reference machine's speed,
//! as `FixedFrequency` does). Voltage-dip freezing (`Vdip`, `Vup`, `Vfrz`) and
//! the plant's remote branch flow are not modelled, as in
//! PowerSimulationsDynamics.jl.

use super::blocks::{ordered, pi_nonwindup, LIMIT_TAU};
use super::system::C64;

/// Lower bound of a voltage divisor (PowerSimulationsDynamics.jl's
/// `VOLTAGE_DIVISION_LOWER_BOUND`).
const V_LB: f64 = 0.01;

/// `REGCA1`: the converter interface.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Regca1 {
    pub lvpl_sw: bool,
    pub t_g: f64,
    pub rrpwr: f64,
    pub brkpt: f64,
    pub zerox: f64,
    pub lvpl1: f64,
    pub vo_lim: f64,
    pub lv_pnt0: f64,
    pub lv_pnt1: f64,
    pub io_lim: f64,
    pub t_fltr: f64,
    pub k_hv: f64,
    pub iqr_min: f64,
    pub iqr_max: f64,
    /// [`super::RmsOptions::psid_lv_gain`].
    pub psid_lv_gain: bool,
    /// [`super::RmsOptions::psid_converter_ramp`].
    pub psid_ramp: bool,
}

/// `REECB1`: the electrical control.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Reecb1 {
    pub pf_flag: u8,
    pub v_flag: u8,
    pub q_flag: u8,
    pub pq_flag: u8,
    pub t_rv: f64,
    pub dbd1: f64,
    pub dbd2: f64,
    pub k_qv: f64,
    pub iq_l1: f64,
    pub iq_h1: f64,
    pub v_ref0: f64,
    pub q_min_inner: f64,
    pub q_max_inner: f64,
    pub v_min: f64,
    pub v_max: f64,
    pub k_qp: f64,
    pub k_qi: f64,
    pub k_vp: f64,
    pub k_vi: f64,
    pub t_iq: f64,
    pub dp_max: f64,
    pub dp_min: f64,
    pub p_max_inner: f64,
    pub p_min_inner: f64,
    pub i_max: f64,
    pub t_pord: f64,
}

/// `REPCA1`: the plant control.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Repca1 {
    pub vc_flag: u8,
    pub ref_flag: u8,
    pub freq_flag: u8,
    pub t_fltr: f64,
    pub k_p: f64,
    pub k_i: f64,
    pub t_ft: f64,
    pub t_fv: f64,
    pub r_c: f64,
    pub x_c: f64,
    pub k_c: f64,
    pub e_max: f64,
    pub e_min: f64,
    pub dbd1: f64,
    pub dbd2: f64,
    pub q_max: f64,
    pub q_min: f64,
    pub k_pg: f64,
    pub k_ig: f64,
    pub t_p: f64,
    pub fdbd1: f64,
    pub fdbd2: f64,
    pub fe_max: f64,
    pub fe_min: f64,
    pub p_max: f64,
    pub p_min: f64,
    pub t_g: f64,
    pub d_dn: f64,
    pub d_up: f64,
}

/// A renewable plant: the three models and the references its operating
/// point sets.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Plant {
    pub regc: Regca1,
    pub reec: Reecb1,
    pub repc: Repca1,
    /// `MBASE / Sbase`: plant-base current to system-base current.
    pub scale: f64,
    pub r_source: f64,
    pub x_source: f64,
    pub p_ref: f64,
    pub q_ref: f64,
    pub v_ref: f64,
    /// The REECB1 voltage reference (`Vref0`, or the operating voltage when
    /// the record states 0).
    pub v_ref0: f64,
    /// The converter's `Q_ref` (its sign picks the `Iq` rate limit).
    pub q_ref_converter: f64,
    pub omega_ref: f64,
}

fn flag(v: f64, name: &str, allowed: &[u8]) -> Result<u8, String> {
    let f = v.round();
    if (f - v).abs() > 1e-9 || !(0.0..=255.0).contains(&f) || !allowed.contains(&(f as u8)) {
        return Err(format!("{name} = {v} is not one of {allowed:?}"));
    }
    Ok(f as u8)
}

/// The regulated bus `IREG` of a REECB1 or REPCA1 record: 0 or the plant's
/// own bus `bus`. A remote regulated bus is refused: the controllers measure
/// the plant's terminal, and a plant simulated as regulating its own
/// terminal when the data names another bus would be wrong without a word.
fn local_regulation(model: &str, ireg: f64, bus: usize) -> Result<(), String> {
    if ireg == 0.0 || ireg == bus as f64 {
        return Ok(());
    }
    Err(format!(
        "{model} regulates bus {ireg} (IREG), not the plant's own bus {bus}: \
         remote voltage regulation is not supported"
    ))
}

fn numbers(model: &str, tokens: &[String], arity: usize) -> Result<Vec<f64>, String> {
    if tokens.len() != arity {
        return Err(format!(
            "{model} states {} parameters, the PSS/E schema has {arity}",
            tokens.len()
        ));
    }
    tokens
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let t = t.trim().trim_matches('\'').trim();
            t.parse::<f64>()
                .ok()
                .filter(|v| v.is_finite())
                .ok_or_else(|| {
                    format!("{model} parameter {} ({t:?}) is not a finite number", i + 1)
                })
        })
        .collect()
}

impl Regca1 {
    /// From the 15 DYR values (`LVPLSW, Tg, Rrpwr, Brkpt, Zerox, Lvpl1,
    /// Volim, Lvpnt1, Lvpnt0, Iolim, Tfltr, Khv, Iqrmax, Iqrmin, Accel`).
    pub(crate) fn from_tokens(tokens: &[String]) -> Result<Self, String> {
        let p = numbers("REGCA1", tokens, 15)?;
        let v = |i: usize| p[i - 1];
        let lvpl_sw = flag(v(1), "REGCA1 LVPLSW", &[0, 1])? == 1;
        // The two gains divide by these differences.
        if lvpl_sw && !(v(5) < v(4)) {
            return Err(format!(
                "REGCA1 Zerox = {} must be below Brkpt = {}",
                v(5),
                v(4)
            ));
        }
        if !(v(9) < v(8)) {
            return Err(format!(
                "REGCA1 Lvpnt0 = {} must be below Lvpnt1 = {}",
                v(9),
                v(8)
            ));
        }
        Ok(Self {
            lvpl_sw,
            t_g: v(2),
            rrpwr: v(3),
            brkpt: v(4),
            zerox: v(5),
            lvpl1: v(6),
            vo_lim: v(7),
            lv_pnt1: v(8),
            lv_pnt0: v(9),
            io_lim: v(10),
            t_fltr: v(11),
            k_hv: v(12),
            iqr_max: v(13),
            iqr_min: v(14),
            psid_lv_gain: false,
            psid_ramp: false,
        })
    }
}

impl Reecb1 {
    /// From the 30 DYR values (`IREG, PFFLAG, VFLAG, QFLAG, PQFLAG, Vdip,
    /// Vup, Trv, dbd1, dbd2, Kqv, Iqh1, Iql1, Vref0, Tp, Qmax, Qmin, Vmax,
    /// Vmin, Kqp, Kqi, Kvp, Kvi, Tiq, dPmax, dPmin, Pmax, Pmin, Imax, Tpord`)
    /// of the plant at `bus`. `IREG` must be 0 or `bus`.
    pub(crate) fn from_tokens(tokens: &[String], bus: usize) -> Result<Self, String> {
        let p = numbers("REECB1", tokens, 30)?;
        let v = |i: usize| p[i - 1];
        local_regulation("REECB1", v(1), bus)?;
        for (lo, hi) in [
            (("Iql1", v(13)), ("Iqh1", v(12))),
            (("Qmin", v(17)), ("Qmax", v(16))),
            (("Vmin", v(19)), ("Vmax", v(18))),
            (("dPmin", v(26)), ("dPmax", v(25))),
            (("Pmin", v(28)), ("Pmax", v(27))),
            (("0", 0.0), ("Imax", v(29))),
        ] {
            ordered("REECB1", lo, hi)?;
        }
        let pf_flag = flag(v(2), "REECB1 PFFLAG", &[0, 1])?;
        if pf_flag == 1 {
            return Err("REECB1 with PFFLAG = 1 (power factor control) is not supported".into());
        }
        Ok(Self {
            pf_flag,
            v_flag: flag(v(3), "REECB1 VFLAG", &[0, 1])?,
            q_flag: flag(v(4), "REECB1 QFLAG", &[0, 1])?,
            pq_flag: flag(v(5), "REECB1 PQFLAG", &[0, 1])?,
            t_rv: v(8),
            dbd1: v(9),
            dbd2: v(10),
            k_qv: v(11),
            iq_h1: v(12),
            iq_l1: v(13),
            v_ref0: v(14),
            q_max_inner: v(16),
            q_min_inner: v(17),
            v_max: v(18),
            v_min: v(19),
            k_qp: v(20),
            k_qi: v(21),
            k_vp: v(22),
            k_vi: v(23),
            t_iq: v(24),
            dp_max: v(25),
            dp_min: v(26),
            p_max_inner: v(27),
            p_min_inner: v(28),
            i_max: v(29),
            t_pord: v(30),
        })
    }
}

impl Repca1 {
    /// From the 34 DYR values (`IREG, from, to, circuit, VCFlag, RefFlag,
    /// Fflag, Tfltr, Kp, Ki, Tft, Tfv, Vfrz, Rc, Xc, Kc, emax, emin, dbd1,
    /// dbd2, Qmax, Qmin, Kpg, Kig, Tp, fdbd1, fdbd2, femax, femin, Pmax,
    /// Pmin, Tlag, Ddn, Dup`) of the plant at `bus`. The regulated bus must be
    /// 0 or `bus`, and the monitored branch none (0, 0): remote regulation and
    /// a remote branch flow are not modelled.
    pub(crate) fn from_tokens(tokens: &[String], bus: usize) -> Result<Self, String> {
        if tokens.len() != 34 {
            return Err(format!(
                "REPCA1 states {} parameters, the PSS/E schema has 34",
                tokens.len()
            ));
        }
        // The circuit id (4th) may be a quoted string; the rest are numbers.
        let mut t = tokens.to_vec();
        t[3] = "0".into();
        let p = numbers("REPCA1", &t, 34)?;
        let v = |i: usize| p[i - 1];
        local_regulation("REPCA1", v(1), bus)?;
        for (lo, hi) in [
            (("emin", v(18)), ("emax", v(17))),
            (("Qmin", v(22)), ("Qmax", v(21))),
            (("femin", v(29)), ("femax", v(28))),
            (("Pmin", v(31)), ("Pmax", v(30))),
        ] {
            ordered("REPCA1", lo, hi)?;
        }
        if v(2) != 0.0 || v(3) != 0.0 {
            return Err(format!(
                "REPCA1 monitoring branch {}-{}: a remote branch flow is not supported",
                v(2),
                v(3)
            ));
        }
        Ok(Self {
            vc_flag: flag(v(5), "REPCA1 VCFlag", &[0, 1])?,
            ref_flag: flag(v(6), "REPCA1 RefFlag", &[0, 1])?,
            freq_flag: flag(v(7), "REPCA1 Fflag", &[0, 1])?,
            t_fltr: v(8),
            k_p: v(9),
            k_i: v(10),
            t_ft: v(11),
            t_fv: v(12),
            r_c: v(14),
            x_c: v(15),
            k_c: v(16),
            e_max: v(17),
            e_min: v(18),
            dbd1: v(19),
            dbd2: v(20),
            q_max: v(21),
            q_min: v(22),
            k_pg: v(23),
            k_ig: v(24),
            t_p: v(25),
            fdbd1: v(26),
            fdbd2: v(27),
            fe_max: v(28),
            fe_min: v(29),
            p_max: v(30),
            p_min: v(31),
            t_g: v(32),
            d_dn: v(33),
            d_up: v(34),
        })
    }
}

fn deadband(x: f64, low: f64, high: f64) -> f64 {
    if x > high {
        x - high
    } else if x < low {
        x - low
    } else {
        0.0
    }
}

/// Lead-lag `K (1 + s T1)/(1 + s T2)`: output and the mass-matrix row
/// `T2 ẋ = K (1 − T1/T2) u − x`.
fn lead_lag(u: f64, x: f64, k: f64, t1: f64, t2: f64) -> (f64, f64) {
    let r = if t2 < f64::EPSILON { 0.0 } else { t1 / t2 };
    (x + k * r * u, k * (1.0 - r) * u - x)
}

/// LVPL: the active current ceiling against the filtered voltage.
fn lvpl_gain(v: f64, zerox: f64, brkpt: f64, lvpl1: f64) -> f64 {
    if v < zerox {
        0.0
    } else if v > brkpt {
        lvpl1
    } else {
        lvpl1 / (brkpt - zerox) * (v - zerox)
    }
}

/// Low-voltage active current management: the PSS/E definition, or with
/// `psid` PowerSimulationsDynamics.jl 0.16.2's `(v - p0) / (p0 - p1)` between
/// the two points, which is negative there (`saturation_models.jl`,
/// `get_LV_current_gain`; [`super::RmsOptions::psid_lv_gain`]).
fn lv_gain(v: f64, p0: f64, p1: f64, psid: bool) -> f64 {
    if v < p0 {
        0.0
    } else if v > p1 {
        1.0
    } else if psid {
        (v - p0) / (p0 - p1)
    } else {
        (v - p0) / (p1 - p0)
    }
}

/// Number of states of each part: `(active, reactive, inner, converter)`.
fn parts(repc: &Repca1, reec: &Reecb1) -> [usize; 4] {
    let active = if repc.freq_flag == 1 { 4 } else { 1 };
    let reactive = if reec.v_flag == 1 { 4 } else { 3 };
    [active, reactive, 2, 3]
}

/// Converter currents from its states: `(I_cnv, LVPL)` with `I_cnv` in the
/// network frame, plant base, and the low-voltage power limit on `Ip`
/// (infinite with `LVPLSW = 0`). The active current is `min(Ip, LVPL)`.
fn converter_current(regc: &Regca1, ip: f64, iq: f64, vmeas: f64, v: C64) -> (C64, f64) {
    let vt = v.norm();
    let lvpl = if regc.lvpl_sw {
        lvpl_gain(vmeas, regc.zerox, regc.brkpt, regc.lvpl1)
    } else {
        f64::INFINITY
    };
    let ip_sat = ip.min(lvpl);
    let g_lv = lv_gain(vt, regc.lv_pnt0, regc.lv_pnt1, regc.psid_lv_gain);
    // High-voltage reactive current management (REGC_A): above `Volim` the
    // injected reactive current falls by `Khv (V − Volim)`, down to `Iolim`
    // (negative: absorbing) but never below the command itself.
    // PowerSimulationsDynamics.jl 0.16.2 adds the term (more injection at
    // high voltage) and drops `Iolim`.
    let iq_extra = (regc.k_hv * (vt - regc.vo_lim)).max(0.0);
    let iq_inj = if iq_extra > 0.0 {
        (iq - iq_extra).max(regc.io_lim.min(iq))
    } else {
        iq
    };
    let id_cnv = g_lv * ip_sat;
    let iq_cnv = -iq_inj;
    let (s, c) = v.im.atan2(v.re).sin_cos();
    (
        C64::new(id_cnv * c - iq_cnv * s, id_cnv * s + iq_cnv * c),
        lvpl,
    )
}

impl Plant {
    pub(crate) fn n_states(&self) -> usize {
        parts(&self.repc, &self.reec).iter().sum()
    }

    /// State names (ASCII forms of PowerSimulationsDynamics.jl's).
    pub(crate) fn state_names(&self) -> Vec<&'static str> {
        let mut names = Vec::new();
        if self.repc.freq_flag == 1 {
            names.extend(["p_flt", "xi_P", "p_ext", "p_ord"]);
        } else {
            names.push("p_ord");
        }
        names.push(if self.repc.ref_flag == 1 {
            "V_cflt"
        } else {
            "q_flt"
        });
        names.extend(["xi_q_oc", "q_LL"]);
        if self.reec.v_flag == 1 {
            names.push("xi_Q");
        }
        names.push("Vt_filt");
        names.push(if self.reec.q_flag == 0 {
            "I_icv"
        } else {
            "xi_icv"
        });
        names.extend(["Ip", "Iq", "Vmeas"]);
        names
    }

    /// Local mass diagonal (the time constants of the mass-matrix rows).
    pub(crate) fn mass(&self) -> Vec<f64> {
        let (repc, reec, regc) = (&self.repc, &self.reec, &self.regc);
        let mut m = Vec::with_capacity(self.n_states());
        if repc.freq_flag == 1 {
            m.extend([repc.t_p, 1.0, repc.t_g, reec.t_pord]);
        } else {
            m.push(reec.t_pord);
        }
        m.extend([repc.t_fltr, 1.0, repc.t_fv]);
        if reec.v_flag == 1 {
            m.push(1.0);
        }
        m.push(reec.t_rv);
        m.push(if reec.q_flag == 0 { reec.t_iq } else { 1.0 });
        m.extend([regc.t_g, regc.t_g, regc.t_fltr]);
        m
    }

    /// The current into the network (plant base) at states `x` and terminal
    /// voltage `v`: the converter current less `V/Z_source`.
    pub(crate) fn output_current(&self, x: &[f64], v: C64) -> C64 {
        let k = self.n_states();
        let (i_cnv, _) = converter_current(&self.regc, x[k - 3], x[k - 2], x[k - 1], v);
        i_cnv - v / C64::new(self.r_source, self.x_source)
    }

    /// The right-hand side `f` (mass-matrix rows) and the current into the
    /// network, system base, at terminal voltage `vr + j vi` and system
    /// frequency `omega_sys`.
    pub(crate) fn eval(&self, x: &[f64], vr: f64, vi: f64, omega_sys: f64, f: &mut [f64]) -> C64 {
        let (repc, reec, regc) = (&self.repc, &self.reec, &self.regc);
        let [na, nr, _, _] = parts(repc, reec);
        let v = C64::new(vr, vi);
        let vt = v.norm();
        let (xa, rest) = x.split_at(na);
        let (xr, rest) = rest.split_at(nr);
        let (xi, xc) = rest.split_at(2);
        let (fa, rest) = f.split_at_mut(na);
        let (fr, rest) = rest.split_at_mut(nr);
        let (fi, fc) = rest.split_at_mut(2);

        // Converter currents from the states (PowerSimulationsDynamics.jl's
        // `_update_inner_vars!`): the plant controller measures them.
        let (ip, iq, vmeas) = (xc[0], xc[1], xc[2]);
        let (i_cnv, lvpl) = converter_current(regc, ip, iq, vmeas, v);
        let s_cnv = v * i_cnv.conj();
        let (p_elec, q_elec) = (s_cnv.re, s_cnv.im);
        let vt_filt = xi[0];

        // Plant and electrical control, active path.
        let ip_oc = if repc.freq_flag == 1 {
            let (p_flt, xi_p, p_ext, p_ord) = (xa[0], xa[1], xa[2], xa[3]);
            let f_err = deadband(self.omega_ref - omega_sys, repc.fdbd1, repc.fdbd2);
            let p_droop = (repc.d_dn * f_err).min(0.0) + (repc.d_up * f_err).max(0.0);
            let p_err = (self.p_ref + p_droop - p_flt).clamp(repc.fe_min, repc.fe_max);
            fa[0] = p_elec - p_flt;
            let (p_pi, dxi) =
                pi_nonwindup(p_err, xi_p, repc.k_pg, repc.k_ig, repc.p_min, repc.p_max);
            fa[1] = dxi;
            fa[2] = p_pi - p_ext;
            // Ramp-limited, non-windup lag (IEEE Std 421.5).
            // With `Tpord = 0` it is algebraic (no ramp), so its row stays
            // nonsingular at a limit.
            let t = reec.t_pord;
            fa[3] = if t < f64::EPSILON {
                p_ext.clamp(reec.p_min_inner, reec.p_max_inner) - p_ord
            } else {
                let rate = (p_ext - p_ord).clamp(t * reec.dp_min, t * reec.dp_max);
                let stop = (p_ord >= reec.p_max_inner && rate > 0.0)
                    || (p_ord <= reec.p_min_inner && rate < 0.0);
                if stop {
                    0.0
                } else {
                    rate
                }
            };
            p_ord.clamp(reec.p_min_inner, reec.p_max_inner) / vt_filt.max(V_LB)
        } else {
            fa[0] = self.p_ref - xa[0];
            xa[0] / vt_filt.max(V_LB)
        };

        // Plant and electrical control, reactive path.
        let (input, error) = if repc.ref_flag == 1 {
            let i_out = i_cnv - v / C64::new(self.r_source, self.x_source);
            let v_flt_input = if repc.vc_flag == 0 {
                vt + repc.k_c * q_elec
            } else {
                (v - C64::new(repc.r_c, repc.x_c) * i_out).norm()
            };
            (v_flt_input, self.v_ref - xr[0])
        } else {
            (q_elec, self.q_ref - xr[0])
        };
        let q_err = deadband(error, repc.dbd1, repc.dbd2).clamp(repc.e_min, repc.e_max);
        fr[0] = input - xr[0];
        let (q_pi, dxi) = pi_nonwindup(q_err, xr[1], repc.k_p, repc.k_i, repc.q_min, repc.q_max);
        fr[1] = dxi;
        let (q_ext, dll) = lead_lag(q_pi, xr[2], 1.0, repc.t_ft, repc.t_fv);
        fr[2] = dll;
        let v_oc = if reec.v_flag == 1 {
            let u = q_ext.clamp(reec.q_min_inner, reec.q_max_inner) - q_elec;
            let (v_pi, dxq) = pi_nonwindup(u, xr[3], reec.k_qp, reec.k_qi, reec.v_min, reec.v_max);
            fr[3] = dxq;
            v_pi - vt_filt
        } else if repc.ref_flag == 1 {
            q_ext.clamp(reec.v_min, reec.v_max) - vt_filt
        } else {
            q_ext - vt_filt
        };
        let iq_oc = q_ext / vt_filt.max(V_LB);

        // Electrical control, current commands.
        let v_err = deadband(self.v_ref0 - vt_filt, reec.dbd1, reec.dbd2);
        let iq_inj = (reec.k_qv * v_err).clamp(reec.iq_l1, reec.iq_h1);
        fi[0] = vt - vt_filt;
        let i_icv = if reec.q_flag == 0 {
            fi[1] = iq_oc - xi[1];
            xi[1]
        } else {
            fi[1] = v_oc;
            reec.k_vp * v_oc + reec.k_vi * xi[1]
        };
        let (ip_max, iq_max) = current_limits(reec, ip_oc, i_icv + iq_inj);
        let iq_cmd = (i_icv + iq_inj).clamp(-iq_max, iq_max);
        let ip_cmd = ip_oc.clamp(0.0, ip_max);

        // Converter: the `Tg` lags with their ramp limits `Rrpwr`, `Iqrmax`,
        // `Iqrmin` in pu/s on the currents (mass form: the right-hand side
        // `Tg dI/dt` is limited to `Tg` times the rate). With
        // `psid_ramp`, PowerSimulationsDynamics.jl 0.16.2's limit on the
        // lag's error instead (an effective rate `Rrpwr/Tg`). With `Tg = 0`
        // the currents follow their commands (algebraic; no ramp).
        // LVPL caps the active current (`converter_current`). With a lag,
        // `Ip` holds above the ceiling, as in PowerSimulationsDynamics.jl
        // 0.16.2, but below it its rising rate is capped at `(LVPL − Ip)/τ`
        // rather than switched to zero at the ceiling: `Ip` can ride a
        // recovering ceiling, where a switched rate leaves the implicit step
        // without a solution. Algebraic (`Tg = 0`), the state
        // is the command and the cap on the output alone suffices.
        if regc.t_g < f64::EPSILON {
            fc[0] = ip_cmd - ip;
            fc[1] = iq_cmd - iq;
        } else {
            let k = if regc.psid_ramp { 1.0 } else { regc.t_g };
            let (rp_dn, rp_up) = if ip >= 0.0 {
                (f64::NEG_INFINITY, k * regc.rrpwr)
            } else {
                (-k * regc.rrpwr, f64::INFINITY)
            };
            let ceiling = (regc.t_g * (lvpl - ip) / LIMIT_TAU).max(0.0);
            let rate = (ip_cmd - ip).clamp(rp_dn, rp_up).min(ceiling);
            // Above the ceiling `Ip` holds (a falling `Ip` moves away from
            // the ceiling, so this switch has no sliding mode).
            fc[0] = if ip > lvpl { rate.max(0.0) } else { rate };
            let (rq_dn, rq_up) = if self.q_ref_converter >= 0.0 {
                (f64::NEG_INFINITY, k * regc.iqr_max)
            } else {
                (k * regc.iqr_min, f64::INFINITY)
            };
            fc[1] = (iq_cmd - iq).clamp(rq_dn, rq_up);
        }
        fc[2] = vt - vmeas;

        (i_cnv - v / C64::new(self.r_source, self.x_source)) * self.scale
    }

    /// Initialize at the power flow: `s` the plant's output (system base)
    /// at terminal voltage `v`. Sets the references and returns the states
    /// and notes on an operating point outside the plant's ranges where the
    /// plant still rests there.
    ///
    /// # Errors
    /// When a gain the initialization divides by is zero, or when the
    /// operating point has no steady state: a controller output past its
    /// limit (REPCA1 `Qmin`/`Qmax` or `Pmin`/`Pmax`, REECB1 `Pmin`/`Pmax`,
    /// `Qmin`/`Qmax` or `Vmin`/`Vmax`, the current limits) moves the plant
    /// off the power flow at `t = 0`, and the LVPL ceiling, the low-voltage
    /// gain below `Lvpnt1` and high-voltage management above `Volim` make
    /// it inject another current than the power flow's. The error
    /// names each limit and value (or, failing that, the state that moves),
    /// and the caller holds the plant at its power flow output.
    pub(crate) fn initialize(&mut self, s: C64, v: C64) -> Result<(Vec<f64>, Vec<String>), String> {
        let mut notes = Vec::new();
        let vt = v.norm();
        let s_plant = s / self.scale;
        let i_out = (s_plant / v).conj();
        let i_cnv = i_out + v / C64::new(self.r_source, self.x_source);
        let s_cnv = v * i_cnv.conj();
        let (p_elec, q_elec) = (s_cnv.re, s_cnv.im);
        // The converter current in the terminal's frame.
        let rot = i_cnv * C64::from_polar(1.0, -v.im.atan2(v.re));
        let (ip, iq) = (rot.re, -rot.im);
        let (repc, reec, regc) = (&self.repc, &self.reec, &self.regc);
        if vt > regc.vo_lim || vt < regc.lv_pnt1 {
            notes.push(format!(
                "terminal voltage {vt:.4} pu outside the converter's range [{}, {}]",
                regc.lv_pnt1, regc.vo_lim
            ));
        }
        self.p_ref = p_elec;
        self.q_ref = q_elec;
        self.q_ref_converter = q_elec;
        self.omega_ref = 1.0;
        if reec.v_ref0 == 0.0 {
            self.v_ref0 = vt;
        } else {
            self.v_ref0 = reec.v_ref0;
        }
        let ip_oc = p_elec / vt.max(V_LB);
        let mut x = Vec::with_capacity(self.n_states());
        if repc.freq_flag == 1 {
            if repc.k_ig == 0.0 {
                return Err("REPCA1 Kig = 0 with Fflag = 1".into());
            }
            x.extend([p_elec, p_elec / repc.k_ig, p_elec, p_elec]);
        } else {
            x.push(p_elec);
        }
        // Reactive path: the plant controller holds `Q_ext`, its lead-lag
        // at steady state.
        let q_ext = if repc.ref_flag == 0 && reec.v_flag == 0 {
            self.q_ref
        } else {
            q_elec
        };
        let r = if repc.t_fv < f64::EPSILON {
            0.0
        } else {
            repc.t_ft / repc.t_fv
        };
        if repc.k_i == 0.0 {
            return Err("REPCA1 Ki = 0".into());
        }
        let first = if repc.ref_flag == 1 {
            let v_flt_input = if repc.vc_flag == 0 {
                vt + repc.k_c * q_elec
            } else {
                (v - C64::new(repc.r_c, repc.x_c) * i_out).norm()
            };
            self.v_ref = v_flt_input;
            v_flt_input
        } else {
            self.v_ref = vt;
            q_ext
        };
        x.extend([first, q_ext / repc.k_i, (1.0 - r) * q_ext]);
        if reec.v_flag == 1 {
            if reec.k_qi == 0.0 {
                return Err("REECB1 Kqi = 0 with VFLAG = 1".into());
            }
            x.push(vt / reec.k_qi);
        }
        let iq_oc = q_ext / vt.max(V_LB);
        let (ip_max, iq_max) = current_limits(reec, ip_oc, iq);
        let tol = 1e-6;
        if ip_oc > ip_max + tol || ip_oc < -tol {
            notes.push(format!(
                "active current {ip_oc:.4} pu outside [0, {ip_max:.4}]"
            ));
        }
        if iq_oc.abs() > iq_max + tol {
            notes.push(format!(
                "reactive current {iq_oc:.4} pu outside ±{iq_max:.4}"
            ));
        }
        x.push(vt);
        if reec.q_flag == 0 {
            x.push(iq_oc);
        } else {
            if reec.k_vi == 0.0 {
                return Err("REECB1 Kvi = 0 with QFLAG = 1".into());
            }
            x.push(iq / reec.k_vi);
        }
        x.extend([ip, iq, vt]);
        if regc.lvpl_sw && ip > lvpl_gain(vt, regc.zerox, regc.brkpt, regc.lvpl1) + tol {
            notes.push(format!("active current {ip:.4} pu above the LVPL ceiling"));
        }
        // The controllers' limits at the operating point.
        let mut outside = |what: &str, value: f64, lo: f64, hi: f64| {
            if value > hi + tol || value < lo - tol {
                notes.push(format!("{what} {value:.4} pu outside [{lo}, {hi}]"));
            }
        };
        outside("REPCA1 Q PI output", q_ext, repc.q_min, repc.q_max);
        if repc.freq_flag == 1 {
            outside("REPCA1 P PI output", p_elec, repc.p_min, repc.p_max);
            outside("REECB1 Pord", p_elec, reec.p_min_inner, reec.p_max_inner);
        }
        // The voltage command reaches the currents only with `QFLAG = 1`.
        if reec.v_flag == 1 {
            outside(
                "REECB1 Q command",
                q_ext,
                reec.q_min_inner,
                reec.q_max_inner,
            );
            if reec.q_flag == 1 {
                outside(
                    "REECB1 V PI output (terminal voltage)",
                    vt,
                    reec.v_min,
                    reec.v_max,
                );
            }
        } else if repc.ref_flag == 1 && reec.q_flag == 1 {
            outside("REECB1 voltage command", q_ext, reec.v_min, reec.v_max);
        }
        let v_err = deadband(self.v_ref0 - vt, reec.dbd1, reec.dbd2);
        if (reec.k_qv * v_err).abs() > tol {
            notes.push(format!(
                "REECB1 Vref0 = {} pu with the terminal at {vt:.4} pu injects a voltage-dip current at rest",
                self.v_ref0
            ));
        }
        // A plant past a limit that moves it rests elsewhere: no steady
        // state at the power flow, so the caller holds it.
        let mut f = vec![0.0; x.len()];
        self.eval(&x, v.re, v.im, 1.0, &mut f);
        let (worst, row) = f.iter().enumerate().fold((0.0_f64, 0), |(a, k), (i, r)| {
            if r.abs() > a {
                (r.abs(), i)
            } else {
                (a, k)
            }
        });
        // The LVPL ceiling, the low-voltage gain and high-voltage reactive
        // current management act on the injection only, so a plant past
        // them can rest with every rate zero yet inject less than the power
        // flow.
        let i_now = self.output_current(&x, v);
        let off = (i_now - i_out).norm();
        if worst > 1e-8 || off > 1e-8 {
            let why = if !notes.is_empty() {
                notes.join("; ")
            } else if worst > 1e-8 {
                format!(
                    "state {} moves at rate {worst:.3e}",
                    self.state_names()[row]
                )
            } else {
                format!(
                    "the converter injects {:.4} pu, {off:.3e} pu off the power flow current",
                    i_now.norm()
                )
            };
            return Err(format!("its operating point has no steady state ({why})"));
        }
        Ok((x, notes))
    }
}

/// REECB1 current limits `(Ip_max, Iq_max)` (`Ip_min = 0`, `Iq_min =
/// −Iq_max`), Q priority (`PQFLAG = 0`) or P priority.
fn current_limits(reec: &Reecb1, ip_cmd: f64, iq_cmd: f64) -> (f64, f64) {
    let i_max = reec.i_max;
    let rest = |i: f64| (i_max * i_max - i * i).max(0.0).sqrt().min(i_max);
    if reec.pq_flag == 0 {
        (rest(iq_cmd), i_max)
    } else {
        (i_max, rest(ip_cmd))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tok(s: &str) -> Vec<String> {
        s.split_whitespace().map(str::to_owned).collect()
    }

    /// High-voltage reactive current management: above `Volim` = 1.2 the
    /// injected reactive current falls by `Khv (V − Volim)` and stops at
    /// `Iolim` = −1.3; a command already below `Iolim` is left alone.
    #[test]
    fn high_voltage_clamp_absorbs_down_to_iolim() {
        let mut regc = plant().regc;
        regc.k_hv = 10.0;
        // Injected reactive power `Q = V Iq_inj` at a terminal on the real axis.
        let q = |iq: f64, v: f64| {
            let vt = C64::new(v, 0.0);
            (vt * converter_current(&regc, 0.0, iq, v, vt).0.conj()).im / v
        };
        assert!((q(0.2, 1.1) - 0.2).abs() < 1e-12);
        assert!((q(0.2, 1.25) - (0.2 - 0.5)).abs() < 1e-12);
        assert!((q(0.2, 1.4) + 1.3).abs() < 1e-12);
        assert!((q(-1.5, 1.4) + 1.5).abs() < 1e-12);
    }

    const REGCA1: &str = "1 0.02 10.0 0.9 0.4 1.22 1.2 0.9 0.5 -1.3 0.02 0.0 100.0 -100.0 0.7";
    const REECB1: &str = "0 0 1 0 0 -99.0 99.0 0.02 0.0 0.0 0.0 1.1 -1.1 0.0 0.05 0.44 -0.44 1.05 0.9 0.0 0.01 10.0 60.0 0.02 99.0 -99.0 1.0 0.0 1.11 0.02";
    const REPCA1: &str = "0 0 0 '0' 0 0 1 0.02 18.0 5.0 0.0 0.05 0.0 0.0 0.0 0.02 0.1 -0.1 -1.0 1.0 0.43 -0.43 1.0 0.05 0.25 -1.0 1.0 99.0 -99.0 1.0 0.0 0.1 0.0 0.0";

    /// `text` with its 1-based parameter `i` set to `value`.
    fn set(text: &str, i: usize, value: &str) -> Vec<String> {
        let mut t = tok(text);
        t[i - 1] = value.into();
        t
    }

    /// A regulated bus (`IREG`) other than the plant's own is refused, in
    /// REECB1 and REPCA1; 0 and the own bus are local.
    #[test]
    fn a_remote_regulated_bus_is_refused() {
        for ireg in ["0", "1032"] {
            assert!(Reecb1::from_tokens(&set(REECB1, 1, ireg), 1032).is_ok());
            assert!(Repca1::from_tokens(&set(REPCA1, 1, ireg), 1032).is_ok());
        }
        let e = Reecb1::from_tokens(&set(REECB1, 1, "1031"), 1032).unwrap_err();
        assert!(e.contains("REECB1 regulates bus 1031"), "{e}");
        let e = Repca1::from_tokens(&set(REPCA1, 1, "1031"), 1032).unwrap_err();
        assert!(e.contains("REPCA1 regulates bus 1031"), "{e}");
    }

    /// Swapped limits are refused with the parameters named, not passed to
    /// a clamp that panics.
    #[test]
    fn swapped_limits_are_refused() {
        let swap = |text: &str, (i, a): (usize, &str), (j, b): (usize, &str)| {
            let mut t = set(text, i, a);
            t[j - 1] = b.into();
            t
        };
        for (lo, hi, name) in [
            ((13, "0.5"), (12, "-0.5"), "Iql1"),
            ((17, "0.4"), (16, "-0.4"), "Qmin"),
            ((19, "1.1"), (18, "0.9"), "Vmin"),
            ((26, "1.0"), (25, "-1.0"), "dPmin"),
            ((28, "1.0"), (27, "0.0"), "Pmin"),
        ] {
            let e = Reecb1::from_tokens(&swap(REECB1, lo, hi), 1032).unwrap_err();
            assert!(e.contains("swapped") && e.contains(name), "{e}");
        }
        assert!(Reecb1::from_tokens(&set(REECB1, 29, "-1.0"), 1032)
            .unwrap_err()
            .contains("Imax"));
        for (lo, hi, name) in [
            ((18, "0.1"), (17, "-0.1"), "emin"),
            ((22, "0.43"), (21, "-0.43"), "Qmin"),
            ((29, "99.0"), (28, "-99.0"), "femin"),
            ((31, "1.0"), (30, "0.0"), "Pmin"),
        ] {
            let e = Repca1::from_tokens(&swap(REPCA1, lo, hi), 1032).unwrap_err();
            assert!(e.contains("swapped") && e.contains(name), "{e}");
        }
        let e = Regca1::from_tokens(&swap(REGCA1, (4, "0.4"), (5, "0.9"))).unwrap_err();
        assert!(e.contains("Zerox"), "{e}");
        let e = Regca1::from_tokens(&swap(REGCA1, (8, "0.5"), (9, "0.9"))).unwrap_err();
        assert!(e.contains("Lvpnt0"), "{e}");
    }

    /// LVPL caps the active current at once and stops `Ip` rising past the
    /// ceiling with a rate continuous in `Ip` (Lipschitz `Tg/τ`), not
    /// switched to zero, which leaves an implicit step on a rising ceiling
    /// without a solution.
    #[test]
    fn lvpl_caps_the_active_current_continuously() {
        let mut p = plant();
        let v = C64::new(1.0, 0.0);
        let (mut x, _) = p.initialize(C64::new(1.0, 0.1), v).unwrap();
        let k = p.n_states();
        let ip_row = k - 3;
        // Measured voltage 0.55: LVPL = 1.22 (0.55 − 0.4)/0.5 = 0.366, below
        // the command (about 0.5).
        x[k - 1] = 0.55;
        let mut f = vec![0.0; k];
        let rate = |x: &[f64], f: &mut Vec<f64>| {
            p.eval(x, v.re, v.im, 1.0, f);
            f[ip_row]
        };
        let ip0 = 0.366;
        for d in [1e-6, 1e-9] {
            x[ip_row] = ip0 + d;
            let above = rate(&x, &mut f);
            x[ip_row] = ip0 - d;
            let below = rate(&x, &mut f);
            assert_eq!(above, 0.0);
            let lipschitz = p.regc.t_g / LIMIT_TAU;
            assert!(
                (above - below).abs() <= 2.0 * d * lipschitz * (1.0 + 1e-6),
                "{below}"
            );
        }
        // Above the ceiling `Ip` holds, below or above the command (as
        // PowerSimulationsDynamics.jl).
        for ip in [0.45, 0.8] {
            x[ip_row] = ip;
            assert_eq!(rate(&x, &mut f), 0.0);
        }
        let i = p.output_current(&x, v) + v / C64::new(p.r_source, p.x_source);
        assert!((i.re - 0.366).abs() < 1e-12, "{i}");
        p.regc.lvpl_sw = false;
        let i = p.output_current(&x, v) + v / C64::new(p.r_source, p.x_source);
        assert!((i.re - 0.8).abs() < 1e-12, "{i}");
    }

    /// The WECC 240-bus plant at bus 1032.
    fn plant() -> Plant {
        Plant {
            regc: Regca1::from_tokens(&tok(REGCA1)).unwrap(),
            reec: Reecb1::from_tokens(&tok(REECB1), 1032).unwrap(),
            repc: Repca1::from_tokens(&tok(REPCA1), 1032).unwrap(),
            scale: 2.0,
            r_source: 0.0,
            x_source: 1e5,
            p_ref: 0.0,
            q_ref: 0.0,
            v_ref: 0.0,
            v_ref0: 0.0,
            q_ref_converter: 0.0,
            omega_ref: 1.0,
        }
    }

    #[test]
    fn reads_the_psse_parameter_order() {
        let p = plant();
        assert_eq!((p.regc.lv_pnt0, p.regc.lv_pnt1), (0.5, 0.9));
        assert_eq!((p.regc.iqr_min, p.regc.iqr_max), (-100.0, 100.0));
        assert_eq!((p.reec.v_flag, p.reec.q_flag, p.reec.pq_flag), (1, 0, 0));
        assert_eq!((p.reec.iq_l1, p.reec.iq_h1), (-1.1, 1.1));
        assert_eq!((p.reec.k_vp, p.reec.k_vi, p.reec.i_max), (10.0, 60.0, 1.11));
        assert_eq!(
            (p.repc.freq_flag, p.repc.ref_flag, p.repc.k_c),
            (1, 0, 0.02)
        );
        assert_eq!((p.repc.fdbd1, p.repc.fdbd2, p.repc.t_g), (-1.0, 1.0, 0.1));
        assert_eq!(p.n_states(), p.state_names().len());
        assert_eq!(p.n_states(), p.mass().len());
        assert!(Repca1::from_tokens(&tok(
            "0 7 8 '1' 0 0 1 0.02 18.0 5.0 0.0 0.05 0.0 0.0 0.0 0.02 0.1 -0.1 -1.0 1.0 0.43 -0.43 1.0 0.05 0.25 -1.0 1.0 99.0 -99.0 1.0 0.0 0.1 0.0 0.0"
        ), 1032)
        .unwrap_err()
        .contains("remote branch"));
    }

    /// Every flag combination starts at an equilibrium that injects the power
    /// flow's current.
    #[test]
    fn initializes_at_equilibrium_for_every_flag_combination() {
        let v = C64::from_polar(1.02, 0.3);
        let s = C64::new(1.2, 0.25);
        for (freq, ref_flag, v_flag, q_flag, pq) in [
            (1, 0, 1, 0, 0),
            (0, 0, 1, 0, 0),
            (1, 0, 0, 0, 0),
            (1, 1, 1, 0, 0),
            (1, 1, 0, 0, 0),
            (1, 0, 1, 1, 0),
            (1, 0, 1, 0, 1),
        ] {
            let mut p = plant();
            p.repc.freq_flag = freq;
            p.repc.ref_flag = ref_flag;
            p.reec.v_flag = v_flag;
            p.reec.q_flag = q_flag;
            p.reec.pq_flag = pq;
            p.repc.t_ft = 0.02;
            let (x, notes) = p.initialize(s, v).unwrap();
            assert!(notes.is_empty(), "{notes:?}");
            assert_eq!(x.len(), p.n_states());
            let mut f = vec![0.0; x.len()];
            let i = p.eval(&x, v.re, v.im, 1.0, &mut f);
            let worst = f.iter().fold(0.0_f64, |a, b| a.max(b.abs()));
            let flags = (freq, ref_flag, v_flag, q_flag, pq);
            assert!(worst < 1e-12, "{flags:?}: |f| = {worst:e} {f:?}");
            let s_out = v * i.conj();
            assert!((s_out - s).norm() < 1e-12, "{flags:?}: {s_out} vs {s}");
        }
    }

    /// An operating point past a controller limit has no steady state: the
    /// initialization refuses it naming the limit (the caller then holds the
    /// plant), rather than starting the plant off rest.
    #[test]
    fn operating_points_past_controller_limits_are_named() {
        let v = C64::from_polar(1.02, 0.3);
        let cases: [(C64, f64, u8, &str); 3] = [
            // Plant Q = 0.5 above REPCA1 Qmax = 0.43.
            (C64::new(1.2, 1.0), 1.02, 0, "REPCA1 Q PI output"),
            // QFLAG = 1 with the terminal above REECB1 Vmax = 1.05.
            (C64::new(1.2, 0.25), 1.07, 1, "REECB1 V PI output"),
            // Plant P = 1.1 above REPCA1 Pmax = 1.0 (Fflag = 1).
            (C64::new(2.2, 0.1), 1.02, 0, "REPCA1 P PI output"),
        ];
        for (s, vm, q_flag, limit) in cases {
            let mut p = plant();
            p.reec.i_max = 5.0;
            p.reec.q_flag = q_flag;
            let v = C64::from_polar(vm, v.arg());
            let e = p.initialize(s, v).unwrap_err();
            assert!(
                e.contains(limit) && e.contains("no steady state"),
                "{limit}: {e}"
            );
        }
        // With Fflag = 0 (and, at high voltage, RefFlag = 1 with Kc = 0) no
        // rate reads the converter's output, so the plant rests past the
        // LVPL ceiling, the low-voltage gain or high-voltage management with
        // every rate zero, but injects another current than the power flow.
        let lvpl = C64::new(2.6, 0.1);
        let cases: [(C64, f64, u8, &str); 3] = [
            // Ip = 1.368 above LVPL1 = 1.22.
            (lvpl, 0.95, 0, "above the LVPL ceiling"),
            // The terminal below Lvpnt1 = 0.9: the active current gain < 1.
            (C64::new(1.2, 0.1), 0.88, 0, "outside the converter's range"),
            // The terminal above Volim = 1.2 with Khv > 0.
            (C64::new(1.2, 0.1), 1.25, 1, "outside the converter's range"),
        ];
        for (s, vm, ref_flag, limit) in cases {
            let mut p = plant();
            p.reec.i_max = 5.0;
            p.repc.freq_flag = 0;
            p.repc.ref_flag = ref_flag;
            p.repc.k_c = 0.0;
            p.regc.k_hv = 0.7;
            let v = C64::from_polar(vm, v.arg());
            let e = p.initialize(s, v).unwrap_err();
            assert!(
                e.contains(limit) && e.contains("no steady state"),
                "{limit}: {e}"
            );
        }
        // Without the LVPL ceiling the same point rests.
        let mut p = plant();
        p.reec.i_max = 5.0;
        p.repc.freq_flag = 0;
        p.regc.lvpl_sw = false;
        let w = C64::from_polar(0.95, v.arg());
        let (x, _) = p.initialize(lvpl, w).unwrap();
        let i = p.output_current(&x, w) * p.scale;
        assert!((i - (lvpl / w).conj()).norm() < 1e-12, "{i}");

        // Inside every limit it rests, without a note.
        let mut p = plant();
        p.reec.i_max = 5.0;
        let (_, notes) = p.initialize(C64::new(1.2, 0.25), v).unwrap();
        assert!(notes.is_empty(), "{notes:?}");
    }

    /// A voltage dip below `Lvpnt1` scales the active current down (PSS/E
    /// definition, not the negative gain of PowerSimulationsDynamics.jl
    /// 0.16.2), and below `Lvpnt0` it is zero.
    #[test]
    fn low_voltage_gain_is_the_psse_one() {
        assert_eq!(lv_gain(0.95, 0.5, 0.9, false), 1.0);
        assert!((lv_gain(0.7, 0.5, 0.9, false) - 0.5).abs() < 1e-15);
        assert_eq!(lv_gain(0.4, 0.5, 0.9, false), 0.0);
        assert!((lv_gain(0.7, 0.5, 0.9, true) + 0.5).abs() < 1e-15);
        assert!((lvpl_gain(0.65, 0.4, 0.9, 1.22) - 0.61).abs() < 1e-12);
    }
}
