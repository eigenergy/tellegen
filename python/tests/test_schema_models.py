"""Generated engine argument shapes, tested without loading the native module."""

import importlib.util
from pathlib import Path

import pytest
from pydantic import ValidationError

_SPEC = importlib.util.spec_from_file_location(
    "tellegen_schema_models",
    Path(__file__).parents[1] / "tellegen/mcp/_schema_models.py",
)
_MODELS = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(_MODELS)


@pytest.mark.parametrize("node", [False, {"type": "number", "multipleOf": 2}, {"type": "string", "format": "email"}, {"$ref": "https://example.com/schema"}])
def test_new_schema_constraints_require_explicit_support(node):
    with pytest.raises(ValueError, match="Unsupported Rust schema"):
        _MODELS._type(node, "FutureInput")


def test_rust_defaults_and_tagged_sensitivity_enums():
    edits = _MODELS.model("Edits").model_validate({"deltas": {"2": 30}})
    assert edits.model_dump() == {"deltas": {"2": 30.0}, "rates": {}}
    request = _MODELS.model("SensRequest").model_validate(
        {"operand": {"Price": "Active"}, "parameter": {"Demand": "Active"}}
    )
    assert request.model_dump(exclude_none=True) == {
        "operand": {"Price": "Active"},
        "parameter": {"Demand": "Active"},
        "mode": "Auto",
    }
    with pytest.raises(ValidationError):
        _MODELS.model("SensRequest").model_validate(
            {"operand": {"Price": "Invented"}, "parameter": "LineLimit"}
        )
    with pytest.raises(ValidationError):
        _MODELS.model("SensRequest").model_validate(
            {"operand": {"Price": "Active"}, "parameter": "LineLimit", "indices": [-1]}
        )


def test_planning_limits_and_unknown_fields_are_checked():
    spec = {
        "objective": {"kind": "weighted_lmp", "weights": [{"bus": 2, "weight": 1}]},
        "candidates": ["line"],
        "max_increase_per_branch_mw": 20,
        "budget_mw": 20,
        "increment_mw": 2,
        "max_changed_lines": 1,
        "exact_solve_budget": 5,
    }
    assert _MODELS.model("CapacityPlanSpec").model_validate(spec).budget_mw == 20
    for bad in [{"budget_mw": 0}, {"candidates": []}, {"exact_solve_budget": 0}]:
        with pytest.raises(ValidationError):
            _MODELS.model("CapacityPlanSpec").model_validate({**spec, **bad})
    with pytest.raises(ValidationError):
        _MODELS.model("Edits").model_validate({"delta": {"2": 30}})
