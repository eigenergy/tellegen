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
//! - The lead-lag state starts at its steady state `(1 − T_ft/T_fv) Q`, and
//!   with `Ref_Flag = 1` the voltage reference is the compensated voltage at
//!   the operating point; 0.16.2 starts both at values that are only steady
//!   when `T_ft = 0` and `K_c = 0` (or `R_c = X_c = 0`).
//!
//! The plant measures the system frequency (the reference machine's speed,
//! as `FixedFrequency` does). Voltage-dip freezing (`Vdip`, `Vup`, `Vfrz`) and
//! the plant's remote branch flow are not modelled, as in
//! PowerSimulationsDynamics.jl.

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
        Ok(Self {
            lvpl_sw: flag(v(1), "REGCA1 LVPLSW", &[0, 1])? == 1,
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
        })
    }
}

impl Reecb1 {
    /// From the 30 DYR values (`IREG, PFFLAG, VFLAG, QFLAG, PQFLAG, Vdip,
    /// Vup, Trv, dbd1, dbd2, Kqv, Iqh1, Iql1, Vref0, Tp, Qmax, Qmin, Vmax,
    /// Vmin, Kqp, Kqi, Kvp, Kvi, Tiq, dPmax, dPmin, Pmax, Pmin, Imax, Tpord`).
    pub(crate) fn from_tokens(tokens: &[String]) -> Result<Self, String> {
        let p = numbers("REECB1", tokens, 30)?;
        let v = |i: usize| p[i - 1];
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
    /// Pmin, Tlag, Ddn, Dup`). The regulated bus and the monitored branch
    /// must be the plant's own (0): a remote branch flow is not modelled.
    pub(crate) fn from_tokens(tokens: &[String]) -> Result<Self, String> {
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

/// `pi_block_nonwindup`: the clamped output and the integrator's rate
/// (stopped while the unclamped output is at or beyond a limit).
fn pi_nonwindup(u: f64, x: f64, kp: f64, ki: f64, lo: f64, hi: f64) -> (f64, f64) {
    let y = kp * u + ki * x;
    let rate = if lo < y && y < hi { u } else { 0.0 };
    (y.clamp(lo, hi), rate)
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

/// Converter currents from its states: `(I_cnv, Ip_sat, Ip_binary)` with
/// `I_cnv` in the network frame, plant base.
fn converter_current(regc: &Regca1, ip: f64, iq: f64, vmeas: f64, v: C64) -> (C64, f64, f64) {
    let vt = v.norm();
    let (mut ip_sat, mut binary) = (ip, 1.0);
    if regc.lvpl_sw {
        let lvpl = lvpl_gain(vmeas, regc.zerox, regc.brkpt, regc.lvpl1);
        if ip > lvpl {
            ip_sat = lvpl;
            binary = 0.0;
        }
    }
    let g_lv = lv_gain(vt, regc.lv_pnt0, regc.lv_pnt1, regc.psid_lv_gain);
    let iq_extra = (regc.k_hv * (vt - regc.vo_lim)).max(0.0);
    let id_cnv = g_lv * ip_sat;
    let iq_cnv = -iq - iq_extra;
    let (s, c) = v.im.atan2(v.re).sin_cos();
    (
        C64::new(id_cnv * c - iq_cnv * s, id_cnv * s + iq_cnv * c),
        ip_sat,
        binary,
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
        let (i_cnv, _, _) = converter_current(&self.regc, x[k - 3], x[k - 2], x[k - 1], v);
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
        let (i_cnv, ip_sat, ip_binary) = converter_current(regc, ip, iq, vmeas, v);
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
            let t = reec.t_pord;
            let rate = (p_ext - p_ord).clamp(t * reec.dp_min, t * reec.dp_max);
            let stop = (p_ord >= reec.p_max_inner && rate > 0.0)
                || (p_ord <= reec.p_min_inner && rate < 0.0);
            fa[3] = if stop { 0.0 } else { rate };
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

        // Converter.
        let (rp_dn, rp_up) = if ip >= 0.0 {
            (f64::NEG_INFINITY, regc.rrpwr)
        } else {
            (-regc.rrpwr, f64::INFINITY)
        };
        fc[0] = ip_binary * (ip_cmd - ip_sat).clamp(rp_dn, rp_up);
        let (rq_dn, rq_up) = if self.q_ref_converter >= 0.0 {
            (f64::NEG_INFINITY, regc.iqr_max)
        } else {
            (regc.iqr_min, f64::INFINITY)
        };
        fc[1] = (iq_cmd - iq).clamp(rq_dn, rq_up);
        fc[2] = vt - vmeas;

        (i_cnv - v / C64::new(self.r_source, self.x_source)) * self.scale
    }

    /// Initialize at the power flow: `s` the plant's output (system base)
    /// at terminal voltage `v`. Sets the references and returns the states
    /// and notes on an operating point outside the plant's ranges (which
    /// PowerSimulationsDynamics.jl reports and simulates as well).
    ///
    /// # Errors
    /// When a gain the initialization divides by is zero.
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

    /// The WECC 240-bus plant at bus 1032.
    fn plant() -> Plant {
        Plant {
            regc: Regca1::from_tokens(&tok(
                "1 0.02 10.0 0.9 0.4 1.22 1.2 0.9 0.5 -1.3 0.02 0.0 100.0 -100.0 0.7",
            ))
            .unwrap(),
            reec: Reecb1::from_tokens(&tok(
                "0 0 1 0 0 -99.0 99.0 0.02 0.0 0.0 0.0 1.1 -1.1 0.0 0.05 0.44 -0.44 1.05 0.9 0.0 0.01 10.0 60.0 0.02 99.0 -99.0 1.0 0.0 1.11 0.02",
            ))
            .unwrap(),
            repc: Repca1::from_tokens(&tok(
                "0 0 0 '0' 0 0 1 0.02 18.0 5.0 0.0 0.05 0.0 0.0 0.0 0.02 0.1 -0.1 -1.0 1.0 0.43 -0.43 1.0 0.05 0.25 -1.0 1.0 99.0 -99.0 1.0 0.0 0.1 0.0 0.0",
            ))
            .unwrap(),
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
        ))
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
