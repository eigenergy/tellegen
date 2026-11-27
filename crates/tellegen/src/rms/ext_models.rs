//! Controller models beyond PowerIO's typed set, read from their DYR records
//! and run as block graphs (the blocks of `blocks.rs`, the same evaluation
//! and initialization as every graph controller):
//!
//! | model | role | records |
//! |---|---|---|
//! | `ESST1A` | exciter | IEEE 421.5 ST1A static exciter, PSS/E ICONs UEL VOS + 18 CONs |
//! | `ESAC6A` | exciter | IEEE 421.5 AC6A brushless AC exciter, 23 CONs |
//! | `PSS2B` | stabilizer | IEEE 421.5 PSS2B dual-input stabilizer, ICONs J1 K1 J2 K2 M N + 23 CONs |
//! | `OELIT` | over-excitation limiter | tellegen: inverse-time field current limiter, summing point, 9 CONs |
//! | `UELPQ` | under-excitation limiter | tellegen: P–Q straight-line limiter, summing point, 6 CONs |
//!
//! ESST1A and PSS2B follow PowerSimulationsDynamics.jl 0.16.2
//! (`avr_models.jl`, `pss_models.jl`) block for block, ESAC6A the IEEE
//! 421.5-2005 AC6A diagram with the AC exciter of PowerSimulationsDynamics.jl's
//! ESAC1A. The limiters enter the voltage error at the summing point with the
//! stabilizer signal, for any exciter (a compiled exciter is replaced by its
//! template graph). Their outputs are zero at rest by construction (a dead
//! zone ahead of a lag), so a unit starts with its limiters inactive.
//!
//! Experimental.
//!
//! @block Extension controller models

use super::blocks::quadratic_saturation;
use super::graph::{Block, BlockGraph, GraphNode, Slot};

/// Name, role, number of parameters (ICONs first, as in the DYR record).
pub(crate) const EXT_MODELS: &[(&str, &str, usize)] = &[
    ("ESST1A", "exciter", 20),
    ("ESAC6A", "exciter", 23),
    ("PSS2B", "stabilizer", 29),
    ("OELIT", "oel", 9),
    ("UELPQ", "uel", 6),
];

/// The role of an extension model, if `name` is one.
pub(crate) fn ext_role(name: &str) -> Option<&'static str> {
    let n = name.trim().to_ascii_uppercase();
    EXT_MODELS.iter().find(|(m, _, _)| *m == n).map(|(_, r, _)| *r)
}

/// The records of one unit that name extension models: `(model, numbers)`
/// per role, the last record of a role winning (as PSS/E reads DYR).
#[derive(Clone, Debug, Default)]
pub(crate) struct ExtRecords {
    pub exciter: Option<(String, Vec<f64>)>,
    pub stabilizer: Option<(String, Vec<f64>)>,
    pub oel: Option<Vec<f64>>,
    pub uel: Option<Vec<f64>>,
}

impl ExtRecords {
    pub(crate) fn is_empty(&self) -> bool {
        self.exciter.is_none() && self.stabilizer.is_none() && self.oel.is_none() && self.uel.is_none()
    }
}

/// The numbers of a record of `model` (at least the model's count).
pub(crate) fn numbers(model: &str, tokens: &[String]) -> Result<Vec<f64>, String> {
    let n = EXT_MODELS
        .iter()
        .find(|(m, _, _)| *m == model)
        .map_or(0, |(_, _, n)| *n);
    let v: Vec<f64> = tokens
        .iter()
        .take(n)
        .map(|t| {
            t.trim()
                .trim_matches('\'')
                .parse::<f64>()
                .map_err(|_| format!("{model}: parameter '{t}' is not a number"))
        })
        .collect::<Result<_, _>>()?;
    if v.len() < n {
        return Err(format!("{model}: {} parameters, {n} expected", v.len()));
    }
    Ok(v)
}

