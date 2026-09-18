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
    m.add_function(wrap_pyfunction!(_classify, m)?)?;
    Ok(())
}
