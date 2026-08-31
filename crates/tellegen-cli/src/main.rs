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
//! `powerio.module/1` document on stdin, and a stored
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

use powerio::{stored, try_into_typed, PioModule, PioValue};
use powerio_prob::DcOpfInstance;
use schemars::JsonSchema;
use serde::Serialize;

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

/// Read stdin into the typed DC OPF instance: a stored module holding a
/// `dc_opf_instance` is consumed natively, one holding a `balanced_network`
/// becomes the default instance. Any other kind is
/// refused by name — never silently reinterpreted.
fn instance_from_stdin() -> Result<PioModule<DcOpfInstance>, String> {
    let text = read_stdin()?;
    instance_from_module_json(&text)
}

fn instance_from_module_json(text: &str) -> Result<PioModule<DcOpfInstance>, String> {
    let module = stored::read_module(text).map_err(|e| e.to_string())?;
    match module.value().kind().as_str() {
        "dc_opf_instance" => try_into_typed(module).map_err(|m| m.actual().as_str().to_owned()),
        "balanced_network" => {
            let module: PioModule<powerio::BalancedNetwork> =
                try_into_typed(module).map_err(|m| m.actual().as_str().to_owned())?;
            module
                .try_map_value(DcOpfInstance::from_network)
                .map_err(|e| e.to_string())
        }
        other => Err(format!(
            "the module holds a {other} value; solve-module and plan accept \
             dc_opf_instance or balanced_network"
        )),
    }
}

fn solution_module_json(
    source_module: PioModule<DcOpfInstance>,
    solution: powerio_prob::DcOpfSolution,
) -> Result<String, String> {
    let mut module = source_module
        .map_value(|_| PioValue::DcOpfSolution(solution))
        .sever_source()
        .with_producer(
            powerio::Producer::new("tellegen", env!("CARGO_PKG_VERSION"))
                .map_err(|e| e.to_string())?,
        );
    module.sever_value_targets();
    stored::write_module(&module).map_err(|e| e.to_string())
}

fn solve_module() -> Result<String, String> {
    let source_module = instance_from_stdin()?;
    let instance = Arc::new(source_module.value().clone());
    let solution = tellegen::solve_dc_opf_instance(instance, producer_string())?;
    solution_module_json(source_module, solution)
}

#[derive(serde::Deserialize)]
struct PlanRequest {
    module: serde_json::Value,
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
    let text = read_stdin()?;
    let (module_json, spec) = parse_plan_request(&text)?;
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

#[derive(Serialize, JsonSchema)]
struct PlanResponse {
    plan: tellegen::plan::CapacityPlanOutcome,
    solution_module: powerio::stored::StoredModuleV1,
}

fn contract_value() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "contract": CONTRACT_ID,
        "tellegen_version": tellegen::VERSION,
        "powerio_version": env!("TELLEGEN_POWERIO_VERSION"),
        "schemas": {
            "capacity_plan_spec": serde_json::to_value(schemars::schema_for!(CapacityPlanSpec))
                .map_err(|error| error.to_string())?,
            "plan_response": serde_json::to_value(schemars::schema_for!(PlanResponse))
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

    #[test]
    fn planning_request_accepts_a_candidate_list_larger_than_windows_argv() {
        let candidates: Vec<String> = (0..5_000)
            .map(|index| format!("branches:{index}"))
            .collect();
        let request = serde_json::json!({
            "module": {"schema": "powerio.module", "version": 1},
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
        });
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

        let response_properties = contract["schemas"]["plan_response"]["properties"]
            .as_object()
            .expect("PlanResponse properties");
        assert_eq!(response_properties.len(), 2);
        assert!(response_properties.contains_key("plan"));
        assert!(response_properties.contains_key("solution_module"));
    }
}
