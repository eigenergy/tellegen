"""The Tellegen MCP server, over the in-process engine.

Nine tools solve, plan and keep Studies; two resources list the case files an
agent may open; one prompt walks an agent through a congestion investigation.
The tools call the engine through the compiled extension, so no native CLI or
Rust toolchain is involved.

Tool names, argument names and result keys match PowerMCP's Tellegen server,
so a prompt written against either host behaves the same way. PowerMCP's
`contract` and `study_contract` tools are absent: they return JSON Schemas
generated in `crates/tellegen-cli`, and every tool here publishes its input and
output schemas instead. `capabilities` reports the engine's versions.

Filesystem containment is `powerio.mcp.sandbox`, imported rather than copied,
so this server and PowerMCP apply one policy and honour the same
`POWERIO_MCP_ALLOWED_ROOTS`.
"""

from __future__ import annotations

import functools
import hashlib
import inspect
import json
import os
from importlib import resources
from pathlib import Path
from typing import Annotated, Any, Dict, List, Literal, Optional

from mcp.server.mcpserver import MCPServer
from mcp.server.mcpserver.exceptions import ToolError
from mcp.server.mcpserver.resources import TextResource
from mcp.types import ToolAnnotations
from powerio.mcp.sandbox import (
    allowed_roots,
    checked_path,
    checked_read_tree,
    staged_file_write,
)
from pydantic import WithJsonSchema

from tellegen import _tellegen
from tellegen import capabilities as _capabilities
from tellegen import versions as _versions

from ._modules import (
    bounded,
    module_ir,
    module_summary,
    over_input,
    study_summary,
)
from ._schemas import (
    EDITS_SCHEMA,
    SENSITIVITIES_SCHEMA,
    SPEC_SCHEMA,
    CapabilitiesOutput,
    CapacityPlanSpec,
    PlanOutput,
    SensitivityRequest,
    SolveEdits,
    SolveModuleOutput,
    SolveOutput,
    StudyExportOutput,
    StudyInspectOutput,
    StudySummaryOutput,
    parsed,
    parsed_list,
)

INSTRUCTIONS = """\
Tellegen solves power network cases: DC power flow, DC optimal power flow with \
locational marginal prices (LMPs) and KKT sensitivities, AC power flow, and the \
SOCWR conic relaxation. It also runs a bounded capacity-planning search and keeps \
durable Studies.

Start by reading the `tellegen://cases` resource: it lists the case files under \
the allowed roots, with absolute paths to pass as `path`, and a bundled example \
(`tellegen://cases/case3`) that works as a `path` too. Files outside the allowed \
roots are refused. For a case with more than a few dozen buses, call `solve` with \
`detail="summary"` first; it reports the LMP range and the binding and most \
loaded branches without the per-element arrays. Proposals are never applied by a \
tool: applying one is a human action.
"""

mcp = MCPServer(
    "Tellegen",
    title="Tellegen",
    instructions=INSTRUCTIONS,
    version=str(_versions().get("tellegen", "")),
    website_url="https://tellegen.dev",
)

MAX_BUNDLE_BYTES = 512 * 1024 * 1024
DEFAULT_MAX_ELEMENTS = 2000
STUDY_FRAGMENT_BYTES = 8192

#: A `detail="auto"` solve returns the per-element arrays only when the whole
#: result fits in this many characters of JSON, about ten thousand tokens.
AUTO_DETAIL_CHARS = 40_000
#: Binding branches listed in a solve summary, by relief per MW.
SUMMARY_BINDING = 25
#: Most loaded branches listed in a solve summary.
SUMMARY_LOADED = 5

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

#: Bundled example cases, served as resources and accepted as a `path`.
EXAMPLE_SCHEME = "tellegen://cases/"
EXAMPLES_DIR = Path(str(resources.files("tellegen.mcp") / "examples"))