/// Every extension record of the unit `(bus, id)` in `dyr`.
pub(crate) fn unit_records(
    dyr: &powerio::DynamicCase,
    bus: usize,
    id: &str,
) -> Result<ExtRecords, String> {
    let mut out = ExtRecords::default();
    for r in dyr.records() {
        if r.bus_id().map(|b| b as usize) != Some(bus)
            || !r.device_id().unwrap_or("1").trim().eq_ignore_ascii_case(id.trim())
        {
            continue;
        }
        let model = r.model_name().trim().to_ascii_uppercase();
        let Some(role) = ext_role(&model) else {
            continue;
        };
        let p = numbers(&model, r.parameters())
            .map_err(|e| format!("generator {bus}-{id}: {e}"))?;
        match role {
            "exciter" => out.exciter = Some((model, p)),
            "stabilizer" => out.stabilizer = Some((model, p)),
            "oel" => out.oel = Some(p),
            "uel" => out.uel = Some(p),
            _ => {}
        }
    }
    Ok(out)
}

fn nd(id: &str, block: Block, inputs: &[&str]) -> GraphNode {
    GraphNode {
        id: id.into(),
        block,
        inputs: inputs.iter().map(|s| (*s).to_string()).collect(),
        states: Vec::new(),
        start: None,
        pos: None,
    }
}

fn input(id: &str, port: &str) -> GraphNode {
    nd(id, Block::Input { port: port.into() }, &[])
}

fn sum(id: &str, signs: &[f64], inputs: &[&str]) -> GraphNode {
    nd(
        id,
        Block::Sum {
            signs: signs.to_vec(),
        },
        inputs,
    )
}

fn konst(id: &str, value: f64) -> GraphNode {
    nd(id, Block::Const { value }, &[])
}

fn gain(id: &str, k: f64, from: &str) -> GraphNode {
    nd(id, Block::Gain { k }, &[from])
}

/// The exciter graph of an ESST1A record (PowerSimulationsDynamics.jl 0.16.2):
/// `VI = clamp(V_ref [+ Vs] − Vm − VF, VIMIN, VIMAX)`, two lead-lags, the
/// non-windup lag `KA/(1+sTA)` whose state is `VA`, the field current limiter
/// `max(0, KLR (Ifd − ILR))` subtracted from it, the rate feedback
/// `KF s/(1+sTF)` of that sum, and `Efd = clamp(VA − ILIM [+ Vs], Vt VRMIN,
/// Vt VRMAX − KC Ifd)`. VOS = 1 adds the stabilizer signal at the voltage
/// error, 2 after the regulator. The UEL code is read and not used (as in
/// PowerSimulationsDynamics.jl); a UELPQ record enters at the summing point.
pub(crate) fn esst1a(p: &[f64]) -> Result<BlockGraph, String> {
    let vos = p[1].round() as i64;
    if vos != 1 && vos != 2 {
        return Err(format!("ESST1A: VOS {} must be 1 or 2", p[1]));
    }
    let c = &p[2..];
    let [tr, vimax, vimin, tc, tb, tc1, tb1, ka, ta, vamax, vamin, vrmax, vrmin, kc, kf, tf, klr, ilr] =
        <[f64; 18]>::try_from(&c[..18]).expect("18 constants");
    let mut nodes = vec![
        input("vt", "vt"),
        input("vs", "vs"),
        input("ifd", "xad_ifd"),
        input("v_ref", "v_ref"),
        nd("Vm", Block::Lag { k: 1.0, t: tr }, &["vt"]),
    ];
    if vos == 1 {
        nodes.push(sum("error", &[1.0, 1.0, -1.0, -1.0], &["v_ref", "vs", "Vm", "VF"]));
    } else {
        nodes.push(sum("error", &[1.0, -1.0, -1.0], &["v_ref", "Vm", "VF"]));
    }
    nodes.extend([
        nd("VI", Block::Clamp { min: vimin, max: vimax }, &["error"]),
        nd("Vr1", Block::LeadLag { k: 1.0, t1: tc, t2: tb }, &["VI"]),
        nd("Vr2", Block::LeadLag { k: 1.0, t1: tc1, t2: tb1 }, &["Vr1"]),
        nd(
            "Va",
            Block::LagLimited {
                k: ka,
                t: ta,
                min: vamin,
                max: vamax,
                clamp_output: false,
            },
            &["Vr2"],
        ),
        konst("ilr", ilr),
        sum("ifd_over", &[1.0, -1.0], &["ifd", "ilr"]),
        gain("klr_over", klr, "ifd_over"),
        konst("zero", 0.0),
        nd("ILIM", Block::Max, &["klr_over", "zero"]),
    ]);
    if vos == 1 {
        nodes.push(sum("Va_sum", &[1.0, -1.0], &["Va", "ILIM"]));
    } else {
        nodes.push(sum("Va_sum", &[1.0, -1.0, 1.0], &["Va", "ILIM", "vs"]));
    }
    nodes.extend([
        nd("VF", Block::Washout { k: kf, t: tf }, &["Va_sum"]),
        gain("lo", vrmin, "vt"),
        gain("vt_max", vrmax, "vt"),
        gain("kc_ifd", kc, "ifd"),
        sum("hi", &[1.0, -1.0], &["vt_max", "kc_ifd"]),
        nd("Efd", Block::ClampVar, &["Va_sum", "lo", "hi"]),
    ]);
    Ok(BlockGraph {
        slot: Slot::Exciter,
        name: "ESST1A".into(),
        nodes,
        output: "Efd".into(),
    })
}

