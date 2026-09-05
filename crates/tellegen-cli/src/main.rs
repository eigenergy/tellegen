//! `tellegen` — a thin CLI over the engine's JSON contracts, for
//! reproducible solves, capacity proposals, parity checks, and scripting.
//!
//! ```text
//! tellegen capabilities                       # the support matrix
//! tellegen contract                           # versioned machine contract
//! tellegen < case.pio.json                    # base-case DC OPF response
//! tellegen '{"formulation":"socwr"}' < case.pio.json
//! tellegen solve-module < case.pio.json       # stored module in, solution module out
//! tellegen plan < plan-request.json          # capacity planning proposal
//! ```
//!
//! `solve-module` and `plan` are the headless MCP boundary: a stored
//! generation 2 `pio-ir` document on stdin, and a stored
//! DC OPF solution module — nodal values and thermal multipliers attached —
//! or a capacity proposal and its exact proposed solution on stdout. A module holding a
//! typed `dc_opf_instance` is consumed natively; a balanced network becomes
//! the default instance first. Any other value kind is refused by name.
//!
//! This is the stateless face of the engine; interactive, build-once
//! workflows use the `Study` API in the `tellegen` crate.

use std::io::Read;
use std::process::ExitCode;
use std::sync::Arc;

use powerio::DcOpfInstance;
use powerio::{PioModule, PioValue};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tellegen::ir::{balanced_module, deserialize_module, serialize_module};

use tellegen::plan::CapacityPlanSpec;

const USAGE: &str =
    "usage: tellegen [REQUEST_JSON | capabilities | --help]   (PowerIO module on stdin)\n\
     \n\
     REQUEST_JSON  a solve request; default '{}' is a base-case DC OPF.\n\
     capabilities  print the formulation/operand/parameter capability matrix.\n\
     contract      print the versioned CLI contract and generated JSON Schemas.\n\
     solve-module\n\
                   read a stored PowerIO module on stdin and\n\
                   print the solved dc_opf_solution stored module.\n\
     study create|inspect|run|export|import PATH\n\
                   create, inspect, continue or move a durable Study.\n\
     plan\n\
                   read {\"module\": POWERIO_MODULE, \"spec\": CAPACITY_PLAN_SPEC}\n\
                   from stdin and print the proposal and exact proposed solution.";

fn main() -> ExitCode {
    let arg = std::env::args().nth(1).unwrap_or_default();

    match arg.as_str() {
        "capabilities" => {
            println!("{}", tellegen::capabilities_json());
            return ExitCode::SUCCESS;
        }
        "contract" => return run(contract_json),
        "study" => return run(study_command),
        "-h" | "--help" => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        "solve-module" => {
            if std::env::args().nth(2).is_some() {
                eprintln!(
                    "tellegen solve-module: this command accepts no request argument\n\n{USAGE}"
                );
                return ExitCode::FAILURE;
            }
            return run(solve_module);
        }
        "plan" => {
            if std::env::args().nth(2).is_some() {
                eprintln!("tellegen plan: the planning request belongs on stdin\n\n{USAGE}");
                return ExitCode::FAILURE;
            }
            return run(plan_from_stdin);
        }
        _ => {}
    }

    let request = if arg.is_empty() { "{}" } else { arg.as_str() };
    run(|| {
        let module_json = read_stdin()?;
        tellegen::solve_module_json(&module_json, request)
    })
}

