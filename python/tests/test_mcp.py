"""The MCP tool surface, in process.

The tool names and signatures are PowerMCP #67's, so these assertions are
deliberately close to that suite's: the point of reusing the surface is that a
prompt written against either host behaves identically, and a divergence here
is a bug rather than a variation.

Skipped whole when the `mcp` extra is not installed, so the base package's
tests still run on an interpreter without the SDK.
"""

from __future__ import annotations

import asyncio
import json
from pathlib import Path

import pytest

mcp_sdk = pytest.importorskip("mcp", reason="needs the [mcp] extra")
pytest.importorskip("powerio", reason="needs the [mcp] extra")

from mcp.server.mcpserver.exceptions import (  # noqa: E402
    ToolError,
    UnexpectedToolError,
)
from tellegen.mcp import server  # noqa: E402

CASE3 = Path(__file__).resolve().parents[2] / "evidence" / "studies" / "case3.m"

#: The nine tools this server registers. `contract` and `study_contract` are
#: deliberately absent: they return schemars-generated schemas assembled in the
#: CLI crate, which would pull the engine's `schema` feature into the wheel.
EXPECTED_TOOLS = {
    "capabilities",
    "solve",
    "solve_module",
    "plan",
    "study_create",
    "study_inspect",
    "study_run",
    "study_export",
    "study_import",
}


@pytest.fixture(autouse=True)
def allowed_roots(tmp_path, monkeypatch):
    """Confine every test's paths, and give each its own root.

    powerio captures a default root at import when the variable is unset, so
    leaving it unset would make these tests depend on the directory pytest
    happened to start in.
    """
    import os

    monkeypatch.setenv("POWERIO_MCP_ALLOWED_ROOTS", os.path.realpath(tmp_path))
    return Path(os.path.realpath(tmp_path))


@pytest.fixture
def case3(allowed_roots) -> str:
    if not CASE3.exists():  # pragma: no cover
        pytest.skip(f"fixture missing: {CASE3}")
    local = allowed_roots / "case3.m"
    local.write_bytes(CASE3.read_bytes())
    return str(local)


def call(tool: str, **arguments):
    return asyncio.run(server.mcp.call_tool(tool, arguments))


# -- registration ------------------------------------------------------------


def test_the_registered_tools_are_the_nine_this_server_claims():
    tools = asyncio.run(server.mcp.list_tools())
    assert {t.name for t in tools} == EXPECTED_TOOLS


def test_every_tool_has_a_description_the_model_can_act_on():
    for tool in asyncio.run(server.mcp.list_tools()):
        assert tool.description, f"{tool.name} has no description"


@pytest.mark.parametrize("argument", ["powerio_ir", "edits", "sensitivities"])
def test_json_carrying_arguments_are_plain_strings(argument):
    """The SDK rewrites a string that parses as JSON into an object before
    validation, so these must be annotated `str` with an empty default."""
    tools = {t.name: t for t in asyncio.run(server.mcp.list_tools())}
    schema = tools["solve"].input_schema["properties"][argument]
    assert schema["type"] == "string"
    assert schema["default"] == ""


# -- solving -----------------------------------------------------------------


def test_capabilities_reports_versions_and_the_support_matrix():
    result = asyncio.run(server.capabilities())
    assert set(result["versions"]) == {"tellegen", "engine", "powerio_ir"}
    available = {c["formulation"] for c in result["capabilities"] if c["available"]}
    assert available == {"dcpf", "dcopf", "acpf", "socwr"}


def test_solve_reads_a_case_file_and_returns_the_input_tail(case3):
    result = asyncio.run(server.solve(path=case3))
    assert result["formulation"] == "dcopf"
    assert result["value_type"] == "powerio.BalancedNetwork"
    assert result["response"]["status"] == "optimal"
    assert result["response"]["objective"] == pytest.approx(626.9487, abs=1e-3)
    # The input side travels with every response.
    assert set(result) >= {"value_type", "selection", "diagnostics", "warnings"}


def test_solve_applies_edits(case3):
    base = asyncio.run(server.solve(path=case3))["response"]["objective"]
    moved = asyncio.run(
        server.solve(path=case3, edits=json.dumps({"deltas": {"2": 30.0}}))
    )["response"]
    assert moved["objective"] > base
    assert len({round(b["value"], 6) for b in moved["lmp"]}) > 1


def test_solve_refuses_an_unavailable_formulation_before_any_engine_work(case3):
    with pytest.raises(ValueError, match="formulation must be one of"):
        asyncio.run(server.solve(path=case3, formulation="acopf"))


def test_solve_needs_exactly_one_input(case3):
    with pytest.raises(ValueError, match="exactly one"):
        asyncio.run(server.solve(path=case3, powerio_ir="{}"))
    with pytest.raises(ValueError, match="exactly one"):
        asyncio.run(server.solve())


def test_malformed_json_arguments_name_the_argument(case3):
    with pytest.raises(ValueError, match="edits must be JSON"):
        asyncio.run(server.solve(path=case3, edits="{not json"))


def test_solve_module_returns_a_solution_module(case3):
    result = asyncio.run(server.solve_module(path=case3))
    assert result["value_type"] == "powerio.DcOpfSolution"
    assert result["termination"] == "converged"
    assert result["objective"] == pytest.approx(626.9487, abs=1e-3)
    module = json.loads(result["powerio_ir"])
    assert module["schema"] == "pio-ir" and module["version"] == 2


def test_solve_module_writes_to_a_checked_path_and_refuses_overwrite(
    case3, allowed_roots
):
    out = allowed_roots / "solution.pio.json"
    written = asyncio.run(server.solve_module(path=case3, out_path=str(out)))
    assert Path(written["path"]).exists()
    assert "powerio_ir" not in written, "a written module is not also returned inline"
    with pytest.raises(ValueError, match="overwrite"):
        asyncio.run(server.solve_module(path=case3, out_path=str(out)))


