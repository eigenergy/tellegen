//! Public native API walkthrough. Run without arguments for the bundled demo,
//! or pass INSTANCE.module.json EDITS.json [RESULT.module.json].
//! The printed summary is illustrative; the saved PowerIO report is the
//! supported persistence boundary. No files are written by default.
use powerio::{McAcOpfInstance, ParseOptions, PioValue, Source};
use serde_json::{json, Value};
use std::{error::Error, fs, io, process::ExitCode};
use tellegen::{
    validate_mc_opf_input_diagnostics, McOpfEdit, McOpfError, McOpfOptions, McOpfOutputWeight,
    McOpfParameter, McOpfParameterDirection, McOpfParameterKind, McOpfParameterOwner, McOpfResult,
    McOpfSensitivityError, McOpfSensitivityOutput, McOpfSession,
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
const FEEDER: &[u8] = include_bytes!("../tests/data/mc_opf/feeder.json");

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn declared_instance(input: &str) -> Result<McAcOpfInstance> {
    let module = powerio::deserialize(Source::from_memory(
        "instance.pio.json",
        input.as_bytes().to_vec(),
    )?)?;
    // Check loss/unsupported-field diagnostics before extracting the value.
    validate_mc_opf_input_diagnostics(&module)?;
    match module.into_value() {
        PioValue::McAcOpfInstance(instance) => Ok(instance),
        _ => Err(invalid("expected a declared McAcOpfInstance PowerIO module").into()),
    }
}

fn demo_instance() -> Result<McAcOpfInstance> {
    let module = powerio::parse_with_options(
        Source::from_memory("feeder.bmopf.json", FEEDER.to_vec())?,
        &ParseOptions::default().format("bmopf-json")?,
    )?;
    validate_mc_opf_input_diagnostics(&module)?;
    let PioValue::MulticonductorNetwork(network) = module.into_value() else {
        return Err(invalid("expected the demo's multiconductor network").into());
    };
    // Deliberate promotion for this demo only. User-supplied instances retain
    // their declared objective, constraint selection and initial point.
    Ok(McAcOpfInstance::from_network(network)?)
}

fn load_parameter(instance: &McAcOpfInstance) -> Result<McOpfParameter> {
    let load = instance
        .network()
        .loads()
        .iter()
        .find(|load| !load.p_nom.is_empty())
        .ok_or_else(|| invalid("this walkthrough requires a load with at least one coil"))?;
    // Canonical equipment identity and zero-based coil index, not a global phase.
    Ok(McOpfParameter::new(
        McOpfParameterOwner::Load,
        &load.name,
        Some(0),
        McOpfParameterKind::LoadActivePower,
    ))
}

fn demo_edits(instance: &McAcOpfInstance, parameter: &McOpfParameter) -> Vec<McOpfEdit> {
    let load = instance
        .network()
        .loads()
        .iter()
        .find(|load| load.name == parameter.component)
        .expect("parameter selected from this instance");
    let mut p_w = load.p_nom.clone();
    p_w[0] += 1.;
    vec![McOpfEdit::LoadPower {
        load: load.name.clone(),
        p_w,
        q_var: load.q_nom.clone(),
    }]
}

fn physical_summary(result: &McOpfResult) -> Value {
    json!({
        "objective_currency_per_hour": result.solution.objective(),
        "maximum_normalized_residual": result.residuals.maximum(),
        "terminals": result.terminals,
        "branch_count": result.branches.len(),
        "device_count": result.devices.len(),
        "transformer_count": result.transformers.len(),
        "fingerprint": result.fingerprint,
        "diagnostics": result.diagnostics,
    })
}

fn local_derivatives(
    result: &McOpfResult,
    parameter: &McOpfParameter,
) -> std::result::Result<Value, McOpfSensitivityError> {
    // One factorization, reused for forward and adjoint backsolves. These are
    // conditional local derivatives, not a global optimum certificate.
    let view = result.sensitivities()?;
    let (forward, forward_solve) = view.forward_with_diagnostics(
        &[McOpfParameterDirection {
            parameter: parameter.clone(),
            delta: 1., // one watt per unit path step
        }],
        &[McOpfSensitivityOutput::Objective],
    )?;
    let (adjoint, adjoint_solve) = view.adjoint_with_diagnostics(
        &[McOpfOutputWeight {
            output: McOpfSensitivityOutput::Objective,
            weight: 1.,
        }],
        std::slice::from_ref(parameter),
    )?;
    Ok(json!({
        "status": "available",
        "qualification": view.diagnostics(),
        "forward": forward, "forward_solve": forward_solve,
        "adjoint": adjoint, "adjoint_solve": adjoint_solve,
    }))
}

fn sensitivity_summary(result: &McOpfResult, parameter: &McOpfParameter) -> Value {
    match local_derivatives(result, parameter) {
        Ok(value) => value,
        // A physically accepted result remains useful when dual/rank/linear
        // solve qualification prevents returning local derivatives.
        Err(error) => json!({"status": "unavailable", "error": error}),
    }
}

fn workflow(
    session: &mut McOpfSession,
    edits: &[McOpfEdit],
    parameter: &McOpfParameter,
) -> Result<(Value, String)> {
    let cold = &session.solve()?.result;
    let before = physical_summary(cold);
    let without_duals = sensitivity_summary(cold, parameter);

    session.apply_edits(edits)?;
    session.set_options(McOpfOptions {
        collect_duals: true,
        ..session.options().clone()
    })?;
    // Edits/options invalidate current detail but retain the previous primal.
    let stale = session.sensitivities().err();
    let warm = session.solve()?;
    let start = warm.start;
    let report_json = warm.result.to_module_json()?;
    let restored = McOpfResult::from_module_json(&report_json)?;
    let sensitivities = sensitivity_summary(&restored, parameter);
    Ok((
        json!({
            "before": before, "after": physical_summary(&restored),
            "without_duals": without_duals, "after_edit": stale,
            "start": start, "revision": session.revision(),
            "solves": session.solve_count(), "compilations": session.compilation_count(),
            "parameter": parameter, "parameter_unit": parameter.unit(),
            "dual_status": restored.duals.as_ref().map(|report| report.status),
            "sensitivities": sensitivities,
        }),
        report_json,
    ))
}

fn run() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if !matches!(args.len(), 0 | 2 | 3) {
        return Err(invalid(
            "usage: mc_opf_workflow [INSTANCE.module.json EDITS.json [RESULT.module.json]]",
        )
        .into());
    }
    let instance = if args.is_empty() {
        demo_instance()?
    } else {
        declared_instance(&fs::read_to_string(&args[0])?)?
    };
    let parameter = load_parameter(&instance)?;
    let edits: Vec<McOpfEdit> = if args.is_empty() {
        demo_edits(&instance, &parameter)
    } else {
        serde_json::from_str(&fs::read_to_string(&args[1])?)?
    };
    let mut session = McOpfSession::new(
        instance,
        McOpfOptions {
            max_iterations: 100,
            ..Default::default()
        },
    )?;
    let (summary, report) = workflow(&mut session, &edits, &parameter)?;
    if let Some(path) = args.get(2) {
        fs::write(path, report)?;
    }
    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            if let Some(error) = error.downcast_ref::<McOpfError>() {
                eprintln!("{}", serde_json::to_string_pretty(error).unwrap());
            } else {
                eprintln!("{error}");
            }
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{atomic::AtomicBool, Arc};
    use tellegen::{
        ir, McOpfErrorKind, McOpfMarginalStatus, McOpfParameterUnit, McOpfSensitivityErrorKind,
        McOpfStage, McOpfStart, McOpfStartUsed, McOpfTerminalId, MC_OPF_RESULT_EXTENSION,
    };

    #[test]
    fn public_workflow_contract_two_solves_one_factorization() {
        let timer = std::time::Instant::now();
        let instance = demo_instance().unwrap();
        assert_eq!(instance.network().buses().len(), 4);
        let parameter = load_parameter(&instance).unwrap();
        let edits = demo_edits(&instance, &parameter);
        let mut session = McOpfSession::new(
            instance,
            McOpfOptions {
                max_iterations: 100,
                ..Default::default()
            },
        )
        .unwrap();
        let (summary, report) = workflow(&mut session, &edits, &parameter).unwrap();
        assert_eq!(summary["solves"], 2);
        assert_eq!(summary["compilations"], 2);
        assert_eq!(summary["revision"], 2);
        assert_eq!(summary["start"], json!(McOpfStartUsed::PreviousPrimal));
        assert_eq!(summary["without_duals"]["status"], "unavailable");
        assert_eq!(
            summary["without_duals"]["error"]["kind"],
            json!(McOpfSensitivityErrorKind::MissingDuals)
        );
        assert_eq!(
            summary["after_edit"]["kind"],
            json!(McOpfSensitivityErrorKind::StaleResult)
        );
        assert_eq!(
            summary["dual_status"],
            json!(McOpfMarginalStatus::Conditional)
        );
        for stage in ["before", "after"] {
            assert!(
                summary[stage]["maximum_normalized_residual"]
                    .as_f64()
                    .unwrap()
                    <= 1e-6
            );
            assert!(!summary[stage]["terminals"].as_array().unwrap().is_empty());
        }
        assert_ne!(
            summary["before"]["objective_currency_per_hour"],
            summary["after"]["objective_currency_per_hour"]
        );
        let sensitivities = &summary["sensitivities"];
        assert_eq!(sensitivities["status"], "available", "{sensitivities}");
        assert!(
            sensitivities["qualification"]["kkt_dimension"]
                .as_u64()
                .unwrap()
                < 600
        );
        for method in ["forward_solve", "adjoint_solve"] {
            assert!(
                sensitivities[method]["residual_tolerance_ratio"]
                    .as_f64()
                    .unwrap()
                    <= 1.
            );
        }
        let forward = sensitivities["forward"][0]["derivative"].as_f64().unwrap();
        let adjoint = sensitivities["adjoint"][0]["derivative"].as_f64().unwrap();
        assert!((forward - adjoint).abs() < 1e-9);
        assert_eq!(
            sensitivities["adjoint"][0]["parameter_unit"],
            json!(McOpfParameterUnit::Watt)
        );
        let current = &session.current_result().unwrap().result;
        assert_eq!(summary["after"]["fingerprint"], current.fingerprint);
        let marginal = current
            .duals
            .as_ref()
            .unwrap()
            .marginals
            .iter()
            .find(|m| m.parameter == parameter)
            .unwrap();
        assert!((forward - marginal.derivative.unwrap()).abs() < 1e-8);

        let mut saved: Value = serde_json::from_str(&report).unwrap();
        assert_eq!(saved["extensions"][MC_OPF_RESULT_EXTENSION]["version"], 9);
        saved["extensions"][MC_OPF_RESULT_EXTENSION]["version"] = json!(999);
        let error = McOpfResult::from_module_json(&saved.to_string()).unwrap_err();
        assert_eq!(error.stage, McOpfStage::Import);

        // Rejected batches leave the accepted revision and result untouched.
        let mut rejected = edits;
        rejected.push(McOpfEdit::LoadPower {
            load: "missing".into(),
            p_w: vec![1.],
            q_var: vec![0.],
        });
        assert!(session.apply_edits(&rejected).is_err());
        assert_eq!(session.revision(), 2);
        assert!(session.current_result().is_some());
        // A cancelled attempt performs no optimization and preserves history.
        let error = session
            .solve_cancellable(
                McOpfStart::LastAccepted,
                Some(Arc::new(AtomicBool::new(true))),
            )
            .unwrap_err();
        assert_eq!(error.kind, McOpfErrorKind::Cancelled);
        assert!(session.current_result().is_none());
        assert!(session.last_accepted().is_some());
        eprintln!(
            "public workflow: 2 solves, 1 factorization, {:.3}s",
            timer.elapsed().as_secs_f64()
        );
    }

    #[test]
    fn declared_input_and_edit_contract_without_solving() {
        let instance = demo_instance().unwrap();
        let mut constraints = instance.constraints().clone();
        constraints.conductor_limits = powerio_prob::ConstraintSelection::None;
        let point = powerio_prob::MulticonductorOperatingPointBuilder::for_point(
            instance.network().clone(),
        )
        .build_point()
        .unwrap();
        let instance = instance
            .with_constraints(constraints)
            .with_initial_point(point);
        let parameter = load_parameter(&instance).unwrap();
        let edits = demo_edits(&instance, &parameter);
        let encoded = serde_json::to_string(&edits).unwrap();
        let decoded: Vec<McOpfEdit> = serde_json::from_str(&encoded).unwrap();
        assert_eq!(
            serde_json::to_value(decoded).unwrap(),
            serde_json::to_value(edits).unwrap()
        );
        assert!(serde_json::from_str::<Vec<McOpfEdit>>(
            r#"[{"edit":"load_power","load":"d","p_w":[1],"q_var":[0],"typo":true}]"#
        )
        .is_err());
        let module = powerio::PioModule::new(PioValue::McAcOpfInstance(instance.clone()));
        let text = ir::serialize_module(&module).unwrap();
        let restored = declared_instance(&text).unwrap();
        assert_eq!(
            ir::serialize_module(&powerio::PioModule::new(PioValue::McAcOpfInstance(
                restored
            )))
            .unwrap(),
            text
        );
        let lossy = module
            .with_diagnostic(powerio::Diagnostic::new(
                powerio::DiagnosticCode::new("PARSE.TEST.FIELD_DROPPED").unwrap(),
                powerio::DiagnosticSeverity::Warning,
                "a physical field was dropped upstream",
            ))
            .unwrap();
        let error = declared_instance(&ir::serialize_module(&lossy).unwrap()).unwrap_err();
        let error = error.downcast_ref::<McOpfError>().unwrap();
        assert_eq!(error.kind, McOpfErrorKind::InvalidInput);
        assert!(!error.diagnostics.is_empty());
        let bare =
            powerio::PioModule::new(PioValue::MulticonductorNetwork(instance.network().clone()));
        assert!(declared_instance(&ir::serialize_module(&bare).unwrap()).is_err());
        assert!(declared_instance("{}").is_err());
    }

    #[test]
    fn selectors_use_canonical_identity_and_physical_units() {
        let parameter = McOpfParameter::new(
            McOpfParameterOwner::Load,
            "load with spaces",
            Some(2),
            McOpfParameterKind::LoadActivePower,
        );
        assert_eq!(parameter.unit(), McOpfParameterUnit::Watt);
        assert_eq!(
            serde_json::to_value(&parameter).unwrap(),
            json!({"owner":"load","component":"load with spaces","axis":2,"kind":"load_active_power"})
        );
        let terminal = McOpfTerminalId::new("bus with spaces", "neutral");
        assert_eq!(
            serde_json::to_value(terminal).unwrap(),
            json!({"bus":"bus with spaces","terminal":"neutral"})
        );
    }
}
