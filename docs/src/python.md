# Python

The `tellegen` Python package binds the engine through PyO3 and ships as a
`cp39-abi3` wheel built with maturin. One wheel per platform covers Python
3.9 and newer. The base package has no runtime dependencies: case parsing and
the solvers are compiled in, so `import tellegen` never reaches for numpy,
scipy, or the `powerio` wheel.

```python
import tellegen

case = tellegen.load("case30.m")              # parsed in Rust
solution = case.solve()                        # dcpf | dcopf | acpf | socwr
solution = case.update(demand={2: 50.0})       # absolute deltas, exact re-solve
column = case.sensitivity({"Price": "Active"}, {"Demand": "Active"})
```

Demand and rating edits are absolute deltas from the base case, the same
convention the browser protocol uses: `update(demand={2: 50})` twice is the
same state as once. `reset()` returns to the base case. Engine errors arrive as
`TellegenError` subclasses with a stable `.code` such as `SOLVE.INFEASIBLE` or
`EDIT.UNKNOWN_ELEMENT`; a panic that crosses the boundary is converted to a
`TellegenError` with code `BIND.PY.PANIC` rather than escaping as a
`BaseException`.

## Installing

The wheel is not on PyPI yet; publishing it is tracked in
[#139](https://github.com/eigenergy/tellegen/issues/139). Until then, build it
from a checkout:

```sh
python -m pip install "maturin>=1.7,<2.0"
maturin build --profile release-py --out dist
python -m pip install dist/tellegen-*.whl
```

Install the wheel rather than running `maturin develop`: `python/tellegen`
holds no compiled extension, so a develop install on `sys.path` would shadow
the wheel with a source tree that cannot import its own extension.

The `release-py` profile builds the engine at `opt-level = 3`. The workspace
`release` profile is `opt-level = "s"` for the browser bundle, which is the
wrong trade for a native solver.

## MCP server

`pip install "tellegen[mcp]"` adds the `mcp` SDK and the `powerio` wheel, and
needs Python 3.10 or newer. It exposes the same capability, solve, plan, and
Study tools as the CLI contract in [Persistent Studies](studies.md), in
process:

```sh
tellegen-mcp                 # or: python -m tellegen.mcp
```

Study paths are confined to `POWERIO_MCP_ALLOWED_ROOTS`, the policy shared
with PowerMCP. Proposals stay unapplied; applying one is a human action, and
there is no tool for it.

## Versions

`crates/tellegen-py/Cargo.toml` carries the same version as the `tellegen`
crate so that `pip install tellegen==X` and `cargo add tellegen@X` name the
same solver. release-plz does not bump the binding crate; the Python CI gate
fails when the two versions differ, so a crate release must bump both.

## Source layout

- `crates/tellegen-py/`: the PyO3 extension, a JSON-in/JSON-out seam that
  mirrors `crates/tellegen-wasm` and releases the GIL around every solve.
- `python/tellegen/`: the pure-Python `load`, `Case`, and error layer, plus
  the optional `tellegen.mcp` server.
- `python/tests/`: pytest suites run against the installed wheel in CI on
  Python 3.9 and 3.13, with wheels built for Linux, macOS, and Windows.
