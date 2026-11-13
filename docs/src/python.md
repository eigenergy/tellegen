# Python

The `tellegen` Python package binds the engine through PyO3 and ships as a
`cp311-abi3` wheel built with maturin. One wheel per platform covers Python
3.11 and newer. The base package has no runtime dependencies: case parsing and
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

Install the published package from PyPI:

```sh
python -m pip install tellegen
```

Wheels are provided for Linux x86_64/aarch64, macOS arm64/x86_64, and Windows
x64. A wheel installation needs no Rust toolchain. To build from a checkout
instead:

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

The optional `mcp` extra adds the `mcp` SDK and the `powerio` wheel. It runs
capability, solve, plan, and Study tools in process over stdio, with a case
index resource and a congestion prompt. [MCP server](mcp.md) covers setup in
Claude Code and Claude Desktop, the tools, and an example session.

```sh
python -m pip install "tellegen[mcp]"
tellegen-mcp                 # or: python -m tellegen.mcp
```

The MCP extra uses the PowerIO 0.12 series of the Python wheel, the line the
compiled engine is built on, so it reads the IR documents the engine writes
(a 0.11 wheel refuses them). Installing the base package does not install or require
the Python PowerIO package.

Case and Study paths are confined to `POWERIO_MCP_ALLOWED_ROOTS`, the policy
shared with PowerMCP. Proposals stay unapplied; applying one is a human action, and
there is no tool for it.

## Versions

`crates/tellegen-py/Cargo.toml` carries the same version as the `tellegen`
crate so that `pip install tellegen==X` and `cargo add tellegen@X` name the
same solver. The release version job updates both manifests together, and
CI checks their parity. After a crate release, the crate workflow dispatches
Python publishing for the same tag. The publisher uploads only the wheels
and source archive that passed the Python gates.

## Source layout

- `crates/tellegen-py/`: the PyO3 extension, a JSON-in/JSON-out seam that
  mirrors `crates/tellegen-wasm` and releases the GIL around every solve.
- `python/tellegen/`: the pure-Python `load`, `Case`, and error layer, plus
  the optional `tellegen.mcp` server.
- `python/tests/`: pytest suites run against the installed wheel in CI on
  Python 3.11 and 3.13, with wheels built for Linux, macOS, and Windows.