/// The exciter graph of an ESAC6A record (IEEE 421.5-2005 AC6A):
/// `VA = clamp(KA (1+sTK)/(1+sTA) (V_ref + Vs − Vm), VAMIN, VAMAX)`, the
/// lead-lag `(1+sTC)/(1+sTB)`, the exciter field current limiter
/// `VH = clamp(KH (1+sTJ)/(1+sTH) (VFE − VFELIM), 0, VHMAX)` subtracted,
/// `VR = clamp(·, Vt VRMIN, Vt VRMAX)`, and the AC exciter of ESAC1A:
/// `TE dVE/dt = VR − VFE`, `VFE = VE (KE + SE(VE)) + KD Ifd`,
/// `Efd = VE FEX(KC Ifd / VE)`. The VA limit clamps the lead-lag output
/// (no non-windup on a lead-lag).
pub(crate) fn esac6a(p: &[f64]) -> Result<BlockGraph, String> {
    let [tr, ka, ta, tk, tb, tc, vamax, vamin, vrmax, vrmin, te, vfelim, kh, vhmax, th, tj, kc, kd, ke, e1, se1, e2, se2] =
        <[f64; 23]>::try_from(&p[..23]).expect("23 constants");
    if te <= 0.0 {
        return Err("ESAC6A: TE must be positive".into());
    }
    let (sat_a, sat_b) = quadratic_saturation((e1, e2), (se1, se2))?;
    let nodes = vec![
        input("vt", "vt"),
        input("vs", "vs"),
        input("ifd", "xad_ifd"),
        input("v_ref", "v_ref"),
        nd("Vm", Block::Lag { k: 1.0, t: tr }, &["vt"]),
        sum("error", &[1.0, 1.0, -1.0], &["v_ref", "vs", "Vm"]),
        nd("Va_ll", Block::LeadLag { k: ka, t1: tk, t2: ta }, &["error"]),
        nd("Va", Block::Clamp { min: vamin, max: vamax }, &["Va_ll"]),
        nd("Vr_ll", Block::LeadLag { k: 1.0, t1: tc, t2: tb }, &["Va"]),
        konst("vfelim", vfelim),
        sum("vfe_over", &[1.0, -1.0], &["VFE", "vfelim"]),
        nd("VH_ll", Block::LeadLag { k: kh, t1: tj, t2: th }, &["vfe_over"]),
        nd("VH", Block::Clamp { min: 0.0, max: vhmax }, &["VH_ll"]),
        sum("Vr_in", &[1.0, -1.0], &["Vr_ll", "VH"]),
        gain("lo", vrmin, "vt"),
        gain("hi", vrmax, "vt"),
        nd("VR", Block::ClampVar, &["Vr_in", "lo", "hi"]),
        sum("ve_in", &[1.0, -1.0], &["VR", "VFE"]),
        nd("Ve", Block::Integrator { k: 1.0 / te }, &["ve_in"]),
        gain("kd_ifd", kd, "ifd"),
        gain("ke_ve", ke, "Ve"),
        nd("Se", Block::SatQuadratic { a: sat_a, b: sat_b }, &["Ve"]),
        nd("se_ve", Block::Product, &["Se", "Ve"]),
        sum("VFE", &[1.0, 1.0, 1.0], &["kd_ifd", "ke_ve", "se_ve"]),
        gain("kc_ifd", kc, "ifd"),
        konst("ve_min", super::models::VE_MIN),
        nd("ve_div", Block::Max, &["Ve", "ve_min"]),
        nd("IN", Block::Divide, &["kc_ifd", "ve_div"]),
        nd("FEX", Block::Rectifier, &["IN"]),
        nd("Efd", Block::Product, &["Ve", "FEX"]),
    ];
    Ok(BlockGraph {
        slot: Slot::Exciter,
        name: "ESAC6A".into(),
        nodes,
        output: "Efd".into(),
    })
}

