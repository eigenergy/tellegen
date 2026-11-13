"""Typed arguments and declared output schemas for the MCP tools.

The argument models describe what `solve` and `plan` forward to the engine:
the `edits` and `sensitivities` fields of a `SolveRequest`, and a
`CapacityPlanSpec`. Clients see them as JSON Schema in each tool's input
schema. The tools still accept the same object as JSON text, which is what
earlier versions of this server required, so a prompt written against either
form works.

The SDK validates arguments before a tool runs, and its messages for a union of
an object and a string name both branches. The tools therefore take a loosely
typed value, publish the typed schema with `WithJsonSchema`, and validate
inside the tool with `parsed`, whose messages name the field and what to fix.

The output types are what the SDK validates each structured result against and
publishes as `outputSchema`. Every one allows extra keys, and fields that come
from the engine are typed loosely: a result that failed its own declared schema
would reach the model as a bare "Error executing tool", which is worse than an
undeclared field.
"""

from __future__ import annotations

import json
from typing import Any, Dict, List, Literal, Optional, Type, TypeVar, Union

from pydantic import (
    BaseModel,
    ConfigDict,
    Field,
    TypeAdapter,
    ValidationError,
    with_config,
)
from typing_extensions import NotRequired, TypedDict

# ---- argument models ---------------------------------------------------------


class SolveEdits(BaseModel):
    """Operating-point edits applied before the model is built.

    Keys are a bus or branch's original numeric id (as a string) or its
    PowerIO uid, as reported in a solve response.
    """

    model_config = ConfigDict(extra="forbid")

    deltas: Dict[str, float] = Field(
        default_factory=dict,
        description="Active-power demand change in MW per bus key; demand becomes base + delta.",
    )
    rates: Dict[str, float] = Field(
        default_factory=dict,
        description=(
            "Thermal rating change in MW per branch key; the limit becomes base + delta. "
            "DC OPF and SOCWR only."
        ),
    )


class SensitivityRequest(BaseModel):
    """One sensitivity cell: an operand differentiated with respect to a parameter."""

    model_config = ConfigDict(extra="forbid")

    operand: Union[str, Dict[str, Any]] = Field(
        description=(
            'What is differentiated, e.g. {"Price": "Active"} (LMP), {"Dispatch": "Active"}, '
            'or {"Flow": {"power": "Active", "end": "From"}}.'
        ),
    )
    parameter: Union[str, Dict[str, Any]] = Field(
        description=(
            'What it is differentiated with respect to, e.g. {"Demand": "Active"} or "LineLimit".'
        ),
    )
    indices: Optional[List[int]] = Field(
        default=None,
        description="Zero-based parameter columns to compute; omit for the whole axis.",
    )
    mode: Optional[Literal["Forward", "Adjoint", "Auto"]] = Field(
        default=None, description="Differentiation mode; Auto when omitted."
    )


class BusWeight(BaseModel):
    """One bus term of a weighted LMP objective."""

    model_config = ConfigDict(extra="forbid")

    bus: int = Field(
        description="Numeric bus id: the `bus` of a solve response's lmp entry."
    )
    weight: float = Field(description="Coefficient applied to this bus's LMP.")


class WeightedLmpObjective(BaseModel):
    """Minimize the weighted sum of the selected buses' LMPs."""

    model_config = ConfigDict(extra="forbid")

    kind: Literal["weighted_lmp"] = "weighted_lmp"
    weights: List[BusWeight] = Field(min_length=1)


class CapacityPlanSpec(BaseModel):
    """The bounded decision space and search limits of one planning run. Power quantities are MW."""

    model_config = ConfigDict(extra="forbid")

    objective: WeightedLmpObjective
    candidates: List[str] = Field(
        min_length=1,
        description='Branch uids (e.g. "2-3") whose thermal ratings the search may increase.',
    )
    max_increase_per_branch_mw: float = Field(
        gt=0, description="Largest increase for one branch."
    )
    budget_mw: float = Field(
        gt=0, description="Largest total increase across all branches."
    )
    increment_mw: float = Field(gt=0, description="Capacity step tried in one trial.")
    max_changed_lines: int = Field(
        ge=1, description="Most branches the final proposal may change."
    )
    exact_solve_budget: int = Field(
        ge=1,
        description="Most exact OPF solves, counting the baseline and every trial.",
    )


_EXAMPLES = {
    "edits": '{"deltas": {"2": 30.0}, "rates": {"2-3": 20.0}}',
    "sensitivities": '[{"operand": {"Price": "Active"}, "parameter": {"Demand": "Active"}}]',
    "spec": (
        '{"objective": {"kind": "weighted_lmp", "weights": [{"bus": 2, "weight": 1.0}]}, '
        '"candidates": ["2-3"], "max_increase_per_branch_mw": 30, "budget_mw": 30, '
        '"increment_mw": 10, "max_changed_lines": 1, "exact_solve_budget": 6}'
    ),
}

