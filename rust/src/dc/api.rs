//! The browser-facing DC entry point. Step 4 of issue #2: parse a network,
//! apply demand deltas, solve, and serve the dispatch, flows, LMPs, and an
//! optional dLMP/dd column in the shapes the Julia backend serves (see
//! `backend/src/state.jl` `solution_payload` / `sensitivity_payload`).
//!
//! Keeping the JSON layer here (not behind `#[wasm_bindgen]`) makes it testable
//! natively; `lib.rs` wraps it as the `solve_dc` wasm export.

use std::collections::HashMap;

use powerio::network::Network;
use serde::Deserialize;
use serde_json::json;

use super::model::DcNetwork;
use super::{dlmp_dd, solve};

/// A solve request: demand deltas in MW keyed by original bus id (the operating
/// point is `base demand + deltas`), and an optional bus to return the dLMP/dd
/// column for. Mirrors the backend's `d=bus:mw,...` and `sens` parameters.
#[derive(Deserialize, Default)]
struct SolveRequest {
    #[serde(default)]
    deltas: HashMap<i64, f64>,
    #[serde(default)]
    sens_bus: Option<i64>,
}

/// Solve the DC OPF for `network_json` at `base demand + deltas` and return
/// `{ objective, lmp, flows, dispatch, dlmp_dd }` as JSON. `dlmp_dd` is the
/// sensitivity column for `sens_bus` (or null when none is requested).
///
/// LMPs and the sensitivity column are keyed by original bus id; flows and
/// dispatch by 1-based in-service branch / generator index, matching the
/// backend's `id_map`. Powers are MW, prices $/MWh, sensitivities ($/MWh)/MW.
pub fn solve_dc_json(network_json: &str, deltas_json: &str) -> Result<String, String> {
    let net = Network::from_json(network_json).map_err(|e| e.to_string())?;
    let mut dc = DcNetwork::from_network(&net)?;
    let base = dc.base_mva;

    // Original bus id -> dense index, for routing deltas and the sensitivity bus.
    let bus_idx: HashMap<usize, usize> = dc
        .bus_ids
        .iter()
        .enumerate()
        .map(|(i, &id)| (id, i))
        .collect();

    let req: SolveRequest = if deltas_json.trim().is_empty() {
        SolveRequest::default()
    } else {
        serde_json::from_str(deltas_json).map_err(|e| format!("bad deltas JSON: {e}"))?
    };

    // Establish the operating point: demand = base + deltas (per unit).
    for (&bus, &mw) in &req.deltas {
        if let Some(&i) = bus_idx.get(&(bus as usize)) {
            dc.demand[i] += mw / base;
        }
    }

    let sol = solve(&dc)?;
    let lmp = sol.lmp_usd_per_mwh(base);

    let lmp_payload: Vec<_> = (0..dc.n)
        .map(|i| json!({ "bus": dc.bus_ids[i], "usd_per_mwh": lmp[i] }))
        .collect();
    let flows_payload: Vec<_> = (0..dc.m)
        .map(|e| {
            let loading = if dc.fmax[e] > 0.0 {
                sol.f[e].abs() / dc.fmax[e]
            } else {
                0.0
            };
            json!({ "branch": e + 1, "mw": sol.f[e] * base, "loading": loading })
        })
        .collect();
    let dispatch_payload: Vec<_> = (0..dc.k)
        .map(|j| json!({ "gen": j + 1, "mw": sol.pg[j] * base }))
        .collect();

    let dlmp = match req.sens_bus.and_then(|b| bus_idx.get(&(b as usize)).copied()) {
        Some(si) => {
            let col = dlmp_dd(&dc, &sol, &[si])?;
            let values: Vec<_> = (0..dc.n)
                .map(|i| json!({ "bus": dc.bus_ids[i], "value": col[0][i] }))
                .collect();
            json!({
                "bus": dc.bus_ids[si],
                "operand": "lmp",
                "parameter": "d",
                "units": "($/MWh)/MW",
                "values": values,
            })
        }
        None => serde_json::Value::Null,
    };

    serde_json::to_string(&json!({
        "objective": sol.objective,
        "lmp": lmp_payload,
        "flows": flows_payload,
        "dispatch": dispatch_payload,
        "dlmp_dd": dlmp,
    }))
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::super::model::CASE3;
    use super::*;
    use serde_json::Value;

    fn case3_json() -> String {
        powerio::parse_str(CASE3, "matpower")
            .expect("parse")
            .network
            .to_json()
            .expect("to_json")
    }

    #[test]
    fn base_solution_payload_shapes() {
        let out = solve_dc_json(&case3_json(), "").expect("solve_dc");
        let v: Value = serde_json::from_str(&out).unwrap();
        assert!(v["objective"].as_f64().unwrap() > 0.0);
        let lmp = v["lmp"].as_array().unwrap();
        assert_eq!(lmp.len(), 3);
        // LMPs keyed by original bus id, all positive (uncongested).
        let buses: Vec<i64> = lmp.iter().map(|e| e["bus"].as_i64().unwrap()).collect();
        assert_eq!(buses, vec![1, 2, 3]);
        for e in lmp {
            assert!(e["usd_per_mwh"].as_f64().unwrap() > 0.0);
        }
        assert_eq!(v["flows"].as_array().unwrap().len(), 3);
        assert_eq!(v["dispatch"].as_array().unwrap().len(), 2);
        // Dispatch balances the 90 MW load (DC lossless), no sensitivity asked.
        let total: f64 = v["dispatch"]
            .as_array()
            .unwrap()
            .iter()
            .map(|g| g["mw"].as_f64().unwrap())
            .sum();
        assert!((total - 90.0).abs() < 1e-2, "dispatch total {total}");
        assert!(v["dlmp_dd"].is_null());
    }

    #[test]
    fn sensitivity_column_present_when_requested() {
        let out = solve_dc_json(&case3_json(), r#"{"sens_bus": 2}"#).expect("solve_dc");
        let v: Value = serde_json::from_str(&out).unwrap();
        let s = &v["dlmp_dd"];
        assert_eq!(s["bus"].as_i64().unwrap(), 2);
        assert_eq!(s["units"].as_str().unwrap(), "($/MWh)/MW");
        let values = s["values"].as_array().unwrap();
        assert_eq!(values.len(), 3);
        // Uncongested: every price rises with demand at bus 2.
        for e in values {
            assert!(e["value"].as_f64().unwrap() > 0.0);
        }
    }

    /// `norm(mine - server) / norm(server)` over entries matched by `key`.
    #[cfg(test)]
    fn column_rel(server: &Value, mine: &Value, key: &str, val: &str) -> f64 {
        let to_map = |v: &Value| -> HashMap<i64, f64> {
            v.as_array()
                .unwrap()
                .iter()
                .map(|e| (e[key].as_i64().unwrap(), e[val].as_f64().unwrap()))
                .collect()
        };
        let (s, m) = (to_map(server), to_map(mine));
        let mut num = 0.0;
        let mut den = 0.0;
        for (k, &sv) in &s {
            let mv = m[k];
            num += (mv - sv).powi(2);
            den += sv * sv;
        }
        num.sqrt() / den.sqrt().max(f64::EPSILON)
    }

    #[test]
    #[ignore = "needs the running Julia backend (set TELLEGEN_SERVER, default :8000)"]
    fn parity_against_julia_server() {
        // Direct cross-check: the Rust solve and dLMP/dd must agree with the
        // PowerDiff/Ipopt backend on the served ACTIVSg cases. Skips when the
        // server or a case file is absent.
        let base =
            std::env::var("TELLEGEN_SERVER").unwrap_or_else(|_| "http://localhost:8000".into());
        if ureq::get(format!("{base}/api/cases")).call().is_err() {
            eprintln!("skipping server parity: {base} not reachable");
            return;
        }
        let get_json = |url: String| -> Value {
            let body = ureq::get(url).call().unwrap().body_mut().read_to_string().unwrap();
            serde_json::from_str(&body).unwrap()
        };
        for (id, dir) in [
            ("case200", "ACTIVSg200"),
            ("case500", "ACTIVSg500"),
            ("case2000", "ACTIVSg2000"),
        ] {
            let path = format!("{}/../data/{dir}/case_{dir}.m", env!("CARGO_MANIFEST_DIR"));
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let net = powerio::parse_str(&text, "matpower").unwrap().network;
            let net_json = net.to_json().unwrap();
            let dc = DcNetwork::from_network(&net).unwrap();

            // Base solution: LMPs by bus, dispatch by gen index.
            let mine: Value = serde_json::from_str(&solve_dc_json(&net_json, "").unwrap()).unwrap();
            let server = get_json(format!("{base}/api/cases/{id}/solution"));
            let lmp_rel = column_rel(&server["lmp"], &mine["lmp"], "bus", "usd_per_mwh");
            let disp_rel = column_rel(&server["dispatch"], &mine["dispatch"], "gen", "mw");

            // dLMP/dd column at the highest-demand bus (a large, stable column).
            let jmax = (0..dc.n).max_by(|&a, &b| dc.demand[a].total_cmp(&dc.demand[b])).unwrap();
            let bus = dc.bus_ids[jmax];
            let mine_s: Value =
                serde_json::from_str(&solve_dc_json(&net_json, &format!(r#"{{"sens_bus":{bus}}}"#)).unwrap())
                    .unwrap();
            let server_s = get_json(format!("{base}/api/cases/{id}/sensitivity/lmp/d/{bus}"));
            let sens_rel = column_rel(&server_s["values"], &mine_s["dlmp_dd"]["values"], "bus", "value");

            eprintln!("{id}: lmp_rel={lmp_rel:.2e} dispatch_rel={disp_rel:.2e} dlmp_dd_rel={sens_rel:.2e}");
            assert!(lmp_rel < 1e-3, "{id} LMP vs server rel {lmp_rel}");
            assert!(sens_rel < 1e-3, "{id} dLMP/dd vs server rel {sens_rel}");
        }
    }

    #[test]
    fn deltas_shift_the_operating_point() {
        let base: Value = serde_json::from_str(&solve_dc_json(&case3_json(), "").unwrap()).unwrap();
        let bumped: Value =
            serde_json::from_str(&solve_dc_json(&case3_json(), r#"{"deltas": {"2": 50.0}}"#).unwrap())
                .unwrap();
        let lmp0 = base["lmp"][0]["usd_per_mwh"].as_f64().unwrap();
        let lmp1 = bumped["lmp"][0]["usd_per_mwh"].as_f64().unwrap();
        // More demand at bus 2 raises the system marginal price.
        assert!(lmp1 > lmp0, "LMP should rise with demand: {lmp0} -> {lmp1}");
    }
}