/// The stabilizer graph of a PSS2B record (PowerSimulationsDynamics.jl
/// 0.16.2): input 1 speed deviation (code 1), input 2 electrical power on the
/// machine base times speed (code 3), each clamped, washouts TW1/TW2 and
/// TW3/TW4 (a zero TW2 or TW4 bypasses), lags T6 and KS2/(1+sT7), the
/// ramp-tracking filter `[(1+sT8)/(1+sT9)^M]^N`, `KS1 (y − y7)`, three
/// lead-lags and the output clamp.
pub(crate) fn pss2b(p: &[f64]) -> Result<BlockGraph, String> {
    let icon: Vec<i64> = p[..6].iter().map(|v| v.round() as i64).collect();
    let (j1, k1, j2, k2, m, n) = (icon[0], icon[1], icon[2], icon[3], icon[4], icon[5]);
    if (j1, j2) != (1, 3) || k1 != 0 || k2 != 0 {
        return Err("PSS2B: only input 1 = speed (code 1) and input 2 = electrical power (code 3), local bus, are supported".into());
    }
    if !(0..=5).contains(&m) || !(0..=4).contains(&n) {
        return Err("PSS2B: M must be 0..5 and N 0..4".into());
    }
    let c = &p[6..];
    let [tw1, tw2, t6, tw3, tw4, t7, ks2, ks3, t8, t9, ks1, t1, t2, t3, t4, t10, t11, vs1max, vs1min, vs2max, vs2min, vstmax, vstmin] =
        <[f64; 23]>::try_from(&c[..23]).expect("23 constants");
    if tw1 <= 0.0 || tw3 <= 0.0 || t7 <= 0.0 {
        return Err("PSS2B: TW1, TW3 and T7 must be positive".into());
    }
    let mut nodes = vec![
        input("omega", "omega"),
        input("pe_m", "pe_m"),
        konst("one", 1.0),
        sum("u1_raw", &[1.0, -1.0], &["omega", "one"]),
        nd("u1", Block::Clamp { min: vs1min, max: vs1max }, &["u1_raw"]),
        nd("u2_raw", Block::Product, &["pe_m", "omega"]),
        nd("u2", Block::Clamp { min: vs2min, max: vs2max }, &["u2_raw"]),
        nd("w1", Block::Washout { k: tw1, t: tw1 }, &["u1"]),
    ];
    let mut a = "w1".to_string();
    if tw2 != 0.0 {
        nodes.push(nd("w2", Block::Washout { k: tw2, t: tw2 }, &[&a]));
        a = "w2".into();
    }
    if t6 != 0.0 {
        nodes.push(nd("l6", Block::Lag { k: 1.0, t: t6 }, &[&a]));
        a = "l6".into();
    }
    nodes.push(nd("w3", Block::Washout { k: tw3, t: tw3 }, &["u2"]));
    let mut b = "w3".to_string();
    if tw4 != 0.0 {
        nodes.push(nd("w4", Block::Washout { k: tw4, t: tw4 }, &[&b]));
        b = "w4".into();
    }
    nodes.push(nd("l7", Block::Lag { k: ks2, t: t7 }, &[&b]));
    nodes.push(sum("rtf_in", &[1.0, ks3], &[&a, "l7"]));
    let mut r = "rtf_in".to_string();
    if m > 0 {
        for i in 0..n {
            let id = format!("rtf{i}_ll");
            nodes.push(nd(&id, Block::LeadLag { k: 1.0, t1: t8, t2: t9 }, &[&r]));
            r = id;
            for j in 1..m {
                let id = format!("rtf{i}_lag{j}");
                nodes.push(nd(&id, Block::Lag { k: 1.0, t: t9 }, &[&r]));
                r = id;
            }
        }
    }
    nodes.extend([
        sum("x", &[1.0, -1.0], &[&r, "l7"]),
        gain("ks1", ks1, "x"),
        nd("ll1", Block::LeadLag { k: 1.0, t1, t2 }, &["ks1"]),
        nd("ll2", Block::LeadLag { k: 1.0, t1: t3, t2: t4 }, &["ll1"]),
        nd("ll3", Block::LeadLag { k: 1.0, t1: t10, t2: t11 }, &["ll2"]),
        nd("Vst", Block::Clamp { min: vstmin, max: vstmax }, &["ll3"]),
    ]);
    Ok(BlockGraph {
        slot: Slot::Stabilizer,
        name: "PSS2B".into(),
        nodes,
        output: "Vst".into(),
    })
}

