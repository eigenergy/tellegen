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
- `ingestJsonDrop(bytes, { retain })`: classifies and parses a JSON drop in one
  call. With `retain: true`, a multiconductor module stays parsed in the engine
  and the payload's `input` holds a `BrowserMcInput` in place of `module_json`
  (see [Multiconductor PF](multiconductor-pf.md)). `ingestDistCaseBytes(bytes,
  format, { retain })` takes the same option.
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

DC requests and Study options accept `branch_susceptance_formula`. See the
[DC optimality compatibility matrix](formulations.md#dc-compatibility) for tokens, equations,
benchmark assumptions and persistence semantics.

- `capabilities()`: returns available formulations, operands, and parameters.
- `solveModule(moduleJson, request)`: stateless solve from a PowerIO module. A
  `dcopf` request may carry `constraints`, linear limits on branch flows, bus
  net injections, and generator outputs; the response's `constraints` block
  reports each row's value and shadow price. See
  [Formulations](formulations.md#linear-constraints).
  An `acpf` request may set `start: "case"` to start Newton from the case's
  stored bus voltages (see [Formulations](formulations.md#ac-power-flow-polar)).
  AC responses include `initialization`: the requested start, the strategy
  that converged (`used: "case" | "flat" | "perturbed"`), and `attempts`
  including failed starts. A perturbed success also gives its one-based
  `restart` number. Compact browser and server solutions retain this block;
  older saved results may omit it.
- `createStudy(moduleJson, formulation, options?)`: builds a browser `Study` from a
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

## RMS Dynamic Simulation

Runs execute in a dedicated worker (`rms-worker.js`) with its own wasm
instance, so a run never blocks the page or the static-study worker.

- `rmsSupport()`: `{ rms, experimental }` for this engine build.
- `looksLikeDyr(text)`: whether text reads as PSS/E dynamic data.
- `openRmsCase(moduleJson, dyrText, options?)`: prepare a case once
  (`options`: `load_model` and the `RmsOptions` fields). Resolves to a
  `BrowserRmsCase`.
- `BrowserRmsCase.inspection`: the linked models, elements, initial residual,
  reference unit, held units and notes, without a run.
- `BrowserRmsCase.flatCheck(seconds = 2)`: the largest speed and voltage drift
  of a run without events.
- `BrowserRmsCase.run(request, { signal, onProgress, criteria })`: run the
  events (`RmsEvent[]`, default 20 s at 2.5 ms, recorded every 10 ms) and
  resolve to an `RmsRun`: `time`, `keys`, `series(key)` (a zero-copy view of
  one transferred buffer), the Rust `screening`, `notes`, `events`, `steps`,
  `newtonIterations`, `outputStep` and `workerMs`. Keys are the engine's
  channels (`vm:`, `omega:`, `delta:`, `pe:`, `pl:`, `ql:`) and the derived
  `coi:f`, `coi:omega`, `coi:delta` and `f:<bus>`. Aborting `signal`
  terminates the RMS worker; the case reopens itself on its next call.
- `BrowserRmsCase.linear(request?, { signal })` (experimental): modes with
  participation factors; throws `ExperimentalUnavailable` without the feature.
- `rmsDynamize(moduleJson, dyrText, request)` and `rmsLibrary()`
  (experimental): default dynamics with typical parameters.
- `setRmsWorkerFactory(factory)`: supply the worker (tests, custom bundling).

The request, inspection, options and screening types are generated from the
Rust schema (`src/generated/rms-types.ts`, checked by `npm run types:check`).

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
  - `ingestWithStudy(formulation?)` resolves to `{ payload, study }`: the
    payload of `ingest()` and a promise of a Study of the same module, which
    builds on the study worker while the payload is prepared;
  - `cancel(reason?)` terminates the synthetic worker (interactive studies
    are untouched), and `free()` releases the session;
  - `hostKind` is `"worker"`, or `"direct"` when the page runs it.
- `generateSynthetic(request, options?)`: every stage in one request; returns
  `{ moduleJson, report, ingest }`.
- `syntheticOverpassQueries(request)`: the Overpass QL for a region, one query
  per bounding box. Pass the fetched documents as the `osm` option with
  `source: "osm"`.
- `loadSyntheticPlaces(bytes)`, `fetchSyntheticPlaces()`, `syntheticPlacesUrl()`:
  the GeoNames places table (CC BY 4.0) shipped as
  `data/populated_places.json`; sessions load it on first use.
- Refusals reject with `SynthEngineError`, whose `code` is one of
  `invalid_request`, `region`, `places`, `osm`, `osm_insufficient` (with bus
  and branch counts), `build`, `engine`, `state`, `cancelled`.
- `@tellegen/engine/synth` exports the generated request and report types;
  `synthSchema`, exported from the package root, validates a pasted request
  at run time.

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


<a id="transports"></a>

<a id="transports-transports"></a>

## Transports

The transport boundary is the line between a host app and the tellegen engine.

<a id="transports-browser-wasm"></a>

### Browser Wasm

`@tellegen/engine` ships the browser wasm transport today. It wraps the `wasm-pack` output from `crates/tellegen-wasm` and exposes direct functions plus `browserWasmTransport`.

Use this transport when cases should stay local to the browser. Dropped case
files are parsed in WebAssembly, and solves, `Study.preview`, `Study.commit`,
and sensitivity requests run there as well. No case text or PowerIO module
leaves the browser unless the host app sends it.

The wasm instance lives in a dedicated Web Worker, so a multi-second solve
never blocks the page. When the browser cannot run a module worker the same
requests execute on the calling thread instead; the API is identical either
way.

The transport has one wasm package carrying parsing, all solves, `Study`,
capabilities, and generalized sensitivity requests.

The loader is lazy. Host apps can call `preloadEngine()` to control when the browser downloads and initializes wasm.

<a id="transports-http"></a>

### HTTP

The hosted demo also uses HTTP for bundled case metadata and native server fallback paths. That server is a demo consumer, not a requirement for using `@tellegen/engine`.

An HTTP transport can implement the same shape as `EngineTransport`: parse or
fetch a PowerIO module, call a native module solve endpoint, keep a server side
study handle, and return the same generated TypeScript types.

That transport is optional for apps that need server sized cases, audit logs, or native deployment.

<a id="transports-tauri"></a>

### Tauri

The desktop and mobile path can use the same transport contract with a Tauri command layer. The UI keeps the `Study` workflow; only the call boundary changes from browser wasm to native commands.

The Rust types stay the source of truth. New transports should return the generated `SolveResponse`, `ProblemCaps`, and sensitivity matrix shapes instead of defining parallel types.

<a id="http-api"></a>

<a id="http-api-http-api"></a>

## HTTP API

The demo server's surface. Data endpoints are always served; the compute
endpoints ship disabled and answer 403 unless `TELLEGEN_SERVER_COMPUTE=1`
(see [Deployment](deployment.md)).

<a id="http-api-data"></a>

### Data

- `GET /api/health` — liveness and the served case ids.
- `GET /api/compute` — `{"enabled": bool}`, whether the compute endpoints are on.
- `GET /api/cases` — case summaries.
- `GET /api/cases/{id}/case` — the portable PowerIO module the browser engine consumes.
- `GET /api/cases/{id}/network` — the map view (buses, branches, coordinates).
- `GET /api/cases/{id}/solution` — the cached base DC OPF solution, computed once at startup.

Case summaries report canonical PowerIO row counts as `n_bus`/`n_branch` and
rendered, three-winding-lowered counts as
`n_analysis_bus`/`n_analysis_branch`. Older servers may omit the analysis
counts.

Summaries include `model: "balanced"` or `"multiconductor"`; an omitted model
on an older server means balanced. Hosted distribution modules are registered
through the [distribution manifest](deployment.md#hosted-distribution-cases).
For them, bus/branch counts describe the distribution graph and the analysis
counts equal those graph counts. `/case` retains the full module, including
geometry and diagnostics. The browser builds the distribution view and runs
supported AC power flow locally. Balanced-only `/network`, `/solution`,
`/snapshot`, and enabled compute routes return 400 for distribution IDs.
The global disabled-compute gate still returns 403 when compute is off.

<a id="http-api-compute"></a>

### Compute

- `GET /api/cases/{id}/sensitivity/lmp/d/{bus}` — the ∂LMP/∂demand column at a bus.
- `GET /api/cases/{id}/sensitivity/lmp/fmax/{branch}` — the ∂LMP/∂rating column at a branch.
- `GET /api/cases/{id}/solve` — a DC OPF solve streamed over server-sent events:
  `status`, `solution`, optional `sensitivity`, and `done`.

The sensitivity and solve endpoints accept `?d=bus:mw,bus:mw`, each value a MW
delta from the base case. They take no other request fields: linear
constraints and the rest of the solve request go through the engine's
`solveModule`, the `tellegen` CLI, or the crate (see
[Formulations](formulations.md#linear-constraints)).

<a id="http-api-limits"></a>

### Limits

Solve work is bounded by `TELLEGEN_SOLVER_CONCURRENCY` (default 2) and
`TELLEGEN_SOLVER_TIMEOUT_SECS` (default 30). Compute routes are rate limited
per client: 5 solve and 25 sensitivity requests per 10 seconds by default,
tuned with `TELLEGEN_RATE_LIMIT_WINDOW_SECS`,
`TELLEGEN_SOLVE_RATE_LIMIT_EVENTS`, and
`TELLEGEN_SENSITIVITY_RATE_LIMIT_EVENTS`.

Hosted distribution summaries may include `distribution` metadata: description,
HTTPS source URL, related transmission case ID, and PF option defaults. The
`/api/cases/{id}/case` endpoint negotiates gzip for portable module downloads.

## Result validation

The engine package checks imports, solves, studies, settings, layouts, generation
and RMS responses against schemas generated from their Rust records. This includes
the separate AC OPF worker and the dimensions and indices of packed arrays.
HTTP metadata, solutions, saved results and streamed sensitivities use the
server's serialized types. Invalid results name the affected field; an invalid
stream closes and leaves the previous display intact. Rejected retained-input
responses release the new worker allocation. Older responses may omit newer
optional fields, which are checked when present. A failed calculation retains
its engine error and diagnostics.

The validators are split into one module per result family. The package root
imports none of them: `parseApiResult(name, text)` and
`validateApiResult(name, value)` import a family the first time one of its
results is checked, so both are async, as is `unpackRun`, which checks RMS
metadata. `preloadValidators(names)` imports the families for those results
ahead of their first use. The `@tellegen/engine/validators` entry imports every
family when it loads and exports `parseApiResultSync` and
`validateApiResultSync` for a host that cannot await validation.

```ts
parseApiResult<T>(name: ResultName, text: unknown): Promise<T>;
validateApiResult<T>(name: ResultName, value: unknown): Promise<T>;
preloadValidators(names: readonly ResultName[]): Promise<void>;
unpackRun(request, value, workerMs): Promise<RmsRun>;
```

After changing those Rust types, regenerate the schemas and TypeScript/Python
files using the commands in the [engine package guide](https://github.com/eigenergy/tellegen/tree/main/packages/engine#generated-types).
CI runs `scripts/schemas.sh --check`, which rejects a committed schema that differs from the Rust records, including changes in shared result records.
