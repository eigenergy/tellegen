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

## Optimal transmission switching

```python
import io
import powerio
import tellegen

module = powerio.parse("case.m").to_ac_ots_instance()
report = tellegen.solve_ots(powerio.serialize(module).text, model="lacots")
if report["best"]:
    print(dict(zip(report["branch_ids"], report["best"]["closed"])))
if report["ac_solution_ir"]:
    solution = powerio.deserialize(io.StringIO(report["ac_solution_ir"])).value
    print(solution.objective, solution.branches_open)
```

Use `model="dcots"` for DC dispatch or `model="acots"` for nonlinear AC dispatch.
With a plain case, `tellegen.load("case.m").module_json` works directly.
A typed PowerIO instance supplies the switchable branch IDs, any opening
budget and switching costs. The default permits multiple openings. The native
solver enumerates candidate status vectors and minimizes dispatch cost plus
applicable transition costs. Clarabel solves the fixed-topology DC and
flat-start linearized AC problems. POUNCE solves the nonlinear AC problem
and redispatches a linear model's chosen topology when `restore=True`.
The result reports the approximate objective under `best` and the restored
AC objective under `restoration`. If redispatch fails, inspect the error there.

`topology_limit` defaults to 1,000 visited status vectors. Inspect `exhausted`
and `failed` for an incomplete search. AC dispatch uses a local solver, so
even an exhaustive ACOTS search does not certify global AC optimality.

### LACOTS relaxation gap

Install `tellegen[switching]` to compare the connected binary on/off
formulation against the same model with each line status in [0, 1]. This extra
uses SCIP through PySCIPOpt. The native `solve_ots` API requires only Tellegen.

```python
gap = tellegen.solve_lacots_gap(powerio.serialize(module).text, time_limit=120)
print(gap["integer"]["status"], gap["relaxed"]["status"])
print(gap["gap_percent"], gap["gap_interval"])
```

Each solve has its own time limit. For positive integer cost and two optimal
terminations, `gap_percent` is 100 times the cost difference divided by the
integer optimum. Otherwise it is `None`. The absolute gap interval uses both
solvers' primal and dual bounds. A missing endpoint means SCIP did not return
the required bound. This gap quantifies fractional switching within LACOTS.
AC feasibility can be checked separately by redispatching the selected topology.

