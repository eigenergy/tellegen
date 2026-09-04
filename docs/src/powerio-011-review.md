# PowerIO 0.11 consumer review

PowerIO v0.11.0 is the baseline for this integration. The release preparation
merged in [powerio#482](https://github.com/eigenergy/powerio/pull/482), with
dependency maintenance in
[powerio#485](https://github.com/eigenergy/powerio/pull/485).
Tellegen pins merge revision `e852f1902a582d5dbf20dedd169056aeb7cdceba`
until the component crates are published. The earlier 1.0 candidate in
[powerio#454](https://github.com/eigenergy/powerio/pull/454) is no longer the
release target.

Before the PowerIO release, run the `PowerIO Candidate` workflow on this branch
with the proposed full PowerIO commit SHA. It rewrites the temporary manifest
pin on an isolated runner, resolves all six PowerIO components together, and
then exercises the Rust, WebAssembly, WebMCP package, and browser integration.
Normal CI independently rejects a manifest and lockfile that name different
PowerIO revisions.

Tellegen consumes PowerIO modules at its public entry points. `DcNetwork` and
`AcNetwork` are private solver workspaces built from a PowerIO problem
instance. The browser and CLI save PowerIO case and solution modules; Tellegen
defines no second portable network, study, or experiment format. PowerIO IR
text is written and read through `tellegen::ir`, which calls
`powerio::serialize` and `powerio::deserialize`.

## Release, IR, and ABI versions

The PowerIO release, stored IR generation, and C ABI are independent:

| Concern | Current value | Tellegen integration |
| --- | --- | --- |
| Rust crate release | `0.11.0` | dependency requirement `0.11` |
| Stored IR | `"schema": "pio-ir"`, `"version": 2` | sole durable browser and CLI JSON boundary |
| Producer | `powerio` `0.11.0` | records which release wrote an IR document |
| C ABI | `7` | unchanged and not used as an IR or crate version |

The historical `powerio.module` version 1 document and bare balanced-network
model JSON are not current input formats. Regenerate those documents from
their original case data. Checked-in evidence produced with the old candidate
is historical and must be rerun before it is cited for this pin.

## Current module API

Tellegen uses the v0.11 facade directly:

- `powerio::parse(input)` for automatic routing and
  `powerio::parse_with_options(input, &ParseOptions)` for an explicit format;
- `powerio::serialize` and `powerio::deserialize` for `.pio.json` documents;
- `PioModule::value()` for reads and `PioModule::value_mut()` for edits;
- `PioModule::try_map_value` for typed narrowing;
- `PioValue::type_name()` for canonical structural type names;
- `powerio::emit` for grid exchange formats.

Calling `value_mut()` is material to correctness: PowerIO drops retained
source bytes and severs value source-map targets before an in-place edit.
Tellegen therefore applies geographic data and auxiliary substation locations
through the retained module, then serializes that updated module back to the
browser.

PowerIO JSON classification now has five families: `module`, `transmission`,
`distribution`, `ambiguous`, and `unknown`. The removed `model-json` family is
not recreated in Tellegen. A PowerWorld `.pwd` display also follows the
universal parse route and narrows to `powerio.GeoLayer`.

## Browser boundary

Every solvable browser payload carries `module_json`. That generation-2 IR is
used for Study construction, geographic transforms, saved cases, and exact
solution modules. Counts, topology, and map views remain derived response data;
there is no separately serialized `network_json` integration point.

This keeps provenance and mutation behavior intact across the whole browser
flow. It also prevents a stale generation-1 shape from being accepted by a
display helper while the solver receives a different module.

## Preparation and results

Tellegen continues to consume typed PowerIO problem instances and shared
numerical preparation. Objective and constraint selections, persistent
identities, analysis-to-source row mappings, three-winding lowering, and
declared thermal limits come from PowerIO. DC and AC solution modules retain
convention-neutral demand marginals and terminal multipliers.

When Tellegen commits edits, it retains valid module diagnostics, history,
extensions, and producer data; replaces the module value with the committed
network; and severs obsolete source targets. A saved exact solution contains
the amended OPF instance that was solved and is emitted as generation-2
`pio-ir` with value type `powerio.DcOpfSolution`.
