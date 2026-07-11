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

- `ingestCase(text, format)`
- `parseDisplay(bytes)`
- `capabilities()`
- `solveJson(networkJson, request)`
- `createStudy(networkJson, formulation)`

## Case And Display Helpers

- `formatOf(name)`: returns `m`, `raw`, or `aux` for supported case names.
- `isDisplayFile(name)`: returns true for PowerWorld `.pwd` display files.
- `ingestCase(text, format)`: parses a case and returns a network JSON payload plus summary and topology.
- `parseDisplay(bytes)`: parses PowerWorld display data for diagram overlays.

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

## Types

Generated public types include:

- `SolveRequest`, `SolveResponse`, `ProblemCaps`
- `SensRequest`, `SensitivityMatrix`, `SensitivityColumn`
- `Network`, `NetworkBus`, `NetworkBranch`
- `Solution`, `SolveIteration`, `DemandDeltas`
- `BrowserFormulation`, `FormulationId`, `SolveStatus`

The generated file is committed at `packages/engine/src/generated/contracts.ts` and checked in CI.
