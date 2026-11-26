import json
from pathlib import Path

import powerio
import pytest
import tellegen


@pytest.mark.parametrize(
    ("input_kind", "model"),
    [("network", "acots"), ("ac_opf", "lacots"), ("ac_ots", "dcots")],
)
def test_solution_records_the_instance_used_for_dispatch(input_kind, model):
    case = Path(__file__).resolve().parents[2] / "validation/evidence/studies/case3.m"
    module = powerio.parse(case)
    if input_kind == "ac_opf":
        module = module.to_ac_opf_instance()
    elif input_kind == "ac_ots":
        module = module.to_ac_ots_instance(max_switched=0)

    ir = powerio.serialize(module).text
    expected = json.loads(ir).get("history", [])
    if input_kind != "ac_ots":
        expected = json.loads(powerio.serialize(module.to_ac_ots_instance()).text)["history"]
    solution = tellegen.solve_ots(ir, model=model, topology_limit=1)["ac_solution_ir"]
    history = json.loads(solution)["history"]
    assert history[:-1] == expected
    assert history[-1]["name"] == "ac_ots_solution"
    assert history[-1]["input_type"] == "powerio.AcOtsInstance"
    assert history[-1]["output_type"] == "powerio.AcOtsSolution"
