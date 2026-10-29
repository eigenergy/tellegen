# PowerIO 0.11 consumer review

Tellegen uses the published PowerIO 0.11.7 crates. All eight PowerIO components
resolve from crates.io at 0.11.7 with registry checksums.
`docs/evidence/webmcp/powerio-releases.json` maps each release tag, such as
[`v0.11.7`](https://github.com/eigenergy/powerio/releases/tag/v0.11.7), to its
commit.

Pull requests that change the dependency manifests, lockfile, or pin scripts
run the `PowerIO Candidate` workflow against the committed dependency set.
Manual dispatch can test a full PowerIO commit SHA using temporary Git patches
and a disposable lockfile. Both paths verify one common version and source for
all eight components before exercising Rust, WebAssembly, WebMCP, and browser
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
| Rust crate release | `0.11.7`                             | dependency requirement `0.11.7`                  |
| Stored IR          | `"schema": "pio-ir"`, `"version": 2` | sole durable browser and CLI JSON boundary       |
| Producer           | `powerio` `0.11.7`                   | records which release wrote an IR document       |
| C ABI              | `7`                                  | unchanged and not used as an IR or crate version |

The historical `powerio.module` version 1 document and bare balanced-network
model JSON are not current input formats. Regenerate those documents from
their original case data. Checked-in evidence produced with the old candidate
is historical and must be rerun before it is cited for this release.

## Cargo features

The `powerio` facade enables a `sincal` feature by default. It reads SINCAL
projects through `powerio-tnd` and a bundled SQLite, which the wasm engine
could only link with a C compiler that targets WebAssembly. Tellegen reads no
SINCAL projects, so the workspace depends on `powerio` with
`default-features = false`, and on `powerio-tx` and `powerio-dist` the same
way so no SINCAL code comes in through them. The CLI, server and browser
therefore list no SINCAL format, and PowerIO refuses a SINCAL source or target
with `REQUEST.SINCAL.FEATURE_DISABLED`. `powerio-tnd` itself, which composes
transmission and distribution models, is always part of the facade.

## HVDC lines

Since PowerIO 0.11.7, an AC power flow instance built from a network adds each
in-service HVDC line's terminal powers to its buses' stated injections, and the
DC OPF preparation states the same transfers as `p_hvdc`. A stored instance
without `hvdc_treatment` reads as `ignore`, so earlier documents solve as
before.

Tellegen's DC OPF, DC power flow and AC power flow carry each in-service HVDC
line as these fixed injections. The DC models subtract `p_hvdc` from the fixed
withdrawal, so the balance is `p_d + g_s + p_shift - p_hvdc`, and an emitted DC
OPF solution restates the same bus injections. The AC power flow solves the
instance's stated injections; a network solved directly goes through that
instance when it has in-service HVDC lines. A line with a terminal bus outside
the solve, for example in an island with no generator, injects at neither end.
Each solve reports PowerIO's `BUILD.HVDC.FIXED_INJECTION` and
`BUILD.HVDC.TERMINAL_INACTIVE` diagnostics. The AC OPF and the SOC relaxation
have no HVDC term yet and still refuse a network with an in-service HVDC line.

`tellegen-synth` validates on a copy that replaces each HVDC link with a fixed
transfer, so its verdicts do not change; the viewer and the CLI now solve the
generated network as written.

OpenDSS imports carry the circuit's source impedance: PowerIO places the
Vsource EMF on an internal bus behind a series impedance line to the source
bus, and the distribution graph shows both.

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
