//! Tests for what crosses the engine boundary beside the numbers: the kind of
//! objective an OPF solved, and the code each error carries.

use super::super::model::CASE3;
use super::*;
use serde_json::Value;

fn case3() -> BalancedNetwork {
    crate::model::parse_matpower(CASE3).expect("parse")
}

fn module_json(value: powerio::PioValue) -> String {
    crate::ir::serialize_module(&powerio::PioModule::new(value)).expect("module JSON")
}

fn case3_module() -> String {
    module_json(powerio::PioValue::BalancedNetwork(case3()))
}

fn feasibility_dc_module() -> String {
    let instance = DcOpfInstance::from_network(case3())
        .expect("instance")
        .with_objective(powerio_prob::Objective::none());
    module_json(powerio::PioValue::DcOpfInstance(instance))
}

fn solved(module: &str, request: &str) -> Value {
    let text = solve_module_json(module, request).expect("solve");
    serde_json::from_str(&text).expect("response JSON")
}

fn failure(module: &str, request: &str) -> EngineError {
    let error = solve_module_output_json(module, request).expect_err("must fail");
    // The older entry point returns the same text with no code.
    let plain = solve_module_json(module, request).expect_err("must fail");
    assert_eq!(plain, error.message);
    assert_eq!(error.to_string(), plain);
    error
}

#[test]
fn dc_opf_reports_cost_for_a_case_with_generator_costs() {
    let response = solved(&case3_module(), "");
    assert_eq!(response["formulation"], "dcopf");
    assert_eq!(response["objective_kind"], "cost");
    assert!(response["objective"].as_f64().unwrap() > 0.0);
    assert!(response["lmp"].is_array());
}

#[test]
fn dc_opf_reports_feasibility_when_the_instance_declares_no_cost() {
    let response = solved(&feasibility_dc_module(), "");
    assert_eq!(response["objective_kind"], "feasibility");
    assert_eq!(response["objective"], 0.0);
    assert!(response["lmp"].is_null());
}

#[test]
fn the_declared_kind_agrees_with_the_prepared_objective() {
    let cost = DcOpfInstance::from_network(case3()).expect("instance");
    let none = cost.clone().with_objective(powerio_prob::Objective::none());
    for (instance, kind) in [
        (cost, ObjectiveKind::Cost),
        (none, ObjectiveKind::Feasibility),
    ] {
        assert_eq!(ObjectiveKind::declared(instance.objective()), Some(kind));
        let model = DcNetwork::from_instance(&instance).expect("model");
        assert_eq!(ObjectiveKind::of(model.objective), Some(kind));
        let output = solve_instance_output(&instance, &SolveRequest::default()).expect("solve");
        assert_eq!(output.objective_kind, Some(kind));
        assert_eq!(kind.as_str(), serde_json::to_value(kind).unwrap());
    }
}

