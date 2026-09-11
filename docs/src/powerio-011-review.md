# PowerIO 0.11 consumer review

Tellegen uses the published PowerIO 0.11.1 crates. All six PowerIO components
resolve from crates.io at 0.11.1 with registry checksums. The release tag
[`v0.11.1`](https://github.com/eigenergy/powerio/releases/tag/v0.11.1) identifies
commit `432f795a3cc10ad3ee2ed5041406e562e6eb929b`.
`evidence/webmcp/powerio-releases.json` records published release revisions.

Pull requests that change the dependency manifests, lockfile, or pin scripts
run the `PowerIO Candidate` workflow against the committed dependency set.
Manual dispatch can test a full PowerIO commit SHA using temporary Git patches
and a disposable lockfile. Both paths verify one common version and source for
all six components before exercising Rust, WebAssembly, WebMCP, and browser
integration.

Tellegen consumes PowerIO modules at its public entry points. `DcNetwork` and
`AcNetwork` are private solver workspaces built from a PowerIO problem
instance. The browser and CLI save PowerIO case and solution modules; Tellegen
uses PowerIO IR for portable networks and solutions. Its optional experiment
journal records browser tool activity and never replaces the electrical module. PowerIO IR
text is written and read through `tellegen::ir`, which calls
`powerio::serialize` and `powerio::deserialize`.

## Release, IR, and ABI versions

The PowerIO release, stored IR generation, and C ABI are independent:

| Concern            | Current value                        | Tellegen integration                             |
| ------------------ | ------------------------------------ | ------------------------------------------------ |
| Rust crate release | `0.11.1`                             | dependency requirement `0.11.1`                  |
| Stored IR          | `"schema": "pio-ir"`, `"version": 2` | sole durable browser and CLI JSON boundary       |
| Producer           | `powerio` `0.11.1`                   | records which release wrote an IR document       |
| C ABI              | `7`                                  | unchanged and not used as an IR or crate version |

The historical `powerio.module` version 1 document and bare balanced-network
model JSON are not current input formats. Regenerate those documents from
their original case data. Checked-in evidence produced with the old candidate
is historical and must be rerun before it is cited for this release.

## Current module API

Tellegen uses the v0.11 facade directly:

- `powerio::parse(input)` for automatic routing and
  `powerio::parse_with_options(input, &ParseOptions)` for an explicit format;
- `powerio::serialize` and `powerio::deserialize` for `.pio.json` documents;
- `PioModule::value()` and `diagnostics()` for reads, with `value_mut()` for edits;
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
