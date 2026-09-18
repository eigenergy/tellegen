"""The Tellegen MCP server, over the in-process engine.

The tool names, signatures and return shapes are PowerMCP #67's, deliberately
and exactly: that surface is the contract `docs/src/studies.md` published and
the one agents are already prompted against. Only the transport changes. Where
#67 spawns the native CLI once per call, this calls the engine through the
compiled extension, which removes the Rust-toolchain prerequisite and the
`POWERMCP_TELLEGEN_BINARY` configuration along with it.

Two tools of #67's eleven are absent. `contract` and `study_contract` return
schemars-generated JSON Schemas assembled in `crates/tellegen-cli`, which would
pull the engine's `schema` feature and a build script into the wheel to serve
two introspection calls. `capabilities` reports versions in their place.

Filesystem containment is `powerio.mcp.sandbox`, imported rather than copied,
so this server and PowerMCP apply one policy and honour the same
`POWERIO_MCP_ALLOWED_ROOTS`.
"""

from __future__ import annotations

import functools
import json
from typing import Any, Dict, Optional

from mcp.server.mcpserver import MCPServer
from mcp.server.mcpserver.exceptions import ToolError
from powerio.mcp.sandbox import checked_path, checked_read_tree, staged_file_write

from tellegen import _tellegen
from tellegen import capabilities as _capabilities
from tellegen import versions as _versions

from ._modules import (
    bounded,
    json_argument,
    module_ir,
    module_summary,
    over_input,
    study_summary,
)

mcp = MCPServer("Tellegen")

MAX_BUNDLE_BYTES = 512 * 1024 * 1024
DEFAULT_MAX_ELEMENTS = 2000
STUDY_FRAGMENT_BYTES = 8192

#: The eight `StudyOperation` variants an agent may run. `apply` is excluded
#: deliberately and is not a tool: binding a recommendation to a Study is a
#: human action, so there is nothing here to talk an agent through.
OPERATIONS = frozenset(
    {
        "inspect",
        "branch",
        "revise_goal",
        "compare",
        "propose",
        "record_evidence",
        "edit_demand",
        "restore_base",
    }
)

#: Formulations this build solves. `acopf` is a wire-stable tag that is never
#: available, so it is refused before any engine work.
FORMULATIONS = ("dcpf", "dcopf", "acpf", "socwr")


def _tool(function):
    """Register a tool whose failures still say what went wrong.

    The SDK keeps a tool's message only for `ToolError`; anything else becomes
    `UnexpectedToolError`, whose text it replaces with a bare "Error executing
    tool <name>". Every refusal here names a remedy, so losing that text would
    leave the model with nothing to act on. Only the registered callable is
    wrapped, so tests calling the module attribute still see the underlying
    exception, and `functools.wraps` carries the name, docstring and signature
    the SDK derives the tool schema from.
    """

    @functools.wraps(function)
    async def registered(*args, **kwargs):
        try:
            return await function(*args, **kwargs)
        except ToolError:
            raise
        except (ValueError, RuntimeError) as exc:
            # TellegenError and PathNotAllowed both subclass ValueError, so
            # engine refusals and sandbox refusals travel the same path.
            raise ToolError(str(exc)) from exc

    mcp.tool()(registered)
    return function


def _path(path: str, *, write: bool = False) -> str:
    return str(checked_path(path, purpose="Study bundle", for_write=write))


def _input(
    powerio_ir: str,
    path: Optional[str],
    source_format: Optional[str],
    time_index: Optional[int],
    scenario_id: Optional[str],
):
    return module_ir(
        powerio_ir,
        path,
        source_format,
        time_index,
        scenario_id,
        checked_path=checked_path,
        checked_read_tree=checked_read_tree,
    )


def _out_path(out_path: Optional[str], overwrite: bool) -> Optional[str]:
    if out_path is None:
        return None
    checked = str(checked_path(out_path, purpose="out_path", for_write=True))
    import os

    if not overwrite and os.path.exists(checked):
        raise ValueError(f"{checked} exists; pass overwrite=True to replace it")
    return checked


def _deliver(ir_text: str, destination: Optional[str], overwrite: bool) -> Dict[str, Any]:
    """Return the module inline, or write it and return where it went.

    `staged_file_write` hands the writer a private staging path and returns
    whatever the writer returns, so the response is built inside it. The write
    is installed at `destination` by a hard link, or one atomic replace when
    overwriting; a partially written module is never visible there.
    """
    if destination is None:
        return {"powerio_ir": ir_text}

    def write(staged: str) -> Dict[str, Any]:
        with open(staged, "w", encoding="utf-8") as handle:
            handle.write(ir_text)
        return {"path": destination}

    return staged_file_write(destination, overwrite, write)