#: Suffixes the case index lists. `.json` alone is left out: under an allowed
#: root it is as likely a Study bundle as a case.
CASE_SUFFIXES = (
    ".m",
    ".raw",
    ".rawx",
    ".aux",
    ".epc",
    ".dss",
    ".xiidm",
    ".uct",
    ".ucte",
    ".cdf",
    ".pio.json",
)
CASE_INDEX_LIMIT = 200
CASE_INDEX_DEPTH = 4
_SKIPPED_DIRECTORIES = frozenset(
    {"node_modules", "target", "__pycache__", "site-packages", "dist", "build"}
)

# ---- typed arguments ---------------------------------------------------------

# Typed as `Any` so the SDK admits every value and `parsed` reports what is
# wrong with it; the published schema is the typed one.
EditsArgument = Annotated[Any, WithJsonSchema(EDITS_SCHEMA)]
SensitivitiesArgument = Annotated[Any, WithJsonSchema(SENSITIVITIES_SCHEMA)]
SpecArgument = Annotated[Any, WithJsonSchema(SPEC_SCHEMA)]
Detail = Literal["auto", "summary", "full"]
InlineModule = Literal["auto", "inline", "omit"]


def _tool(
    *,
    title: str,
    output: Any,
    read_only: bool,
    idempotent: bool,
    destructive: bool = False,
):
    """Register a tool with its annotations, and keep its failures readable.

    The SDK keeps a tool's message only for `ToolError`; anything else becomes
    `UnexpectedToolError`, whose text it replaces with a bare "Error executing
    tool <name>". Every refusal here names a remedy, so losing that text would
    leave the model with nothing to act on. Only the registered callable is
    wrapped, so tests calling the module attribute still see the underlying
    exception.

    `output` is the result's declared type, which the SDK publishes as the
    tool's `outputSchema` and validates each result against. It is set on the
    registered callable's signature only, so the module functions keep their
    plain `Dict[str, Any]` annotations.

    No tool reaches beyond the local engine and the allowed roots, so every
    tool is marked closed-world.
    """

    def decorate(function):
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
            except (KeyError, TypeError, OSError) as exc:
                # A missing record, a wrongly typed argument, or a store path
                # that is not a readable file.
                raise ToolError(f"{type(exc).__name__}: {exc}") from exc

        signature = inspect.signature(function, eval_str=True)
        registered.__signature__ = signature.replace(return_annotation=output)
        mcp.tool(
            title=title,
            annotations=ToolAnnotations(
                title=title,
                read_only_hint=read_only,
                destructive_hint=destructive,
                idempotent_hint=idempotent,
                open_world_hint=False,
            ),
        )(registered)
        return function

    return decorate


# ---- paths -------------------------------------------------------------------


def _examples() -> Dict[str, Path]:
    return {
        item.name.split(".")[0]: item
        for item in sorted(EXAMPLES_DIR.iterdir())
        if item.is_file() and not item.name.startswith(".")
    }


def _example(name: str) -> Path:
    examples = _examples()
    if name not in examples:
        known = ", ".join(EXAMPLE_SCHEME + item for item in examples)
        raise ValueError(f"No bundled example named {name!r}; bundled: {known}")
    return examples[name]


def _case_path(value: str, *, purpose: str = "path", for_write: bool = False) -> str:
    """`checked_path`, plus the read-only bundled examples by URI."""
    if isinstance(value, str) and value.startswith(EXAMPLE_SCHEME):
        if for_write:
            raise ValueError(
                "A bundled example is read-only; write under an allowed root"
            )
        return str(_example(value[len(EXAMPLE_SCHEME) :]))
    return str(checked_path(value, purpose=purpose, for_write=for_write))


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
        checked_path=_case_path,
        checked_read_tree=checked_read_tree,
    )


def _out_path(out_path: Optional[str], overwrite: bool) -> Optional[str]:
    if out_path is None:
        return None
    checked = str(checked_path(out_path, purpose="out_path", for_write=True))
    if not overwrite and os.path.exists(checked):
        raise ValueError(f"{checked} exists; pass overwrite=True to replace it")
    return checked


