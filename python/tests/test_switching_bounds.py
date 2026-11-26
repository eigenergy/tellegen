from pathlib import Path

import powerio
import pytest
import tellegen
from tellegen._switching import _path_bounds


def test_line_with_empty_on_state_can_open(tmp_path):
    fixture = Path(__file__).resolve().parents[2] / "validation/evidence/studies/case3.m"
    # With x=1, an angle of at least 30 degrees requires more than the 0.1 pu
    # rating even at flat voltage. Bus 2 can still be supplied over line 1-2.
    case = fixture.read_text().replace(
        "2 3 0.01 0.1 0 60 60 60 0 0 1 -360 360;",
        "2 3 0 1 0 10 10 10 0 0 1 30 40;",
    )
    path = tmp_path / "case3.m"
    path.write_text(case)
    module = powerio.parse(path)
    ir = powerio.serialize(module.to_ac_ots_instance()).text
    native = tellegen.solve_ots(ir, restore=False)["best"]
    assert native["closed"] == [True, True, False]
    result = tellegen.solve_lacots_gap(ir)
    for name in ("integer", "relaxed"):
        solve = result[name]
        assert solve["status"] == "optimal"
        assert solve["closed"] == pytest.approx([1, 1, 0])
        assert solve["objective"] == pytest.approx(native["objective"], abs=5e-4)

    ir = powerio.serialize(module.to_ac_ots_instance(switchable="none")).text
    result = tellegen.solve_lacots_gap(ir)
    for name in ("integer", "relaxed"):
        assert result[name]["status"] == "infeasible"
        assert result[name]["objective"] is None


def test_path_bounds_exclude_dominant_edge_without_cancellation():
    bounds = [(0, 1e18), (-7, 0), (0, 7), None]
    global_bound, off = _path_bounds(bounds, 2)
    assert global_bound == 1e18
    assert off == [14, 1e18, 1e18, 1e18]


def test_positive_gap_with_parallel_lines_and_an_opening_cost(tmp_path):
    path = tmp_path / "parallel.m"
    path.write_text("""function mpc = parallel
mpc.version = '2';
mpc.baseMVA = 100;
mpc.bus = [
1 3   0 0 0 0 1 1 0 230 1 1 1;
2 2 100 0 0 0 1 1 0 230 1 1 1;
];
mpc.gen = [
1 0 0 100 -100 1 100 1 100 0;
2 0 0 100 -100 1 100 1 100 0;
];
mpc.branch = [
1 2 0 0.1 0  10  10  10 0 0 1 -90 90;
1 2 0 0.1 0 200 200 200 0 0 1 -90 90;
];
mpc.gencost = [
2 0 0 2 1 0;
2 0 0 2 2 0;
];
""")
    module = powerio.parse(path)
    weak, _ = module.to_ac_ots_instance().value.branch_order
    instance = module.to_ac_ots_instance(switchable=[weak], switching_costs={weak: (20, 0)})
    ir = powerio.serialize(instance).text
    result = tellegen.solve_lacots_gap(ir)
    # Integer dispatch opens the 10 MW line for 20 and imports 100 MW for 100.
    # The relaxation carries that load with z=5/9 and pays just 4/9 of the opening cost.
    integer_cost = 120
    relaxed_cost = 100 + 20 * 4 / 9
    difference = integer_cost - relaxed_cost
    assert tellegen.solve_ots(ir, restore=False)["best"]["objective"] == pytest.approx(integer_cost)
    assert result["integer"]["objective"] == pytest.approx(integer_cost)
    assert result["relaxed"]["closed"] == pytest.approx([5 / 9, 1])
    assert result["relaxed"]["objective"] == pytest.approx(relaxed_cost)
    lower, upper = result["gap_interval"]
    assert lower <= difference <= upper
    assert upper - lower < 1e-3
    assert result["gap_percent"] == pytest.approx(100 * difference / integer_cost)


def test_cost_cancellation_near_zero_does_not_create_a_percentage_gap(tmp_path):
    path = tmp_path / "nearzero.m"
    path.write_text("""function mpc = nearzero
mpc.version = '2';
mpc.baseMVA = 1;
mpc.bus = [
1 3 0  0 0 0 1 1 0 10 1 1 1;
2 2 .3 0 0 0 1 1 0 10 1 1 1;
3 1 .2 0 0 0 1 1 0 10 1 1 1;
];
mpc.gen = [
1 .5 0 0 0 1 1 1 2 0;
2 0  0 0 0 1 1 1 2 0;
];
mpc.branch = [
1 2 0 1 0 0 0 0 0 0 1 -360 360;
2 3 0 1 0 0 0 0 0 0 1 -360 360;
1 3 0 1 0 0 0 0 0 0 1 -360 360;
];
mpc.gencost = [
2 0 0 3 1 1 -.749999;
2 0 0 2 3 0;
];
""")
    # Both models can serve all 0.5 pu from generator one. A large negative
    # cost offset leaves just 1e-6, below the resolution of a gap percentage.
    ir = powerio.serialize(powerio.parse(path)).text
    result = tellegen.solve_lacots_gap(ir)
    for name in ("integer", "relaxed"):
        assert result[name]["status"] == "optimal"
        assert result[name]["objective"] == pytest.approx(1e-6, abs=1e-7)
    assert result["gap_interval"][0] == 0
    assert result["gap_percent"] is None