/// Nodes of an OELIT record, output `VOEL` (≤ 0), on the exciter ports.
/// CONs: IFDRAT (rated field current, pu of the air-gap line), IPICK
/// (heating pickup / IFDRAT), ICONT (continuous limit / IFDRAT), ILIM
/// (instantaneous limit / IFDRAT), C (inverse-time constant, s: a field
/// current I above pickup reaches the continuous limit after
/// `C / ((I/Ipick)² − 1)` s, IEEE C50.13 gives C ≈ 33.75 s), TCOOL (s),
/// KP (pu voltage per pu field current), KI (per s), VOELMIN (< 0).
///
/// `heat = max(0, (Ifd/Ipick)² − 1)`, `acc` with `dacc/dt = heat − acc/TCOOL`
/// (zero at rest), the limit `ILIM` until `acc` reaches 95 % of C and
/// `ICONT` from C on (linear in between). With `e = Ifd − limit`:
/// `VOEL = −clamp(KP max(0, e) + x, 0, −VOELMIN)` and the dead-zone
/// integrator ([`dead_zone_pi`]).
fn oel_nodes(p: &[f64]) -> Result<Vec<GraphNode>, String> {
    let [ifdrat, ipick, icont, ilim, c, tcool, kp, ki, voelmin] =
        <[f64; 9]>::try_from(&p[..9]).expect("9 constants");
    if ifdrat <= 0.0 || ipick <= 0.0 || c <= 0.0 || tcool <= 0.0 || voelmin > 0.0 {
        return Err("OELIT: IFDRAT, IPICK, C, TCOOL must be positive and VOELMIN ≤ 0".into());
    }
    Ok(vec![
        gain("oel_r", 1.0 / (ipick * ifdrat), "ifd"),
        nd("oel_r2", Block::Product, &["oel_r", "oel_r"]),
        konst("oel_one", 1.0),
        sum("oel_h", &[1.0, -1.0], &["oel_r2", "oel_one"]),
        konst("oel_hzero", 0.0),
        nd("oel_heat", Block::Max, &["oel_h", "oel_hzero"]),
        nd("oel_acc", Block::Lag { k: tcool, t: tcool }, &["oel_heat"]),
        konst("oel_c95", 0.95 * c),
        sum("oel_ad", &[1.0, -1.0], &["oel_acc", "oel_c95"]),
        gain("oel_as", 1.0 / (0.05 * c), "oel_ad"),
        nd("oel_s", Block::Clamp { min: 0.0, max: 1.0 }, &["oel_as"]),
        konst("oel_ilim", ilim * ifdrat),
        gain("oel_drop", (ilim - icont) * ifdrat, "oel_s"),
        sum("oel_limit", &[1.0, -1.0], &["oel_ilim", "oel_drop"]),
        sum("oel_e", &[1.0, -1.0], &["ifd", "oel_limit"]),
    ]
    .into_iter()
    .chain(dead_zone_pi("oel", "oel_e", kp, ki, -voelmin))
    .chain([gain("VOEL", -1.0, "oel_pi")])
    .collect())
}