def _inline(key: str, ir_text: str, module: InlineModule) -> Dict[str, Any]:
    """A module returned inline, or a note saying why it was not.

    "auto" leaves out a module longer than `AUTO_DETAIL_CHARS`: a solved
    500-bus case is over half a megabyte of IR, more than a client will pass
    to a model.
    """
    if module == "inline" or (module == "auto" and len(ir_text) <= AUTO_DETAIL_CHARS):
        return {key: ir_text}
    return {
        f"{key}_omitted": (
            f"The module is {len(ir_text):,} characters, so it is not returned "
            "inline. Pass out_path to write it to a file, or module='inline'."
        )
    }


def _deliver(
    ir_text: str,
    destination: Optional[str],
    overwrite: bool,
    module: InlineModule = "inline",
) -> Dict[str, Any]:
    """Return the module inline, or write it and return where it went.

    `staged_file_write` hands the writer a private staging path and returns
    whatever the writer returns, so the response is built inside it. The write
    is installed at `destination` by a hard link, or one atomic replace when
    overwriting; a partially written module is never visible there.
    """
    if destination is None:
        return _inline("powerio_ir", ir_text, module)

    def write(staged: str) -> Dict[str, Any]:
        with open(staged, "w", encoding="utf-8") as handle:
            handle.write(ir_text)
        return {"path": destination}

    return staged_file_write(destination, overwrite, write)


# ---- solve summaries ---------------------------------------------------------

#: Response keys a summary keeps. Everything else that is an array is a
#: per-element result and is left out of a summary.
_SUMMARY_KEEPS = frozenset({"sensitivities", "constraints"})


def _solve_summary(response: Dict[str, Any]) -> Dict[str, Any]:
    """Sizes, the LMP range, and the binding and most loaded branches."""
    lmp = response.get("lmp") or []
    flows = response.get("flows") or []
    buses = lmp or response.get("va") or response.get("vm") or response.get("w") or []
    summary: Dict[str, Any] = {
        "buses": len(buses),
        "branches": len(flows),
        "generators": len(response.get("dispatch") or []),
    }
    if lmp:
        # The positions are the dense bus indices a sensitivity's `indices` take.
        low = min(range(len(lmp)), key=lambda index: lmp[index]["value"])
        high = max(range(len(lmp)), key=lambda index: lmp[index]["value"])
        summary["lmp"] = {
            "min": lmp[low]["value"],
            "min_bus": lmp[low].get("bus"),
            "min_index": low,
            "max": lmp[high]["value"],
            "max_bus": lmp[high].get("bus"),
            "max_index": high,
            "mean": sum(item["value"] for item in lmp) / len(lmp),
        }

    def described(flow: Dict[str, Any]) -> Dict[str, Any]:
        info = flow.get("loading_info") or {}
        return {
            "branch": flow.get("branch"),
            "uid": flow.get("uid"),
            "pf": flow.get("pf"),
            "rating": info.get("rating"),
            "loading": flow.get("loading"),
        }

    by_branch = {flow.get("branch"): flow for flow in flows}
    constraints = response.get("branch_constraints")
    if isinstance(constraints, list):
        binding = [row for row in constraints if row.get("binding")]
        binding.sort(key=lambda row: -abs(row.get("relief_per_mw") or 0.0))
        summary["binding_count"] = len(binding)
        summary["binding_branches"] = [
            {
                **described(
                    by_branch.get(row.get("branch"), {"branch": row.get("branch")})
                ),
                "relief_per_mw": row.get("relief_per_mw"),
            }
            for row in binding[:SUMMARY_BINDING]
        ]
    loaded = [flow for flow in flows if isinstance(flow.get("loading"), (int, float))]
    loaded.sort(key=lambda flow: -flow["loading"])
    summary["most_loaded"] = [described(flow) for flow in loaded[:SUMMARY_LOADED]]
    return summary


