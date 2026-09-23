//! Python bindings for the tellegen engine.
//!
//! The seam is JSON in, JSON out, mirroring `crates/tellegen-wasm` almost line
//! for line. That is not a shortcut: several engine enums (`NetworkEdit`,
//! `Operand`, `Parameter`, `SensitivityMatrix`) are `#[non_exhaustive]`, so a
//! crate outside `tellegen` cannot build them with struct literals even from
//! inside this workspace, and `SolveRequest`/`Edits`/`SensRequest` derive
//! `Deserialize` only. Serde is the only available boundary, and it is the one
//! the browser already proves.
//!
//! Every entry releases the GIL around engine work (`Python::detach`, which is
//! what pyo3 0.29 calls the old `allow_threads`). The engine is single
//! threaded, allocation local and free of global mutable state, so a solve is
//! safe to run with the interpreter unlocked.

use powerio::Source;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

pyo3::create_exception!(
    tellegen,
    TellegenError,
    PyValueError,
    "Base error raised by the tellegen engine. Carries a `.code` string naming \
     the failure class, so a caller can branch without matching on message text."
);
pyo3::create_exception!(
    tellegen,
    TellegenInputError,
    TellegenError,
    "A malformed request, an unknown bus or branch, or an edit outside the \
     engine's invariants."
);
pyo3::create_exception!(
    tellegen,
    TellegenSolveError,
    TellegenError,
    "The solver ran and did not produce an answer: infeasible, unbounded, \
     non-convergent, or cancelled."
);

/// The code attached to a panic that crossed the boundary. PyO3 raises
/// `PanicException`, a `BaseException` that escapes `except Exception`, so the
/// Python layer converts it to a `TellegenError` carrying this code.
const PANIC_CODE: &str = "BIND.PY.PANIC";

/// Recent experiments carried in a Study summary. Matches the CLI's `summary(8)`
/// so both hosts return the same shape.
const STUDY_SUMMARY_LIMIT: usize = 8;

/// Classify an engine error string into a stable code.
///
/// The engine returns free-form `Result<_, String>` with no error enum at its
/// edge, so this matches the message substrings its tests pin. That is fragile
/// by construction and the reason a typed error enum is worth adding upstream;
/// until then, this is the single place the fragility lives, and every arm is
/// covered by a test below.
fn classify(message: &str) -> &'static str {
    // Order matters: the solve-status arms are checked before the generic
    // "bad request" arm because a cancelled solve also mentions its request.
    if message.contains("solve infeasible") {
        "SOLVE.INFEASIBLE"
    } else if message.contains("solve unbounded") {
        "SOLVE.UNBOUNDED"
    } else if message.contains("did not converge") {
        "SOLVE.NOT_CONVERGED"
    } else if message.contains("cancelled") {
        "SOLVE.CANCELLED"
    } else if message.contains("bad request JSON") || message.contains("unknown field") {
        "REQUEST.MALFORMED"
    } else if message.contains("unknown demand delta bus")
        || message.contains("unknown rating delta branch")
    {
        "EDIT.UNKNOWN_ELEMENT"
    } else if message.contains("would make demand negative")
        || message.contains("would make the line limit non-positive")
        || message.contains("rating edits are not supported")
    {
        "EDIT.OUT_OF_RANGE"
    } else if message.contains("does not support d(") {
        "SENS.UNSUPPORTED"
    } else if message.contains("is not available in this build")
        || message.contains("requires the `conic` feature")
        || message.contains("requires the `sensitivity` feature")
    {
        "FORMULATION.UNAVAILABLE"
    } else if message.contains("which this solve entry does not support")
        || message.contains("duplicate bus uid")
        || message.contains("ambiguous with a numeric element id")
    {
        "MODULE.UNSUPPORTED"
    } else if message.contains("unknown PowerIO format") {
        "FORMAT.UNKNOWN"
    } else {
        "ENGINE.FAILED"
    }
}

/// Turn an engine error string into the matching Python exception, with `.code`
/// set on the instance.
fn engine_error(py: Python<'_>, message: String) -> PyErr {
    let code = classify(&message);
    let err = match code {
        "SOLVE.INFEASIBLE" | "SOLVE.UNBOUNDED" | "SOLVE.NOT_CONVERGED" | "SOLVE.CANCELLED" => {
            TellegenSolveError::new_err(message)
        }
        "ENGINE.FAILED" => TellegenError::new_err(message),
        _ => TellegenInputError::new_err(message),
    };
    // `.code` is an instance attribute, so it is set at raise time. A failure
    // to set it must not mask the original error.
    if let Err(problem) = err.value(py).setattr("code", code) {
        problem.restore(py);
        PyErr::fetch(py);
    }
    err
}

