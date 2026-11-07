//! Run an external BMOPF case without vendoring it. Optional arguments: power
//! base (VA), objective scale, maximum iterations, profile (true/false).
//! --qualify adds rank and one forward/adjoint query; no extra optimization.
//! Emits one JSON result record. Diagnostic refusal preserves physical acceptance.
#[cfg(feature = "mc-opf")]
fn qualification(result: &tellegen::McOpfResult) -> serde_json::Value {
    use serde_json::json;
    use std::time::Instant;
    use tellegen::*;
    let d = result
        .duals
        .as_ref()
        .expect("qualification requests dual collection");
    let mut record = json!({"dual_status":d.status, "rank":d.active_rank, "kkt":d.kkt});
    let start = Instant::now();
    let view = match result.sensitivities() {
        Ok(view) => view,
        Err(error) => {
            record["sensitivity"] = json!({"status":"unavailable","error":error,
                "view_s":start.elapsed().as_secs_f64()});
            return record;
        }
    };
    let view_s = start.elapsed().as_secs_f64();
    let parameters = view.parameters();
    let parameter = parameters
        .iter()
        .find(|p| p.kind == McOpfParameterKind::LoadActivePower)
        .or(parameters.first());
    let Some(parameter) = parameter else {
        record["sensitivity"] =
            json!({"status":"no_supported_parameter", "view":view.diagnostics(), "view_s":view_s});
        return record;
    };
    let start = Instant::now();
    let queries = || -> Result<_, McOpfSensitivityError> {
        let forward = view.forward_with_diagnostics(
            &[McOpfParameterDirection {
                parameter: parameter.clone(),
                delta: 1.,
            }],
            &[McOpfSensitivityOutput::Objective],
        )?;
        let adjoint = view.adjoint_with_diagnostics(
            &[McOpfOutputWeight {
                output: McOpfSensitivityOutput::Objective,
                weight: 1.,
            }],
            std::slice::from_ref(parameter),
        )?;
        Ok((forward, adjoint))
    };
    record["sensitivity"] = match queries() {
        Ok((forward, adjoint)) => json!({"status":"available", "view":view.diagnostics(),
            "parameter":parameter, "forward":forward.0, "forward_solve":forward.1,
            "adjoint":adjoint.0, "adjoint_solve":adjoint.1}),
        Err(error) => json!({"status":"unavailable", "view":view.diagnostics(), "error":error}),
    };
    record["sensitivity"]["view_s"] = json!(view_s);
    record["sensitivity"]["queries_s"] = json!(start.elapsed().as_secs_f64());
    record
}
#[cfg(feature = "mc-opf")]
fn parse_case(
    input: Vec<u8>,
) -> Result<powerio::PioModule<powerio::PioValue>, tellegen::McOpfError> {
    use powerio::{ParseOptions, Source};
    let module = powerio::parse_with_options(
        Source::from_memory("external.bmopf.json", input)
            .map_err(|e| tellegen::McOpfError::from_powerio(tellegen::McOpfStage::Input, e))?,
        &ParseOptions::default()
            .format("bmopf-json")
            .map_err(|e| tellegen::McOpfError::from_powerio(tellegen::McOpfStage::Input, e))?,
    )
    .map_err(|e| tellegen::McOpfError::from_powerio(tellegen::McOpfStage::Input, e))?;
    tellegen::mc_opf::validate_mc_opf_input_diagnostics(&module)?;
    Ok(module)
}

