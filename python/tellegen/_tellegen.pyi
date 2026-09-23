"""Type stub for the compiled extension.

Hand-written to match `crates/tellegen-py/src/lib.rs`. The seam is deliberately
narrow — JSON strings in, JSON strings out — so this file stays short and a
Rust-side rename is caught by `mypy.stubtest` in CI rather than shipping as a
stale stub.
"""

from typing import Optional

__version__: str
"""The binding crate's version, from CARGO_PKG_VERSION."""

ENGINE_VERSION: str
"""The `tellegen` engine crate's version. Can differ from `__version__`."""

PANIC_CODE: str
"""The `.code` value carried by a TellegenError raised from a Rust panic."""

class TellegenError(ValueError):
    """Base engine error. Carries a `.code` naming the failure class."""

    code: str

class TellegenInputError(TellegenError):
    """A malformed request, an unknown element, or an out-of-range edit."""

class TellegenSolveError(TellegenError):
    """The solver ran without producing an answer."""

def solve_module(module_json: str, request_json: str = ...) -> str:
    """Solve a stored PowerIO `pio-ir` module. Returns a SolveResponse as JSON."""

def capabilities_json() -> str:
    """The formulation/operand/parameter support matrix of this build, as JSON."""

def parse_case(bytes: bytes, format: str) -> str:
    """Parse case-file bytes into a stored PowerIO `pio-ir` module."""

def resolve_format(token: str) -> Optional[str]:
    """The canonical spelling of a PowerIO format token, or None."""

def solve_module_to_solution(module_json: str) -> str:
    """Solve a stored module's DC OPF instance. Returns the solution module as IR."""

def plan_capacity(module_json: str, spec_json: str) -> str:
    """Run the bounded capacity search. Returns {"plan", "solution_module"} as JSON."""

def study_create(path: str, request_json: str) -> str:
    """Create a durable Study at `path`. Returns its summary as JSON."""

def study_inspect(path: str) -> str:
    """The saved Study's summary, as JSON."""

def study_export(path: str) -> str:
    """The saved Study as a portable bundle, as JSON."""

def study_import(path: str, bundle_json: str) -> str:
    """Import a portable bundle into a new Study. Returns its summary as JSON."""

def study_run(
    path: str, request_json: str, timeout_seconds: Optional[float] = ...
) -> str:
    """Execute one Study operation and commit it.

    Returns {"result", "progress"} as JSON. `timeout_seconds` cancels at the
    next exact-trial checkpoint, letting the trial in flight finish.
    """

def _classify(message: str) -> str:
    """The code `classify` would assign to an engine message. Diagnostics only."""