# -- studies -----------------------------------------------------------------


@pytest.fixture
def study(case3, allowed_roots) -> str:
    path = allowed_roots / "demo.study.json"
    asyncio.run(
        server.study_create(
            path=str(path),
            request={
                "id": "demo",
                "title": "Demo",
                "formulation": "dcopf",
                "objective": None,
                "decisions": None,
            },
            input_path=case3,
        )
    )
    return str(path)


def test_a_study_round_trips_through_the_filesystem_store(study):
    summary = asyncio.run(server.study_inspect(path=study))
    assert summary["id"] == "demo"
    assert summary["state_count"] >= 1

    ran = asyncio.run(
        server.study_run(
            path=study,
            expected_revision=summary["revision"],
            operation={"kind": "inspect", "state": summary["inspected_state"]},
        )
    )
    assert ran["revision"] == summary["revision"] + 1

    exported = asyncio.run(server.study_export(path=study))
    assert exported["format"] == "tellegen-study"
    assert exported["revision"] == ran["revision"]
    assert len(exported["sha256"]) == 64


def test_a_stale_revision_is_refused(study):
    summary = asyncio.run(server.study_inspect(path=study))
    asyncio.run(
        server.study_run(
            path=study,
            expected_revision=summary["revision"],
            operation={"kind": "inspect", "state": summary["inspected_state"]},
        )
    )
    with pytest.raises(ValueError):
        asyncio.run(
            server.study_run(
                path=study,
                expected_revision=summary["revision"],  # now stale
                operation={"kind": "inspect", "state": summary["inspected_state"]},
            )
        )


def test_apply_is_not_an_agent_operation(study):
    """Binding a recommendation to a Study is a human action, so there is no
    tool for it and the refusal happens before any engine work."""
    with pytest.raises(ValueError, match="explicit user action"):
        asyncio.run(
            server.study_run(
                path=study,
                expected_revision=0,
                operation={"kind": "apply", "proposal": "p", "state": "s", "base_state": "b"},
            )
        )


def test_an_inspected_view_never_reaches_the_model(study):
    """`StudyOperationResult.inspected_view` is a full SolveResponse. It is
    dropped so a Study operation cannot flood a context window."""
    summary = asyncio.run(server.study_inspect(path=study))
    ran = asyncio.run(
        server.study_run(
            path=study,
            expected_revision=summary["revision"],
            operation={"kind": "inspect", "state": summary["inspected_state"]},
        )
    )
    assert "inspected_view" not in ran
    assert "demand_changes" not in ran


def test_study_inspect_pages_a_section_in_bounded_fragments(study):
    page = asyncio.run(server.study_inspect(path=study, section="states"))
    assert page["encoding"] == "json"
    assert page["offset"] == 0
    assert len(page["fragment"]) <= server.STUDY_FRAGMENT_BYTES


def test_an_unknown_section_says_what_the_sections_are(study):
    with pytest.raises(ValueError, match="section must be"):
        asyncio.run(server.study_inspect(path=study, section="nope"))


def test_an_unknown_or_missing_record_id_is_named(study):
    """A missing record used to escape as `KeyError`, which the SDK turned into
    a bare "Error executing tool" with no message."""
    with pytest.raises(ValueError, match="Unknown experiment 'nope'"):
        asyncio.run(server.study_inspect(path=study, section="experiment", record_id="nope"))
    with pytest.raises(ValueError, match="record_id is required"):
        asyncio.run(server.study_inspect(path=study, section="evidence"))


def test_a_study_export_imports_into_a_new_destination(study, allowed_roots):
    exported = asyncio.run(server.study_export(path=study))
    copy = allowed_roots / "copy.study.json"
    imported = asyncio.run(
        server.study_import(source_path=exported["path"], path=str(copy))
    )
    assert imported["id"] == "demo"
    assert copy.exists()


# -- containment and error delivery -------------------------------------------


def test_a_path_outside_the_allowed_roots_is_refused(allowed_roots):
    with pytest.raises(Exception, match="outside"):
        asyncio.run(server.study_export(path=str(allowed_roots.parent / "escape.json")))


@pytest.mark.parametrize(
    ("tool", "arguments", "remedy"),
    [
        (
            "study_run",
            {"path": "s.json", "expected_revision": 0, "operation": {"kind": "apply"}},
            "explicit user action",
        ),
        ("solve", {"powerio_ir": "{}", "formulation": "acopf"}, "formulation must be one of"),
    ],
)
def test_a_refusal_reaches_the_model_with_its_remedy(tool, arguments, remedy):
    """The SDK preserves a tool's message only for `ToolError`; any other
    exception is replaced with a bare "Error executing tool <name>"."""
    with pytest.raises(ToolError) as caught:
        call(tool, **arguments)
    assert remedy in str(caught.value)
    assert not isinstance(caught.value, UnexpectedToolError), (
        "UnexpectedToolError discards the message before the model sees it"
    )


def test_a_bad_timeout_reaches_the_model_as_a_refusal(study):
    """A negative or non-finite deadline used to panic inside the binding; a
    `PanicException` is a `BaseException`, so it escaped the tool wrapper."""
    with pytest.raises(ToolError) as caught:
        call(
            "study_run",
            path=study,
            expected_revision=0,
            operation={"kind": "inspect", "state": "s"},
            timeout_seconds=-1,
        )
    assert "timeout_seconds" in str(caught.value)
    assert not isinstance(caught.value, UnexpectedToolError)
