"""Power flow and optimal power flow in the browser's engine, from Python.

`load` reads a case file, `Case` holds the solved operating point, and the
edit methods re-solve it. Results come back as plain dicts, so nothing here
imports numpy or scipy.

    >>> import tellegen
    >>> case = tellegen.load("case30.m")
    >>> sol = case.solve()
    >>> sol["objective"]  # doctest: +SKIP
    576.89
"""

from __future__ import annotations

import json
import os
from typing import Any, Dict, Iterable, Mapping, Optional, Sequence, Union

from . import _tellegen
from ._guard import guard, guard_class

TellegenError = _tellegen.TellegenError
TellegenInputError = _tellegen.TellegenInputError
TellegenSolveError = _tellegen.TellegenSolveError
PANIC_CODE = _tellegen.PANIC_CODE

__version__ = _tellegen.__version__

__all__ = [
    "PANIC_CODE",
    "Case",
    "TellegenError",
    "TellegenInputError",
    "TellegenSolveError",
    "__version__",
    "capabilities",
    "formulations",
    "load",
    "load_ir",
    "resolve_format",
    "versions",
]

#: Formulations this build can solve. `acopf` is a wire-stable tag that is never
#: available; it is deliberately absent here.
FORMULATIONS = ("dcpf", "dcopf", "acpf", "socwr")

# Extension -> PowerIO format token. `.epc` is PSLF, whose token is not its
# extension; everything else is spelled the same as the suffix.
_EXTENSION_FORMATS = {
    ".m": "matpower",
    ".raw": "psse",
    ".aux": "aux",
    ".epc": "pslf",
    ".pwb": "pwb",
    ".json": "json",
}

_PathLike = Union[str, "os.PathLike[str]"]


def _format_for(path: str, explicit: Optional[str]) -> str:
    if explicit is not None:
        return explicit
    suffix = os.path.splitext(path)[1].lower()
    token = _EXTENSION_FORMATS.get(suffix)
    if token is None:
        raise TellegenInputError(
            f"cannot infer a PowerIO format from {suffix!r}; pass format=..."
        )
    return token


@guard
def resolve_format(token: str) -> Optional[str]:
    """The canonical spelling of a PowerIO format token, or None if unknown."""
    return _tellegen.resolve_format(token)


@guard
def capabilities() -> Any:
    """What this build supports, per formulation.

    A list of `{formulation, available, blocks, operands, parameters}`. Any
    (operand, parameter) pair listed for a formulation is a valid sensitivity
    request against it.
    """
    return json.loads(_tellegen.capabilities_json())


@guard
def formulations() -> tuple:
    """The formulations this build can actually solve, in capability order."""
    return tuple(
        entry["formulation"] for entry in capabilities() if entry.get("available")
    )


@guard
def versions() -> Dict[str, str]:
    """Versions behind this wheel: the binding, the engine crate, and PowerIO IR."""
    return {
        "tellegen": _tellegen.__version__,
        "engine": _tellegen.ENGINE_VERSION,
        "powerio_ir": "pio-ir/2",
    }


@guard
def load(source: _PathLike, *, format: Optional[str] = None) -> "Case":
    """Read a case file and solve it.

    `source` is a path. The format is inferred from the extension unless
    `format` names a PowerIO token explicitly. Parsing happens in Rust through
    the compiled PowerIO reader, so no `powerio` wheel is required.
    """
    path = os.fspath(source)
    token = _format_for(path, format)
    with open(path, "rb") as handle:
        raw = handle.read()
    return load_ir(_tellegen.parse_case(raw, token))


@guard
def load_ir(module_json: str) -> "Case":
    """Wrap a stored PowerIO `pio-ir` module that is already in hand.

    This is the boundary the engine speaks, so it is also how a module produced
    by the `powerio` wheel (`powerio.serialize(module).text`) enters tellegen.
    """
    if not isinstance(module_json, str):
        raise TellegenInputError("module_json must be a str of PowerIO IR")
    return Case(module_json)