/// Reset time of the limiters' dead-zone integrators, s.
const LIMITER_RESET: f64 = 1.0;

/// A PI on the violation `e` (> 0 when the limit is exceeded) that rests at
/// zero while the limit holds: output `clamp(KP max(0, e) + x, 0, xmax)`
/// with `dx/dt = KI min(max(e, −x/(KI TR)), (xmax − x)/(KI TR))`, TR the
/// reset time. It integrates the violation to zero (the limit is held
/// exactly), winds back with time constant TR once the limit is met, and
/// stays within `[0, xmax]` by itself; at rest (`e < 0`) the only steady
/// state is `x = 0`, so a unit starts with the limiter inactive.
fn dead_zone_pi(pre: &str, e: &str, kp: f64, ki: f64, xmax: f64) -> Vec<GraphNode> {
    let id = |s: &str| format!("{pre}_{s}");
    let c = 1.0 / (ki.max(1e-9) * LIMITER_RESET);
    let mut x = nd(&id("x"), Block::Integrator { k: ki }, &[&id("in")]);
    x.start = Some([0.0, 0.0]);
    vec![
        konst(&id("zero"), 0.0),
        gain(&id("unwind"), -c, &id("x")),
        konst(&id("cap"), c * xmax),
        sum(&id("room"), &[1.0, -c], &[&id("cap"), &id("x")]),
        nd(&id("lo"), Block::Max, &[e, &id("unwind")]),
        nd(&id("in"), Block::Min { tie: 0.0 }, &[&id("lo"), &id("room")]),
        x,
        nd(&id("over"), Block::Max, &[e, &id("zero")]),
        gain(&id("p"), kp, &id("over")),
        sum(&id("sum"), &[1.0, 1.0], &[&id("p"), &id("x")]),
        nd(&id("pi"), Block::Clamp { min: 0.0, max: xmax }, &[&id("sum")]),
    ]
}

/// Nodes of a UELPQ record, output `VUEL` (≥ 0). CONs: Q0 (reactive power
/// limit at P = 0, pu of MBASE, < 0), KQP (its slope with P), KV (voltage
/// exponent 0, 1 or 2), KP (pu voltage per pu Q), KI (per s), VUELMAX. The
/// limit is `Qlim = (Q0 + KQP P) Vt^KV`; `VUEL` is the dead-zone PI
/// ([`dead_zone_pi`]) of `Qlim − Q`, within `[0, VUELMAX]`.
fn uel_nodes(p: &[f64]) -> Result<Vec<GraphNode>, String> {
    let [q0, kqp, kv, kp, ki, vuelmax] = <[f64; 6]>::try_from(&p[..6]).expect("6 constants");
    let kv = kv.round() as i64;
    if !(0..=2).contains(&kv) || vuelmax < 0.0 {
        return Err("UELPQ: KV must be 0, 1 or 2 and VUELMAX ≥ 0".into());
    }
    let mut nodes = vec![
        input("uel_pin", "p"),
        input("uel_qin", "q"),
        konst("uel_q0", q0),
        gain("uel_kp", kqp, "uel_pin"),
        sum("uel_line", &[1.0, 1.0], &["uel_q0", "uel_kp"]),
    ];
    let lim = match kv {
        0 => "uel_line",
        1 => {
            nodes.push(nd("uel_qlim", Block::Product, &["uel_line", "vt"]));
            "uel_qlim"
        }
        _ => {
            nodes.push(nd("uel_qlim", Block::Product, &["uel_line", "vt", "vt"]));
            "uel_qlim"
        }
    };
    nodes.extend([
        sum("uel_e", &[1.0, -1.0], &[lim, "uel_qin"]),
    ]);
    nodes.extend(dead_zone_pi("uel", "uel_e", kp, ki, vuelmax));
    nodes.push(gain("VUEL", 1.0, "uel_pi"));
    Ok(nodes)
}