/// Solve one stored PowerIO module.
///
/// `module_json` is a PowerIO generation-2 `pio-ir` document; bare network JSON
/// is refused by the engine. An empty `request_json` is a base-case DC OPF.
#[pyfunction]
#[pyo3(signature = (module_json, request_json = ""))]
fn solve_module(py: Python<'_>, module_json: &str, request_json: &str) -> PyResult<String> {
    let module = module_json.to_owned();
    let request = request_json.to_owned();
    py.detach(move || tellegen::solve_module_json(&module, &request))
        .map_err(|message| engine_error(py, message))
}

/// The formulation, operand and parameter support matrix of this build, as JSON.
///
/// Static: it takes no network and cannot fail. `acopf` reports
/// `available: false` by contract.
#[pyfunction]
fn capabilities_json() -> String {
    tellegen::capabilities_json()
}

/// Parse a case file and return it as a stored PowerIO `pio-ir` module.
///
/// Takes bytes rather than decoded text, because powerio refuses a text format
/// whose bytes are not UTF-8 where a lossy decode would have parsed on, and
/// because PowerWorld `.pwb` has no text form at all.
#[pyfunction]
fn parse_case(py: Python<'_>, bytes: &[u8], format: &str) -> PyResult<String> {
    let owned = bytes.to_vec();
    let token = format.to_owned();
    py.detach(move || {
        let info = powerio::resolve_format(&token)
            .ok_or_else(|| format!("unknown PowerIO format {token:?}"))?;
        let id = powerio::FormatId::new(info.token).map_err(|error| error.to_string())?;
        // The angle-bracketed name marks an anonymous in-memory source, so the
        // reader takes the case's own name over a file-stem hint.
        let source = Source::from_memory("<case>", owned)
            .map_err(|error| error.to_string())?
            .with_format(id);
        let module = powerio::parse(source).map_err(|error| error.to_string())?;
        let balanced = tellegen::ir::balanced_module(module)?;
        tellegen::ir::serialize_module(&balanced)
    })
    .map_err(|message| engine_error(py, message))
}

/// Resolve a PowerIO format token, returning its canonical spelling or `None`.
#[pyfunction]
fn resolve_format(token: &str) -> Option<String> {
    powerio::resolve_format(token).map(|info| info.token.to_owned())
}

// ---------------------------------------------------------------------------
// Stored-module solving, planning, and Studies.
//
// These mirror `crates/tellegen-cli/src/main.rs` rather than inventing a second
// contract: the CLI is the reference implementation of the headless surface and
// PowerMCP #67 already speaks it. The glue below — narrowing a module to a DC
// OPF instance, and re-emitting the solution as a stored module — is copied
// from there, because both hosts must produce byte-identical artifacts.
// ---------------------------------------------------------------------------

/// The producer identity every emitted artifact carries.
///
/// Deliberately the engine's version rather than this binding's: the artifact
/// records which solver produced it, and a wheel rebuild that changes no engine
/// code should not change the provenance of its output.
fn producer_string() -> String {
    format!("tellegen {} (b-theta, kkt-implicit)", tellegen::VERSION)
}

/// Narrow a stored module to a DC OPF instance, materializing the default
/// instance for a bare network. Mirrors the CLI's `instance_from_module_json`.
fn instance_from_module_json(
    text: &str,
) -> Result<powerio::PioModule<powerio::DcOpfInstance>, String> {
    use powerio::{DcOpfInstance, PioValue};
    let module = tellegen::ir::deserialize_module(text)?;
    match module.value() {
        PioValue::DcOpfInstance(_) => module.try_map_value(|value| match value {
            PioValue::DcOpfInstance(instance) => Ok(instance),
            other => Err(other.type_name().to_owned()),
        }),
        PioValue::BalancedNetwork(_) => tellegen::ir::balanced_module(module)?
            .try_map_value(DcOpfInstance::from_network)
            .map_err(|error| error.to_string()),
        other => Err(format!(
            "the module holds a {} value; solve_module and plan accept \
             powerio.DcOpfInstance or powerio.BalancedNetwork",
            other.type_name()
        )),
    }
}

