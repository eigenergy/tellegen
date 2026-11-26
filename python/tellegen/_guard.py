"""Convert a Rust panic into an ordinary tellegen error.

PyO3 raises `pyo3_runtime.PanicException` when Rust panics. It derives from
`BaseException`, not `Exception`, so it walks straight through a caller's
`except Exception` and out of their program. The engine has a handful of
`unwrap`/`unreachable!` sites on internal invariants and installs no
`catch_unwind`, so a panic is reachable in principle; every public callable is
wrapped so it surfaces as `TellegenError` with `code == PANIC_CODE` instead.
"""

from __future__ import annotations

import functools
from typing import Any, Callable, TypeVar

from . import _tellegen

_F = TypeVar("_F", bound=Callable[..., Any])


def guard(function: _F) -> _F:
    """Wrap one callable so a Rust panic becomes a `TellegenError`."""

    @functools.wraps(function)
    def wrapper(*args: Any, **kwargs: Any) -> Any:
        try:
            return function(*args, **kwargs)
        except _tellegen._PanicException as exc:
            error = _tellegen.TellegenError(
                f"the tellegen engine panicked: {exc}".strip()
            )
            error.code = _tellegen.PANIC_CODE
            raise error from exc

    return wrapper  # type: ignore[return-value]


def guard_class(cls: type) -> type:
    """Wrap every public method of a class with `guard`."""
    for name, value in list(vars(cls).items()):
        if name.startswith("_") and name != "__init__":
            continue
        if isinstance(value, staticmethod):
            continue
        if isinstance(value, property):
            fget = guard(value.fget) if value.fget else None
            fset = guard(value.fset) if value.fset else None
            setattr(cls, name, property(fget, fset, value.fdel, value.__doc__))
        elif callable(value):
            setattr(cls, name, guard(value))
    return cls
