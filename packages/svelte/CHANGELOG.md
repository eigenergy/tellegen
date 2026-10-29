# @tellegen/svelte

## 0.4.4

### Patch Changes

- [#194](https://github.com/eigenergy/tellegen/pull/194) [`65dd440`](https://github.com/eigenergy/tellegen/commit/65dd4403665c2682850175ef75f3dcbcfa86e4b2) Thanks [@samtalki](https://github.com/samtalki)! - Add an opt-in, capability-gated browser AC OPF client backed by a separate
  WASI worker, with hard worker cancellation and no main-thread fallback.

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

- [#229](https://github.com/eigenergy/tellegen/pull/229) [`feb9cb0`](https://github.com/eigenergy/tellegen/commit/feb9cb0a9f991479ab5888a0ac83e3548268fc33) Thanks [@samtalki](https://github.com/samtalki)! - The multiconductor overview ranks phase–ground voltages against declared
  phase-to-ground bounds, alongside the phase–neutral and phase–phase bands. Each
  declared reference gets its own pairs (`a–ground` and so on) in the issues, the
  voltage ranges, the phase bars, value labels, tables and CSV export; `BusLimits`
  holds one entry per reference. Non-uniform phase–phase bounds are ranked per pair,
  in the pair order the engine names. A case that declares no voltage limits or ratings
  says so plainly, and one whose limits the overview does not rank says that instead.

  The overview and the schematic share one result snapshot. `MulticonductorCase`
  derives `overviewSnapshot` from `overviewResult` once per operating point, and
  `overviewInputs` and `overviewSearch` once per graph. `McWorkbench` takes
  `snapshot`, `inputs` and `search` in place of `result`.

- [#194](https://github.com/eigenergy/tellegen/pull/194) [`c721344`](https://github.com/eigenergy/tellegen/commit/c7213444e5c6e9a904a9db73e3195c755b05d875) Thanks [@samtalki](https://github.com/samtalki)! - Expose AC OPF recovery diagnostics from the POUNCE 0.13.0 worker and show the
  total iteration count, including retries, in the solve card. Mark recovered
  solves while preserving support for responses without recovery diagnostics.

- [#225](https://github.com/eigenergy/tellegen/pull/225) [`12b79b5`](https://github.com/eigenergy/tellegen/commit/12b79b56a8e509aae12dbbbfa1c0375c2745b008) Thanks [@samtalki](https://github.com/samtalki)! - The multiconductor map hands its transformer symbol to deck.gl as a prepacked
  icon atlas, so nothing is fetched. It was a `data:` URL that deck.gl fetched,
  and a Content Security Policy whose `connect-src` leaves out `data:` blocked it,
  so the map drew without symbols.
- Updated dependencies [[`1fbc8f1`](https://github.com/eigenergy/tellegen/commit/1fbc8f1ee9f432aa9ee5bb390281b67af16f21ed), [`65dd440`](https://github.com/eigenergy/tellegen/commit/65dd4403665c2682850175ef75f3dcbcfa86e4b2), [`a54557f`](https://github.com/eigenergy/tellegen/commit/a54557f23d755ec77814f74e0dcca5d09ff11a82), [`d768cb2`](https://github.com/eigenergy/tellegen/commit/d768cb2a96f0fbd70b93169b9f7b50af3176e332), [`5bd83ed`](https://github.com/eigenergy/tellegen/commit/5bd83ed4ccd79e36290aef146702b3ca398b8d18), [`20c0f73`](https://github.com/eigenergy/tellegen/commit/20c0f7348482106f286818ee3ecf4c0d50b2e17d), [`21b8892`](https://github.com/eigenergy/tellegen/commit/21b8892712aa5995e78d2bb5ca01b362016e8c9b), [`feb9cb0`](https://github.com/eigenergy/tellegen/commit/feb9cb0a9f991479ab5888a0ac83e3548268fc33), [`c721344`](https://github.com/eigenergy/tellegen/commit/c7213444e5c6e9a904a9db73e3195c755b05d875)]:
  - @tellegen/engine@0.4.3

## 0.4.3

### Patch Changes

- [#219](https://github.com/eigenergy/tellegen/pull/219) [`5ebcad8`](https://github.com/eigenergy/tellegen/commit/5ebcad80c08495ec01d80dc06a885a2817585e40) Thanks [@samtalki](https://github.com/samtalki)! - The demand slider's Nearby and Full range buttons are named by their visible
  text again. 0.4.2 labelled them "Nearby demand range" and "Full demand range",
  which do not contain the visible "Full range", so speech input could not reach
  the button by what it shows (WCAG 2.5.3, axe `label-content-name-mismatch`).
  The "Demand range" group still names the pair.

## 0.4.2

### Patch Changes

- [#211](https://github.com/eigenergy/tellegen/pull/211) [`1f30672`](https://github.com/eigenergy/tellegen/commit/1f30672b5f7b05e0314d05b1248c0bf8bb4fa291) Thanks [@samtalki](https://github.com/samtalki)! - Components show data, labels and controls, and leave notes to the host. The
  "parsed in your browser" and "never uploaded" lines are gone from
  `LocalCaseDetails`, `DropZone` and `AppHeader`, and coordinate provenance is a
  labelled value (Coordinates: Generated, Synthetic layout, Manual layout, From
  file or From the geographic file's name) beside a File row. The solver card lists
  the calculation, iterations and time as labelled values, and the compute-off
  notices no longer carry a contact address.

  A host adds its own note through two optional snippets: `ControlPanel`'s
  `caseNote`, rendered under a local case's stats with the case as its argument,
  and `DropZone`'s children, rendered under the drop prompt.

  Visible text is in sentence case, and the monospace face is kept for numbers
  and identifiers. The case actions read Save case, Save solution, Export as and
  Save geography (Save drawing in diagram view). Hosts that match on the old
  strings need updating: the solver card no longer shows "OPF solve", the bus
  lookup's accessible name is "Bus lookup", and the full demand range button is
  named "Full demand range".

- [#214](https://github.com/eigenergy/tellegen/pull/214) [`4ea6b1c`](https://github.com/eigenergy/tellegen/commit/4ea6b1c7126b348bc07b6ac66d211b6d261bcde8) Thanks [@samtalki](https://github.com/samtalki)! - `TellegenProvider` disposes its controller when it unmounts. The new
  `Controller.dispose()` frees every case's browser Study, including one still
  being built, and every multiconductor session, cancels their pending edits and
  in-flight requests, and builds nothing afterwards, so a provider's Studies and
  sessions no longer stay in the engine worker.

  `theme` and `cartoApiKey` on `TellegenProvider` and `TellegenViewer` follow
  their props: switching either updates the root `data-theme` attribute and
  rebuilds the default shell's map in place. The other props are still read once,
  at mount.

  `TellegenMap` assigns its teardown before it calls `onMapReady`, so a host
  callback that throws no longer keeps the MapLibre map and its window and
  document listeners alive.

- [#215](https://github.com/eigenergy/tellegen/pull/215) [`2abf68f`](https://github.com/eigenergy/tellegen/commit/2abf68f1d659f77344934e4200719d0c543194f5) Thanks [@samtalki](https://github.com/samtalki)! - `maplibre-gl`, `@deck.gl/core`, `@deck.gl/layers` and `@deck.gl/maplibre` are
  peer dependencies instead of dependencies, with the same ranges. The map shares
  their instances with the host through `extraLayers` and `onMapReady`, so the
  host installs them and one copy of each resolves. npm 7 and later and pnpm 8
  and later install missing peers on their own; with Yarn or
  `--legacy-peer-deps`, add them to the app's dependencies.
- Updated dependencies [[`2abf68f`](https://github.com/eigenergy/tellegen/commit/2abf68f1d659f77344934e4200719d0c543194f5), [`0cc5d77`](https://github.com/eigenergy/tellegen/commit/0cc5d77bfb79760fd326943aacafd0c89db14640)]:
  - @tellegen/engine@0.4.1

## 0.4.1

### Patch Changes

- [#209](https://github.com/eigenergy/tellegen/pull/209) [`107aa8c`](https://github.com/eigenergy/tellegen/commit/107aa8c9e68e7b19531f33ef18029a7cc7e081de) Thanks [@samtalki](https://github.com/samtalki)! - Show bus demand as colour. A `demand` display mode shades buses by their
  demand in MW, committed changes included, on a square-root scale with a cool
  viridis ramp that cannot be mistaken for the nodal value ramp.
  `AppState.busSizing` chooses how markers are sized: `power` (the default)
  scales them by the larger of demand and generation as before, and `uniform`
  draws every bus alike. Uniform markers on the map keep one ground radius
  between 2 and 4.5 pixels, so a whole country reads as a mesh with its lines
  showing and a closer view as distinct buses. Changing the calculation keeps a
  demand colouring, and the bus tooltip does not repeat demand under it.
  `@tellegen/svelte/map` exports `busMarkerRadius`, `busRadiusFor`,
  `demandColor`, `demandDomain`, `demandGradient`, `UNIFORM_BUS_RADIUS`,
  `UNIFORM_BUS_MIN_RADIUS` and `UNIFORM_BUS_GROUND_RADIUS`.

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

- [#202](https://github.com/eigenergy/tellegen/pull/202) [`41471e1`](https://github.com/eigenergy/tellegen/commit/41471e191c520531b1392ab4e55fe19f9e87520b) Thanks [@samtalki](https://github.com/samtalki)! - Accessibility fixes in the panels:

  - Text in the Network panel, the solver card, the size legend and the residual sparkline is at least 12 px; sizes from 10 to 11.5 px grow by 2 px, so the hierarchy holds.
  - The dark `--ink-faint` is `#95a6b6`, 7.14:1 against the panel composited over `#4a4a4a`.
  - The dark `--border-strong`, which marks a panel's edge over the map, is `#859cb4`. It measures 6.30:1 against the panel and 3.13:1 against bare `#4a4a4a`; the old `#6f8aa6` measured only 2.47:1 against the grey.
  - The panel close button has a 44 px target under coarse pointers.
  - When a panel closes, focus moves to its toolbar button. Without one, it returns to the control that opened the panel, else to the panel's dock (a region named "Left panels" or "Right panels"), or to the drawer on narrow screens.
  - Panel content that overflows becomes a focusable, labelled region.
  - The demand range toggle is a labelled group, and the "full range" button's accessible name now contains its visible label.
  - Arrow keys on the demand and rating sliders step about 1% of the range, and never less than 0.5 MW, so every press leaves the 0.25 MW dead zone.
  - An infeasible DC OPF reports "No feasible dispatch: demand cannot be served within line and generator limits". The solver text moves to the new `app.errorDetail` and into the notice's disclosure.
  - The `neutral` entry of `MAP_PALETTE` (new in this release) fills a neutral terminal in the multiconductor detail, whose outline now follows `--ink`.

- [#196](https://github.com/eigenergy/tellegen/pull/196) [`02c308e`](https://github.com/eigenergy/tellegen/commit/02c308e4504ffbba9e296c55670331189595bfa0) Thanks [@samtalki](https://github.com/samtalki)! - `TellegenMap` takes its basemap from props instead of a hardcoded CARTO URL,
  and the default order depends on a CARTO basemap key. CARTO's terms ask for
  one; keys are free for non-commercial use from carto.com/basemaps/apikey.

  - With `cartoApiKey`, the keyed CARTO style leads (positron-nolabels or
    dark-matter-nolabels), and OpenFreeMap is the fallback.
  - Without a key, OpenFreeMap leads, and the keyless CARTO style is the
    fallback. The key is trimmed, so a blank one counts as none.

  Whenever an OpenFreeMap style renders, road lines start at zoom 8. In the dark
  theme, water and land are recoloured towards CARTO Dark Matter
  (`openFreeMapEdits`, matched by `source-layer`).

  New props:

  - `mapStyle` and `fallbackMapStyle`;
  - `cartoApiKey`;
  - `theme` (`'light' | 'dark'`);
  - `projection` (`'mercator' | 'globe'`, re-applied after every style load);
  - `initialCamera`;
  - `hideBasemapLabels`;
  - `extraLayers`;
  - `attribution` (`'compact' | 'full'`, never off);
  - `onMapReady(map, overlay)`, `onBasemapFallback` and `onBasemapError`.

  `basemap.ts` exports `OPENFREEMAP_STYLES`, `CARTO_STYLES`, `cartoStyleUrl`,
  `defaultBasemap`, `resolveBasemap`, `attachBasemapFallback`,
  `isOpenFreeMapStyle` and `openFreeMapEdits`. `defaultBasemap` and
  `resolveBasemap` name the provider of each style in `primaryProvider` and
  `fallbackProvider` (`'carto'`, `'openfreemap'`, or `'custom'` for a style the
  host passed; `defaultBasemap` returns the narrower `DefaultBasemap`, never
  custom and never without a fallback), and the reasons given to `onBasemapFallback` and
  `onBasemapError` carry `from` and `to` in the same terms. The overlay palette
  is `MAP_PALETTE[theme]`. `TellegenProvider` and `TellegenViewer` take
  `cartoApiKey`, and the default shell passes it and `theme` to its map.

  The map now runs in a host's Vite dev server with no `optimizeDeps` settings.
  MapLibre's worker URL comes from a new internal `./maplibre-worker` export,
  whose `development` condition keeps the worker's `?worker&url` import out of
  the dev server's pre-bundle, which stopped the dev server at startup.
  `maplibre-gl` is now `^6.15.0`.

  The controller gains `openBalancedModule({ name, moduleJson | payload,
geoLayer?, formulation?, activate?, frame? })` and
  `replaceLocalCase(previous, input)` to open a balanced powerio module from
  memory, `ControllerOptions.loadDefaultCases` and `apiBase: null`, and an
  internal gate so `load()` fetches nothing when default cases are off;
  `createApiClient({ apiBase: null })` returns `null`. `TellegenProvider`
  accepts `theme="dark"` (sets `data-theme` on the document root for the new
  dark tokens in `styles.css`) and marks the shell loaded when default cases are
  off. `@tellegen/svelte/components` exports `PanelHost` (with `showToolbar`
  and `--tellegen-dock-left/right/bottom` insets), `NotificationCenter` and the
  renderless `ShellEnvironment` for custom shells. Hosts with a Content Security
  Policy must allow `https://tiles.openfreemap.org` and `https://*.cartocdn.com`
  in `connect-src` and `img-src`.

### Patch Changes

- [#203](https://github.com/eigenergy/tellegen/pull/203) [`6f661e1`](https://github.com/eigenergy/tellegen/commit/6f661e1b007555e886ebf5b05be5d62e677621ba) Thanks [@samtalki](https://github.com/samtalki)! - Stop showing an in-progress calculation after a failed solve, and distinguish
  map initialization failures from case-file errors in notifications.

  Reject a missing AC/SOCWR browser input instead of silently falling back to the
  DC-only server solver, carrying forward the fail-closed boundary from the
  experimental AC OPF integration.

- [#203](https://github.com/eigenergy/tellegen/pull/203) [`13c761a`](https://github.com/eigenergy/tellegen/commit/13c761abb465fb17b57ddef807d7fcc151e236da) Thanks [@samtalki](https://github.com/samtalki)! - The basemap also switches to its fallback when the tile server answers HTTP
  5xx for every tile, not only when tile requests fail at the network level.

- [#197](https://github.com/eigenergy/tellegen/pull/197) [`ef46197`](https://github.com/eigenergy/tellegen/commit/ef461978ddf59b077ad801c9e8a32356e2300e72) Thanks [@samtalki](https://github.com/samtalki)! - The local case export menu offers PSS/E v35 (`psse35`) beside the v33 RAW.

- [#197](https://github.com/eigenergy/tellegen/pull/197) [`708d2f9`](https://github.com/eigenergy/tellegen/commit/708d2f9ae17fc19e9cf737c7079498d794cc0fa0) Thanks [@samtalki](https://github.com/samtalki)! - The case panel describes the coordinates of a generated grid (an ingest
  payload with `coords_method: "generated"`) as a synthetic grid sited on
  real geography, no longer as a topology layout.

- [#203](https://github.com/eigenergy/tellegen/pull/203) [`295c0f3`](https://github.com/eigenergy/tellegen/commit/295c0f3a03f977284e960e389d1309531088082c) Thanks [@samtalki](https://github.com/samtalki)! - Close the panel options menu when clicking outside it.
- Updated dependencies [[`a3fdbd3`](https://github.com/eigenergy/tellegen/commit/a3fdbd3923b964654582bb384b57bd0b4ab90887), [`2ccb61c`](https://github.com/eigenergy/tellegen/commit/2ccb61c3429e4488c06c90063fbd927614d445de), [`d476c3d`](https://github.com/eigenergy/tellegen/commit/d476c3d0fba3306a8fc5695e29cb936499c6112a), [`01f9a1e`](https://github.com/eigenergy/tellegen/commit/01f9a1e990bbec5a0adbc0545b9abb6edeb483dc), [`786ebc8`](https://github.com/eigenergy/tellegen/commit/786ebc893b2b17b0ef55be3e6cb608dc4cf545bf)]:
  - @tellegen/engine@0.4.0

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

### Patch Changes

- [#141](https://github.com/eigenergy/tellegen/pull/141) [`9c46ae4`](https://github.com/eigenergy/tellegen/commit/9c46ae441a60979b0cc42eedc7eb9934db5f03b9) Thanks [@samtalki](https://github.com/samtalki)! - Show the case file button on every device. It was hidden whenever the primary
  input could not hover, which included touchscreen laptops with a mouse and all
  phones, so local files could not be opened there at all. Drag and drop is now
  gated on `any-hover`/`any-pointer`, and the button no longer depends on it.
  Diagnosed by @MohamedNumair in [#110](https://github.com/eigenergy/tellegen/issues/110).

- [#126](https://github.com/eigenergy/tellegen/pull/126) [`7695a83`](https://github.com/eigenergy/tellegen/commit/7695a8397fd935859d498291f3629afd21df41ea) Thanks [@samtalki](https://github.com/samtalki)! - Restore the geographic map background in production builds by bundling the
  MapLibre worker and its dependencies. Grid overlays and map tiles render together.

- [#119](https://github.com/eigenergy/tellegen/pull/119) [`f645948`](https://github.com/eigenergy/tellegen/commit/f64594898b6f9d8c6f7c780946e71813f0af2712) Thanks [@dependabot](https://github.com/apps/dependabot)! - Update MapLibre GL to a release that passes the dependency audit.

  Keep Deck.gl packages on the same version and use the official MapLibre overlay for MapLibre 6 so the interleaved map remains usable.

- Updated dependencies [[`5d93591`](https://github.com/eigenergy/tellegen/commit/5d93591d74ecbba1b80fc1e0f9a9fc8e4de4cc31), [`4101e61`](https://github.com/eigenergy/tellegen/commit/4101e61cb6918e1c74164412de4ca0b5aee4c26c), [`653bf18`](https://github.com/eigenergy/tellegen/commit/653bf186dbc8528d95633364cc42e2529708542f)]:
  - @tellegen/engine@0.3.0

## 0.2.0

### Minor Changes

- [#60](https://github.com/eigenergy/tellegen/pull/60) [`75021aa`](https://github.com/eigenergy/tellegen/commit/75021aa821ec5a93f35ff63ca87506aea52b42cf) Thanks [@samtalki](https://github.com/samtalki)! - Add one-pass typed JSON ingestion and bound browser file-drop batches by count
  and bytes.

- [#60](https://github.com/eigenergy/tellegen/pull/60) [`bf35bb3`](https://github.com/eigenergy/tellegen/commit/bf35bb3988a03efc43c1201ae960859c152d26b4) Thanks [@samtalki](https://github.com/samtalki)! - Recognize PowerIO 0.9 `.pio.json` packages, accept `.epc` and `.pwb` case files,
  and export studies as PowerWorld `.aux` or PSLF `.epc`.

- [#83](https://github.com/eigenergy/tellegen/pull/83) [`670958b`](https://github.com/eigenergy/tellegen/commit/670958b95cc05c7f24fe04694c51dfc9dd907e9a) Thanks [@samtalki](https://github.com/samtalki)! - A saved study now states the powerio release that wrote it, and a study saved by
  an earlier build no longer loads. Open the source case and save the study again.
  Case uploads are parsed as bytes, so a `.raw` or `.aux` exported in CP1252 is
  refused rather than silently mangled.

### Patch Changes

- [#60](https://github.com/eigenergy/tellegen/pull/60) [`e6da333`](https://github.com/eigenergy/tellegen/commit/e6da333a54fbb2972cc5072a81c16601c2f1fe85) Thanks [@samtalki](https://github.com/samtalki)! - Keep valid network coordinates visible when a case also contains points that
  cannot be rendered on a Web Mercator map.

- [#85](https://github.com/eigenergy/tellegen/pull/85) [`de85124`](https://github.com/eigenergy/tellegen/commit/de851243086697c893a69d2ecabb8465fd8fbb75) Thanks [@samtalki](https://github.com/samtalki)! - Require deck.gl 9.3.10 or newer. The declared range moved from `^9.1.0`, so a
  consumer resolving an older 9.x now gets the newer one.

- [#60](https://github.com/eigenergy/tellegen/pull/60) [`4d5c2a4`](https://github.com/eigenergy/tellegen/commit/4d5c2a44a7bef2e2d0bfd0c16ab6a57a8c3daebb) Thanks [@samtalki](https://github.com/samtalki)! - Retire the wasm instance after a trap instead of serving the next request from
  it, bound the string entry points at the same 128 MiB limit as their byte
  counterparts, frame a single-point selection instead of clamping to maximum
  zoom, and keep the parsing indicator up while dropped JSON cases materialize.

- [#60](https://github.com/eigenergy/tellegen/pull/60) [`bd043f1`](https://github.com/eigenergy/tellegen/commit/bd043f15de3be6d3b78c236285bca570d0f3e8e6) Thanks [@samtalki](https://github.com/samtalki)! - Include lowered three-winding rows in rendered transmission topology and mark
  display-only rows as non-editable. Report canonical and rendered analysis row
  counts separately.
- Updated dependencies [[`75021aa`](https://github.com/eigenergy/tellegen/commit/75021aa821ec5a93f35ff63ca87506aea52b42cf), [`670958b`](https://github.com/eigenergy/tellegen/commit/670958b95cc05c7f24fe04694c51dfc9dd907e9a), [`4d5c2a4`](https://github.com/eigenergy/tellegen/commit/4d5c2a44a7bef2e2d0bfd0c16ab6a57a8c3daebb), [`bd043f1`](https://github.com/eigenergy/tellegen/commit/bd043f15de3be6d3b78c236285bca570d0f3e8e6), [`bf35bb3`](https://github.com/eigenergy/tellegen/commit/bf35bb3988a03efc43c1201ae960859c152d26b4)]:
  - @tellegen/engine@0.2.0