def _summarized(response: Dict[str, Any]) -> Dict[str, Any]:
    omitted = sorted(
        key
        for key, value in response.items()
        if isinstance(value, list) and key not in _SUMMARY_KEEPS
    )
    kept = {key: value for key, value in response.items() if key not in omitted}
    kept["omitted"] = omitted
    return kept


# ---- tools -------------------------------------------------------------------


@_tool(
    title="Report engine capabilities",
    output=CapabilitiesOutput,
    read_only=True,
    idempotent=True,
)
async def capabilities() -> Dict[str, Any]:
    """The installed Tellegen build's versions, and per formulation whether it is available and which sensitivity operands and parameters it supports."""
    return {"versions": _versions(), "capabilities": _capabilities()}


@_tool(
    title="Solve a power network",
    output=SolveOutput,
    read_only=True,
    idempotent=True,
)
async def solve(
    powerio_ir: str = "",
    path: Optional[str] = None,
    source_format: Optional[str] = None,
    time_index: Optional[int] = None,
    scenario_id: Optional[str] = None,
    formulation: str = "dcopf",
    edits: EditsArgument = None,
    sensitivities: SensitivitiesArgument = None,
    max_elements: int = DEFAULT_MAX_ELEMENTS,
    detail: Detail = "auto",
) -> Dict[str, Any]:
    """Solve one network with Tellegen: DC power flow (dcpf), DC OPF with LMPs, dispatch, flows and binding limits (dcopf), AC power flow (acpf), or the SOCWR relaxation (socwr).

    Give `path` (a case file under the allowed roots, or a bundled example such
    as tellegen://cases/case3) or serialized PowerIO IR in `powerio_ir`. Select a
    collection entry with time_index or scenario_id. `edits` changes demand or
    branch ratings before solving; `sensitivities` requests KKT derivatives
    such as LMP with respect to demand (use `indices` to pick columns on a large
    case). Every result carries `summary`: sizes, the LMP range, and the binding
    and most loaded branches. `detail="summary"` leaves out the per-element
    arrays, "full" keeps them, and "auto" (the default) keeps them only when the
    result is under about 40,000 characters. Arrays longer than max_elements come
    back as `{"truncated": true, "count": n, "head": [...]}`.
    """
    if formulation not in FORMULATIONS:
        raise ValueError(f"formulation must be one of {list(FORMULATIONS)}")
    if max_elements < 1:
        raise ValueError("max_elements must be positive")
    if detail not in ("auto", "summary", "full"):
        raise ValueError("detail must be auto, summary or full")
    request_edits = parsed(edits, SolveEdits, "edits")
    request_sensitivities = parsed_list(
        sensitivities, SensitivityRequest, "sensitivities"
    )
    ir_text, tail = _input(powerio_ir, path, source_format, time_index, scenario_id)
    request: Dict[str, Any] = {"formulation": formulation}
    if request_edits is not None and (request_edits.deltas or request_edits.rates):
        request["edits"] = request_edits.model_dump()
    if request_sensitivities:
        request["sensitivities"] = [
            cell.model_dump(exclude_none=True) for cell in request_sensitivities
        ]
    response = json.loads(_tellegen.solve_module(ir_text, json.dumps(request)))
    result: Dict[str, Any] = {
        "formulation": formulation,
        **tail,
        "summary": _solve_summary(response),
        "response": bounded(response, max_elements),
    }
    chosen = detail
    if detail == "auto":
        chosen = "full" if len(json.dumps(result)) <= AUTO_DETAIL_CHARS else "summary"
    if chosen == "summary":
        result["response"] = _summarized(result["response"])
    result["detail"] = chosen
    return result