@guard_class
class Case:
    """A network and its solved operating point.

    Demand and rating edits are absolute deltas from the base case, the way the
    browser's protocol treats them: `update(demand={2: 50})` twice is the same
    state as once, not 100 MW. `reset()` returns to the base case.
    """

    __slots__ = ("_module_json", "_formulation", "_demand", "_ratings", "_solution")

    def __init__(self, module_json: str, formulation: str = "dcopf") -> None:
        self._module_json = module_json
        self._formulation = self._checked_formulation(formulation)
        self._demand: Dict[str, float] = {}
        self._ratings: Dict[str, float] = {}
        self._solution: Optional[Dict[str, Any]] = None

    @staticmethod
    def _checked_formulation(name: str) -> str:
        if name not in FORMULATIONS:
            raise TellegenInputError(
                f"formulation must be one of {list(FORMULATIONS)}, not {name!r}"
            )
        return name

    # -- state ---------------------------------------------------------------

    @property
    def formulation(self) -> str:
        """The formulation the next solve will use."""
        return self._formulation

    @formulation.setter
    def formulation(self, name: str) -> None:
        name = self._checked_formulation(name)
        if name != self._formulation:
            self._formulation = name
            self._solution = None

    @property
    def module_json(self) -> str:
        """The stored PowerIO module, as the engine received it."""
        return self._module_json

    @property
    def edits(self) -> Dict[str, Dict[str, float]]:
        """The current absolute edit set, as MW deltas from the base case."""
        return {"demand": dict(self._demand), "ratings": dict(self._ratings)}

    # -- solving -------------------------------------------------------------

    def _request(self, sensitivities: Optional[Sequence[Mapping[str, Any]]]) -> str:
        request: Dict[str, Any] = {"formulation": self._formulation}
        edits: Dict[str, Any] = {}
        if self._demand:
            edits["deltas"] = self._demand
        if self._ratings:
            edits["rates"] = self._ratings
        if edits:
            request["edits"] = edits
        if sensitivities:
            request["sensitivities"] = list(sensitivities)
        return json.dumps(request)

    def solve(
        self, *, sensitivities: Optional[Sequence[Mapping[str, Any]]] = None
    ) -> Dict[str, Any]:
        """Solve at the current edit set and return the response.

        The response carries only the blocks the formulation defines, so read
        `capabilities()` rather than assuming `lmp` is present: AC power flow has
        no prices, and DC power flow has neither prices nor dispatch.
        """
        raw = _tellegen.solve_module(self._module_json, self._request(sensitivities))
        solution = json.loads(raw)
        if sensitivities is None:
            self._solution = solution
        return solution

    @property
    def solution(self) -> Dict[str, Any]:
        """The solved response at the current edit set, solving if needed."""
        if self._solution is None:
            self.solve()
        assert self._solution is not None
        return self._solution

    def update(
        self,
        *,
        demand: Optional[Mapping[Any, float]] = None,
        ratings: Optional[Mapping[Any, float]] = None,
    ) -> Dict[str, Any]:
        """Set demand and rating deltas absolutely, then re-solve exactly.

        Keys are bus or branch identities: an integer id, or a PowerIO uid
        string such as `"buses:1"`. A delta of 0 removes the edit.
        """
        if demand is None and ratings is None:
            raise TellegenInputError("pass demand=..., ratings=..., or both")
        if demand is not None:
            self._demand = self._checked_deltas(demand)
        if ratings is not None:
            self._ratings = self._checked_deltas(ratings)
        self._solution = None
        return self.solve()

    @staticmethod
    def _checked_deltas(deltas: Mapping[Any, float]) -> Dict[str, float]:
        checked: Dict[str, float] = {}
        for key, value in deltas.items():
            number = float(value)
            if number != number or number in (float("inf"), float("-inf")):
                raise TellegenInputError(f"delta for {key!r} must be finite")
            if number == 0.0:
                continue
            checked[str(key)] = number
        return checked

    def reset(self) -> Dict[str, Any]:
        """Drop every edit, returning to the base case, and re-solve."""
        self._demand = {}
        self._ratings = {}
        self._solution = None
        return self.solve()

    def sensitivity(
        self,
        operand: Mapping[str, Any],
        parameter: Union[str, Mapping[str, Any]],
        *,
        indices: Optional[Iterable[int]] = None,
        mode: str = "Auto",
    ) -> Dict[str, Any]:
        """One sensitivity cell against the current operating point.

        `operand` and `parameter` use the engine's externally tagged spelling,
        e.g. `{"Price": "Active"}` and `{"Demand": "Active"}`, or the bare string
        `"LineLimit"`. Any pair `capabilities()` lists for this formulation is
        valid.

        Note `indices` are DENSE zero-based positions along the parameter axis
        over in-service elements, not source bus or branch ids. Read the
        returned `cols[].element` to map a column back to an identity.
        """
        cell: Dict[str, Any] = {"operand": dict(operand), "mode": mode}
        cell["parameter"] = parameter if isinstance(parameter, str) else dict(parameter)
        if indices is not None:
            cell["indices"] = [int(i) for i in indices]
        response = self.solve(sensitivities=[cell])
        matrices = response.get("sensitivities") or []
        if not matrices:
            raise TellegenError(
                "the engine returned no sensitivity matrix for that cell; "
                "check capabilities() for this formulation"
            )
        return matrices[0]

    def __repr__(self) -> str:
        edits = len(self._demand) + len(self._ratings)
        return (
            f"<tellegen.Case formulation={self._formulation!r} "
            f"edits={edits} solved={self._solution is not None}>"
        )
