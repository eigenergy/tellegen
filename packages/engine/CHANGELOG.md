# @tellegen/engine

## 0.6.0

### Minor Changes

- [#325](https://github.com/eigenergy/tellegen/pull/325) [`6972d33`](https://github.com/eigenergy/tellegen/commit/6972d33b6480d57502cd702e393223ec8e41e2b3) Thanks [@samtalki](https://github.com/samtalki)! - Make incomplete OpenStreetMap models solvable with explicit synthetic reinforcement. Keep mapped routes and voltages, label added circuits and voltage support, and provide `osm.reinforce: false` to preserve original circuit counts. Plan shunts at the feasible DC dispatch instead of an unconstrained dispatch behind network bottlenecks. Add New Zealand's 220/110 kV AC ladder and correct the dropped-line-section denominator.

## 0.5.1

### Patch Changes

- [#271](https://github.com/eigenergy/tellegen/pull/271) [`7c8919e`](https://github.com/eigenergy/tellegen/commit/7c8919eb809868fdbeab6e6cb7513990c106087d) Thanks [@BurhanAbdullah](https://github.com/BurhanAbdullah)! - Generating a grid over a long, thin region takes less memory. The fill that spaces out the generated buses kept a bucket for every cell of the region's bounding box, most of them empty and outside the region. It now keeps buckets only for occupied cells. The same request and seed generate the same grid as before.

- [#276](https://github.com/eigenergy/tellegen/pull/276) [`0b34030`](https://github.com/eigenergy/tellegen/commit/0b340305d141e62d16b64b195adbe857c2467d81) Thanks [@samtalki](https://github.com/samtalki)! - OpenStreetMap lines on shared towers (`voltage=400000;220000`) belong to the line of each level they carry. Before, such a way counted only at its highest level, so the lower-voltage line running on the same towers broke where it reached them, and its pieces ended away from any substation. Now each level gets its own copy of the way, carrying the circuits the tag lists at that level: `voltage=380000;110000;110000` with `circuits=3` is one 380 kV and two 110 kV circuits, and the levels never carry more circuits than the way. A substation that states no voltage takes every level of the shared-tower ways that end at it.

- [#314](https://github.com/eigenergy/tellegen/pull/314) [`b6461f3`](https://github.com/eigenergy/tellegen/commit/b6461f3ed81dc8672fb39582583a211b02cbf453) Thanks [@samtalki](https://github.com/samtalki)! - The engine reads cases with PowerIO 0.11.9, up from 0.11.7. A dropped JSON document that carries PowerModelsDistribution's `data_model` marker now opens as a distribution case even when its tables are shaped like PowerModels', where it used to be reported as ambiguous. PSS/E two-winding transformers stated with CZ=2 or CZ=3 keep the impedance 0.11.7 read; 0.11.8, which this release skips, rescaled it. Two-winding transformers record `psse_magnetizing_referral` and `psse_winding2_ratio` in their extras where they apply, and splitting a three-winding transformer into its star legs keeps each winding's impedance correction table.

- [#318](https://github.com/eigenergy/tellegen/pull/318) [`a20d241`](https://github.com/eigenergy/tellegen/commit/a20d24165ed2c6e7260cbdfe075ca77588e92fb0) Thanks [@samtalki](https://github.com/samtalki)! - Synthetic grids with subtransmission no longer tie pockets through ground outside the requested region. Neighbouring pockets were tied along the shortest edge of a triangulation that spans the region's convex hull, so on a concave region two pockets could be "neighbours" across land the region does not hold: on California, a 741 km 66 kV tie ran through Nevada with `boundary_role` `tie_line`, and other ties of hundreds of kilometres crossed the sea. Now two pockets are tied only by a line that stays inside the region and is no longer than the pocket reach at its voltage (60 km at 66 kV, scaled with voltage). A pocket with no such neighbour is left untied and fed through its own step-down substation, as most pockets are, so the network stays connected. A line whose straight path stays inside the region also keeps its drawn route inside it, instead of being labelled `tie_line` because of the small offset that separates lines on the map. Transmission-only grids are unchanged.

## 0.5.0

### Minor Changes

- [#268](https://github.com/eigenergy/tellegen/pull/268) [`7f1852c`](https://github.com/eigenergy/tellegen/commit/7f1852cc9de537c3abf72a44b916c975c37f6114) Thanks [@frederikgeth](https://github.com/frederikgeth)! - Detect source-less islands on import and show one context-sensitive solve button
  in Network and Studies, with exclusions visible before solving and shared solver
  settings. Scroll Studies to the operating-point overview after successful explicit
  solves. Add explicit energized-network preparation for multiconductor power flow. Retain
  the full input for inspection, identify excluded source-less islands and unsupplied
  loads, and preserve and validate network scope in saved studies, solution modules,
  and CSV exports. Use default voltage-change tolerance 1e-4 V, absolute KCL tolerance 1e-4 A,
  and relative KCL tolerance 1e-5; other solver settings remain unchanged.

  Prepare energized networks directly from retained inputs without an IR text round trip.
  Columnar operating-point results retain load-model qualification and excluded-island provenance.

- [#268](https://github.com/eigenergy/tellegen/pull/268) [`7f1852c`](https://github.com/eigenergy/tellegen/commit/7f1852cc9de537c3abf72a44b916c975c37f6114) Thanks [@frederikgeth](https://github.com/frederikgeth)! - Expose multiconductor power flow settings and distinguish convergence from original load-model fidelity. Results and CSV exports retain qualification and effective settings. Simulation snapshots use version 2, with no canonical PowerIO solution for modified or unverified load equations; version 1 snapshots are reassessed on replay. Load-envelope defaults are unchanged and failed reruns retain the previous result.

- [#286](https://github.com/eigenergy/tellegen/pull/286) [`888859b`](https://github.com/eigenergy/tellegen/commit/888859b6ab4b11d63dabaadd6389e56d3227cd73) Thanks [@samtalki](https://github.com/samtalki)! - A multiconductor session can hand over its complete result as columns. `BrowserMcPfSession.resultTables()` returns `McPfResultTables`: the terminals, equipment ports, and source reactions of `result()`, row for row, with each name stored once and every value in a typed array. The worker transfers the arrays rather than copying them, and `mcPfResultTables(result)` gives a stored `McPfResult` the same form.

  The overview and the diagram now read a live session's operating point this way, so analysing a whole-city model no longer parses a 166 MiB JSON result into an object per terminal and port. `MulticonductorCase.overviewResult` holds either form.

- [#284](https://github.com/eigenergy/tellegen/pull/284) [`ef7e448`](https://github.com/eigenergy/tellegen/commit/ef7e448abf5959b489ac94b728283ea50bd67707) Thanks [@samtalki](https://github.com/samtalki)! - A multiconductor case can stay parsed in the engine instead of crossing the worker boundary as PowerIO IR text. `ingestJsonDrop` and `ingestDistCaseBytes` take `{ retain: true }` and then return the parsed module as `payload.input`, a `BrowserMcInput`, in place of `module_json`. `createMcPfSession` accepts that input, `BrowserMcPfSession.input()` returns the edited input in the same form, and `BrowserMcInput.moduleJson()` writes the IR text only when it is asked for. Without `retain`, both ingest calls return `module_json` as before.

  The Svelte controller imports multiconductor cases this way, so AC power flow on a whole-city model no longer fails with "input exceeds 128 MiB limit". For a 57,757-bus model, the pretty-printed IR was 187.6 MB. `MulticonductorCase.moduleJson` is now `MulticonductorCase.mcInput`, which holds either a `BrowserMcInput` or IR text.

- [#291](https://github.com/eigenergy/tellegen/pull/291) [`01184dd`](https://github.com/eigenergy/tellegen/commit/01184dd88035cfeb8c0186e8075685a3b6b6ec55) Thanks [@samtalki](https://github.com/samtalki)! - Generate state-level grids down to subtransmission, in Synthetic and
  OpenStreetMap mode. A region whose places lie mostly in one US state takes
  that state's demand (EIA-861 sales with an EIA-930 peak factor), capacity mix
  (EIA-860) and voltage ladder; California's is 500/230/115/66 kV. A ladder that
  reaches below 100 kV is built level by level: a meshed top level,
  subtransmission pockets fed from step-downs with tap junctions, regulated
  secondaries and nameplate sized by firm capacity credit. Synthetic California
  is held to the California Test System at three seeds, and OpenStreetMap
  California at its own run, in `docs/evidence/synth/cats/`.

  OpenStreetMap mode keeps 60–70 kV lines where the ladder reaches them, splits
  mapped lines at their junctions into tap buses, prunes spurs that end nowhere,
  leaves out junctions beyond the clip buffer, adds step-downs where the mapped
  network cannot carry its load, and regulates subtransmission buses beyond the
  pocket reach of any step-down. Generation is faster in both modes: angle
  solves use a sparse Cholesky factorisation, line ends snap through a grid
  index, and containment tests read only the polygon edges that can cross the
  point's latitude. OpenStreetMap California takes 1.1 s natively, from 5.1 s,
  and 1.8 s in the wasm engine; Synthetic California, 7,361 buses, 1.3 s in the
  wasm engine.

  The report gains `BusKind` `"tap"`, `context.subdivision`,
  `context.demand_by_subdivision`, `context.fuel_mix_basis`, and the
  `"subdivision"` voltage basis. `limits.max_bus_count` defaults to 12,000,
  from 3,000. A transmission-only synthetic grid sized from demand still stops at
  3,000 buses; one that reaches subtransmission is sized up to the cap.

### Patch Changes

- [#285](https://github.com/eigenergy/tellegen/pull/285) [`3aa86e6`](https://github.com/eigenergy/tellegen/commit/3aa86e6ca4c79d485cf452c6806065644ec5f7ce) Thanks [@samtalki](https://github.com/samtalki)! - Grow the engine's wasm memory in larger steps. The allocator used to grow
  linear memory one 64 KiB page at a time, and on a 57,757-bus distribution case
  those tens of thousands of grows took most of the ingest. Ingesting that case
  in the worker is now about three times faster. Linear memory, which never
  shrinks, can end a few percent higher than before after a large ingest.

## 0.4.6

### Patch Changes

- [#273](https://github.com/eigenergy/tellegen/pull/273) [`5e9b3a8`](https://github.com/eigenergy/tellegen/commit/5e9b3a8a5b1ef616091366e53ce7059d97162d79) Thanks [@samtalki](https://github.com/samtalki)! - Add a structured log of errors and engine findings. `app.log` records what PowerIO reported while reading a case or a geographic layer, what each solve reported, what each export's writer reported, and every `app.error`, with severity, step, source, code, file, element and buses. `DiagnosticLogView` shows it grouped by step and file with filters, Copy and Download; the default shell adds a "Log" panel, and case import notes now list their findings as rows. `Solution` carries the solve's `diagnostics` (islands, HVDC transfers), which the browser Study already returned.

## 0.4.5

### Patch Changes

- [#264](https://github.com/eigenergy/tellegen/pull/264) [`75edb80`](https://github.com/eigenergy/tellegen/commit/75edb800137d908d56f17aa38221fa62f96e0615) Thanks [@samtalki](https://github.com/samtalki)! - The DC OPF, DC power flow and AC power flow solve networks with in-service HVDC lines. Each line's stated transfer enters the bus balance as fixed injections at its two terminal buses, as PowerIO 0.11.7 prepares them, and the solve reports PowerIO's `BUILD.HVDC.FIXED_INJECTION` diagnostic. A line with a terminal bus outside the solve injects at neither end and reports `BUILD.HVDC.TERMINAL_INACTIVE`. Generated OpenStreetMap grids with HVDC links now open, solve and export. The AC OPF and the SOC relaxation still refuse in-service HVDC lines.

## 0.4.4

### Patch Changes

- [#254](https://github.com/eigenergy/tellegen/pull/254) [`020ca51`](https://github.com/eigenergy/tellegen/commit/020ca5140035f294dbb7b14c3747cf6de4b97092) Thanks [@samtalki](https://github.com/samtalki)! - Build against PowerIO 0.11.7. AC power flow on a case with HVDC lines now
  includes their scheduled transfers as fixed bus injections; DC OPF and DC power
  flow still leave them out. OpenDSS feeders read the circuit's source
  impedance, so the distribution graph gains an internal source bus and its
  impedance line, and a `switch=y` line reads as a closed 1 mOhm line. PSS/E
  VSC DC lines are read instead of skipped.

## 0.4.3

### Patch Changes

- [#171](https://github.com/eigenergy/tellegen/pull/171) [`1fbc8f1`](https://github.com/eigenergy/tellegen/commit/1fbc8f1ee9f432aa9ee5bb390281b67af16f21ed) Thanks [@samtalki](https://github.com/samtalki)! - `SolveRequest` accepts `start: "flat" | "case"` (`PowerFlowStart`). With
  `"case"`, the AC power flow starts Newton from the case's stored bus voltages
  (each bus's stored magnitude and its angle relative to the reference bus), and
  falls back to the flat start if that state does not converge. Other
  formulations refuse `"case"`. AC power flow now also accepts a reference bus
  with a nonzero stated angle and reports every angle against it.

- [#194](https://github.com/eigenergy/tellegen/pull/194) [`65dd440`](https://github.com/eigenergy/tellegen/commit/65dd4403665c2682850175ef75f3dcbcfa86e4b2) Thanks [@samtalki](https://github.com/samtalki)! - Add an opt-in, capability-gated browser AC OPF client backed by a separate
  WASI worker, with hard worker cancellation and no main-thread fallback.

- [#175](https://github.com/eigenergy/tellegen/pull/175) [`a54557f`](https://github.com/eigenergy/tellegen/commit/a54557f23d755ec77814f74e0dcca5d09ff11a82) Thanks [@samtalki](https://github.com/samtalki)! - DC OPF sensitivities now cover requests with linear `constraints`: every cell
  differentiates the constrained program. The new `ConstraintLimit` parameter
  differentiates with respect to a constraint's limits, with columns keyed
  `{ Constraint: index }` by request position.

- [#173](https://github.com/eigenergy/tellegen/pull/173) [`d768cb2`](https://github.com/eigenergy/tellegen/commit/d768cb2a96f0fbd70b93169b9f7b50af3176e332) Thanks [@samtalki](https://github.com/samtalki)! - DC OPF requests accept `limits` (`LimitOptions`):
  - `angle_difference: false` drops the angle-difference rows.
  - `thermal` keeps `"all"`, `"rated"`, or `{ branches: [...] }` thermal limits.
  - `lazy` enforces thermal limits incrementally until none is violated.

  `SolveResponse.limit_rounds` (`LimitRound[]`) reports each solve's program size,
  iterations, and violations. The program also no longer carries rows for
  inactive limits or pinned shedding, which lets PGLib's `case78484` converge.

- [#170](https://github.com/eigenergy/tellegen/pull/170) [`5bd83ed`](https://github.com/eigenergy/tellegen/commit/5bd83ed4ccd79e36290aef146702b3ca398b8d18) Thanks [@samtalki](https://github.com/samtalki)! - `SolveRequest` accepts `constraints`: linear constraints on the DC OPF over
  branch flows, bus net injections, and generator outputs, such as interface
  or transfer limits. Each states `lower <= sum(coefficient * quantity) <= upper`
  in MW. `SolveResponse.constraints` reports each row's value, limits, shadow
  price, and whether it binds, and `ProblemCaps.constraints` lists the term kinds
  a formulation accepts. The new types are `LinearConstraint`, `ConstraintTerm`,
  `ConstraintTermKind`, and `ConstraintResult`. Other formulations refuse
  constraints.

- [#228](https://github.com/eigenergy/tellegen/pull/228) [`20c0f73`](https://github.com/eigenergy/tellegen/commit/20c0f7348482106f286818ee3ecf4c0d50b2e17d) Thanks [@samtalki](https://github.com/samtalki)! - Multiconductor graphs carry declared voltage limits and ratings, and the Studies
  view gains an operating-point overview.

  In `@tellegen/engine`, `DistGraphBus.voltage_limits` holds the model's declared
  phase–neutral and phase–phase voltage bounds in volts, and `DistGraphEdge.rating`
  holds per-conductor ampacity (`i_max`) or, for a transformer, its largest winding
  rating at each bus it connects (`s_rating_by_bus`), the same on every winding-pair
  edge. Both are present only when the model declares them.

  In `@tellegen/svelte`, `McWorkbench` summarises a multiconductor power-flow result:
  violations, losses, voltage ranges per declared reference, maximum loading and
  maximum unbalance; a ranked issue list that selects the bus or branch; search
  over buses and branches; per-phase voltage bars against declared limits and
  per-conductor currents at both ends; sortable bus and branch tables; and CSV export. Cases
  larger than 25,000 buses compute the overview on request. Setting
  `MulticonductorCase.schematic` makes `TellegenMap` show the case as a deck.gl
  schematic or at its source positions, with an animated switch between them,
  result colouring, the shared selection and PNG/SVG export. `overviewResult` holds
  the operating point the overview and schematic share. The schematic layout runs in
  the engine: `mcSchematicLayout` in `@tellegen/engine` returns tidy or leaf-order
  geometry and the display forest, reserving room below each bus for its attached
  equipment. The schematic draws transformer, switch (open or closed) and attachment
  symbols (load, generator, inverter-based resource, source, shunt) with a legend.
  Lines show their conductors at close zoom; buses are named with short ids; parts not
  energised from any source are drawn faint; and outside selections fly into view.
  The schematic can be walked bus by bus from the keyboard, with a spoken description.
  Solved P/Q and power-flow arrows show at close zoom, hover is linked across the
  schematic and tables, and each LV network (galvanic zone) is outlined (on geographic
  positions, as a corridor along its lines) and tinted by its worst state, with a summary tag. MV lines draw heavier than LV, and LV
  detail fades when zoomed out. Sources, generators, large IBRs and transformers stay
  visible as fixed-size landmarks, placed by priority. Geographic positions draw over the main
  map's basemap, which fades in and out with the layout switch. The diagram opens on the case's own
  positions and switches to the schematic from there. Branches drawn along the same route fan
  apart on hover, with a card to pick between them, and buses at the same point spread
  apart the same way. The schematic can be downloaded as a PowerIO
  diagram layer, follows a case's own routes in source positions, and a selected
  branch gets a multi-wire view of its conductors. Unbalance is the voltage
  unbalance factor from terminal phasors, shown beside the magnitude spread.
  `controller.multiFullResult`
  reads the full operating point from a stored result or the live session, and
  `TellegenShell` keeps `overviewResult` current for the active case
  (`controller.followOverview`, `readOverview` and `overviewOnRequest`). A
  multi-winding transformer counts once in losses and issues, and its loading is
  judged per connected bus. Graph buses carry `nominal_voltage`, the declared rating
  of a winding or source there; the diagram colours lines by voltage level, and a
  hover shows the feeder and network of what it is over, with an optional path back
  to the source. Graph buses also carry `attachments` (each attachment's terminals in
  its own order and its connection) and transformer edges `windings` (bus, wye or
  delta, terminals, rating). The diagram draws each transformer once with its winding
  connections or centre tap, and each load or shunt with its conductors and, closest
  in, its terminals. The
  renderer-independent logic lives in `src/lib/diagram`, ported from the diagram
  projection draft ([#177](https://github.com/eigenergy/tellegen/issues/177)).

- [#172](https://github.com/eigenergy/tellegen/pull/172) [`21b8892`](https://github.com/eigenergy/tellegen/commit/21b8892712aa5995e78d2bb5ca01b362016e8c9b) Thanks [@samtalki](https://github.com/samtalki)! - Solves accept networks with several islands. Every formulation solves each
  supplied island against its own reference, so each island clears at its own
  price. `SolveResponse.diagnostics` (`SolveDiagnostic[]`) reports how the solve
  treated the islands. An island with no in-service generator is left out with
  its unserved load stated (`island_deenergized`). A supplied island without a
  reference bus gets one at its largest generator
  (`island_reference_designated`). If one island states several references, the
  AC power flow keeps the first as its slack (`island_extra_reference`).

- [#229](https://github.com/eigenergy/tellegen/pull/229) [`feb9cb0`](https://github.com/eigenergy/tellegen/commit/feb9cb0a9f991479ab5888a0ac83e3548268fc33) Thanks [@samtalki](https://github.com/samtalki)! - Multiconductor graphs carry every bus voltage bound PowerIO reads, in volts.
  `DistVoltageLimits` gains the phase-to-ground bounds (BMOPF `v_min`/`v_max`, PMD
  `vm_lb`/`vm_ub`) as `v_min_by_terminal` and `v_max_by_terminal`, keyed by the phase
  terminal PowerIO assigns each bound to, and the sequence and neutral-to-ground
  bounds `vpos_min`, `vpos_max`, `vneg_max`, `vzero_max` and `vn_max`. A bound vector
  that does not match the bus's phase terminals, or a non-finite entry, is left out.
  `vpp_pairs` names the phase-terminal pairs that `vpp_min`/`vpp_max` are ordered by,
  (1,2), (2,3), (3,1) of the phase terminals as BMOPF 0.2.0 states, on a bus with two
  or three phase terminals.

- [#194](https://github.com/eigenergy/tellegen/pull/194) [`c721344`](https://github.com/eigenergy/tellegen/commit/c7213444e5c6e9a904a9db73e3195c755b05d875) Thanks [@samtalki](https://github.com/samtalki)! - Expose AC OPF recovery diagnostics from the POUNCE 0.13.0 worker and show the
  total iteration count, including retries, in the solve card. Mark recovered
  solves while preserving support for responses without recovery diagnostics.

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
