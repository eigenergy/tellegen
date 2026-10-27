# @tellegen/engine

## 0.4.2

### Patch Changes

- [#223](https://github.com/eigenergy/tellegen/pull/223) [`880f6f3`](https://github.com/eigenergy/tellegen/commit/880f6f36be4039674660f84508c1565749117e46) Thanks [@samtalki](https://github.com/samtalki)! - The OpenStreetMap provenance block records the Overpass answer's licence notice (`osm3s.copyright`) as `copyright`.

- [#222](https://github.com/eigenergy/tellegen/pull/222) [`869302d`](https://github.com/eigenergy/tellegen/commit/869302d90c96546b200b2350e78851331656f374) Thanks [@samtalki](https://github.com/samtalki)! - OpenStreetMap grids no longer inflate their generation. A plant and its mapped units count once (the larger of the plant's tag and the units' sum), and a generator with no output tag, mostly rooftop solar, adds nothing instead of an estimated 20 to 200 MW. The Overpass query fetches only generators rated in MW or GW, which shrinks answers (Luxembourg: 1.4 MB to 0.9 MB).

## 0.4.1

### Patch Changes

- [#215](https://github.com/eigenergy/tellegen/pull/215) [`2abf68f`](https://github.com/eigenergy/tellegen/commit/2abf68f1d659f77344934e4200719d0c543194f5) Thanks [@samtalki](https://github.com/samtalki)! - The package exports its two large assets: `@tellegen/engine/wasm` is the
  WebAssembly binary and `@tellegen/engine/places` the populated places table. A
  host that preloads or budgets them names them by export (in Vite,
  `import engineWasmUrl from "@tellegen/engine/wasm?url"`) instead of reaching
  into `dist/wasm-pkg/tellegen_bg.wasm` and `dist/data/populated_places.json`.

- [#213](https://github.com/eigenergy/tellegen/pull/213) [`0cc5d77`](https://github.com/eigenergy/tellegen/commit/0cc5d77bfb79760fd326943aacafd0c89db14640) Thanks [@samtalki](https://github.com/samtalki)! - Synthetic generation refuses an unknown key at every level of the request, not
  only at the top, so a misspelt `limits`, `islands`, `osm`, `demand`,
  `calibration`, `validation`, `region` or `fallback_from` option is an
  `invalid_request` instead of being ignored. `limits.max_area_km2` must be
  positive and finite, and `limits.max_branch_factor` finite and at least 1.

  OpenStreetMap mode keeps to `limits.max_bus_count`. When the mapped substation
  levels and converters make more buses than the cap, the lowest substation
  levels are left out, one at a time, until they fit, and the
  `osm_bus_count_capped` warning says which were kept. A region whose highest
  level alone exceeds the cap is refused as `invalid_request`.

  `data/populated_places.json` records its source: it is now an object that names
  Natural Earth v5.1.2 and the SHA-256 of `ne_10m_populated_places_simple.geojson`,
  with the same rows under `places`, written by
  `scripts/derive-populated-places.py`. The report's `provenance.places` carries
  the new `source` and `source_sha256`. `loadSyntheticPlaces` still accepts a bare
  array of rows, whose provenance names no source.

## 0.4.0

### Minor Changes

- [#154](https://github.com/eigenergy/tellegen/pull/154) [`a3fdbd3`](https://github.com/eigenergy/tellegen/commit/a3fdbd3923b964654582bb384b57bd0b4ab90887) Thanks [@samtalki](https://github.com/samtalki)! - Multiconductor power-flow sessions answer a load edit with a constant-size
  summary instead of the complete terminal and equipment result, in one worker
  round trip. `BrowserMcPfSession.replaceLoadPowers` now resolves to
  `McPfSummary`; session creation returns the initial summary; detail is
  fetched on demand with `detail()` (one bus and a page of equipment ports) and
  `terminalVoltages()`/`terminalCurrents()` (transferred `Float64Array`s), and
  `result()` still returns the full result for export. Sessions report the
  engine, transfer, and parse time of each edit and the engine's live and peak
  heap. In `@tellegen/svelte`, `MulticonductorCase.result` is now the summary,
  `mcFullResult` holds a stored full result when no session does, and
  `McResults` accepts either with an optional `detail` loader.

- [#203](https://github.com/eigenergy/tellegen/pull/203) [`2ccb61c`](https://github.com/eigenergy/tellegen/commit/2ccb61c3429e4488c06c90063fbd927614d445de) Thanks [@samtalki](https://github.com/samtalki)! - Plain names for the generated types and version constants. `CONTRACT_VERSION`
  is now `ENGINE_VERSION` (the package version), `CONTRACT_SOURCE_SHA256` is now
  `API_SOURCE_SHA256` (the hash of `crates/tellegen/src/api.rs` the types were
  generated from), and the `@tellegen/engine/contracts` subpath is now
  `@tellegen/engine/types`. `@tellegen/engine/synth` is unchanged.

- [#197](https://github.com/eigenergy/tellegen/pull/197) [`786ebc8`](https://github.com/eigenergy/tellegen/commit/786ebc893b2b17b0ef55be3e6cb608dc4cf545bf) Thanks [@samtalki](https://github.com/samtalki)! - Synthetic grid generation. `createSynthSession(request, options)` opens a
  `BrowserSynthSession` on a dedicated generation worker, so cancelling a run
  never touches the interactive studies; the options take `osm`, `places` and
  `signal`. Each `advance()` runs one stage (`context`, `site_buses`,
  `topology`, `electrify`, `dc_opf`, `calibrate` once per iteration, `ac_pf`,
  `finalize`), and `run(onStep, signal)` loops them. `preview()` returns
  transferred typed arrays for binary map layers, `report()` the
  `GenerationReport`, `takeModule()` the PowerIO module with bus locations and
  branch routes, and `ingest()` the same `IngestedCase` a dropped `.pio.json`
  of that module produces; `exportGridfm()` writes GridFM graphkit JSON.
  `source: "osm"` builds the grid from Overpass documents passed as the `osm`
  option. `generateSynthetic` runs every stage in one request,
  `syntheticOverpassQueries` returns the Overpass QL for a region, and
  `loadSyntheticPlaces`, `fetchSyntheticPlaces` and `syntheticPlacesUrl` handle
  the bundled Natural Earth places table, now shipped as
  `data/populated_places.json`. Generator refusals reject with
  `SynthEngineError` and its `code` (for example `osm_insufficient`). Request
  and report types are generated from Rust and exported from
  `@tellegen/engine/synth`. `EngineHost` gains `kind` (`"worker"` or
  `"direct"`). Ingest payloads gain `coords_method`, which is
  `"generated"` when the module's history records a synthetic generation,
  so a generated grid's coordinates are not taken for a topology layout.
  Generated rows keep their annotations under `extras.tellegen_synth`, and the
  module is named `tellegen_synth_<request hash prefix>`. A ring edge that jumps
  more than 180° of longitude (a region written across the antimeridian the
  short way) is refused, request values are bounded well outside any real
  system, and OpenStreetMap circuit counts are capped at 12 per way. The
  generator adds about 0.8 MB to the wasm module (about 0.3 MB gzipped).

### Patch Changes

- [#195](https://github.com/eigenergy/tellegen/pull/195) [`d476c3d`](https://github.com/eigenergy/tellegen/commit/d476c3d0fba3306a8fc5695e29cb936499c6112a) Thanks [@samtalki](https://github.com/samtalki)! - Build against PowerIO 0.11.5. Out-of-service loads and shunts no longer enter
  the DC and AC OPF preparations, a saved multiconductor Study whose PowerIO
  solution carries more than 65,536 terminal values reads back instead of
  failing with `READ.MODULE.INVALID`, and PSS/E cases read closer to how PSS/E
  defines them: a two-winding transformer whose winding 2 ratio is not one
  scales its impedance by the square of that ratio, switched shunts read
  `MODSW` 1 as discrete and 2 as continuous voltage control, and double-quoted
  names and circuit ids are accepted. Solves of affected cases change
  accordingly.

- [#203](https://github.com/eigenergy/tellegen/pull/203) [`01f9a1e`](https://github.com/eigenergy/tellegen/commit/01f9a1e990bbec5a0adbc0545b9abb6edeb483dc) Thanks [@samtalki](https://github.com/samtalki)! - Build against PowerIO 0.11.6 with its default `sincal` feature off, so the
  wasm engine still links without a C compiler and lists no SINCAL format.
  MATPOWER cases that convert demand or impedance units after their literal
  data, such as case33bw's kW and ohm blocks, now read with the converted
  values. A multiconductor case with an island that has no voltage source is
  refused instead of being anchored implicitly, and partial three-winding
  service states keep their own IR record. PSS/E loads read `YQ` with the PSS/E
  sign and net out distributed generation, revision 34 node columns sit where
  PSS/E 34 writes them, the two-winding magnetizing admittance reaches bus I,
  and three-winding transformers keep each winding's status and all twelve
  ratings, so solved results change for such cases. Re-import Studies saved
  from the earlier reading from the original file.

## 0.3.0

### Minor Changes

- [#97](https://github.com/eigenergy/tellegen/pull/97) [`5d93591`](https://github.com/eigenergy/tellegen/commit/5d93591d74ecbba1b80fc1e0f9a9fc8e4de4cc31) Thanks [@samtalki](https://github.com/samtalki)! - Save a case with its changes and results as a portable Study. Planning goals are
  optional. Preserve geographic positions, line paths, drawings, camera settings,
  and unsolved cases. Keep cumulative demand changes and reset electrical inputs
  to the base case while retaining history.

  Add searchable equipment selection and bounded capacity or demand planning.
  Share supported objective expressions, verified candidate results, activity,
  and explicit proposal approval across browser and native tools. Every attempted
  solve counts against the stated budget.

  Restore Texas7k through reusable generator-cost preparation, with original cost
  data and measured fit errors available in Model details. Show every configured
  demo case, including an explanation when a case is unavailable.

  Add movable, resizable panels with saved layouts. Display geographic data on
  the map and drawing coordinates on a plain canvas. Include balanced AC power
  flow with voltage and angle results.

  Introduce `@tellegen/webmcp` at 0.1.0. Agents can list and select cases, query the
  same saved state displayed on screen, and continue a Study. Combine connection
  instructions and recorded tool activity in one Agent panel.

- [#129](https://github.com/eigenergy/tellegen/pull/129) [`4101e61`](https://github.com/eigenergy/tellegen/commit/4101e61cb6918e1c74164412de4ca0b5aee4c26c) Thanks [@frederikgeth](https://github.com/frederikgeth)! - Add retained fixed-point multiconductor power-flow sessions. Reuse the sparse
  factorization and last converged voltage for automatic load P/Q edits, expose
  the live session through the browser engine, and materialize portable snapshots
  only when they are needed for save, export, or geography operations.

- [#113](https://github.com/eigenergy/tellegen/pull/113) [`653bf18`](https://github.com/eigenergy/tellegen/commit/653bf186dbc8528d95633364cc42e2529708542f) Thanks [@frederikgeth](https://github.com/frederikgeth)! - Add multiconductor AC power flow for supported BMOPF networks and typed PowerIO
  inputs, including explicit neutral conductors and finite-leakage transformers.
  Inspect terminal voltages, currents, source powers, convergence, and KCL residuals.
  Save and reopen distribution studies with their input and solution modules.

  Use published PowerIO 0.11.1 dependencies. Portable electrical modules continue
  to use PowerIO IR generation 2.

  Apply the OpenDSS load-voltage envelope and report voltage validity separately
  from convergence. Support fixed-tap single-phase autotransformers, including
  ANSI Type A/B ratios, winding impedance, and excitation shunts.

  Retain electrical results when attaching geography to saved distribution studies.
  Share cancellable calculations between browser controls and WebMCP, expose
  terminal results to agent queries, and add compact notifications and
  privacy-filtered usage events.

## 0.2.0

### Minor Changes

- [#60](https://github.com/eigenergy/tellegen/pull/60) [`75021aa`](https://github.com/eigenergy/tellegen/commit/75021aa821ec5a93f35ff63ca87506aea52b42cf) Thanks [@samtalki](https://github.com/samtalki)! - Add one-pass typed JSON ingestion and bound browser file-drop batches by count
  and bytes.

- [#83](https://github.com/eigenergy/tellegen/pull/83) [`670958b`](https://github.com/eigenergy/tellegen/commit/670958b95cc05c7f24fe04694c51dfc9dd907e9a) Thanks [@samtalki](https://github.com/samtalki)! - A saved study now states the powerio release that wrote it, and a study saved by
  an earlier build no longer loads. Open the source case and save the study again.
  Case uploads are parsed as bytes, so a `.raw` or `.aux` exported in CP1252 is
  refused rather than silently mangled.

- [#60](https://github.com/eigenergy/tellegen/pull/60) [`bf35bb3`](https://github.com/eigenergy/tellegen/commit/bf35bb3988a03efc43c1201ae960859c152d26b4) Thanks [@samtalki](https://github.com/samtalki)! - Parse dropped cases from bytes, preserving decoding errors and enabling
  PowerWorld `.pwb`. Add `ingestModelJson` and route `.epc` files through the PSLF
  reader.

### Patch Changes

- [#60](https://github.com/eigenergy/tellegen/pull/60) [`4d5c2a4`](https://github.com/eigenergy/tellegen/commit/4d5c2a44a7bef2e2d0bfd0c16ab6a57a8c3daebb) Thanks [@samtalki](https://github.com/samtalki)! - Retire the wasm instance after a trap instead of serving the next request from
  it, bound the string entry points at the same 128 MiB limit as their byte
  counterparts, frame a single-point selection instead of clamping to maximum
  zoom, and keep the parsing indicator up while dropped JSON cases materialize.

- [#60](https://github.com/eigenergy/tellegen/pull/60) [`bd043f1`](https://github.com/eigenergy/tellegen/commit/bd043f15de3be6d3b78c236285bca570d0f3e8e6) Thanks [@samtalki](https://github.com/samtalki)! - Include lowered three-winding rows in rendered transmission topology and mark
  display-only rows as non-editable. Report canonical and rendered analysis row
  counts separately.
