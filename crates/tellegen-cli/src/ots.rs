//! Compare discrete dispatch models on a common typed PowerIO problem.
use powerio::{AcOtsInstance, PioValue};
use serde_json::json;
use std::{sync::Arc, time::Instant};
use tellegen::ots::{restore_ac, solve_ots, OtsModel};

pub(super) const USAGE: &str = "tellegen ots-compare CASE [--max-switched K] [--topology-limit N]";

pub(super) fn compare(mut args: impl Iterator<Item = String>) -> Result<String, String> {
    let path = args.next().ok_or(USAGE)?;
    let mut budget = None;
    let mut limit = 1000;
    while let Some(flag) = args.next() {
        let value = args
            .next()
            .ok_or(USAGE)?
            .parse::<usize>()
            .map_err(|_| USAGE)?;
        match flag.as_str() {
            "--max-switched" => budget = Some(value),
            "--topology-limit" => limit = value,
            _ => return Err(USAGE.into()),
        }
    }
    let module = if path.ends_with(".pio.json") {
        powerio::deserialize(&path)
    } else {
        powerio::parse(&path)
    }
    .map_err(|e| e.to_string())?;
    let typed = matches!(module.value(), PioValue::AcOtsInstance(_));
    let mut ots = match module.into_value() {
        PioValue::AcOtsInstance(ots) => ots,
        PioValue::AcOpfInstance(base) => {
            AcOtsInstance::from_ac_opf(base).map_err(|e| e.to_string())?
        }
        PioValue::BalancedNetwork(network) => {
            AcOtsInstance::from_network(network).map_err(|e| e.to_string())?
        }
        other => {
            return Err(format!(
                "OTS requires a balanced network, AC OPF or AC OTS instance, got {}",
                other.type_name()
            ))
        }
    };
    let max_switched = match (budget, typed) {
        (None, false) => Some(usize::from(ots.branches().iter().any(|b| b.switchable))),
        _ => budget,
    };
    if let Some(max) = max_switched {
        let mut options = ots.options().clone();
        options.max_switched = Some(max);
        ots = AcOtsInstance::new(ots.base().clone(), options).map_err(|e| e.to_string())?;
    }
    let ots = Arc::new(ots);
    let mut record = json!({
        "case": path,
        "branch_ids": ots.branch_order().collect::<Vec<_>>(),
        "max_switched": ots.options().max_switched,
        "topology_limit": limit,
    });
    for (key, model) in [
        ("dcots", OtsModel::Dcots),
        ("lacots", OtsModel::Lacots),
        ("acots", OtsModel::Acots),
    ] {
        let started = Instant::now();
        let result = solve_ots(Arc::clone(&ots), model, limit)?;
        let elapsed = started.elapsed().as_secs_f64();
        let mut row = serde_json::to_value(&result).map_err(|e| e.to_string())?;
        row["search_seconds"] = json!(elapsed);
        if let Some(plan) = &result.best {
            row["changed_branches"] = json!(ots
                .branches()
                .iter()
                .zip(&plan.closed)
                .filter(|(b, c)| b.initial_closed != **c)
                .map(|(b, _)| &b.id)
                .collect::<Vec<_>>());
            if let Some(ac) = &plan.ac_solution {
                row["residuals"] = json!(ac.residuals());
            } else {
                let started = Instant::now();
                row["restoration"] = match restore_ac(Arc::clone(&ots), plan) {
                    Ok(r) => {
                        json!({
                            "status": "ac_feasible",
                            "objective": r.solution.objective(),
                            "dispatch": r.dispatch,
                            "residuals": r.solution.residuals(),
                            "initial_p_mismatch_mw": r.initial_p_mismatch_mw,
                            "initial_q_mismatch_mvar": r.initial_q_mismatch_mvar,
                            "redispatch_l1_mw": r.redispatch_l1_mw,
                            "warm_start_error": r.warm_start_error,
                        })
                    }
                    Err(e) => json!({"status":"failed","error":e}),
                };
                row["restoration"]["seconds"] = json!(started.elapsed().as_secs_f64());
            }
        }
        record[key] = row;
    }
    serde_json::to_string(&record).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_reports_before_and_after_for_both_plans() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tellegen/tests/data/dc_variants/case3_variants.m"
        );
        let text = compare([path, "--max-switched", "1"].into_iter().map(str::to_owned)).unwrap();
        let record: serde_json::Value = serde_json::from_str(&text).unwrap();
        for key in ["dcots", "lacots"] {
            let s = &record[key];
            assert_eq!(s["visited"], 4);
            assert_eq!(s["exhausted"], true);
            assert_eq!(s["changed_branches"], json!(["1-2"]));
            assert_eq!(s["restoration"]["status"], "ac_feasible");
            assert!(s["restoration"]["dispatch"]["vm_pu"].is_array());
            assert!(
                s["best"]["objective"].as_f64().unwrap()
                    < s["restoration"]["objective"].as_f64().unwrap()
            );
        }
        assert_eq!(record["acots"]["changed_branches"], json!(["1-2"]));
    }
    #[test]
    fn typed_ir_preserves_the_budget_and_island_rejections() {
        let input = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tellegen/tests/data/dc_variants/case3_variants.m"
        );
        let net = tellegen::ir::balanced_module(powerio::parse(input).unwrap())
            .unwrap()
            .into_value();
        let default = AcOtsInstance::from_network(net).unwrap();
        let mut options = default.options().clone();
        options.max_switched = Some(2);
        let instance = AcOtsInstance::new(default.base().clone(), options).unwrap();
        let module = powerio::PioModule::new(PioValue::AcOtsInstance(instance));
        let file =
            std::env::temp_dir().join(format!("tellegen-ots-{}.pio.json", std::process::id()));
        std::fs::write(&file, tellegen::ir::serialize_module(&module).unwrap()).unwrap();
        let text = compare([file.to_str().unwrap().to_owned()].into_iter());
        std::fs::remove_file(file).unwrap();
        let record: serde_json::Value = serde_json::from_str(&text.unwrap()).unwrap();
        assert_eq!(record["max_switched"], 2);
        assert_eq!(record["lacots"]["visited"], 7);
        assert_eq!(record["lacots"]["policy_rejected"], 3);
    }
}
