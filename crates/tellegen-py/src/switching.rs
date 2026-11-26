//! Native switching over PowerIO modules. The result document keeps the input's records.
use std::sync::Arc;

use powerio::{AcOtsInstance, AcOtsSolution, PioModule, PioValue};
use pyo3::prelude::*;
use serde_json::json;
use tellegen::ots::{restore_ac, solve_ots, OtsModel};

use super::{engine_error, Failure};

fn solution_ir(
    instance: PioModule<Arc<AcOtsInstance>>,
    solution: &AcOtsSolution,
) -> Result<String, Failure> {
    let module = instance.map_value(|ots| PioValue::AcOtsInstance(Arc::unwrap_or_clone(ots)));
    let result = powerio::ac_ots_solution(
        &module,
        solution.instance(),
        solution.branch_closed().to_vec(),
        solution.ac_solution().clone(),
    )
    .map_err(|error| Failure::new("MODULE.UNSUPPORTED", error.to_string()))?;
    Ok(tellegen::ir::serialize_module(&result)?)
}

#[pyfunction]
pub(super) fn solve_ots_module(
    py: Python<'_>,
    module_json: String,
    model: &str,
    topology_limit: usize,
    restore: bool,
) -> PyResult<String> {
    let model = match model {
        "lacots" => OtsModel::Lacots,
        "dcots" => OtsModel::Dcots,
        "acots" => OtsModel::Acots,
        _ => {
            return Err(engine_error(
                py,
                Failure::new("REQUEST.MALFORMED", "expected lacots, dcots or acots"),
            ))
        }
    };
    py.detach(move || -> Result<String, Failure> {
        if topology_limit == 0 {
            return Err(Failure::new("REQUEST.MALFORMED", "topology_limit must be positive"));
        }
        let instance = powerio::to_ac_ots_instance(&tellegen::ir::deserialize_module(&module_json)?)
            .map_err(|e| Failure::new("MODULE.UNSUPPORTED", e.to_string()))?
            .map_value(Arc::new);
        let ots = Arc::clone(instance.value());
        let result = solve_ots(Arc::clone(&ots), model, topology_limit)?;
        let mut report = serde_json::to_value(&result).map_err(|e| e.to_string())?;
        report["branch_ids"] = json!(ots.branch_order().collect::<Vec<_>>());
        report["ac_solution_ir"] = serde_json::Value::Null;
        report["restoration"] = serde_json::Value::Null;
        if let Some(plan) = &result.best {
            if let Some(solution) = &plan.ac_solution {
                report["ac_solution_ir"] = json!(solution_ir(instance, solution)?);
            } else if restore {
                match restore_ac(ots, plan) {
                    Ok(r) => {
                        report["ac_solution_ir"] = json!(solution_ir(instance, &r.solution)?);
                        report["restoration"] = json!({"status":"ac_feasible","objective":r.solution.objective(),
                            "residuals":r.solution.residuals(),"dispatch":r.dispatch,
                            "redispatch_l1_mw":r.redispatch_l1_mw,"warm_start_error":r.warm_start_error,
                            "initial_p_mismatch_mw":r.initial_p_mismatch_mw,"initial_q_mismatch_mvar":r.initial_q_mismatch_mvar});
                    }
                    Err(error) => report["restoration"] = json!({"status":"failed","error":error}),
                }
            }
        }
        Ok(report.to_string())
    }).map_err(|error| engine_error(py, error))
}

#[pyfunction]
pub(super) fn lacots_perspective_data(py: Python<'_>, module_json: String) -> PyResult<String> {
    py.detach(move || -> Result<String, Failure> {
        let module = tellegen::ir::deserialize_module(&module_json)?;
        let instance = powerio::to_ac_ots_instance(&module)
            .map_err(|error| Failure::new("MODULE.UNSUPPORTED", error.to_string()))?;
        tellegen::ots::lacots_perspective_data(instance.value())
            .map(|value| value.to_string())
            .map_err(|error| Failure::new("MODULE.UNSUPPORTED", error))
    })
    .map_err(|error| engine_error(py, error))
}
