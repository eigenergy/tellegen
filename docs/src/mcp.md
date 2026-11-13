# MCP server

`tellegen-mcp` serves the Tellegen engine to an MCP client, such as Claude Code
or Claude Desktop, over stdio. It reads case files from disk, solves them in
process, and keeps durable Studies. It offers nine tools, two kinds of
resource, and one prompt. The browser's [WebMCP](webmcp.md) tools work on the
case open in a tab; this server works on files.

## Install

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

## Allowed roots

The server opens and writes files only under the directories listed in
`POWERIO_MCP_ALLOWED_ROOTS`, separated by `:` (`;` on Windows). A path outside
them is refused. When the variable is unset, the only root is the directory
the server started in, which depends on the client, so set it explicitly. The
policy is `powerio.mcp.sandbox`, shared with PowerMCP.

## Claude Code

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

## Claude Desktop

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

## Tools

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

### Results

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

### Object arguments

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

## Resources

| URI                      | Content                                                                                                                                                                                                            |
| ------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `tellegen://cases`       | JSON index of case files under the allowed roots (up to 200, four directories deep, by suffix: `.m`, `.raw`, `.rawx`, `.aux`, `.epc`, `.dss`, `.xiidm`, `.uct`, `.ucte`, `.cdf`, `.pio.json`) with absolute paths. |
| `tellegen://cases/case3` | A bundled three-bus MATPOWER case. Its URI also works as a tool's `path`.                                                                                                                                          |

In Claude Code, `@tellegen:tellegen://cases` attaches the index to a message.

## Prompt

`investigate_congestion(case_path)` asks the agent to confirm capabilities,
solve the case's DC OPF as a summary, add demand if nothing binds, compute the
LMP sensitivity at the highest-price bus, explain the binding branches, and,
when a branch binds in the stored case, run `plan` and report the unapplied
proposal.

## Example session

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

## WebMCP and MCP

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