/// Re-emit a solved instance as a stored solution module.
fn solution_module_json(
    source_module: powerio::PioModule<powerio::DcOpfInstance>,
    solution: powerio::DcOpfSolution,
) -> Result<String, String> {
    let mut module = source_module
        .map_value(|_| powerio::PioValue::DcOpfSolution(solution))
        .sever_source()
        .with_producer(
            powerio::Producer::new("tellegen", tellegen::VERSION)
                .map_err(|error| error.to_string())?,
        );
    module.sever_value_targets();
    tellegen::ir::serialize_module(&module)
}

/// Solve a stored module's DC OPF instance and return the solution module.
#[pyfunction]
fn solve_module_to_solution(py: Python<'_>, module_json: &str) -> PyResult<String> {
    let text = module_json.to_owned();
    py.detach(move || {
        let source = instance_from_module_json(&text)?;
        let instance = std::sync::Arc::new(source.value().clone());
        let solution = tellegen::solve_dc_opf_instance(instance, producer_string())?;
        solution_module_json(source, solution)
    })
    .map_err(|message| engine_error(py, message))
}

/// Run the bounded capacity-planning search.
///
/// Returns `{"plan": CapacityPlanOutcome, "solution_module": <IR>}`, the same
/// envelope the CLI's `plan` writes.
#[pyfunction]
fn plan_capacity(py: Python<'_>, module_json: &str, spec_json: &str) -> PyResult<String> {
    let text = module_json.to_owned();
    let spec_text = spec_json.to_owned();
    py.detach(move || {
        let spec: tellegen::CapacityPlanSpec = serde_json::from_str(&spec_text)
            .map_err(|error| format!("unreadable planning spec: {error}"))?;
        let source = instance_from_module_json(&text)?;
        let instance = std::sync::Arc::new(source.value().clone());
        let execution = tellegen::plan::plan_capacity(instance, &spec)?;
        let (outcome, solution) = execution.into_solution(producer_string())?;
        let solution_module = solution_module_json(source, solution)?;
        let response = serde_json::json!({
            "plan": outcome,
            "solution_module": serde_json::from_str::<serde_json::Value>(&solution_module)
                .map_err(|error| error.to_string())?,
        });
        serde_json::to_string(&response).map_err(|error| error.to_string())
    })
    .map_err(|message| engine_error(py, message))
}

/// Create a durable Study at `path` from a `CreateStudy` request.
#[pyfunction]
fn study_create(py: Python<'_>, path: &str, request_json: &str) -> PyResult<String> {
    let destination = path.to_owned();
    let text = request_json.to_owned();
    py.detach(move || {
        let request: tellegen::study_ops::CreateStudy =
            serde_json::from_str(&text).map_err(|error| error.to_string())?;
        let bundle = tellegen::study_ops::create_study(request)?;
        tellegen::study_storage::FileStudyStore::new(&destination).create(&bundle)?;
        serde_json::to_string(&bundle.summary(STUDY_SUMMARY_LIMIT))
            .map_err(|error| error.to_string())
    })
    .map_err(|message| engine_error(py, message))
}

/// The saved Study's summary.
#[pyfunction]
fn study_inspect(py: Python<'_>, path: &str) -> PyResult<String> {
    let source = path.to_owned();
    py.detach(move || {
        let bundle = tellegen::study_storage::FileStudyStore::new(&source).load()?;
        serde_json::to_string(&bundle.summary(STUDY_SUMMARY_LIMIT))
            .map_err(|error| error.to_string())
    })
    .map_err(|message| engine_error(py, message))
}

/// The saved Study as a portable bundle.
#[pyfunction]
fn study_export(py: Python<'_>, path: &str) -> PyResult<String> {
    let source = path.to_owned();
    py.detach(move || {
        tellegen::study_storage::FileStudyStore::new(&source)
            .load()?
            .export()
    })
    .map_err(|message| engine_error(py, message))
}

/// Validate a portable bundle and write it to a new Study at `path`.
///
/// Import restores no approvals; that is the engine's rule, not this layer's.
#[pyfunction]
fn study_import(py: Python<'_>, path: &str, bundle_json: &str) -> PyResult<String> {
    let destination = path.to_owned();
    let text = bundle_json.to_owned();
    py.detach(move || {
        let bundle = tellegen::document::StudyBundle::import(&text)?;
        tellegen::study_storage::FileStudyStore::new(&destination).create(&bundle)?;
        serde_json::to_string(&bundle.summary(STUDY_SUMMARY_LIMIT))
            .map_err(|error| error.to_string())
    })
    .map_err(|message| engine_error(py, message))
}

