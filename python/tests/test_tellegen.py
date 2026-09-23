"""Tests for the tellegen Python package.

These run against the INSTALLED wheel, not the source tree: `python/tellegen`
cannot satisfy `import tellegen` on its own because the compiled extension is
dropped into the package at build time. The CI gate installs the wheel and runs
pytest from a directory where the repo tree cannot shadow it.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

import pytest
import tellegen

# evidence/studies/case3.m: a three-bus MATPOWER case already in the repo, small
# enough to reason about and congested once demand moves.
CASE3 = Path(__file__).resolve().parents[2] / "evidence" / "studies" / "case3.m"


@pytest.fixture(scope="module")
def case3_path() -> Path:
    if not CASE3.exists():  # pragma: no cover - a source checkout always has it
        pytest.skip(f"fixture missing: {CASE3}")
    return CASE3


@pytest.fixture
def case(case3_path: Path) -> tellegen.Case:
    return tellegen.load(case3_path)


# -- packaging ---------------------------------------------------------------


def test_the_base_import_pulls_in_no_numeric_stack():
    """`import tellegen` must stay dependency free; arrays are an extra."""
    assert "numpy" not in sys.modules
    assert "scipy" not in sys.modules


def test_versions_separates_the_binding_from_the_engine():
    versions = tellegen.versions()
    assert set(versions) == {"tellegen", "engine", "powerio_ir"}
    # The two can drift: nothing bumps the binding automatically, so they are
    # reported separately rather than conflated into one number.
    assert versions["powerio_ir"] == "pio-ir/2"


def test_capabilities_reports_acopf_as_unavailable():
    """`acopf` is a wire-stable tag that this build never solves."""
    by_name = {entry["formulation"]: entry for entry in tellegen.capabilities()}
    assert by_name["acopf"]["available"] is False
    assert tellegen.formulations() == ("dcpf", "dcopf", "acpf", "socwr")
    assert "acopf" not in tellegen.FORMULATIONS


# -- loading -----------------------------------------------------------------


def test_load_infers_the_format_from_the_extension(case3_path: Path):
    case = tellegen.load(case3_path)
    module = json.loads(case.module_json)
    assert module["schema"] == "pio-ir"
    assert module["version"] == 2
    assert module["value"]["type"] == "powerio.BalancedNetwork"


def test_an_unknown_extension_asks_for_an_explicit_format(tmp_path: Path):
    stray = tmp_path / "case.unknown"
    stray.write_bytes(b"not a case")
    with pytest.raises(tellegen.TellegenInputError, match="cannot infer"):
        tellegen.load(stray)


def test_load_ir_refuses_a_non_string():
    with pytest.raises(tellegen.TellegenInputError, match="must be a str"):
        tellegen.load_ir({"schema": "pio-ir"})  # type: ignore[arg-type]


def test_a_bad_format_token_is_an_input_error_with_a_code(case3_path: Path):
    with pytest.raises(tellegen.TellegenError) as caught:
        tellegen.load(case3_path, format="no-such-format")
    assert caught.value.code == "FORMAT.UNKNOWN"


# -- solving -----------------------------------------------------------------


def test_the_base_case_solves_as_dc_opf(case: tellegen.Case):
    solution = case.solve()
    assert solution["formulation"] == "dcopf"
    assert solution["status"] == "optimal"
    assert solution["objective"] == pytest.approx(626.9487, abs=1e-3)
    # Uncongested, so one price clears the whole network.
    prices = [bus["value"] for bus in solution["lmp"]]
    assert prices == pytest.approx([prices[0]] * len(prices))


def test_a_demand_edit_congests_the_network_and_splits_prices(case: tellegen.Case):
    before = [bus["value"] for bus in case.solve()["lmp"]]
    after = [bus["value"] for bus in case.update(demand={2: 30.0})["lmp"]]
    assert len(set(round(p, 6) for p in after)) > 1, "expected prices to separate"
    assert max(after) > max(before)


def test_edits_are_absolute_not_cumulative(case: tellegen.Case):
    """Applying the same edit twice is the same state, as in the browser."""
    once = case.update(demand={2: 30.0})["objective"]
    twice = case.update(demand={2: 30.0})["objective"]
    assert once == pytest.approx(twice)
    assert case.edits["demand"] == {"2": 30.0}


def test_a_zero_delta_removes_the_edit(case: tellegen.Case):
    case.update(demand={2: 30.0})
    assert case.edits["demand"] == {"2": 30.0}
    case.update(demand={2: 0.0})
    assert case.edits["demand"] == {}


def test_reset_returns_to_the_base_case(case: tellegen.Case):
    base = case.solve()["objective"]
    case.update(demand={2: 30.0})
    assert case.reset()["objective"] == pytest.approx(base)
    assert case.edits == {"demand": {}, "ratings": {}}


def test_update_needs_something_to_do(case: tellegen.Case):
    with pytest.raises(tellegen.TellegenInputError, match="demand"):
        case.update()


def test_a_non_finite_delta_is_refused(case: tellegen.Case):
    with pytest.raises(tellegen.TellegenInputError, match="finite"):
        case.update(demand={2: float("nan")})


def test_an_unknown_bus_is_an_edit_error_with_a_code(case: tellegen.Case):
    with pytest.raises(tellegen.TellegenError) as caught:
        case.update(demand={99999: 10.0})
    assert caught.value.code in {"EDIT.UNKNOWN_ELEMENT", "ENGINE.FAILED"}


def test_demand_cannot_go_negative(case: tellegen.Case):
    """An engine invariant, enforced by the engine and surfaced with a code."""
    with pytest.raises(tellegen.TellegenError) as caught:
        case.update(demand={2: -1e9})
    assert caught.value.code == "EDIT.OUT_OF_RANGE"


# -- formulations ------------------------------------------------------------


def test_switching_formulation_reuses_the_network(case: tellegen.Case):
    dcopf = case.solve()["objective"]
    case.formulation = "socwr"
    socwr = case.solve()
    assert socwr["formulation"] == "socwr"
    assert socwr["status"] == "optimal"
    # A relaxation of a different problem: close to, but not equal to, DC OPF.
    assert socwr["objective"] != pytest.approx(dcopf, abs=1e-6)


def test_ac_power_flow_returns_voltages_and_no_prices(case: tellegen.Case):
    case.formulation = "acpf"
    solution = case.solve()
    assert solution["status"] == "feasible"
    assert "vm" in solution and solution["vm"]
    assert solution.get("lmp") is None, "AC power flow has no prices"


def test_an_unavailable_formulation_is_refused_before_solving(case: tellegen.Case):
    with pytest.raises(tellegen.TellegenInputError, match="formulation must be"):
        case.formulation = "acopf"


def test_rating_edits_are_rejected_by_dc_power_flow(case: tellegen.Case):
    case.formulation = "dcpf"
    with pytest.raises(tellegen.TellegenError) as caught:
        case.update(ratings={1: -5.0})
    assert caught.value.code == "EDIT.OUT_OF_RANGE"


# -- sensitivities -----------------------------------------------------------


def test_the_price_demand_column_comes_back_with_units_and_metadata(
    case: tellegen.Case,
):
    matrix = case.sensitivity({"Price": "Active"}, {"Demand": "Active"})
    assert matrix["units"] == "(objective_unit/MW)/MW"
    assert len(matrix["values"]) == len(matrix["rows"])
    assert len(matrix["values"][0]) == len(matrix["cols"])
    # Column metadata is how a caller maps a dense index back to an identity.
    assert "element" in matrix["cols"][0]
    assert "index" in matrix["cols"][0]


def test_a_sensitivity_request_does_not_disturb_the_committed_solution(
    case: tellegen.Case,
):
    before = case.solution["objective"]
    case.sensitivity({"Price": "Active"}, {"Demand": "Active"})
    assert case.solution["objective"] == pytest.approx(before)


def test_ac_power_flow_has_no_price_sensitivity(case: tellegen.Case):
    case.formulation = "acpf"
    with pytest.raises(tellegen.TellegenError):
        case.sensitivity({"Price": "Active"}, {"Demand": "Active"})


def test_repr_says_what_the_case_is_holding(case: tellegen.Case):
    assert "dcopf" in repr(case)
    case.update(demand={2: 10.0})
    assert "edits=1" in repr(case)


# -- error classification ----------------------------------------------------
#
# The engine returns free-form error strings with no enum at its edge, so the
# binding maps message substrings onto codes. That mapping is fragile by
# construction, so every arm is pinned here: a reworded engine message should
# fail this test rather than silently reclassify at runtime. Tested through the
# loaded extension because a cdylib-only pyo3 crate has no portable cargo-test
# harness (see `_classify`'s docstring).

CLASSIFICATIONS = [
    ("DC OPF solve infeasible", "SOLVE.INFEASIBLE"),
    ("DC OPF solve unbounded", "SOLVE.UNBOUNDED"),
    ("DC OPF solve did not converge", "SOLVE.NOT_CONVERGED"),
    ("DC OPF solve cancelled", "SOLVE.CANCELLED"),
    ("bad request JSON: trailing comma", "REQUEST.MALFORMED"),
    ("unknown demand delta bus 42", "EDIT.UNKNOWN_ELEMENT"),
    ("unknown rating delta branch 7", "EDIT.UNKNOWN_ELEMENT"),
    ("edit would make demand negative", "EDIT.OUT_OF_RANGE"),
    ("edit would make the line limit non-positive", "EDIT.OUT_OF_RANGE"),
    ("branch rating edits are not supported by dcpf", "EDIT.OUT_OF_RANGE"),
    ("acpf does not support d(Price)/d(Demand)", "SENS.UNSUPPORTED"),
    (
        "acopf (full nonlinear AC OPF) is not available in this build",
        "FORMULATION.UNAVAILABLE",
    ),
    ("socwr requires the `conic` feature", "FORMULATION.UNAVAILABLE"),
    (
        "PowerIO module holds powerio.TimeSeries, which this solve entry does not support",
        "MODULE.UNSUPPORTED",
    ),
    ("duplicate bus uid buses:1", "MODULE.UNSUPPORTED"),
    ('unknown PowerIO format "nope"', "FORMAT.UNKNOWN"),
]


@pytest.mark.parametrize(("message", "code"), CLASSIFICATIONS)
def test_every_documented_engine_message_classifies(message: str, code: str):
    from tellegen import _tellegen

    assert _tellegen._classify(message) == code


def test_an_unrecognized_message_is_a_generic_engine_failure():
    from tellegen import _tellegen

    assert _tellegen._classify("something new went wrong") == "ENGINE.FAILED"


@pytest.mark.parametrize("seconds", [-1.0, float("nan"), float("inf"), 1e300])
def test_a_bad_study_timeout_is_refused_before_any_work(tmp_path, seconds):
    """`Duration::from_secs_f64` panics on these; the binding must refuse them
    as input errors instead, and before it touches the store."""
    from tellegen import TellegenInputError, _tellegen

    missing = str(tmp_path / "absent.study.json")
    with pytest.raises(TellegenInputError) as caught:
        _tellegen.study_run(missing, '{"expected_revision": 0, "operation": {"kind": "inspect", "state": "s"}}', seconds)
    assert caught.value.code == "REQUEST.MALFORMED"
    assert "timeout_seconds" in str(caught.value)