_Model = TypeVar("_Model", bound=BaseModel)


def _inline_refs(schema: Dict[str, Any]) -> Dict[str, Any]:
    """Resolve `$defs` references in place.

    `WithJsonSchema` embeds a schema verbatim inside the tool's argument model,
    whose own `$defs` would not contain these definitions.
    """
    definitions = schema.pop("$defs", {})

    def resolve(node: Any) -> Any:
        if isinstance(node, dict):
            reference = node.get("$ref")
            if isinstance(reference, str) and reference.startswith("#/$defs/"):
                return resolve(definitions[reference.removeprefix("#/$defs/")])
            return {key: resolve(value) for key, value in node.items()}
        if isinstance(node, list):
            return [resolve(item) for item in node]
        return node

    return resolve(schema)


def argument_schema(
    annotation: Any, description: str, *, optional: bool
) -> Dict[str, Any]:
    """The published schema of a typed argument: the object, or the same object as JSON text."""
    typed = _inline_refs(TypeAdapter(annotation).json_schema())
    branches: List[Dict[str, Any]] = [
        typed,
        {
            "type": "string",
            "description": "The same value as JSON text, for older clients.",
        },
    ]
    if optional:
        branches.append({"type": "null"})
    schema: Dict[str, Any] = {"anyOf": branches, "description": description}
    if optional:
        schema["default"] = None
    return schema


def _location(error: Any) -> str:
    return ".".join(str(part) for part in error["loc"]) or "value"


def _invalid(name: str, exc: ValidationError) -> ValueError:
    problems = "; ".join(
        f"{_location(error)}: {error['msg']}" for error in exc.errors()
    )
    return ValueError(
        f"Argument `{name}` is invalid ({problems}). Example: {_EXAMPLES[name]}"
    )


def _decoded(value: Any, name: str) -> Any:
    if not isinstance(value, str):
        return value
    try:
        return json.loads(value)
    except json.JSONDecodeError as exc:
        raise ValueError(
            f"Argument `{name}` must be JSON ({exc}). Pass an object, or JSON text "
            f"such as {_EXAMPLES[name]}"
        ) from exc


def parsed(value: Any, model: Type[_Model], name: str) -> Optional[_Model]:
    """Validate an object argument given as a dict, a model, or JSON text.

    An empty string or `None` means the argument was not given.
    """
    if value is None or value == "":
        return None
    if isinstance(value, model):
        return value
    decoded = _decoded(value, name)
    if decoded is None:
        return None
    if not isinstance(decoded, dict):
        raise ValueError(
            f"Argument `{name}` must be an object, not {type(decoded).__name__}. "
            f"Example: {_EXAMPLES[name]}"
        )
    try:
        return model.model_validate(decoded)
    except ValidationError as exc:
        raise _invalid(name, exc) from exc


def parsed_list(value: Any, model: Type[_Model], name: str) -> List[_Model]:
    """Validate a list-of-objects argument given as a list or JSON text."""
    if value is None or value == "":
        return []
    decoded = _decoded(value, name)
    if decoded is None:
        return []
    if isinstance(decoded, dict):
        # One cell given on its own.
        decoded = [decoded]
    if not isinstance(decoded, list):
        raise ValueError(
            f"Argument `{name}` must be a list, not {type(decoded).__name__}. "
            f"Example: {_EXAMPLES[name]}"
        )
    try:
        return TypeAdapter(List[model]).validate_python(decoded)  # type: ignore[valid-type]
    except ValidationError as exc:
        raise _invalid(name, exc) from exc


EDITS_SCHEMA = argument_schema(
    SolveEdits,
    "Demand and rating edits applied before solving. Example: " + _EXAMPLES["edits"],
    optional=True,
)
SENSITIVITIES_SCHEMA = argument_schema(
    List[SensitivityRequest],
    "Sensitivity cells computed against the solved system, in order (DC OPF). "
    "Example, LMP with respect to demand: " + _EXAMPLES["sensitivities"],
    optional=True,
)
SPEC_SCHEMA = argument_schema(
    CapacityPlanSpec,
    "The planning objective, candidate branches and search limits. Example: "
    + _EXAMPLES["spec"],
    optional=False,
)

# ---- output types ------------------------------------------------------------

_OPEN = ConfigDict(extra="allow")

#: An engine array, or its truncated form when longer than `max_elements`:
#: `{"truncated": true, "count": n, "head": [...]}`.
Rows = Union[List[Any], Dict[str, Any]]