#[cfg(not(feature = "mc-opf"))]
fn main() {
    eprintln!("enable --features mc-opf");
    std::process::exit(2);
}
#[cfg(feature = "mc-opf")]
fn main() {
    use powerio::{McAcOpfInstance, PioValue};
    use serde_json::json;
    use sha2::{Digest, Sha256};
    use std::{sync::Arc, time::Instant};
    let qualify = std::env::args().any(|a| a == "--qualify");
    let args: Vec<_> = std::env::args().filter(|a| a != "--qualify").collect();
    if args.len() < 2 {
        eprintln!("usage: mc_opf_case_probe INPUT [POWER_BASE OBJECTIVE_SCALE MAX_ITER PROFILE] [--qualify]");
        std::process::exit(2);
    }
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
        collect_duals: qualify,
        ..Default::default()
    };
    let t = Instant::now();
    let mut record = json!({"input":args[1],"sha256":hash,"options":options});
    let run = || -> Result<_, tellegen::McOpfError> {
        let parse_start = Instant::now();
        let module = parse_case(input)?;
        let diagnostics: Vec<_> = module
            .diagnostics()
            .iter()
            .map(|d| json!({"code":d.code(),"message":d.message()}))
            .collect();
        let PioValue::MulticonductorNetwork(net) = module.into_value() else {
            return Err(tellegen::McOpfError::new(
                tellegen::McOpfErrorKind::InvalidInput,
                tellegen::McOpfStage::Input,
                "not multiconductor",
            ));
        };
        let counts = json!({"buses":net.buses().len(),"terminals":net.buses().iter().map(|b| b.terminals.len()).sum::<usize>(),"lines":net.lines().len(),"loads":net.loads().len(),"inverters":net.ibrs().len()});
        let axes: Vec<_> = net
            .buses()
            .iter()
            .map(|b| (b.id.clone(), b.terminals.clone()))
            .collect();
        let instance = Arc::new(
            McAcOpfInstance::from_network(net)
                .map_err(|e| tellegen::McOpfError::from_powerio(tellegen::McOpfStage::Input, e))?,
        );
        let parse_s = parse_start.elapsed().as_secs_f64();
        let solve_start = Instant::now();
        let result = tellegen::solve_mc_ac_opf_instance(instance, &options)?;
        let solve_s = solve_start.elapsed().as_secs_f64();
        let qualification = qualify.then(|| qualification(&result));
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
            json!({"status":"accepted","parse_instance_s":parse_s,"solve_s":solve_s,"qualification":qualification,"profile":result.profile,"counts":counts,"diagnostics":diagnostics,"objective":result.solution.objective(),"iterations":result.iterations,"residuals":result.residuals,"bus":bus,"devices":result.devices,"branches":result.branches,"transformers":result.transformers,"shunts":result.shunts,"terminals":result.terminals}),
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

#[cfg(all(test, feature = "mc-opf"))]
mod tests {
    use super::parse_case;

    #[test]
    fn qualification_refusal_does_not_turn_physical_success_into_failure() {
        use powerio::McAcOpfInstance;
        use powerio_dist::{
            Configuration, DistBus, DistLine, DistLineCode, DistLoad, MulticonductorNetwork,
            VoltageSource,
        };
        use std::sync::Arc;
        let terminals = || vec!["a".to_string(), "n".to_string()];
        let mut n = MulticonductorNetwork::new();
        let mut source = DistBus::new("s", terminals());
        source.grounded = vec!["n".into()];
        n.buses_mut()
            .extend([source, DistBus::new("b", terminals())]);
        n.line_codes_mut().push(DistLineCode::new(
            "c",
            vec![vec![0.1, 0.], vec![0., 0.1]],
            vec![vec![0.; 2]; 2],
        ));
        n.lines_mut().push(DistLine::new(
            "l",
            "s",
            "b",
            terminals(),
            terminals(),
            "c",
            1.,
        ));
        n.lines_mut()[0].i_max = Some(vec![0.; 2]);
        n.loads_mut().push(DistLoad::new(
            "d",
            "b",
            terminals(),
            Configuration::Wye,
            vec![0.],
            vec![0.],
        ));
        n.sources_mut().push(VoltageSource::new(
            "grid",
            "s",
            terminals(),
            vec![230., 0.],
            vec![0., 0.],
        ));
        let r = tellegen::solve_mc_ac_opf_instance(
            Arc::new(McAcOpfInstance::from_network(n).unwrap()),
            &tellegen::McOpfOptions {
                collect_duals: true,
                max_iterations: 100,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(r.residuals.maximum() <= 1e-6);
        let diagnostic = super::qualification(&r);
        assert_eq!(diagnostic["dual_status"], "zero_limit_reduction");
        assert_eq!(diagnostic["sensitivity"]["status"], "unavailable");
        assert_eq!(
            diagnostic["sensitivity"]["error"]["kind"],
            "unreliable_duals"
        );
    }

    #[test]
    fn probe_rejects_dropped_sequence_voltage_bounds() {
        let input = include_bytes!("../tests/data/mc_opf/unbalanced.json");
        assert!(parse_case(input.to_vec()).is_ok());
        let mut case: serde_json::Value = serde_json::from_slice(input).unwrap();
        case["bus"]["s"]["vsym_min"] = serde_json::json!([100, 0, 100]);
        let error = parse_case(serde_json::to_vec(&case).unwrap()).unwrap_err();
        assert!(
            error.to_string().contains("READ.BMOPF.FIELD_DROPPED"),
            "{error}"
        );
        assert!(
            error.to_string().contains("sequence lower bounds"),
            "{error}"
        );
    }

    #[test]
    fn probe_accepts_lossless_converter_remarks() {
        let cases: serde_json::Value =
            serde_json::from_str(include_str!("../tests/data/mc_opf/components.json")).unwrap();
        let module = parse_case(serde_json::to_vec(&cases["capacitor_wye"]).unwrap()).unwrap();
        assert!(module
            .diagnostics()
            .iter()
            .any(|d| d.code() == "READ.BMOPF.CAPACITOR_LOWERED"));
    }
}