/// Execute one Study operation and commit it under its expected revision.
///
/// Returns `{"result": StudyOperationResult, "progress": [...]}`. The progress
/// entries carry the same `{"event": "study_checkpoint", "index": n}` shape the
/// CLI prints on stderr under `--progress`, so a caller reading either host
/// sees one contract.
///
/// `timeout_seconds` is how cancellation works here. The CLI cancels on SIGTERM
/// through `ctrlc::set_handler`, which this binding must not reuse: it would
/// hijack the interpreter's own SIGINT handling and errors when called twice in
/// one process. A deadline read at each exact-trial checkpoint gives the same
/// graceful stop — the trial in flight finishes and its evidence is committed —
/// without touching signal disposition.
#[pyfunction]
#[pyo3(signature = (path, request_json, timeout_seconds = None))]
fn study_run(
    py: Python<'_>,
    path: &str,
    request_json: &str,
    timeout_seconds: Option<f64>,
) -> PyResult<String> {
    let source = path.to_owned();
    let text = request_json.to_owned();
    // Checked before releasing the GIL: `Duration::from_secs_f64` panics on a
    // negative, NaN, or overflowing value, and a panic here would surface as a
    // `PanicException` that the MCP tool wrapper cannot catch.
    let timeout = match timeout_seconds {
        None => None,
        Some(seconds) if seconds.is_finite() && seconds >= 0.0 => Some(
            std::time::Duration::try_from_secs_f64(seconds).map_err(|_| {
                engine_error(
                    py,
                    format!("bad request JSON: timeout_seconds {seconds} is out of range"),
                )
            })?,
        ),
        Some(seconds) => {
            return Err(engine_error(
                py,
                format!("bad request JSON: timeout_seconds must be a finite, nonnegative number, not {seconds}"),
            ));
        }
    };
    py.detach(move || {
        let request: tellegen::study_ops::StudyRequest =
            serde_json::from_str(&text).map_err(|error| error.to_string())?;
        let expected = request.expected_revision;
        let store = tellegen::study_storage::FileStudyStore::new(&source);
        let mut bundle = store.load()?;

        let deadline = timeout.map(|limit| std::time::Instant::now() + limit);
        let mut checkpoints: Vec<serde_json::Value> = Vec::new();
        let result = tellegen::study_ops::execute_study(&mut bundle, request, || {
            checkpoints.push(serde_json::json!({
                "event": "study_checkpoint",
                "index": checkpoints.len() + 1,
            }));
            deadline.is_some_and(|limit| std::time::Instant::now() >= limit)
        })?;
        // Committed even on a cancelled run: the engine records the trials that
        // did finish, and dropping them would lose completed exact solves.
        store.commit(expected, &bundle)?;
        let response = serde_json::json!({ "result": result, "progress": checkpoints });
        serde_json::to_string(&response).map_err(|error| error.to_string())
    })
    .map_err(|message| engine_error(py, message))
}

/// The code `classify` would assign to an engine message.
///
/// Exported so the classification table can be asserted from the Python test
/// suite. A cdylib-only pyo3 crate cannot host a portable `cargo test` harness:
/// without `extension-module` the test binary links libpython and aborts, and
/// with it the binary has undefined Python symbols that fail to link on Linux.
/// Testing through the loaded extension is both portable and closer to how the
/// code actually runs.
#[pyfunction]
fn _classify(message: &str) -> &'static str {
    classify(message)
}

#[pymodule]
fn _tellegen(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = m.py();
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add("ENGINE_VERSION", tellegen::VERSION)?;
    m.add("PANIC_CODE", PANIC_CODE)?;
    m.add("TellegenError", py.get_type::<TellegenError>())?;
    m.add("TellegenInputError", py.get_type::<TellegenInputError>())?;
    m.add("TellegenSolveError", py.get_type::<TellegenSolveError>())?;
    m.add_function(wrap_pyfunction!(solve_module, m)?)?;
    m.add_function(wrap_pyfunction!(capabilities_json, m)?)?;
    m.add_function(wrap_pyfunction!(parse_case, m)?)?;
    m.add_function(wrap_pyfunction!(resolve_format, m)?)?;
    m.add_function(wrap_pyfunction!(solve_module_to_solution, m)?)?;
    m.add_function(wrap_pyfunction!(plan_capacity, m)?)?;
    m.add_function(wrap_pyfunction!(study_create, m)?)?;
    m.add_function(wrap_pyfunction!(study_inspect, m)?)?;
    m.add_function(wrap_pyfunction!(study_export, m)?)?;
    m.add_function(wrap_pyfunction!(study_import, m)?)?;
    m.add_function(wrap_pyfunction!(study_run, m)?)?;
    m.add_function(wrap_pyfunction!(_classify, m)?)?;
    Ok(())
}
