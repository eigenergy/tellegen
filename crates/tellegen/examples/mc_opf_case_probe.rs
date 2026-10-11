//! Run an external BMOPF case without vendoring it. Optional arguments: power
//! base (VA), objective scale, maximum iterations, profile (true/false). Emits one JSON result record.
#[cfg(not(feature = "mc-opf"))]
fn main() {
    eprintln!("enable --features mc-opf");
    std::process::exit(2);
}
#[cfg(feature = "mc-opf")]
fn main() {
    use powerio::{McAcOpfInstance, ParseOptions, PioValue, Source};
    use serde_json::json;
    use sha2::{Digest, Sha256};
    use std::{sync::Arc, time::Instant};
    let args: Vec<_> = std::env::args().collect();
    let input = std::fs::read(&args[1]).expect("read input");
    let hash = Sha256::digest(&input)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let options = tellegen::McOpfOptions {
        power_base_va: args.get(2).map_or(1000.0, |s| s.parse().unwrap()),
        objective_scale: args.get(3).map_or(1.0, |s| s.parse().unwrap()),
        max_iterations: args.get(4).map_or(500, |s| s.parse().unwrap()),
        collect_profile: args.get(5).is_some_and(|s| s == "true"),
        ..Default::default()
    };
    let t = Instant::now();
    let mut record = json!({"input":args[1],"sha256":hash,"options":options});
    let run = || -> Result<_, String> {
        let parse_start = Instant::now();
        let module = powerio::parse_with_options(
            Source::from_memory("external.bmopf.json", input).map_err(|e| e.to_string())?,
            &ParseOptions::default()
                .format("bmopf-json")
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let diagnostics: Vec<_> = module
            .diagnostics()
            .iter()
            .map(|d| json!({"code":d.code(),"message":d.message()}))
            .collect();
        let PioValue::MulticonductorNetwork(net) = module.into_value() else {
            return Err("not multiconductor".into());
        };
        let counts = json!({"buses":net.buses().len(),"terminals":net.buses().iter().map(|b| b.terminals.len()).sum::<usize>(),"lines":net.lines().len(),"loads":net.loads().len(),"inverters":net.ibrs().len()});
        let axes: Vec<_> = net
            .buses()
            .iter()
            .map(|b| (b.id.clone(), b.terminals.clone()))
            .collect();
        let instance = Arc::new(McAcOpfInstance::from_network(net).map_err(|e| e.to_string())?);
        let parse_s = parse_start.elapsed().as_secs_f64();
        let result = tellegen::solve_mc_ac_opf_instance(instance, &options)?;
        let mut bus = serde_json::Map::new();
        for (name, terminals) in axes {
            let mut v = serde_json::Map::new();
            for terminal in terminals {
                if let (Some(m), Some(a)) = (
                    result.solution.terminal_voltage_magnitude(&name, &terminal),
                    result.solution.terminal_voltage_angle(&name, &terminal),
                ) {
                    v.insert(terminal, json!({"vr":m*a.cos(),"vi":m*a.sin()}));
                }
            }
            bus.insert(name, json!(v));
        }
        Ok(
            json!({"status":"accepted","parse_instance_s":parse_s,"profile":result.profile,"counts":counts,"diagnostics":diagnostics,"objective":result.solution.objective(),"iterations":result.iterations,"residuals":result.residuals,"bus":bus,"devices":result.devices,"branches":result.branches}),
        )
    };
    let (outcome, iterations) = if options.collect_profile {
        pounce_rs::with_iter_capture(run)
    } else {
        (run(), Vec::new())
    };
    record["iterations"] = json!(iterations
        .iter()
        .map(|i| json!({
            "iter":i.iter,"objective":i.objective,"inf_pr":i.inf_pr,"inf_du":i.inf_du,
            "mu":i.mu,"d_norm":i.d_norm,"regularization":i.regularization,
            "alpha_dual":i.alpha_dual,"alpha_primal":i.alpha_primal,
            "step":i.alpha_primal_char,"ls_trials":i.ls_trials
        }))
        .collect::<Vec<_>>());
    match outcome {
        Ok(v) => record["result"] = v,
        Err(e) => record["result"] = json!({"status":"rejected","error":e}),
    }
    record["elapsed_s"] = json!(t.elapsed().as_secs_f64());
    println!("{}", serde_json::to_string(&record).unwrap());
}
