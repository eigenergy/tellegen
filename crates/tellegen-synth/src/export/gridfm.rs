//! GridFM graphkit JSON: the network as per-unit feature matrices on a
//! contiguous 0-based bus index, the input layout of graph learning tools in
//! the gridfm-graphkit family. Branch admittances are the exact π model with
//! the branch's tap ratio and phase shift, and every branch appears in both
//! directions of the edge index. HVDC rows have no AC π model and stay out of
//! the graph. This is a convenience format of the synthetic generator, not a
//! PowerIO format (PowerIO's `gridfm` is the Parquet dataset layout).

use std::collections::HashMap;

use num_complex::Complex;
use powerio::{BalancedNetwork, BusType};

use crate::error::SynthError;

/// Format token written into the document.
pub const GRIDFM_FORMAT: &str = "gridfm-graphkit";

const BUS_FEATURES: [&str; 15] = [
    "PD", "QD", "QG", "VM", "VA", "PQ", "PV", "REF", "MIN_VM", "MAX_VM", "MIN_QG", "MAX_QG", "GS",
    "BS", "VN_KV",
];
const BRANCH_FEATURES: [&str; 15] = [
    "P", "Q", "YFF_R", "YFF_I", "YFT_R", "YFT_I", "TAP", "ANG_MIN", "ANG_MAX", "RATE_A", "B_ON",
    "YTF_R", "YTF_I", "YTT_R", "YTT_I",
];
const GEN_FEATURES: [&str; 7] = ["PG", "MIN_PG", "MAX_PG", "C0", "C1", "C2", "G_ON"];

/// The π-model admittances `(Yff, Yft, Ytf, Ytt)` of a branch with series
/// `r + jx`, total charging `b`, off-nominal ratio `tap` (0 reads as 1) and
/// phase shift `shift` (degrees), the MATPOWER convention with the ratio on
/// the from side.
pub fn pi_admittances(r: f64, x: f64, b: f64, tap: f64, shift_deg: f64) -> [Complex<f64>; 4] {
    let y = Complex::new(r, x).inv();
    let ratio = if tap == 0.0 { 1.0 } else { tap };
    let (sin, cos) = shift_deg.to_radians().sin_cos();
    let t = Complex::new(ratio * cos, ratio * sin);
    let ysh = Complex::new(0.0, b / 2.0);
    [(y + ysh) / t.norm_sqr(), -y / t.conj(), -y / t, y + ysh]
}

