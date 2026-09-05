# Engine API Reference

`@tellegen/engine` is the public browser engine package. It is independent of SvelteKit and the hosted demo.

## Constants

- `CONTRACT_VERSION`: the public TypeScript contract version. It matches the package version.
- `CONTRACT_SOURCE_SHA256`: the `crates/tellegen/src/api.rs` hash used to generate the TypeScript contracts.
- `FORMULATION_IDS` and `SOLVE_STATUSES`: generated enum tags from the Rust API layer.
- `FORMULATIONS` and `DEFAULT_FORMULATION`: browser UI formulation list and default formulation.

## Browser Wasm Transport

- `browserWasmTransport`: object facade for the browser wasm transport.
- `createBrowserWasmTransport()`: returns the browser wasm transport facade.
- `preloadEngine()`: initializes the wasm package.

The facade has the same methods as the direct exports:

- `classifyJson(bytes)`
- `ingestJsonDrop(bytes)`
- `ingestCase(bytes, format)`
- `parseDisplay(bytes)`
- `capabilities()`
- `solveModule(moduleJson, request)`
- `createStudy(moduleJson, formulation)`

## Case And Display Helpers

- `formatOf(name)`: returns the powerio format token for a supported case name.
- `isDisplayFile(name)`: returns true for PowerWorld `.pwd` display files.
- `classifyJson(bytes)`: asynchronously classifies JSON bytes and returns
  `{ kind, format }`.
- `ingestJsonDrop(bytes)`: classifies and parses a JSON drop in one call.
- `ingestCase(bytes, format)`: parses case bytes and returns a retained PowerIO
  module plus derived display data, summary, and topology.
- `parseDisplay(bytes)`: parses PowerWorld display data for diagram overlays.

`ingestJsonDrop` returns a discriminated `IngestedJsonDrop` union:

| `kind`                   | `format` | `payload`          |
| ------------------------ | -------- | ------------------ |
| `module`                 | `null`   | balanced or multiconductor payload |
| `transmission`           | `string` | `IngestedCase`     |
| `distribution`           | `string` | `IngestedDistCase` |
| `ambiguous` or `unknown` | `null`   | `null`             |

Use the `kind` discriminant before reading `payload`. `format` carries the
reader selected for transmission and distribution documents.

## Solves And Studies

- `capabilities()`: returns available formulations, operands, and parameters.
- `solveModule(moduleJson, request)`: stateless solve from a PowerIO module.
- `createStudy(moduleJson, formulation)`: builds a browser `Study` from a
  PowerIO module and solves its declared problem instance.
- `Study` / `BrowserStudy`: browser handle with:
  - `currentSolution()`
  - `preview(deltas, rates?)`
  - `commit(caseId, deltas, rates, target)`
  - `sensitivity(caseId, deltas, rates, target)`
  - `saveModule()`
  - `saveSolutionModule()` for an exact DC OPF result
  - `plan(spec, signal?)` for a read only, bounded capacity planning run on a
    disposable Study clone
  - `export(format)`
  - `free()`

`deltas` are demand deltas in MW keyed by bus; `rates` are thermal rating
deltas in MW keyed by branch. A key is the original numeric id (bus id, 1-based
branch position) or a PowerIO row uid string when the source carries one.
`ingestCase` returns `null` for topology and view UIDs when the source has none;
solve responses omit absent UIDs.
Three-winding transformers remain typed in the retained module, while topology and
view payloads include their lowered star rows so the rendered graph matches the
solver. Those display-only rows have `editable: false`; persist edits only on
canonical rows. Closed transmission switches, in-service storage, and
in-service HVDC links are rejected until their solver models are implemented.
`n_bus` and `n_branch` count canonical typed rows;
`n_analysis_bus` and `n_analysis_branch` count the lowered topology rows. The
analysis counts are optional in TypeScript so clients remain compatible with
older engine builds.
`target` is `{ bus }` for the LMP/demand column,
`{ branch }` for the LMP/rating column (nonzero only on binding lines), or
`null` for no column.

Call `free()` when a host app discards a study.

`CapacityPlanSpecJson` accepts a weighted LMP objective, canonical
candidate branch identities, MW bounds and increments, a final line count,
and an exact solve budget. `BrowserStudy.plan` materializes the committed
PowerIO module onto a disposable host, so the returned
`CapacityPlanOutcomeJson` is an unapplied proposal and cancellation does not
invalidate the retained interactive Study.

## Migrating To 0.2

- `ingestCase` now takes `Uint8Array`, not a string. Encode an in-memory string
  with `new TextEncoder().encode(text)`; use `new Uint8Array(await
file.arrayBuffer())` for a browser `File`.
- `classifyJson` now takes bytes, is asynchronous, and returns
  `{ kind, format }`: `const { kind, format } = await classifyJson(bytes)`.
- `@tellegen/svelte` no longer exports `isStudyPackageText`. For classification
  only, check
  `(await classifyJson(bytes)).kind === "module"`. To identify the module value
  family and parse the drop, use `await ingestJsonDrop(bytes)`.
- `JsonDropKind` now uses `transmission` and `distribution` with a separate
  `format`, plus `ambiguous` and `unknown`. The former `bmopf` and `pmd` kinds
  are distribution formats; `not-json` is now `unknown`.

## Types

Public types include:

- `SolveRequest`, `SolveResponse`, `ProblemCaps`
- `SensRequest`, `SensitivityMatrix`, `SensitivityColumn`
- `Network`, `NetworkBus`, `NetworkBranch`
- `Solution`, `SolveIteration`, `DemandDeltas`
- `ImplicitObjectiveJson`, `CapacityPlanSpecJson`, `CapacityPlanOutcomeJson`
- `IngestedJsonDrop`, `JsonDropClassification`, `JsonDropKind`
- `BrowserFormulation`, `FormulationId`, `SolveStatus`

The generated file is committed at `packages/engine/src/generated/contracts.ts` and checked in CI.