@with_config(_OPEN)
class Diagnostic(TypedDict):
    """One PowerIO diagnostic about the input or output module."""

    code: str
    severity: str
    message: str
    target: Any


@with_config(_OPEN)
class InputTail(TypedDict):
    """What PowerIO read: the value type, any collection selection, and its diagnostics."""

    value_type: str
    selection: Dict[str, Any]
    diagnostics: List[Diagnostic]
    warnings: List[str]


@with_config(_OPEN)
class CapabilitiesOutput(TypedDict):
    """The installed build's versions and formulation support matrix."""

    versions: Dict[str, Any]
    capabilities: List[Dict[str, Any]]


@with_config(_OPEN)
class SolveResponse(TypedDict, total=False):
    """The engine's SolveResponse. Arrays may be truncated to `max_elements`; a
    summary lists the arrays it left out in `omitted`."""

    formulation: str
    status: str
    objective: Optional[float]
    lmp: Rows
    va: Rows
    vm: Rows
    flows: Rows
    dispatch: Rows
    branch_constraints: Rows
    sensitivities: Rows
    omitted: List[str]


@with_config(_OPEN)
class BranchLoading(TypedDict, total=False):
    """A branch's flow against its rating. `relief_per_mw` is set for a binding limit."""

    branch: Any
    uid: Optional[str]
    pf: Optional[float]
    rating: Optional[float]
    loading: Optional[float]
    relief_per_mw: Optional[float]


@with_config(_OPEN)
class LmpRange(TypedDict):
    """The LMP extremes by numeric bus id; `min_index` and `max_index` are their sensitivity columns."""

    min: float
    min_bus: Any
    min_index: int
    max: float
    max_bus: Any
    max_index: int
    mean: float


@with_config(_OPEN)
class SolveSummary(TypedDict):
    """Sizes, the LMP range, and the binding and most loaded branches."""

    buses: int
    branches: int
    generators: int
    lmp: NotRequired[LmpRange]
    binding_count: NotRequired[int]
    binding_branches: NotRequired[List[BranchLoading]]
    most_loaded: List[BranchLoading]


@with_config(_OPEN)
class SolveOutput(InputTail):
    """A solve result, with what PowerIO read on the way in."""

    formulation: str
    summary: SolveSummary
    response: SolveResponse
    detail: Literal["summary", "full"]


@with_config(_OPEN)
class ModuleSummary(TypedDict):
    """A solution module's type, diagnostics, termination and objective."""

    value_type: str
    diagnostics: List[Diagnostic]
    diagnostics_counts: Dict[str, int]
    warnings: List[str]
    termination: NotRequired[str]
    objective: NotRequired[float]


@with_config(_OPEN)
class SolveModuleOutput(ModuleSummary):
    """The solution module inline (`powerio_ir`), where it was written (`path`), or
    why it was left out (`powerio_ir_omitted`)."""

    selection: Dict[str, Any]
    powerio_ir: NotRequired[str]
    powerio_ir_omitted: NotRequired[str]
    path: NotRequired[str]


@with_config(_OPEN)
class PlanOutput(InputTail):
    """The planning outcome; with a proposal, its exact solution module too."""

    plan: Any
    solution: NotRequired[ModuleSummary]
    solution_powerio_ir: NotRequired[str]
    solution_powerio_ir_omitted: NotRequired[str]
    path: NotRequired[str]


@with_config(_OPEN)
class StudySummaryOutput(TypedDict):
    """A Study's identity, revision, states and most recent experiments."""

    id: str
    revision: int
    title: NotRequired[str]
    active_goal: NotRequired[Any]
    inspected_state: NotRequired[Optional[str]]
    recommended_state: NotRequired[Optional[str]]
    applied_state: NotRequired[Optional[str]]
    state_count: NotRequired[int]
    experiment_count: NotRequired[int]
    recent_experiments: NotRequired[List[Any]]
    experiment: NotRequired[Any]
    comparison: NotRequired[Dict[str, Any]]
    progress: NotRequired[Any]


@with_config(_OPEN)
class StudyInspectOutput(StudySummaryOutput):
    """The Study summary, or for another section one bounded JSON fragment of it.

    A fragment carries `encoding`, `offset`, `fragment` and `next_offset`, which
    is null on the last page.
    """

    encoding: NotRequired[str]
    offset: NotRequired[int]
    fragment: NotRequired[str]
    next_offset: NotRequired[Optional[int]]


@with_config(_OPEN)
class StudyExportOutput(TypedDict):
    """The validated bundle's path and digest."""

    path: str
    id: str
    revision: int
    sha256: str
    bytes: int
    format: str