/// The GridFM graphkit document of `net`. `branch_flow_mw`, when given, is
/// the DC OPF flow per branch row (absent rows write 0).
pub fn gridfm_graphkit_json(
    net: &BalancedNetwork,
    branch_flow_mw: Option<&[Option<f64>]>,
) -> Result<String, SynthError> {
    let base = net.base_mva();
    let index: HashMap<usize, usize> = net
        .buses()
        .iter()
        .enumerate()
        .map(|(i, b)| (b.id.0, i))
        .collect();
    let n = net.buses().len();
    let mut pd = vec![0.0; n];
    let mut qd = vec![0.0; n];
    for l in net.loads().iter().filter(|l| l.in_service) {
        if let Some(&i) = index.get(&l.bus.0) {
            pd[i] += l.p;
            qd[i] += l.q;
        }
    }
    let mut gs = vec![0.0; n];
    let mut bs = vec![0.0; n];
    for s in net.shunts().iter().filter(|s| s.in_service) {
        if let Some(&i) = index.get(&s.bus.0) {
            gs[i] += s.g;
            bs[i] += s.b;
        }
    }
    let mut has_gen = vec![false; n];
    let mut qmin = vec![0.0; n];
    let mut qmax = vec![0.0; n];
    for g in net.generators().iter().filter(|g| g.in_service) {
        if let Some(&i) = index.get(&g.bus.0) {
            has_gen[i] = true;
            qmin[i] += g.qmin;
            qmax[i] += g.qmax;
        }
    }
    // The declared reference buses; without one, the first bus.
    let mut is_ref: Vec<bool> = net.buses().iter().map(|b| b.kind == BusType::Ref).collect();
    if !is_ref.iter().any(|r| *r) {
        if let Some(first) = is_ref.first_mut() {
            *first = true;
        }
    }
    let bus_rows: Vec<Vec<f64>> = net
        .buses()
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let (pq, pv, rf) = match (is_ref[i], has_gen[i]) {
                (true, _) => (0.0, 0.0, 1.0),
                (false, true) => (0.0, 1.0, 0.0),
                (false, false) => (1.0, 0.0, 0.0),
            };
            vec![
                pd[i] / base,
                qd[i] / base,
                0.0,
                b.vm,
                b.va,
                pq,
                pv,
                rf,
                b.vmin,
                b.vmax,
                qmin[i] / base,
                qmax[i] / base,
                gs[i] / base,
                bs[i] / base,
                b.base_kv,
            ]
        })
        .collect();

    let mut branch_rows = Vec::new();
    let (mut edge_from, mut edge_to) = (Vec::new(), Vec::new());
    for (k, br) in net.branches().iter().enumerate() {
        let (Some(&f), Some(&t)) = (index.get(&br.from.0), index.get(&br.to.0)) else {
            continue;
        };
        if br.r == 0.0 && br.x == 0.0 {
            continue;
        }
        let [yff, yft, ytf, ytt] = pi_admittances(br.r, br.x, br.b, br.tap, br.shift);
        let p = branch_flow_mw
            .and_then(|flows| flows.get(k).copied().flatten())
            .unwrap_or(0.0);
        branch_rows.push(vec![
            p / base,
            0.0,
            yff.re,
            yff.im,
            yft.re,
            yft.im,
            if br.tap == 0.0 { 1.0 } else { br.tap },
            br.angmin,
            br.angmax,
            br.rate_a / base,
            if br.in_service { 1.0 } else { 0.0 },
            ytf.re,
            ytf.im,
            ytt.re,
            ytt.im,
        ]);
        edge_from.extend([f, t]);
        edge_to.extend([t, f]);
    }

    let mut gen_rows = Vec::new();
    let mut gen_bus = Vec::new();
    for g in net.generators() {
        let Some(&i) = index.get(&g.bus.0) else {
            continue;
        };
        let (c2, c1, c0) = match g.cost.as_ref().map(|c| c.coeffs.as_slice()) {
            Some([c2, c1, c0]) => (*c2, *c1, *c0),
            Some([c1, c0]) => (0.0, *c1, *c0),
            _ => (0.0, 0.0, 0.0),
        };
        gen_rows.push(vec![
            g.pg / base,
            g.pmin / base,
            g.pmax / base,
            c0,
            c1,
            c2,
            if g.in_service { 1.0 } else { 0.0 },
        ]);
        gen_bus.push(i);
    }

    let coordinates: Vec<Option<[f64; 2]>> = net
        .buses()
        .iter()
        .map(|b| b.location.map(|l| [l.x, l.y]))
        .collect();
    let document = serde_json::json!({
        "format": GRIDFM_FORMAT,
        "schema": "https://github.com/gridfm/gridfm-graphkit",
        "baseMVA": base,
        "n_bus": n,
        "synthetic_not_real": true,
        "bus_features": BUS_FEATURES,
        "bus": bus_rows,
        "bus_id": net.buses().iter().map(|b| b.id.0).collect::<Vec<_>>(),
        "coordinates": coordinates,
        "branch_features": BRANCH_FEATURES,
        "branch": branch_rows,
        "edge_index": [edge_from, edge_to],
        "gen_features": GEN_FEATURES,
        "gen": gen_rows,
        "gen_bus": gen_bus,
    });
    serde_json::to_string(&document).map_err(|e| SynthError::Build(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use powerio::{Branch, Bus, BusId, Load};

    #[test]
    fn hand_built_pi_model_reference_fallback_and_per_unit_scaling() {
        // A 2:1 transformer with no generators exercises the exact π model, the
        // first-bus reference fallback and per-unit scaling.
        let mut net = BalancedNetwork::new("tiny", 100.0);
        net.buses_mut().push(Bus::new(BusId(1), BusType::Pq, 230.0));
        net.buses_mut().push(Bus::new(BusId(2), BusType::Pq, 115.0));
        net.loads_mut().push(Load::new(BusId(1), 50.0, 10.0));
        let mut br = Branch::new(BusId(1), BusId(2), 0.01, 0.1);
        br.b = 0.02;
        br.tap = 2.0;
        br.rate_a = 100.0;
        net.branches_mut().push(br);
        let doc: serde_json::Value =
            serde_json::from_str(&gridfm_graphkit_json(&net, Some(&[Some(25.0)])).unwrap())
                .unwrap();
        assert_eq!(doc["branch_features"].as_array().unwrap().len(), 15);
        let row = &doc["branch"][0];
        let y = Complex::new(0.01, 0.1).inv();
        let close = |a: &serde_json::Value, b: f64| {
            (a.as_f64().unwrap() - b).abs() <= 1e-9 * b.abs().max(1.0)
        };
        assert_eq!(row[6], 2.0);
        // Yft = -y / conj(t) with t = 2 real; Yff = (y + jb/2) / |t|².
        assert!(close(&row[4], -y.re / 2.0) && close(&row[5], -y.im / 2.0));
        assert!(close(&row[2], y.re / 4.0) && close(&row[3], (y.im + 0.01) / 4.0));
        assert!(close(&row[13], y.re) && close(&row[14], y.im + 0.01));
        assert!(close(&row[0], 0.25));
        assert_eq!(doc["edge_index"], serde_json::json!([[0, 1], [1, 0]]));
        // No generator: the first bus is the reference.
        assert_eq!(doc["bus"][0][7], 1.0);
        assert_eq!(doc["bus"][1][5], 1.0);
        assert!(close(&doc["bus"][0][0], 0.5));
        assert_eq!(doc["gen"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn a_phase_shift_rotates_the_off_diagonal_terms() {
        let [_, yft, ytf, _] = pi_admittances(0.0, 0.1, 0.0, 1.0, 30.0);
        let [_, yft0, _, _] = pi_admittances(0.0, 0.1, 0.0, 1.0, 0.0);
        assert!((yft.norm_sqr() - yft0.norm_sqr()).abs() < 1e-12);
        // With r = 0 and a 30° shift, Yft = 10∠120° and Ytf = 10∠60°.
        assert!((yft.re + 5.0).abs() < 1e-9 && (ytf.re - 5.0).abs() < 1e-9);
    }
}