fn study_command() -> Result<String, String> {
    use tellegen::document::StudyBundle;
    use tellegen::study_ops::{create_study, execute_study, CreateStudy, StudyRequest};
    use tellegen::study_storage::FileStudyStore;
    let args = std::env::args().skip(2).collect::<Vec<_>>();
    let progress = args.len() == 3 && args[0] == "run" && args[2] == "--progress";
    if args.len() != 2 && !progress {
        return Err("usage: tellegen study create|inspect|run|export|import PATH [--progress for run] (JSON request on stdin for create/run/import)".into());
    }
    let store = FileStudyStore::new(&args[1]);
    match args[0].as_str() {
        "create" => {
            let request: CreateStudy =
                serde_json::from_str(&read_stdin()?).map_err(|e| e.to_string())?;
            let bundle = create_study(request)?;
            store.create(&bundle)?;
            serde_json::to_string(&bundle.summary(8)).map_err(|e| e.to_string())
        }
        "inspect" => serde_json::to_string(&store.load()?.summary(8)).map_err(|e| e.to_string()),
        "export" => store.load()?.export(),
        "import" => {
            let bundle = StudyBundle::import(&read_stdin()?)?;
            store.create(&bundle)?;
            serde_json::to_string(&bundle.summary(8)).map_err(|e| e.to_string())
        }
        "run" => {
            let request: StudyRequest =
                serde_json::from_str(&read_stdin()?).map_err(|e| e.to_string())?;
            let expected = request.expected_revision;
            let mut bundle = store.load()?;
            let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let signal = cancelled.clone();
            ctrlc::set_handler(move || signal.store(true, std::sync::atomic::Ordering::Relaxed))
                .map_err(|error| format!("cannot install Study cancellation handler: {error}"))?;
            let mut checkpoint = 0usize;
            let result = execute_study(&mut bundle, request, || {
                checkpoint += 1;
                if progress {
                    eprintln!(
                        "{}",
                        serde_json::json!({ "event": "study_checkpoint", "index": checkpoint })
                    );
                }
                cancelled.load(std::sync::atomic::Ordering::Relaxed)
            })?;
            store.commit(expected, &bundle)?;
            serde_json::to_string(&result).map_err(|e| e.to_string())
        }
        _ => Err("unknown Study command; use create, inspect, run, export or import".into()),
    }
}

fn run(operation: impl FnOnce() -> Result<String, String>) -> ExitCode {
    match operation() {
        Ok(out) => {
            println!("{out}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("tellegen: {e}");
            ExitCode::FAILURE
        }
    }
}

fn read_stdin() -> Result<String, String> {
    let mut text = String::new();
    std::io::stdin()
        .read_to_string(&mut text)
        .map_err(|e| format!("failed to read stdin: {e}"))?;
    if text.trim().is_empty() {
        return Err(format!("no input on stdin\n\n{USAGE}"));
    }
    Ok(text)
}

/// The producer identity every emitted artifact carries.
fn producer_string() -> String {
    format!(
        "tellegen {} (b-theta, kkt-implicit)",
        env!("CARGO_PKG_VERSION")
    )
}

fn instance_from_module_json(text: &str) -> Result<PioModule<DcOpfInstance>, String> {
    let module = deserialize_module(text)?;
    match module.value() {
        PioValue::DcOpfInstance(_) => module.try_map_value(|value| match value {
            PioValue::DcOpfInstance(instance) => Ok(instance),
            other => Err(other.type_name().to_owned()),
        }),
        PioValue::BalancedNetwork(_) => balanced_module(module)?
            .try_map_value(DcOpfInstance::from_network)
            .map_err(|e| e.to_string()),
        other => Err(format!(
            "the module holds a {} value; solve-module and plan accept \
             powerio.DcOpfInstance or powerio.BalancedNetwork",
            other.type_name()
        )),
    }
}

fn solution_module_json(
    source_module: PioModule<DcOpfInstance>,
    solution: powerio::DcOpfSolution,
) -> Result<String, String> {
    let mut module = source_module
        .map_value(|_| PioValue::DcOpfSolution(solution))
        .sever_source()
        .with_producer(
            powerio::Producer::new("tellegen", env!("CARGO_PKG_VERSION"))
                .map_err(|e| e.to_string())?,
        );
    module.sever_value_targets();
    serialize_module(&module)
}

fn solve_module() -> Result<String, String> {
    solve_module_text(&read_stdin()?)
}

/// Read one stored PowerIO document through the universal reader, then solve
/// its DC OPF instance or materialize the default instance for a network.
fn solve_module_text(text: &str) -> Result<String, String> {
    let source_module = instance_from_module_json(text)?;
    let instance = Arc::new(source_module.value().clone());
    let solution = tellegen::solve_dc_opf_instance(instance, producer_string())?;
    solution_module_json(source_module, solution)
}

/// A PowerIO IR document, carried as JSON and described by PowerIO's own
/// schema in the contract.
type PowerIoModule = serde_json::Value;
type CapabilitiesResponse = Vec<tellegen::ProblemCaps>;

/// The JSON Schema of a PowerIO IR document.
fn ir_schema() -> Result<serde_json::Value, String> {
    serde_json::to_value(powerio::generate_ir_schema()).map_err(|error| error.to_string())
}

fn ir_field_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    powerio::generate_ir_schema()
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PlanRequest {
    #[schemars(schema_with = "ir_field_schema")]
    module: PowerIoModule,
    spec: CapacityPlanSpec,
}

fn parse_plan_request(text: &str) -> Result<(String, CapacityPlanSpec), String> {
    let request: PlanRequest =
        serde_json::from_str(text).map_err(|e| format!("unreadable planning request: {e}"))?;
    let module_json =
        serde_json::to_string(&request.module).map_err(|e| format!("unreadable module: {e}"))?;
    Ok((module_json, request.spec))
}

fn plan_from_stdin() -> Result<String, String> {
    plan_from_text(&read_stdin()?)
}

fn plan_from_text(text: &str) -> Result<String, String> {
    let (module_json, spec) = parse_plan_request(text)?;
    let source_module = instance_from_module_json(&module_json)?;
    let instance = Arc::new(source_module.value().clone());
    let execution = tellegen::plan::plan_capacity(instance, &spec)?;
    let (outcome, solution) = execution.into_solution(producer_string())?;
    let solution_module = solution_module_json(source_module, solution)?;

    let out = PlanResponse {
        plan: outcome,
        solution_module: serde_json::from_str(&solution_module).map_err(|e| e.to_string())?,
    };
    serde_json::to_string(&out).map_err(|e| e.to_string())
}

const CONTRACT_ID: &str = "tellegen.cli/1";

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PlanResponse {
    plan: tellegen::plan::CapacityPlanOutcome,
    #[schemars(schema_with = "ir_field_schema")]
    solution_module: PowerIoModule,
}

fn contract_value() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "contract": CONTRACT_ID,
        "tellegen_version": tellegen::VERSION,
        "powerio_version": env!("TELLEGEN_POWERIO_VERSION"),
        "schemas": {
            "study_bundle": schemars::schema_for!(tellegen::document::StudyBundle),
            "study_create": schemars::schema_for!(tellegen::study_ops::CreateStudy),
            "study_request": schemars::schema_for!(tellegen::study_ops::StudyRequest),
            "study_result": schemars::schema_for!(tellegen::study_ops::StudyOperationResult),

            "powerio_module": ir_schema()?,
            "capacity_plan_spec": serde_json::to_value(schemars::schema_for!(CapacityPlanSpec))
                .map_err(|error| error.to_string())?,
            "plan_request": serde_json::to_value(schemars::schema_for!(PlanRequest))
                .map_err(|error| error.to_string())?,
            "plan_response": serde_json::to_value(schemars::schema_for!(PlanResponse))
                .map_err(|error| error.to_string())?,
            "solve_response": ir_schema()?,
            "capabilities_response":
                serde_json::to_value(schemars::schema_for!(CapabilitiesResponse))
                    .map_err(|error| error.to_string())?,
        }
    }))
}