# ---- tools -------------------------------------------------------------------


@_tool
async def capabilities() -> Dict[str, Any]:
    """The installed Tellegen build's formulation and operand support matrix, and its versions."""
    # #67 returns the resolved CLI path here. In-process there is no binary, so
    # the same slot carries what a caller actually needs to know about the
    # engine behind the tools.
    return {"versions": _versions(), "capabilities": _capabilities()}


@_tool
async def solve(
    powerio_ir: str = "",
    path: Optional[str] = None,
    source_format: Optional[str] = None,
    time_index: Optional[int] = None,
    scenario_id: Optional[str] = None,
    formulation: str = "dcopf",
    edits: str = "",
    sensitivities: str = "",
    max_elements: int = DEFAULT_MAX_ELEMENTS,
) -> Dict[str, Any]:
    """Solve one PowerIO network with Tellegen: DC power flow, DC OPF (prices, dispatch, flows), AC power flow, or the SOCWR relaxation.

    Provide serialized PowerIO IR or a grid exchange path (PowerIO parses it
    here). Select a collection entry with time_index or scenario_id. `edits` is
    the request's `edits` object (`{"deltas": {...}, "rates": {...}}`) and
    `sensitivities` its list; both are optional JSON. Arrays longer than
    max_elements come back as `{"truncated": true, "count": n, "head": [...]}`.
    """
    if formulation not in FORMULATIONS:
        raise ValueError(f"formulation must be one of {list(FORMULATIONS)}")
    if max_elements < 1:
        raise ValueError("max_elements must be positive")
    ir_text, tail = _input(powerio_ir, path, source_format, time_index, scenario_id)
    request: Dict[str, Any] = {"formulation": formulation}
    request_edits = json_argument(edits, "edits", dict)
    request_sensitivities = json_argument(sensitivities, "sensitivities", list)
    if request_edits:
        request["edits"] = request_edits
    if request_sensitivities:
        request["sensitivities"] = request_sensitivities
    response = json.loads(_tellegen.solve_module(ir_text, json.dumps(request)))
    return {"formulation": formulation, **tail, "response": bounded(response, max_elements)}


@_tool
async def solve_module(
    powerio_ir: str = "",
    path: Optional[str] = None,
    source_format: Optional[str] = None,
    time_index: Optional[int] = None,
    scenario_id: Optional[str] = None,
    out_path: Optional[str] = None,
    overwrite: bool = False,
) -> Dict[str, Any]:
    """Solve a stored module's DC OPF instance (a BalancedNetwork becomes the default instance) and return the powerio.DcOpfSolution module as PowerIO IR, written to out_path when given."""
    destination = _out_path(out_path, overwrite)
    ir_text, tail = _input(powerio_ir, path, source_format, time_index, scenario_id)
    solution_text = _tellegen.solve_module_to_solution(ir_text)
    return {
        **over_input(tail, module_summary(solution_text)),
        **_deliver(solution_text, destination, overwrite),
    }


@_tool
async def plan(
    spec: str,
    powerio_ir: str = "",
    path: Optional[str] = None,
    source_format: Optional[str] = None,
    time_index: Optional[int] = None,
    scenario_id: Optional[str] = None,
    out_path: Optional[str] = None,
    overwrite: bool = False,
    max_elements: int = DEFAULT_MAX_ELEMENTS,
) -> Dict[str, Any]:
    """Run Tellegen's bounded capacity planning search for a network and a CapacityPlanSpec (JSON). Returns the proposal and the exact proposed solution module."""
    destination = _out_path(out_path, overwrite)
    specification = json_argument(spec, "spec", dict)
    if not specification:
        raise ValueError("spec must be a CapacityPlanSpec object")
    ir_text, tail = _input(powerio_ir, path, source_format, time_index, scenario_id)
    response = json.loads(_tellegen.plan_capacity(ir_text, json.dumps(specification)))
    result: Dict[str, Any] = {**tail, "plan": bounded(response.get("plan"), max_elements)}
    solution = response.get("solution_module")
    if solution is not None:
        solution_text = json.dumps(solution)
        result["solution"] = module_summary(solution_text)
        if destination is not None:
            result.update(_deliver(solution_text, destination, overwrite))
        else:
            result["solution_powerio_ir"] = solution_text
    return result