/// `graph` (an exciter) with the limiters of `oel` / `uel` at its summing
/// point: its `vs` input node becomes `vs_pss`, and `vs` is the sum of the
/// stabilizer signal and the limiter outputs.
// @block Limiters at the summing point
pub(crate) fn with_limiters(
    mut graph: BlockGraph,
    oel: Option<&[f64]>,
    uel: Option<&[f64]>,
) -> Result<BlockGraph, String> {
    if oel.is_none() && uel.is_none() {
        return Ok(graph);
    }
    let has = |g: &BlockGraph, id: &str| g.nodes.iter().any(|n| n.id == id);
    match graph.nodes.iter_mut().find(|n| n.id == "vs") {
        Some(n) if matches!(&n.block, Block::Input { port } if port == "vs") => n.id = "vs_pss".into(),
        Some(_) => return Err("exciter graph: node 'vs' is not the stabilizer input".into()),
        None => graph.nodes.push(input("vs_pss", "vs")),
    }
    for (id, port) in [("vt", "vt"), ("ifd", "xad_ifd")] {
        if !has(&graph, id) {
            graph.nodes.push(input(id, port));
        }
    }
    let mut terms = vec!["vs_pss".to_string()];
    if let Some(p) = oel {
        graph.nodes.extend(oel_nodes(p)?);
        terms.push("VOEL".into());
    }
    if let Some(p) = uel {
        graph.nodes.extend(uel_nodes(p)?);
        terms.push("VUEL".into());
    }
    let refs: Vec<&str> = terms.iter().map(String::as_str).collect();
    graph.nodes.push(sum("vs", &vec![1.0; refs.len()], &refs));
    graph.name = format!(
        "{}{}{}",
        graph.name,
        if oel.is_some() { "+OELIT" } else { "" },
        if uel.is_some() { "+UELPQ" } else { "" }
    );
    Ok(graph)
}

/// The exciter graph of a unit's extension records: its ESST1A / ESAC6A, or
/// the template of its compiled exciter `compiled` when only limiters are
/// given, with the limiters at the summing point. `None` when the unit has
/// neither an extension exciter nor a limiter.
// @block Exciter of a unit's records
pub(crate) fn exciter_graph(
    ext: &ExtRecords,
    compiled: Option<&powerio::DynamicModel>,
) -> Result<Option<BlockGraph>, String> {
    let base = match &ext.exciter {
        Some((m, p)) if m == "ESST1A" => esst1a(p)?,
        Some((m, p)) if m == "ESAC6A" => esac6a(p)?,
        Some((m, _)) => return Err(format!("{m} is not an exciter")),
        None => {
            if ext.oel.is_none() && ext.uel.is_none() {
                return Ok(None);
            }
            let Some(model) = compiled else {
                return Err("an OELIT or UELPQ record needs an exciter".into());
            };
            super::graph::template(model)
                .ok_or_else(|| format!("{} has no block template for the limiters", model.name()))?
        }
    };
    with_limiters(base, ext.oel.as_deref(), ext.uel.as_deref()).map(Some)
}

/// The stabilizer graph of a unit's extension records, if any.
// @block Stabilizer of a unit's records
pub(crate) fn stabilizer_graph(ext: &ExtRecords) -> Result<Option<BlockGraph>, String> {
    match &ext.stabilizer {
        Some((m, p)) if m == "PSS2B" => pss2b(p).map(Some),
        Some((m, _)) => Err(format!("{m} is not a stabilizer")),
        None => Ok(None),
    }
}