The perspective model accepts one connected network with initially closed
two-terminal branches, finite voltage boxes and forbidden islanding. It derives
angle bounds from declared angle or terminal limits, or from nodal balance on
networks with positive reciprocal active-flow slopes. An unsupported input
raises an error before SCIP solves. Branch choices, opening costs and budgets
come from the typed instance.

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
index resource and a congestion prompt. [MCP server](python.md#mcp) covers setup in
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

## DC branch conventions

```python
case = tellegen.load("case.m", branch_susceptance_formula="series_susceptance_no_phase_shift")
result = case.solve()
assert result["branch_susceptance_formula"] == "series_susceptance_no_phase_shift"
case.branch_susceptance_formula = "tap_adjusted_reactance"
```

Changing the property invalidates the cached solution; demand/rating edits and
reset retain it. `None` inherits a typed instance's convention or the legacy
network default. The engine rejects conflicts with typed instances and rejects
DC options for AC solves. See the [optimality compatibility matrix](formulations.md#dc-compatibility).


<a id="mcp"></a>

<a id="mcp-mcp-server"></a>

## MCP server

`tellegen-mcp` serves the Tellegen engine to an MCP client, such as Claude Code
or Claude Desktop, over stdio. It reads case files from disk, solves them in
process, and keeps durable Studies. It offers nine tools, two kinds of
resource, and one prompt. The browser's [WebMCP](webmcp.md) tools work on the
case open in a tab; this server works on files.

<a id="mcp-install"></a>

### Install

The server is the `mcp` extra of the Python package. Install it into a virtual
environment and note the absolute path of the entry point:

```sh
python -m venv ~/.venvs/tellegen
~/.venvs/tellegen/bin/python -m pip install "tellegen[mcp]"
ls ~/.venvs/tellegen/bin/tellegen-mcp
```

From a checkout, build the wheel and install it with the extra
([Python](python.md#installing) explains why `maturin develop` is not used):

```sh
python -m pip install "maturin>=1.7,<2.0"
maturin build --profile release-py --out dist
python -m pip install "$(ls dist/tellegen-*.whl)[mcp]"
```

`python -m tellegen.mcp` starts the same server.

<a id="mcp-allowed-roots"></a>

### Allowed roots

The server opens and writes files only under the directories listed in
`POWERIO_MCP_ALLOWED_ROOTS`, separated by `:` (`;` on Windows). A path outside
them is refused. When the variable is unset, the only root is the directory
the server started in, which depends on the client, so set it explicitly. The
policy is `powerio.mcp.sandbox`, shared with PowerMCP.

<a id="mcp-claude-code"></a>

### Claude Code

```sh
claude mcp add tellegen \
  -e POWERIO_MCP_ALLOWED_ROOTS=/absolute/path/to/cases \
  -- /absolute/path/to/venv/bin/tellegen-mcp
```

`claude mcp list` should report `tellegen` as connected. In a session, `/mcp`
shows its tools, and the prompt runs as
`/mcp__tellegen__investigate_congestion /absolute/path/to/case.m`. The
command adds the server for the current project only (`--scope local`); use
`--scope user` to add it everywhere. `claude mcp remove tellegen` removes it.

<a id="mcp-claude-desktop"></a>

### Claude Desktop

Add the server to `claude_desktop_config.json`
(`~/Library/Application Support/Claude/` on macOS,
`%APPDATA%\Claude\` on Windows) and restart Claude Desktop:

```json
{
  "mcpServers": {
    "tellegen": {
      "command": "/absolute/path/to/venv/bin/tellegen-mcp",
      "env": {
        "POWERIO_MCP_ALLOWED_ROOTS": "/absolute/path/to/cases"
      }
    }
  }
}
```

Claude Desktop does not start servers from your shell, so it does not see your
`PATH` or an activated virtual environment. Use the absolute path to
`tellegen-mcp`.

<a id="mcp-tools"></a>

### Tools

| Tool            | Title                       | Effect                     | What it does                                                                                                                           |
| --------------- | --------------------------- | -------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| `capabilities`  | Report engine capabilities  | read-only                  | Versions, and per formulation whether it is available and which sensitivity operands and parameters it supports.                       |
| `solve`         | Solve a power network       | read-only                  | DC power flow, DC OPF (LMPs, dispatch, flows, binding limits), AC power flow, or SOCWR, with optional edits and sensitivities.         |
| `solve_module`  | Solve to a solution module  | writes `out_path` if given | The DC OPF solution as a PowerIO `DcOpfSolution` module, inline or written to a file.                                                  |
| `plan`          | Plan capacity increases     | writes `out_path` if given | A bounded search for branch rating increases that lower a weighted sum of LMPs, verified by exact solves. The proposal is not applied. |
| `study_create`  | Create a study              | creates a file             | A Study file from a case and a declared goal. Never replaces an existing file.                                                         |
| `study_inspect` | Inspect a study             | read-only                  | A Study's summary, or bounded JSON pages of its goal, states, experiments or evidence.                                                 |
| `study_run`     | Run a study operation       | appends a revision         | Inspect, branch, revise a goal, compare, edit demand, restore the base case, propose, or record evidence. `apply` is refused.          |
| `study_export`  | Validate a study for export | read-only                  | Validates a Study file as a portable bundle and returns its revision and SHA-256 digest.                                               |
| `study_import`  | Import a study bundle       | creates a file             | Imports a bundle to a new path without restoring approvals. Never replaces an existing file.                                           |

Every tool declares MCP annotations. `readOnlyHint` is true for the four
read-only tools. `destructiveHint` is true only for `solve_module` and `plan`,
because `overwrite=true` replaces an existing `out_path`. Every tool sets
`openWorldHint` to false: none reaches beyond the engine and the allowed roots.
Applying a proposal is a human action, so no tool does it.

<a id="mcp-results"></a>

#### Results

Each tool declares an output schema. A result arrives twice: as structured
content that matches the schema, and as the same JSON in a text block for
clients that read only text. Engine fields are typed loosely and every object
admits extra keys, so a result never fails its own schema.

A `solve` result carries `summary`: bus, branch and generator counts, the LMP
range with the buses and sensitivity columns at each end, every binding branch
with its flow, rating and `relief_per_mw` (the objective decrease per extra MW
of rating), and the five most loaded branches. `detail` controls the
per-element arrays (`lmp`, `flows`, `dispatch`, ...): `"summary"` leaves them
out and lists them in `response.omitted`, `"full"` keeps them, and the default
`"auto"` keeps them only when the result is under about 40,000 characters. A
500-bus DC OPF is about 260,000 characters in full and under 2,000 as a
summary. Arrays longer than `max_elements` arrive as
`{"truncated": true, "count": n, "head": [...]}`.

`solve_module` and `plan` return a solution module that can be large. Unless
`out_path` is given, `module="auto"` returns it only when it is under about
40,000 characters and otherwise explains how to write it; `"inline"` always
returns it and `"omit"` never does.

<a id="mcp-object-arguments"></a>

#### Object arguments

`solve`'s `edits` and `sensitivities` and `plan`'s `spec` are objects with
published schemas. Earlier versions took them as JSON text, which still works.

```json
{
  "path": "/absolute/path/to/cases/case3.m",
  "edits": { "deltas": { "2": 30 }, "rates": { "2-3": 20 } },
  "sensitivities": [
    {
      "operand": { "Price": "Active" },
      "parameter": { "Demand": "Active" },
      "indices": [1]
    }
  ],
  "detail": "summary"
}
```

`deltas` and `rates` are MW changes keyed by bus or branch id or PowerIO uid.
`indices` are zero-based sensitivity columns; `summary.lmp.max_index` is the
column of the highest-price bus. Omit `indices` only on small cases: a full
LMP-by-demand matrix has one row and one column per bus.

A `plan` spec:

```json
{
  "objective": {
    "kind": "weighted_lmp",
    "weights": [{ "bus": 337, "weight": 1.0 }]
  },
  "candidates": ["377-337", "182-185"],
  "max_increase_per_branch_mw": 50,
  "budget_mw": 50,
  "increment_mw": 10,
  "max_changed_lines": 2,
  "exact_solve_budget": 8
}
```

An invalid value is refused with each field's problem and an example, for
instance ``Argument `edits` is invalid (deltas.2: Input should be a valid
number, ...). Example: {...}``.

<a id="mcp-resources"></a>

### Resources

| URI                      | Content                                                                                                                                                                                                            |
| ------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `tellegen://cases`       | JSON index of case files under the allowed roots (up to 200, four directories deep, by suffix: `.m`, `.raw`, `.rawx`, `.aux`, `.epc`, `.dss`, `.xiidm`, `.uct`, `.ucte`, `.cdf`, `.pio.json`) with absolute paths. |
| `tellegen://cases/case3` | A bundled three-bus MATPOWER case. Its URI also works as a tool's `path`.                                                                                                                                          |

In Claude Code, `@tellegen:tellegen://cases` attaches the index to a message.

<a id="mcp-prompt"></a>

### Prompt

`investigate_congestion(case_path)` asks the agent to confirm capabilities,
solve the case's DC OPF as a summary, add demand if nothing binds, compute the
LMP sensitivity at the highest-price bus, explain the binding branches, and,
when a branch binds in the stored case, run `plan` and report the unapplied
proposal.

<a id="mcp-example-session"></a>

### Example session

The PGLib 500-bus case ships with the repository at
`crates/tellegen-server/fixtures/pglib/pglib_opf_case500_goc.m`. With its
directory as the allowed root, a session runs as follows; the numbers are from
this build.

1. Read `tellegen://cases`. The index lists `pglib_opf_case500_goc.m` with its
   absolute path.
2. Call `solve` with that `path`, `formulation` `"dcopf"` and `detail`
   `"summary"`. Status `optimal`, objective 440,548.51. LMPs range from 28.15
   at bus 381 to 53.93 at bus 337 (`max_index` 336). One branch binds:
   `377-337` at its 278.49 MW rating, with `relief_per_mw` 32.88.
3. Call `solve` again with
   `sensitivities: [{"operand": {"Price": "Active"}, "parameter": {"Demand": "Active"}, "indices": [336]}]`.
   One more MW of demand at bus 337 raises its LMP by 0.0516 and the LMPs of
   buses 338 to 340 by 0.0316: the buses behind the binding branch move
   together.
4. Call `plan` with the spec above. Three 10 MW trials on `377-337` are
   accepted; the third predicted a change of -1.21 and measured -8.94. The
   proposal is +30 MW on `377-337`, which lowers the LMP at bus 337 from 53.93
   to 42.73 in four exact solves. Nothing is applied.
5. To keep the work, call `study_create` with a Study path under the allowed
   root and `input_path` set to the case, then use `study_run` operations such
   as `propose` and `record_evidence` to save proposals and the reasoning
   behind them. Applying a proposal stays with a person.

<a id="mcp-webmcp-and-mcp"></a>

### WebMCP and MCP

The browser tools act on the case shown in a tab and keep a live session; this
server is stateless between calls except for Study files. The closest
equivalents:

| WebMCP (browser)                                                                                                                          | `tellegen-mcp`                                                                                                            |
| ----------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| `list_cases`                                                                                                                              | `tellegen://cases` resource                                                                                               |
| `select_case`, `import_case`                                                                                                              | the `path` argument of each tool                                                                                          |
| `inspect_case`, `query_network`                                                                                                           | `solve`, with `detail`                                                                                                    |
| `analyze_sensitivity`                                                                                                                     | `solve` with `sensitivities`                                                                                              |
| `preview_case_update`                                                                                                                     | `solve` with `edits`                                                                                                      |
| `update_case`, `reset_case`                                                                                                               | none: there is no live case; pass `edits` on each call                                                                    |
| `propose_capacity_plan`                                                                                                                   | `plan`                                                                                                                    |
| `apply_capacity_plan`                                                                                                                     | none: applying is a human action                                                                                          |
| `create_study`                                                                                                                            | `study_create`                                                                                                            |
| `inspect_study`                                                                                                                           | `study_inspect`                                                                                                           |
| `revise_study_goal`, `branch_study`, `compare_study_states`, `propose_study`, `edit_demand`, `restore_base_case`, `record_study_evidence` | `study_run` with `kind` `revise_goal`, `branch`, `compare`, `propose`, `edit_demand`, `restore_base` or `record_evidence` |
| none                                                                                                                                      | `solve_module`, `study_export`, `study_import`                                                                            |
| `export_case`, settings tools, `run_calculation`, `solve_multiconductor_pf`, `focus_network`, `inspect_webmcp_registration`               | none                                                                                                                      |

Both surfaces set `readOnlyHint` on read-only tools. WebMCP's
`untrustedContentHint` has no MCP counterpart; here, too, case names and
identifiers come from the files an agent opens.

The tool names and arguments match PowerMCP's Tellegen server
(`powermcp run tellegen`), which runs the native CLI instead of the engine in
process.