fn contract_json() -> Result<String, String> {
    serde_json::to_string(&contract_value()?).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_CASE: &str = r#"function mpc = case2_cli
mpc.version = '2';
mpc.baseMVA = 100;
mpc.bus = [
  1 3  0 0 0 0 1 1 0 230 1 1.1 0.9;
  2 1 50 0 0 0 1 1 0 230 1 1.1 0.9;
];
mpc.gen = [
  1 50 0 100 -100 1 100 1 200 0;
];
mpc.branch = [
  1 2 0 0.1 0 100 100 100 0 0 1 -360 360;
];
mpc.gencost = [
  2 0 0 3 0.01 10 0;
];
"#;

    fn test_module_json() -> String {
        let source = powerio::Source::from_memory("case2_cli.m", TEST_CASE.as_bytes().to_vec())
            .expect("case source");
        let options = powerio::ParseOptions::default()
            .format("matpower")
            .expect("MATPOWER format");
        let module = powerio::parse_with_options(source, &options).expect("parse case");
        serialize_module(&module).expect("emit test module")
    }

    fn test_plan_spec() -> CapacityPlanSpec {
        serde_json::from_value(serde_json::json!({
            "objective": {
                "kind": "weighted_lmp",
                "weights": [{"bus": 2, "weight": 1.0}]
            },
            "candidates": ["1-2"],
            "max_increase_per_branch_mw": 5.0,
            "budget_mw": 5.0,
            "increment_mw": 5.0,
            "max_changed_lines": 1,
            "exact_solve_budget": 1
        }))
        .expect("plan spec")
    }

    fn test_plan_request(candidates: Vec<String>) -> serde_json::Value {
        serde_json::json!({
            "module": serde_json::from_str::<PowerIoModule>(&test_module_json())
                .expect("stored test module"),
            "spec": {
                "objective": {
                    "kind": "weighted_lmp",
                    "weights": [{"bus": 1, "weight": 1.0}]
                },
                "candidates": candidates,
                "max_increase_per_branch_mw": 10.0,
                "budget_mw": 10.0,
                "increment_mw": 5.0,
                "max_changed_lines": 1,
                "exact_solve_budget": 2
            }
        })
    }

    #[test]
    fn planning_request_accepts_a_candidate_list_larger_than_windows_argv() {
        let candidates: Vec<String> = (0..5_000)
            .map(|index| format!("branches:{index}"))
            .collect();
        let request = test_plan_request(candidates);
        let text = serde_json::to_string(&request).expect("request JSON");
        assert!(
            text.len() > 32_767,
            "fixture must exceed the Windows argv limit"
        );

        let (_, spec) = parse_plan_request(&text).expect("stdin planning request");
        assert_eq!(spec.candidates.len(), 5_000);
        assert_eq!(spec.candidates[4_999], "branches:4999");
    }

    #[test]
    fn runtime_outputs_match_each_named_contract_type() {
        let module_json = test_module_json();
        let module: PowerIoModule =
            serde_json::from_str(&module_json).expect("powerio_module runtime value");

        let capabilities_json = tellegen::capabilities_json();
        let capabilities: CapabilitiesResponse =
            serde_json::from_str(&capabilities_json).expect("capabilities_response runtime value");
        assert!(!capabilities.is_empty());

        let request = PlanRequest {
            module,
            spec: test_plan_spec(),
        };
        let request_json = serde_json::to_string(&request).expect("plan_request runtime value");
        let _: PlanRequest =
            serde_json::from_str(&request_json).expect("typed plan_request runtime value");

        let plan_json = plan_from_text(&request_json).expect("plan response");
        let plan: PlanResponse =
            serde_json::from_str(&plan_json).expect("plan_response runtime value");
        assert_eq!(plan.plan.exact_solves, 1);
        let plan_module_json =
            serde_json::to_string(&plan.solution_module).expect("plan solution module");
        assert_eq!(
            deserialize_module(&plan_module_json)
                .expect("read plan solution")
                .value()
                .type_name(),
            "powerio.DcOpfSolution"
        );

        let solve_json = solve_module_text(&module_json).expect("solve response");
        let solve: PowerIoModule =
            serde_json::from_str(&solve_json).expect("solve_response runtime value");
        let solve_module_json = serde_json::to_string(&solve).expect("solve solution module");
        assert_eq!(
            deserialize_module(&solve_module_json)
                .expect("read solve solution")
                .value()
                .type_name(),
            "powerio.DcOpfSolution"
        );
    }

    #[test]
    fn contract_schema_matches_the_runtime_plan_envelope() {
        let contract = contract_value().expect("contract");
        assert_eq!(contract["contract"], CONTRACT_ID);
        assert_eq!(contract["tellegen_version"], tellegen::VERSION);
        assert_eq!(
            contract["powerio_version"],
            env!("TELLEGEN_POWERIO_VERSION")
        );

        let spec_properties = contract["schemas"]["capacity_plan_spec"]["properties"]
            .as_object()
            .expect("CapacityPlanSpec properties");
        let expected_spec = [
            "objective",
            "candidates",
            "max_increase_per_branch_mw",
            "budget_mw",
            "increment_mw",
            "max_changed_lines",
            "exact_solve_budget",
        ];
        assert_eq!(spec_properties.len(), expected_spec.len());
        for field in expected_spec {
            assert!(spec_properties.contains_key(field), "missing {field}");
        }

        let module_properties = contract["schemas"]["powerio_module"]["properties"]
            .as_object()
            .expect("IR document properties");
        for field in ["schema", "version", "producer", "value"] {
            assert!(module_properties.contains_key(field), "missing {field}");
        }

        let request_properties = contract["schemas"]["plan_request"]["properties"]
            .as_object()
            .expect("PlanRequest properties");
        assert_eq!(request_properties.len(), 2);
        assert!(request_properties.contains_key("module"));
        assert!(request_properties.contains_key("spec"));

        let response_properties = contract["schemas"]["plan_response"]["properties"]
            .as_object()
            .expect("PlanResponse properties");
        assert_eq!(response_properties.len(), 2);
        assert!(response_properties.contains_key("plan"));
        assert!(response_properties.contains_key("solution_module"));

        assert_eq!(
            contract["schemas"]["solve_response"],
            contract["schemas"]["powerio_module"]
        );
        assert_eq!(
            contract["schemas"]["capabilities_response"]["type"],
            "array"
        );
    }
}