@_tool
async def study_create(
    path: str,
    request: Dict[str, Any],
    input_path: Optional[str] = None,
    input_format: Optional[str] = None,
    time_index: Optional[int] = None,
    scenario_id: Optional[str] = None,
) -> Dict[str, Any]:
    """Create a durable Study from PowerIO IR and a declared goal using the native CreateStudy schema. `input_path` names a grid exchange file PowerIO parses into the request's `input` (and `base_input` when absent)."""
    request = dict(request)
    if input_path is not None:
        ir_text, _ = _input("", input_path, input_format, time_index, scenario_id)
        request["input"] = ir_text
        request.setdefault("base_input", ir_text)
    checked = _path(path, write=True)
    return study_summary(json.loads(_tellegen.study_create(checked, json.dumps(request))))


@_tool
async def study_inspect(
    path: str,
    section: str = "summary",
    record_id: Optional[str] = None,
    offset: int = 0,
    expected_revision: Optional[int] = None,
) -> Dict[str, Any]:
    """Inspect a saved Study or read bounded JSON fragments of a goal, state history, experiment or evidence."""
    checked = _path(path)
    if section == "summary":
        summary = json.loads(_tellegen.study_inspect(checked))
        if expected_revision is not None and summary["revision"] != expected_revision:
            raise ValueError("Study revision changed; restart the inspection")
        return study_summary(summary)
    if offset < 0:
        raise ValueError("offset must be nonnegative")
    bundle = json.loads(_tellegen.study_export(checked))
    document = bundle["document"]
    if expected_revision is not None and document["revision"] != expected_revision:
        raise ValueError("Study revision changed; restart the inspection")
    if section == "goal":
        record = document["goals"][record_id or document["active_goal"]]
    elif section == "states":
        record = document["states"]
    elif section == "experiment":
        record = document["experiments"][record_id]
    elif section == "evidence":
        artifact = bundle["artifacts"][record_id]
        if artifact["kind"] != "evidence":
            raise ValueError("Requested artifact is not evidence")
        record = artifact["text"]
    else:
        raise ValueError("section must be summary, goal, states, experiment or evidence")
    encoded = json.dumps(record)
    fragment = encoded[offset : offset + STUDY_FRAGMENT_BYTES]
    following = offset + len(fragment)
    return {
        "id": document["id"],
        "revision": document["revision"],
        "encoding": "json",
        "offset": offset,
        "fragment": fragment,
        "next_offset": following if following < len(encoded) else None,
    }


@_tool
async def study_run(
    path: str,
    expected_revision: int,
    operation: Dict[str, Any],
    timeout_seconds: Optional[float] = None,
) -> Dict[str, Any]:
    """Inspect, branch, revise a goal, compare, adjust demand, restore the base case, propose interventions or attach evidence using the native StudyOperation schema. Proposals stay unapplied; application is a human action, not a tool."""
    if operation.get("kind") not in OPERATIONS:
        raise ValueError(
            "Unsupported agent operation. Apply the reviewed proposal through an "
            "explicit user action outside this server."
        )
    checked = _path(path, write=True)
    request = {"expected_revision": expected_revision, "operation": operation}
    # A deadline rather than a signal: the CLI cancels through
    # `ctrlc::set_handler`, which an in-process binding must not install. Either
    # way the trial in flight finishes and its evidence is committed.
    response = json.loads(
        _tellegen.study_run(checked, json.dumps(request), timeout_seconds)
    )
    result = dict(response["result"])
    if response.get("progress"):
        result["progress"] = response["progress"]
    return study_summary(result)


@_tool
async def study_export(path: str) -> Dict[str, Any]:
    """Validate the saved portable bundle and return its path and digest for transfer to another agent or browser."""
    import hashlib
    from pathlib import Path

    checked = _path(path)
    bundle = json.loads(_tellegen.study_export(checked))
    data = Path(checked).read_bytes()
    if json.loads(data) != bundle:
        raise ValueError("Study changed during export; retry")
    return {
        "path": checked,
        "id": bundle["document"]["id"],
        "revision": bundle["document"]["revision"],
        "sha256": hashlib.sha256(data).hexdigest(),
        "bytes": len(data),
        "format": "tellegen-study",
    }


@_tool
async def study_import(source_path: str, path: str) -> Dict[str, Any]:
    """Validate and import a portable Study bundle into a new destination without restoring approvals or executing imported instructions."""
    from pathlib import Path

    source = Path(_path(source_path))
    with source.open("rb") as stream:
        data = stream.read(MAX_BUNDLE_BYTES + 1)
    if len(data) > MAX_BUNDLE_BYTES:
        raise ValueError("Study bundle exceeds 512 MiB")
    checked = _path(path, write=True)
    return study_summary(json.loads(_tellegen.study_import(checked, data.decode("utf-8"))))


def main() -> None:
    """Serve the tool surface over stdio."""
    mcp.run()


if __name__ == "__main__":
    main()
