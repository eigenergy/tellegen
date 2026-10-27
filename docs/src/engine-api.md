# Engine API Reference

`@tellegen/engine` is the public browser engine package. It is independent of SvelteKit and the hosted demo.

## Constants

- `ENGINE_VERSION`: the `@tellegen/engine` package version.
- `API_SOURCE_SHA256`: the `crates/tellegen/src/api.rs` hash the generated TypeScript types were built from.
- `FORMULATION_IDS` and `SOLVE_STATUSES`: generated enum tags from the Rust API layer.
- `FORMULATIONS` and `DEFAULT_FORMULATION`: browser UI formulation list and default formulation.

## Browser Wasm Transport

- `browserWasmTransport`: object facade for the browser wasm transport.
- `createBrowserWasmTransport()`: returns the browser wasm transport facade.
- `preloadEngine()`: initializes the wasm package.

The package also exports its two large assets as files, so a host can preload
or budget them without a path into `dist`: `@tellegen/engine/wasm` is the
WebAssembly binary and `@tellegen/engine/places` the populated places table.
Vite imports them as URLs with `?url`
(`import engineWasmUrl from "@tellegen/engine/wasm?url"`).

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
- `solveModule(moduleJson, request)`: stateless solve from a PowerIO module. A
  `dcopf` request may carry `constraints`, linear limits on branch flows, bus
  net injections, and generator outputs; the response's `constraints` block
  reports each row's value and shadow price. See
  [Formulations](formulations.md#linear-constraints).
  An `acpf` request may set `start: "case"` to start Newton from the case's
  stored bus voltages (see [Formulations](formulations.md#ac-power-flow-polar)).
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

## Synthetic Grid Generation

- `createSynthSession(request, { osm?, places?, signal? })`: opens a staged
  generation on a dedicated synthetic worker and
  returns a `BrowserSynthSession`:
  - `advance({ withPreview? })` runs one stage (`context`, `site_buses`,
    `topology`, `electrify`, `dc_opf`, `calibrate` once per iteration,
    `ac_pf`, `finalize`) and returns its `StepOutcome`;
  - `run(onStep?, signal?)` loops it and resolves to the `GenerationReport`;
  - `preview()` returns transferred typed arrays for binary map layers;
  - `report()`, `takeModule()` (pio-ir with bus locations and branch routes),
    `ingest()` (the `IngestedCase` a dropped `.pio.json` of that module
    produces; `coords_method: "generated"` marks its synthetic
    coordinates as sites on real geography), and `exportGridfm()`;
  - `cancel(reason?)` terminates the synthetic worker (interactive studies
    are untouched), and `free()` releases the session;
  - `hostKind` is `"worker"`, or `"direct"` when the page runs it.
- `generateSynthetic(request, options?)`: every stage in one request; returns
  `{ moduleJson, report, ingest }`.
- `syntheticOverpassQueries(request)`: the Overpass QL for a region, one query
  per bounding box. Pass the fetched documents as the `osm` option with
  `source: "osm"`.
- `loadSyntheticPlaces(bytes)`, `fetchSyntheticPlaces()`, `syntheticPlacesUrl()`:
  the Natural Earth places table shipped as `data/populated_places.json`;
  sessions load it on first use.
- Refusals reject with `SynthEngineError`, whose `code` is one of
  `invalid_request`, `region`, `places`, `osm`, `osm_insufficient` (with bus
  and branch counts), `build`, `engine`, `state`, `cancelled`.
- `@tellegen/engine/synth` exports the generated request and report types,
  and `synthSchema` validates a pasted request at run time.

See [Synthetic Grid Generation](synthetic-generation.md) for the pipeline.

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

- `SolveRequest`, `SolveResponse`, `ProblemCaps`, `PowerFlowStart`, `SolveDiagnostic`
- `LinearConstraint`, `ConstraintTerm`, `ConstraintTermKind`, `ConstraintResult`
- `SensRequest`, `SensitivityMatrix`, `SensitivityColumn`
- `Network`, `NetworkBus`, `NetworkBranch`
- `Solution`, `SolveIteration`, `DemandDeltas`
- `ImplicitObjectiveJson`, `CapacityPlanSpecJson`, `CapacityPlanOutcomeJson`
- `IngestedJsonDrop`, `JsonDropClassification`, `JsonDropKind`
- `BrowserFormulation`, `FormulationId`, `SolveStatus`

The generated file is committed at `packages/engine/src/generated/api-types.ts` and checked in CI.