@_tool(
    title="Solve to a solution module",
    output=SolveModuleOutput,
    read_only=False,
    destructive=True,
    idempotent=True,
)
async def solve_module(
    powerio_ir: str = "",
    path: Optional[str] = None,
    source_format: Optional[str] = None,
    time_index: Optional[int] = None,
    scenario_id: Optional[str] = None,
    out_path: Optional[str] = None,
    overwrite: bool = False,
    module: InlineModule = "auto",
) -> Dict[str, Any]:
    """Solve a network's DC OPF instance (a BalancedNetwork becomes the default instance) and return the powerio.DcOpfSolution module as PowerIO IR, or write it to out_path. An existing out_path is replaced only with overwrite=True. Without out_path, module="auto" returns the IR inline only when it is under about 40,000 characters; "inline" always returns it and "omit" never does."""
    destination = _out_path(out_path, overwrite)
    ir_text, tail = _input(powerio_ir, path, source_format, time_index, scenario_id)
    solution_text = _tellegen.solve_module_to_solution(ir_text)
    return {
        **over_input(tail, module_summary(solution_text)),
        **_deliver(solution_text, destination, overwrite, module),
    }


@_tool(
    title="Plan capacity increases",
    output=PlanOutput,
    read_only=False,
    destructive=True,
    idempotent=True,
)
async def plan(
    spec: SpecArgument,
    powerio_ir: str = "",
    path: Optional[str] = None,
    source_format: Optional[str] = None,
    time_index: Optional[int] = None,
    scenario_id: Optional[str] = None,
    out_path: Optional[str] = None,
    overwrite: bool = False,
    max_elements: int = DEFAULT_MAX_ELEMENTS,
    module: InlineModule = "auto",
) -> Dict[str, Any]:
    """Search for branch rating increases that lower a weighted sum of LMPs, steered by implicit KKT gradients and verified by exact DC OPF solves. Returns every trial, the unapplied proposal, and the proposed solution module: written to out_path when given, otherwise inline as for solve_module's `module` argument. max_changed_lines may not exceed the number of candidates."""
    destination = _out_path(out_path, overwrite)
    specification = parsed(spec, CapacityPlanSpec, "spec")
    if specification is None:
        raise ValueError("Argument `spec` is required: pass a CapacityPlanSpec object")
    ir_text, tail = _input(powerio_ir, path, source_format, time_index, scenario_id)
    response = json.loads(
        _tellegen.plan_capacity(ir_text, specification.model_dump_json())
    )
    result: Dict[str, Any] = {
        **tail,
        "plan": bounded(response.get("plan"), max_elements),
    }
    solution = response.get("solution_module")
    if solution is not None:
        solution_text = json.dumps(solution)
        result["solution"] = module_summary(solution_text)
        if destination is not None:
            result.update(_deliver(solution_text, destination, overwrite))
        else:
            result.update(_inline("solution_powerio_ir", solution_text, module))
    return result


@_tool(
    title="Create a study",
    output=StudySummaryOutput,
    read_only=False,
    idempotent=False,
)
async def study_create(
    path: str,
    request: Dict[str, Any],
    input_path: Optional[str] = None,
    input_format: Optional[str] = None,
    time_index: Optional[int] = None,
    scenario_id: Optional[str] = None,
) -> Dict[str, Any]:
    """Create a durable Study file at `path` from a network and a declared goal, using the native CreateStudy schema. `input_path` names a case file PowerIO parses into the request's `input` (and `base_input` when absent). An existing Study file is never replaced."""
    request = dict(request)
    if input_path is not None:
        ir_text, _ = _input("", input_path, input_format, time_index, scenario_id)
        request["input"] = ir_text
        request.setdefault("base_input", ir_text)
    checked = _path(path, write=True)
    return study_summary(
        json.loads(_tellegen.study_create(checked, json.dumps(request)))
    )


def _record(table: Dict[str, Any], record_id: Optional[str], kind: str) -> Any:
    """Look a record up by id, naming the ids that exist when it is missing."""
    if record_id is None:
        raise ValueError(f"record_id is required for the {kind} section")
    if record_id not in table:
        known = ", ".join(sorted(table)) or "none"
        raise ValueError(f"Unknown {kind} {record_id!r}; known: {known}")
    return table[record_id]