/// The block graph of one extension record (for the editor's template).
pub(crate) fn record_template(model: &str, p: &[f64]) -> Result<BlockGraph, String> {
    match model {
        "ESST1A" => esst1a(p),
        "ESAC6A" => esac6a(p),
        "PSS2B" => pss2b(p),
        "OELIT" | "UELPQ" => {
            // A limiter alone: on a unity exciter (Efd = V_ref + Vs − Vt) so
            // the editor shows its nodes in context.
            let unity = BlockGraph {
                slot: Slot::Exciter,
                name: "unity".into(),
                nodes: vec![
                    input("vt", "vt"),
                    input("vs", "vs"),
                    input("v_ref", "v_ref"),
                    sum("Efd", &[1.0, 1.0, -1.0], &["v_ref", "vs", "vt"]),
                ],
                output: "Efd".into(),
            };
            if model == "OELIT" {
                with_limiters(unity, Some(p), None)
            } else {
                with_limiters(unity, None, Some(p))
            }
        }
        _ => Err(format!("{model} is not an extension model")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rms::graph::CompiledGraph;

    // The sets of the model library (`library.rs`).
    const ESST1A: [f64; 20] = [
        1.0, 1.0, 0.0, 99.0, -99.0, 2.5, 13.25, 0.0, 0.0, 200.0, 0.1, 9.5, -9.5, 9.5, -9.5, 0.0,
        0.0, 1.0, 0.0, 999.0,
    ];
    // 3.14 is the first saturation point E1 of the exciter, not π.
    #[allow(clippy::approx_constant)]
    const ESAC6A: [f64; 23] = [
        0.001, 200.0, 0.1, 0.0, 13.25, 2.5, 9.5, -9.5, 7.32, -7.32, 0.8, 99.0, 0.0, 99.0, 0.0, 0.0,
        0.2, 0.38, 1.0, 3.14, 0.03, 4.18, 0.1,
    ];
    const PSS2B: [f64; 29] = [
        1.0, 0.0, 3.0, 0.0, 5.0, 1.0, 1.5, 1.5, 0.0, 1.5, 0.0, 1.5, 0.1875, 1.0, 0.5, 0.1, 2.0,
        0.59451, 0.0447, 0.59451, 0.0447, 1.0, 1.0, 99.0, -99.0, 99.0, -99.0, 0.1, -0.1,
    ];
    const OEL: [f64; 9] = [2.4, 1.0, 1.0, 2.09, 33.75, 1200.0, 0.05, 0.5, -0.5];
    const UEL: [f64; 6] = [-0.5, 0.1, 2.0, 0.05, 0.5, 0.5];

    #[test]
    fn graphs_compile() {
        for g in [
            esst1a(&ESST1A).unwrap(),
            esac6a(&ESAC6A).unwrap(),
            pss2b(&PSS2B).unwrap(),
            with_limiters(esst1a(&ESST1A).unwrap(), Some(&OEL), Some(&UEL)).unwrap(),
            with_limiters(esac6a(&ESAC6A).unwrap(), Some(&OEL), Some(&UEL)).unwrap(),
            record_template("OELIT", &OEL).unwrap(),
            record_template("UELPQ", &UEL).unwrap(),
        ] {
            CompiledGraph::new(&g).unwrap_or_else(|e| panic!("{}: {e}", g.name));
        }
    }

    #[test]
    fn rtf_has_m_n_blocks() {
        let g = pss2b(&PSS2B).unwrap();
        assert_eq!(g.nodes.iter().filter(|n| n.id.starts_with("rtf0")).count(), 5);
    }

    #[test]
    fn roles() {
        assert_eq!(ext_role("esst1a"), Some("exciter"));
        assert_eq!(ext_role("PSS2B"), Some("stabilizer"));
        assert_eq!(ext_role("OELIT"), Some("oel"));
        assert_eq!(ext_role("SEXS"), None);
    }
}
