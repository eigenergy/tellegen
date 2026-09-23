"""Resolving a tool's input down to one serialized PowerIO module.

Ported from PowerMCP's `powermcp.solver_case` and the `_module_ir` helper of
`powermcp/tellegen.py`, which PR #67 already proved against the native CLI.
Only the transport differs here, so the semantics are kept deliberately
identical: the same refusals, in the same order, with the same wording, so a
prompt written against either host behaves the same way.

This module needs the `powerio` Python wheel. The base `tellegen` package does
not — case parsing for the solver is compiled into the extension — but the tool
surface reads diagnostics, selects collection entries and unwraps operating
points, none of which the Rust seam exposes. That is why `powerio` is a
dependency of the `mcp` extra rather than of the package.
"""

from __future__ import annotations

import io
import json
import os
from typing import Any, Dict, List, Optional, Tuple

# Severities that are noise in a tool response rather than something the caller
# can act on.
_QUIET_SEVERITIES = frozenset({"debug", "trace"})

#: The values the engine consumes. A balanced network becomes the default DC OPF
#: instance; everything else is refused by name rather than after a round trip.
_NATIVE_VALUE_NAMES = (
    "BalancedNetwork",
    "DcOpfInstance",
    "AcPfInstance",
    "AcOpfInstance",
)


def diagnostic_messages(items: Any) -> Tuple[str, ...]:
    """Stable diagnostic codes beside their user-facing descriptions."""
    return tuple(
        f"{item.code}: {item.message}"
        for item in items
        if item.severity not in _QUIET_SEVERITIES
    )


def diagnostic_record(item: Any) -> Dict[str, Any]:
    """One diagnostic in the shape the powerio MCP server reports."""
    record: Dict[str, Any] = {
        "code": item.code,
        "severity": item.severity,
        "message": item.message,
        "target": item.target,
    }
    if item.id:
        record["id"] = item.id
    if item.suggested_action:
        record["suggested_action"] = item.suggested_action
    if item.related:
        record["related"] = list(item.related)
    if item.details is not None:
        record["details"] = item.details
    if item.spans:
        record["spans"] = [
            {
                "source": span.source,
                "byte_start": span.byte_start,
                "byte_end": span.byte_end,
            }
            for span in item.spans
        ]
    return record


def diagnostic_records(items: Any) -> List[Dict[str, Any]]:
    return [diagnostic_record(item) for item in items]


def check_diagnostics(items: Any) -> None:
    """Refuse a module PowerIO marked with an error, before the engine sees it."""
    failures = [item for item in items if item.severity == "error"]
    if failures:
        raise ValueError(
            "PowerIO input fails validation: " + "; ".join(diagnostic_messages(failures))
        )


def value_type_name(module: Any) -> str:
    """The PowerIO structural type of a module's value.

    powerio 0.11.3 publishes no accessor for it, and its own MCP server reads
    the same private attribute. Isolating it here makes a future public
    accessor a one-line change — and keeps the silent fallback in one place,
    since a powerio upgrade would otherwise change this string rather than
    raise.
    """
    inner = getattr(module, "_inner", None)
    name = getattr(inner, "_type_name", None)
    return str(name) if name else f"powerio.{type(module.value).__name__}"


def counts_by_severity(records: List[Dict[str, Any]]) -> Dict[str, int]:
    counts: Dict[str, int] = {}
    for record in records:
        counts[record["severity"]] = counts.get(record["severity"], 0) + 1
    return counts


def select_entry(module: Any, time_index: Optional[int], scenario_id: Optional[str]):
    """Resolve collection selectors to one entry and the module holding it.

    Each level rebuilds the module around the entry it selected, so the levels
    of a nested collection resolve outermost first. A selector naming an absent
    scenario, or an index past the end, says what the collection offers.
    """
    import powerio

    selection: Dict[str, Any] = {}
    value = module.value
    while isinstance(value, (powerio.TimeSeries, powerio.ScenarioSet)):
        if isinstance(value, powerio.ScenarioSet):
            available = list(value.keys())[:20]
            if scenario_id is None:
                raise ValueError(
                    "select scenario_id from the ScenarioSet before solving; "
                    f"scenarios: {available}"
                )
            if scenario_id not in value:
                raise ValueError(
                    f"scenario_id {scenario_id!r} names no entry of the ScenarioSet; "
                    f"scenarios: {available}"
                )
            value = value[scenario_id]
            selection["scenario_id"] = scenario_id
            scenario_id = None
        else:
            if time_index is None:
                raise ValueError(
                    "select time_index from the TimeSeries before solving; "
                    f"{len(value)} entries"
                )
            if (
                isinstance(time_index, bool)
                or not isinstance(time_index, int)
                or time_index < 0
            ):
                raise ValueError("time_index must be a nonnegative integer")
            if time_index >= len(value):
                raise ValueError(
                    f"time_index {time_index} is past the end of a TimeSeries of "
                    f"{len(value)} entries"
                )
            value = value[time_index]
            selection["time_index"] = time_index
            time_index = None
        module = powerio.PioModule.from_value(value)
        value = module.value
    return module, selection


