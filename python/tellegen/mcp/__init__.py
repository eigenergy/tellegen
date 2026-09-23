"""The Tellegen MCP server.

Kept out of `tellegen/__init__.py` so `import tellegen` never reaches for the
MCP SDK: the solver is usable with no extras installed. Needs `tellegen[mcp]`,
which adds `mcp>=2,<3` and the `powerio` wheel, and Python 3.10 or newer —
`mcp` 2.x requires it, while the base package still supports 3.9.
"""

from __future__ import annotations

import sys
from typing import Any

__all__ = ["main", "mcp"]

if sys.version_info < (3, 10):  # pragma: no cover - guarded by the env marker
    raise ImportError(
        "the tellegen MCP server needs Python 3.10 or newer, because the `mcp` "
        f"SDK does; this interpreter is {sys.version_info.major}.{sys.version_info.minor}. "
        "The solver itself supports 3.9: `import tellegen`."
    )


def __getattr__(name: str) -> Any:
    """Resolve `main` and `mcp` lazily, so importing this package is cheap.

    The SDK is reached only here, which is what keeps `import tellegen` free of
    it. A missing extra therefore surfaces at first use as a bare
    `ModuleNotFoundError: No module named 'mcp'`; it is rewritten to name what
    to install.
    """
    if name in __all__:
        try:
            from . import server
        except ModuleNotFoundError as exc:
            raise ImportError(
                f"the tellegen MCP server needs the `mcp` extra "
                f"(missing: {exc.name}). Install it with: pip install 'tellegen[mcp]'"
            ) from exc
        return getattr(server, name)
    raise AttributeError(f"module {__name__!r} has no attribute {name!r}")