@_tool(
    title="Inspect a study",
    output=StudyInspectOutput,
    read_only=True,
    idempotent=True,
)
async def study_inspect(
    path: str,
    section: str = "summary",
    record_id: Optional[str] = None,
    offset: int = 0,
    expected_revision: Optional[int] = None,
) -> Dict[str, Any]:
    """Read a saved Study's summary, or page through bounded JSON fragments of a goal, the state history, an experiment or evidence."""
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
        record = _record(
            document["goals"], record_id or document["active_goal"], "goal"
        )
    elif section == "states":
        record = document["states"]
    elif section == "experiment":
        record = _record(document["experiments"], record_id, "experiment")
    elif section == "evidence":
        artifact = _record(bundle["artifacts"], record_id, "artifact")
        if artifact["kind"] != "evidence":
            raise ValueError("Requested artifact is not evidence")
        record = artifact["text"]
    else:
        raise ValueError(
            "section must be summary, goal, states, experiment or evidence"
        )
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


@_tool(
    title="Run a study operation",
    output=StudySummaryOutput,
    read_only=False,
    idempotent=False,
)
async def study_run(
    path: str,
    expected_revision: int,
    operation: Dict[str, Any],
    timeout_seconds: Optional[float] = None,
) -> Dict[str, Any]:
    """Inspect, branch, revise a goal, compare, adjust demand, restore the base case, propose interventions or attach evidence using the native StudyOperation schema. Each operation appends a revision. Proposals stay unapplied; applying one is a human action, not a tool."""
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


@_tool(
    title="Validate a study for export",
    output=StudyExportOutput,
    read_only=True,
    idempotent=True,
)
async def study_export(path: str) -> Dict[str, Any]:
    """Validate a saved Study file as a portable bundle and return its path, revision and SHA-256 digest for transfer to another agent or browser. Writes nothing."""
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


@_tool(
    title="Import a study bundle",
    output=StudySummaryOutput,
    read_only=False,
    idempotent=False,
)
async def study_import(source_path: str, path: str) -> Dict[str, Any]:
    """Validate a portable Study bundle and import it into a new destination, without restoring approvals or executing imported instructions. An existing destination is never replaced."""
    source = Path(_path(source_path))
    with source.open("rb") as stream:
        data = stream.read(MAX_BUNDLE_BYTES + 1)
    if len(data) > MAX_BUNDLE_BYTES:
        raise ValueError("Study bundle exceeds 512 MiB")
    checked = _path(path, write=True)
    return study_summary(
        json.loads(_tellegen.study_import(checked, data.decode("utf-8")))
    )


# ---- resources ---------------------------------------------------------------


def _case_files() -> Dict[str, Any]:
    """Case files under the allowed roots, bounded in depth and count."""
    found: List[Dict[str, Any]] = []
    truncated = False
    roots = [str(root) for root in allowed_roots()]
    for root in roots:
        if not os.path.isdir(root):
            continue
        base_depth = root.rstrip(os.sep).count(os.sep)
        for directory, subdirectories, files in os.walk(root, followlinks=False):
            if directory.count(os.sep) - base_depth >= CASE_INDEX_DEPTH:
                subdirectories[:] = []
            subdirectories[:] = sorted(
                name
                for name in subdirectories
                if not name.startswith(".") and name not in _SKIPPED_DIRECTORIES
            )
            for name in sorted(files):
                lowered = name.lower()
                if not lowered.endswith(CASE_SUFFIXES) or lowered.endswith(
                    ".study.json"
                ):
                    continue
                full = os.path.join(directory, name)
                try:
                    checked = str(checked_path(full, purpose="path"))
                    size = os.path.getsize(checked)
                except (OSError, ValueError):
                    continue
                if len(found) >= CASE_INDEX_LIMIT:
                    truncated = True
                    break
                found.append(
                    {
                        "name": os.path.relpath(full, root),
                        "path": checked,
                        "bytes": size,
                    }
                )
            if truncated:
                break
    examples = [
        {
            "name": name,
            "path": EXAMPLE_SCHEME + name,
            "bytes": item.stat().st_size,
            "description": "Three-bus synthetic DC OPF example (MIT); line 2-3 is the tight one."
            if name == "case3"
            else "Bundled example case.",
        }
        for name, item in _examples().items()
    ]
    return {
        "allowed_roots": roots,
        "cases": found,
        "truncated": truncated,
        "examples": examples,
        "usage": (
            "Pass a case's `path` to solve, solve_module or plan as `path`, or to "
            "study_create as `input_path`. Paths outside allowed_roots are refused."
        ),
    }