#[cfg(feature = "sensitivity")]
#[test]
fn power_flows_report_no_objective_kind() {
    for formulation in ["dcpf", "acpf"] {
        let response = solved(
            &case3_module(),
            &format!(r#"{{"formulation":"{formulation}"}}"#),
        );
        assert_eq!(response["formulation"], formulation);
        assert!(response.get("objective_kind").is_none(), "{formulation}");
    }
}

#[cfg(feature = "conic")]
#[test]
fn socwr_reports_the_objective_kind() {
    let response = solved(&case3_module(), r#"{"formulation":"socwr"}"#);
    assert_eq!(response["objective_kind"], "cost");

    let instance = AcOpfInstance::from_network(case3())
        .expect("instance")
        .with_objective(powerio_prob::Objective::none());
    let module = module_json(powerio::PioValue::AcOpfInstance(instance));
    let response = solved(&module, r#"{"formulation":"socwr"}"#);
    assert_eq!(response["objective_kind"], "feasibility");
    assert!(response["lmp"].is_null());
}

#[cfg(feature = "acopf")]
#[test]
fn ac_opf_reports_the_objective_kind() {
    let response = solved(&case3_module(), r#"{"formulation":"acopf"}"#);
    assert_eq!(response["objective_kind"], "cost");

    let instance = AcOpfInstance::from_network(case3())
        .expect("instance")
        .with_objective(powerio_prob::Objective::none());
    let module = module_json(powerio::PioValue::AcOpfInstance(instance));
    let response = solved(&module, r#"{"formulation":"acopf"}"#);
    assert_eq!(response["objective_kind"], "feasibility");
}

#[test]
fn a_solve_output_round_trips_and_reads_older_responses() {
    let text = solve_module_json(&case3_module(), "").expect("solve");
    let output: SolveOutput = serde_json::from_str(&text).expect("output");
    assert_eq!(output.objective_kind, Some(ObjectiveKind::Cost));
    let again: Value = serde_json::to_value(&output).expect("value");
    assert_eq!(again, serde_json::from_str::<Value>(&text).unwrap());

    // A response written before the field existed has no `objective_kind`,
    // and a reader of the plain response ignores the new field.
    let mut older = again.clone();
    older.as_object_mut().unwrap().remove("objective_kind");
    let read: SolveOutput = serde_json::from_value(older).expect("older output");
    assert_eq!(read.objective_kind, None);
    let plain: SolveResponse = serde_json::from_value(again).expect("plain response");
    assert_eq!(plain.objective, output.response.objective);
}

#[cfg(feature = "sensitivity")]
#[test]
fn a_study_reports_the_objective_kind_of_its_current_case() {
    let cost = crate::Study::new(&case3_module(), Problem::DcOpf).expect("study");
    assert_eq!(cost.objective_kind(), Some(ObjectiveKind::Cost));
    let output = serde_json::to_value(cost.solution_output()).expect("value");
    assert_eq!(output["objective_kind"], "cost");
    assert_eq!(output["formulation"], "dcopf");

    let feasibility = crate::Study::new(&feasibility_dc_module(), Problem::DcOpf).expect("study");
    assert_eq!(
        feasibility.objective_kind(),
        Some(ObjectiveKind::Feasibility)
    );

    let flow = crate::Study::new(&case3_module(), Problem::AcPf).expect("study");
    assert_eq!(flow.objective_kind(), None);
    let output = serde_json::to_value(flow.solution_output()).expect("value");
    assert!(output.get("objective_kind").is_none());
}

#[test]
fn input_and_request_errors_carry_their_codes() {
    let module = case3_module();
    let cases = [
        (
            "not a module".to_owned(),
            String::new(),
            ErrorCode::InvalidInput,
        ),
        (
            module.clone(),
            "{not json".to_owned(),
            ErrorCode::InvalidRequest,
        ),
        (
            module.clone(),
            r#"{"options":{"shed":true}}"#.to_owned(),
            ErrorCode::InvalidRequest,
        ),
        (
            module.clone(),
            r#"{"edits":{"deltas":{"99999":10.0}}}"#.to_owned(),
            ErrorCode::UnknownElement,
        ),
        (
            module.clone(),
            r#"{"edits":{"deltas":{"2":-1e9}}}"#.to_owned(),
            ErrorCode::OutOfRange,
        ),
        (
            module.clone(),
            r#"{"edits":{"rates":{"1":-1e9}}}"#.to_owned(),
            ErrorCode::OutOfRange,
        ),
        (
            module.clone(),
            r#"{"start":"case"}"#.to_owned(),
            ErrorCode::Unsupported,
        ),
        (
            module.clone(),
            r#"{"limits":{"thermal":{"branches":[99]}}}"#.to_owned(),
            ErrorCode::UnknownElement,
        ),
        (
            module.clone(),
            r#"{"limits":{"lazy":{"max_rounds":0}}}"#.to_owned(),
            ErrorCode::OutOfRange,
        ),
        (
            feasibility_dc_module(),
            r#"{"formulation":"socwr"}"#.to_owned(),
            ErrorCode::Unsupported,
        ),
    ];
    for (module, request, code) in cases {
        let error = failure(&module, &request);
        assert_eq!(error.code, code, "{request}: {error}");
    }
}

#[cfg(feature = "sensitivity")]
#[test]
fn power_flow_requests_the_formulation_cannot_honor_are_unsupported() {
    let module = case3_module();
    for request in [
        r#"{"formulation":"dcpf","edits":{"rates":{"1":5.0}}}"#,
        r#"{"formulation":"acpf","edits":{"rates":{"1":5.0}}}"#,
        r#"{"formulation":"acpf","limits":{}}"#,
        r#"{"formulation":"acpf","sensitivities":[{"operand":{"Price":"Active"},"parameter":{"Demand":"Active"}}]}"#,
    ] {
        let error = failure(&module, request);
        assert_eq!(error.code, ErrorCode::Unsupported, "{request}: {error}");
    }
}

#[cfg(feature = "sensitivity")]
#[test]
fn dc_power_flow_refuses_a_sensitivity_request() {
    let module = case3_module();
    let request = r#"{"formulation":"dcpf","sensitivities":[{"operand":{"Flow":{"power":"Active","end":"From"}},"parameter":{"Demand":"Active"}}]}"#;
    let error = failure(&module, request);
    assert_eq!(error.code, ErrorCode::Unsupported);
    assert!(
        error
            .message
            .contains("sensitivities are not supported by dcpf"),
        "{error}"
    );
    // Without the request the same power flow solves and lists none.
    let response = solved(&module, r#"{"formulation":"dcpf"}"#);
    assert!(response.get("sensitivities").is_none());
}

#[test]
fn solver_failures_carry_their_codes() {
    // Generation capacity below the load: no feasible dispatch.
    let mut network = case3();
    for generator in network.generators_mut() {
        generator.pmax = 40.0;
    }
    let instance = DcOpfInstance::from_network(network).expect("instance");
    let error = solve_instance_output(&instance, &SolveRequest::default()).expect_err("infeasible");
    assert_eq!(error.code, ErrorCode::Infeasible);
    assert!(error.message.contains("solve infeasible"), "{error}");
    assert_eq!(
        solve_instance(&instance, &SolveRequest::default()).unwrap_err(),
        error.message
    );

    let cancel = Arc::new(AtomicBool::new(true));
    let error = solve_module_output_json_cancellable(&case3_module(), "", Some(cancel))
        .expect_err("cancelled");
    assert_eq!(error.code, ErrorCode::Cancelled);
    assert_eq!(error.message, "calculation cancelled");
}

#[test]
fn a_typed_code_matches_the_code_read_from_its_message() {
    // Callers of the string entry points get the code from
    // `ErrorCode::from_message`; it must agree with the typed one for the
    // failures both can name.
    let module = case3_module();
    for request in [
        "{not json",
        r#"{"edits":{"deltas":{"99999":10.0}}}"#,
        r#"{"edits":{"deltas":{"2":-1e9}}}"#,
        r#"{"start":"case"}"#,
    ] {
        let error = failure(&module, request);
        assert_eq!(
            ErrorCode::from_message(&error.message),
            Some(error.code),
            "{error}"
        );
    }
}

#[cfg(feature = "sensitivity")]
#[test]
fn a_solution_from_another_model_is_refused_by_the_dc_sensitivity_system() {
    let text = CASE3
        .replace(" 2 0 0 3 0.11  5   0;", " 1 0 0 3 0 0 50 5 250 405;")
        .replace(" 2 0 0 3 0.085 1.2 0;", " 1 0 0 2 0 0 270 135;");
    let piecewise = DcNetwork::from_network(&crate::model::parse_matpower(&text).expect("parse"))
        .expect("piecewise model");
    let plain = DcNetwork::from_network(&case3()).expect("model");
    let plain_solution = dc_opf_cancellable(&plain, None).expect("solve");
    let piecewise_solution = dc_opf_cancellable(&piecewise, None).expect("solve");

    assert!(crate::sens::DcKkt::new(&piecewise, &piecewise_solution).is_ok());
    let error = match crate::sens::DcKkt::new(&piecewise, &plain_solution) {
        Ok(_) => panic!("a solution without piecewise values must be refused"),
        Err(error) => error,
    };
    assert!(
        error.to_string().contains("piecewise cost values"),
        "{error}"
    );
}
