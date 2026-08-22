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
- `solveJson(networkJson, request)`
- `createStudy(networkJson, formulation)`

## Case And Display Helpers

- `formatOf(name)`: returns the powerio format token for a supported case name.
- `isDisplayFile(name)`: returns true for PowerWorld `.pwd` display files.
- `classifyJson(bytes)`: asynchronously classifies JSON bytes and returns
  `{ kind, format }`.
- `ingestJsonDrop(bytes)`: classifies and parses a JSON drop in one call.
- `ingestCase(bytes, format)`: parses case bytes and returns a network JSON
  payload plus summary and topology.
- `parseDisplay(bytes)`: parses PowerWorld display data for diagram overlays.

`ingestJsonDrop` returns a discriminated `IngestedJsonDrop` union:

| `kind`                   | `format` | `payload`          |
| ------------------------ | -------- | ------------------ |
| `balanced-package`       | `null`   | `LoadedPackage`    |
| `multiconductor-package` | `null`   | `IngestedDistCase` |
| `model-json`             | `null`   | `IngestedCase`     |
| `transmission`           | `string` | `IngestedCase`     |
| `distribution`           | `string` | `IngestedDistCase` |
| `ambiguous` or `unknown` | `null`   | `null`             |

Use the `kind` discriminant before reading `payload`. `format` carries the
reader selected for transmission and distribution documents.

## Solves And Studies

- `capabilities()`: returns available formulations, operands, and parameters.
- `solveJson(networkJson, request)`: stateless solve over the generalized Rust API.
- `createStudy(networkJson, formulation)`: builds a browser `Study`.
- `Study` / `BrowserStudy`: browser handle with:
  - `currentSolution()`
  - `preview(deltas, rates?)`
  - `commit(caseId, deltas, rates, target)`
  - `sensitivity(caseId, deltas, rates, target)`
  - `free()`

`deltas` are demand deltas in MW keyed by bus; `rates` are thermal rating
deltas in MW keyed by branch. A key is the original numeric id (bus id, 1-based
branch position) or the powerio row uid string (`"buses:1"`, `"branches:2"`)
stamped at ingest — `ingestCase` payloads carry the uid on every topology and
view element, and solve responses echo it on bus and branch scalars.
`target` is `{ bus }` for the ∂LMP/∂d column,
`{ branch }` for the ∂LMP/∂rating column (nonzero only on binding lines), or
`null` for no column.

Call `free()` when a host app discards a study.

## Migrating To 0.2

- `ingestCase` now takes `Uint8Array`, not a string. Encode an in-memory string
  with `new TextEncoder().encode(text)`; use `new Uint8Array(await
file.arrayBuffer())` for a browser `File`.
- `classifyJson` now takes bytes, is asynchronous, and returns
  `{ kind, format }`: `const { kind, format } = await classifyJson(bytes)`.
- `@tellegen/svelte` no longer exports `isStudyPackageText`. For classification
  only, check
  `(await classifyJson(bytes)).kind === "balanced-package"`. To classify and
  parse a drop, use `await ingestJsonDrop(bytes)`.
- `JsonDropKind` now uses `transmission` and `distribution` with a separate
  `format`, plus `ambiguous` and `unknown`. The former `bmopf` and `pmd` kinds
  are distribution formats; `not-json` is now `unknown`.

## Types

Public types include:

- `SolveRequest`, `SolveResponse`, `ProblemCaps`
- `SensRequest`, `SensitivityMatrix`, `SensitivityColumn`
- `Network`, `NetworkBus`, `NetworkBranch`
- `Solution`, `SolveIteration`, `DemandDeltas`
- `IngestedJsonDrop`, `JsonDropClassification`, `JsonDropKind`
- `BrowserFormulation`, `FormulationId`, `SolveStatus`

The generated file is committed at `packages/engine/src/generated/contracts.ts` and checked in CI.