@mcp.resource(
    "tellegen://cases",
    name="cases",
    title="Case files",
    description=(
        "The case files this server may open: those under POWERIO_MCP_ALLOWED_ROOTS "
        "(up to 200, four directories deep) and the bundled examples, with the "
        "`path` to pass to solve."
    ),
    mime_type="application/json",
)
def case_index() -> str:
    return json.dumps(_case_files(), indent=2)


def _register_examples() -> None:
    for name, item in _examples().items():
        mcp.add_resource(
            TextResource(
                uri=EXAMPLE_SCHEME + name,
                name=name,
                title=f"Example case {name}",
                description=(
                    f"Bundled example case `{name}` ({item.suffix.lstrip('.')} text). "
                    f"Pass {EXAMPLE_SCHEME + name} as a tool's `path` to solve it."
                ),
                mime_type="text/plain",
                text=item.read_text(encoding="utf-8"),
            )
        )


_register_examples()


# ---- prompts -----------------------------------------------------------------


@mcp.prompt(
    name="investigate_congestion",
    title="Investigate congestion",
    description=(
        "Find the binding transmission limits in a case, explain what they do to "
        "prices, and propose a bounded capacity increase."
    ),
)
def investigate_congestion(case_path: str) -> str:
    target = json.dumps(case_path)
    return f"""\
Investigate transmission congestion in the case at {target} using the tellegen tools. \
Report numbers from tool results only; do not estimate them.

1. Call `capabilities` and confirm that dcopf is available and supports \
{{"Price": "Active"}} with respect to {{"Demand": "Active"}}.
2. Call `solve` with path={target}, formulation "dcopf" and detail "summary". \
Report the status, the objective, the LMP range from `summary.lmp`, and each \
branch in `summary.binding_branches` with its flow, rating and relief_per_mw.
3. If no branch binds, say so, name the most loaded branches from \
`summary.most_loaded`, and solve again with `edits` that add demand at the \
highest-LMP bus (for example {{"deltas": {{"<bus>": 30}}}}), at most three \
times, until one binds. Say \
which edits you used.
4. Call `solve` again with the same inputs and \
sensitivities [{{"operand": {{"Price": "Active"}}, "parameter": {{"Demand": \
"Active"}}, "indices": [k]}}], where k is `summary.lmp.max_index`, the \
highest-LMP bus's column. Explain what the column says about how prices respond \
to demand at that bus, and why the binding branches cause the price differences.
5. If a branch binds in the case as stored, call `plan` on it with the \
binding branches' uids, plus the next most loaded branch, as `candidates`; an \
objective of kind "weighted_lmp" weighting the highest-LMP bus (its numeric \
`summary.lmp.max_bus`); and small limits, for example budget_mw 50, \
max_increase_per_branch_mw 50, increment_mw 10, max_changed_lines no larger \
than the number of candidates, and exact_solve_budget 8. Report the proposal, \
each trial's predicted and exact change in the objective, and its baseline and \
final values. State that the proposal is not applied. `plan` takes no edits, \
so if a branch bound only after step 3, say that there is nothing to relieve \
in the stored case and stop.
"""


def main() -> None:
    """Serve the tool surface over stdio."""
    mcp.run()


if __name__ == "__main__":
    main()