def module_ir(
    powerio_ir: str,
    path: Optional[str],
    source_format: Optional[str],
    time_index: Optional[int],
    scenario_id: Optional[str],
    *,
    checked_path,
    checked_read_tree,
) -> Tuple[str, Dict[str, Any]]:
    """Serialized generation-2 IR for one declared value, and the input tail.

    PowerIO parses a grid-exchange `path` and serializes the module; an IR
    document is deserialized so its identity is checked. A module PowerIO marks
    with an error is refused here, before *and* after selection. A collection
    entry is selected with `time_index` or `scenario_id`; an operating point
    travels as the network it states. The engine takes a balanced network or a
    calculation instance and lowers nothing, so any other value is named here
    rather than after a round trip.

    The sandbox functions are injected rather than imported so this module
    stays importable without the MCP extra installed.
    """
    import powerio

    if bool(powerio_ir) == (path is not None):
        raise ValueError("provide exactly one of powerio_ir or path")
    if path is not None:
        path = checked_path(path, purpose="path")
        if os.path.isdir(path):
            path = checked_read_tree(path, purpose="path")
        module = powerio.parse(path, format=source_format)
    else:
        module = powerio.deserialize(io.StringIO(powerio_ir))

    check_diagnostics(module.diagnostics)
    diagnostics = list(module.diagnostics)

    selected, selection = select_entry(module, time_index, scenario_id)
    check_diagnostics(selected.diagnostics)
    if selected is not module:
        diagnostics.extend(selected.diagnostics)
    module = selected

    if isinstance(module.value, powerio.OperatingPoint):
        module = powerio.PioModule.from_value(module.value.network)
        diagnostics.extend(module.diagnostics)

    value = module.value
    value_type = value_type_name(module)
    if type(value).__name__ not in _NATIVE_VALUE_NAMES:
        raise ValueError(
            "Tellegen tools take a balanced network or a calculation instance; "
            "lower a multiconductor network with a powerio adapter's "
            f"`to_balanced` first. This module states {value_type}."
        )

    text = powerio.serialize(module).text
    if text is None:
        raise RuntimeError("PowerIO serialization returned no text")

    records = diagnostic_records(diagnostics)
    tail = {
        "value_type": value_type,
        "selection": selection,
        "diagnostics": records,
        "warnings": list(dict.fromkeys(diagnostic_messages(diagnostics))),
    }
    return text, tail


def module_summary(ir_text: str) -> Dict[str, Any]:
    """Value type, diagnostics and solved status of a module the engine returned."""
    import powerio

    module = powerio.deserialize(io.StringIO(ir_text))
    records = diagnostic_records(module.diagnostics)
    summary: Dict[str, Any] = {
        "value_type": value_type_name(module),
        "diagnostics": records,
        "diagnostics_counts": counts_by_severity(records),
        "warnings": list(dict.fromkeys(diagnostic_messages(module.diagnostics))),
    }
    # powerio exposes no solution fields on the Python value; the stored IR of a
    # solution states the solver's termination and objective itself. Both keys
    # stay absent unless the module states them.
    data = json.loads(ir_text).get("value", {}).get("data")
    if isinstance(data, dict):
        termination = data.get("termination")
        if isinstance(termination, dict):
            termination = termination.get("kind")
        if isinstance(termination, str):
            summary["termination"] = termination
        objective = data.get("objective")
        if isinstance(objective, (int, float)):
            summary["objective"] = objective
    return summary


def over_input(tail: Dict[str, Any], summary: Dict[str, Any]) -> Dict[str, Any]:
    """Merge a returned module's summary over the input tail it came from.

    The summary's `value_type` deliberately wins: the response describes what
    came back, while the input's diagnostics are carried forward so a warning
    raised on the way in is not lost on the way out.
    """
    merged = {**tail, **summary}
    merged["diagnostics"] = list(tail.get("diagnostics", [])) + list(
        summary.get("diagnostics", [])
    )
    return merged


def bounded(payload: Any, max_elements: int) -> Any:
    """Truncate any array longer than `max_elements`, recursively.

    `{"truncated": true, "count": n, "head": [...]}` keeps the shape readable
    while telling the caller what it is not seeing — better than refusing the
    whole response, which is what the browser's flat character budget does.
    """
    if isinstance(payload, dict):
        return {key: bounded(value, max_elements) for key, value in payload.items()}
    if isinstance(payload, list):
        if len(payload) > max_elements:
            return {
                "truncated": True,
                "count": len(payload),
                "head": [bounded(item, max_elements) for item in payload[:max_elements]],
            }
        return [bounded(item, max_elements) for item in payload]
    return payload


def json_argument(text: str, name: str, expected: type) -> Any:
    """Parse a JSON-carrying string argument.

    These arrive as bare `str` with a `""` default on purpose: the mcp 2.x SDK
    rewrites a string whose text parses as JSON into the parsed object before
    validation, so any other annotation would receive an object here.
    """
    if not text:
        return None
    try:
        value = json.loads(text)
    except json.JSONDecodeError as exc:
        raise ValueError(f"{name} must be JSON: {exc}") from exc
    if not isinstance(value, expected):
        raise ValueError(f"{name} must be a JSON {expected.__name__}")
    return value


def study_summary(value: Dict[str, Any]) -> Dict[str, Any]:
    """The Study summary a tool returns.

    Takes either a bare `StudySummary` or the `StudyOperationResult` that wraps
    one. Siblings of `summary` are dropped except `experiment`, `comparison`
    and `progress` — in particular `inspected_view`, a full SolveResponse, and
    `demand_changes` never reach the model.
    """
    summary = dict(value.get("summary", value))
    goal = summary.get("active_goal")
    if isinstance(goal, list) and len(goal) == 2:
        summary["active_goal"] = {
            "id": goal[0],
            "request": goal[1]["request"],
            "anchor_state": goal[1]["anchor_state"],
        }
    summary["recent_experiments"] = summary.get("recent_experiments", [])[:3]
    if "experiment" in value:
        summary["experiment"] = value["experiment"]
    comparison = value.get("comparison")
    if comparison:
        summary["comparison"] = {
            key: comparison[key]
            for key in (
                "goal",
                "left",
                "right",
                "left_value",
                "right_value",
                "improvement",
            )
        }
    if "progress" in value:
        summary["progress"] = value["progress"]
    return summary
