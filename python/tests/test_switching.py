"""Exercise the installed switching wheel across the PowerIO IR boundary."""

import io
from pathlib import Path

import pytest
import tellegen

CASE = Path(__file__).resolve().parents[2] / "validation/evidence/studies/case3.m"


@pytest.mark.parametrize("model", ["dcots", "lacots", "acots"])
def test_typed_switching_roundtrip(model):
    import powerio

    instance = powerio.parse(CASE).to_ac_ots_instance(max_switched=0)
    report = tellegen.solve_ots(powerio.serialize(instance).text, model=model)
    assert report["exhausted"] and report["visited"] == 1
    assert report["best"]["closed"] == [True] * 3
    assert report["branch_ids"] == [row["id"] for row in instance.value.branches]
    solution = powerio.deserialize(io.StringIO(report["ac_solution_ir"])).value
    assert isinstance(solution, powerio.AcOtsSolution)
    assert solution.branch_closed == report["best"]["closed"]
    assert solution.instance.branch_order == report["branch_ids"]
    assert isinstance(solution.ac_solution, powerio.AcOpfSolution)
    assert solution.instance.initial_closed == [True] * len(report["branch_ids"])
    if model == "acots":
        assert solution.objective == pytest.approx(report["best"]["objective"])
    else:
        assert report["restoration"]["status"] == "ac_feasible"
        assert solution.objective == pytest.approx(report["restoration"]["objective"])


def test_unrestricted_switching_and_limit():
    ir = tellegen.load(CASE).module_json
    complete = tellegen.solve_ots(ir, restore=False)
    assert complete["visited"] == 8 and complete["exhausted"]
    assert complete["best"] and complete["ac_solution_ir"] is None
    short = tellegen.solve_ots(ir, topology_limit=1, restore=False)
    assert short["visited"] == 1 and not short["exhausted"]
    with pytest.raises(tellegen.TellegenInputError):
        tellegen.solve_ots(ir, topology_limit=0)


def test_lacots_relaxation_against_exhaustive_search():
    ir = tellegen.load(CASE).module_json
    gap = tellegen.solve_lacots_gap(ir)
    binary, relaxed = gap["integer"], gap["relaxed"]
    assert binary["status"] == relaxed["status"] == "optimal"
    expected = tellegen.solve_ots(ir, restore=False)["best"]["objective"]
    assert binary["objective"] == pytest.approx(expected, abs=5e-4)
    assert relaxed["objective"] <= binary["objective"] + 5e-4
    assert gap["gap_percent"] == pytest.approx(0, abs=1e-4)
    assert all(min(abs(z), abs(z - 1)) < 1e-6 for z in binary["closed"])


def test_relaxation_preserves_fixed_topology_and_rejects_islanding():
    import powerio

    module = powerio.parse(CASE)
    fixed = powerio.serialize(module.to_ac_ots_instance(switchable="none")).text
    report = tellegen.solve_lacots_gap(fixed)
    assert report["integer"]["closed"] == report["relaxed"]["closed"] == [1.0] * 3
    islands = powerio.serialize(module.to_ac_ots_instance(islanding="allowed")).text
    with pytest.raises(tellegen.TellegenError, match="forbidden islanding"):
        tellegen.solve_lacots_gap(islands)
