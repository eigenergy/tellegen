# @tellegen/svelte

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

- [#126](https://github.com/eigenergy/tellegen/pull/126) [`7695a83`](https://github.com/eigenergy/tellegen/commit/7695a8397fd935859d498291f3629afd21df41ea) Thanks [@samtalki](https://github.com/samtalki)! - Restore the geographic map background in production builds by bundling the
  MapLibre worker and its dependencies. Grid overlays and map tiles render together.

- [#119](https://github.com/eigenergy/tellegen/pull/119) [`f645948`](https://github.com/eigenergy/tellegen/commit/f64594898b6f9d8c6f7c780946e71813f0af2712) Thanks [@dependabot](https://github.com/apps/dependabot)! - Update MapLibre GL to a release that passes the dependency audit.

  Keep Deck.gl packages on the same version and use the official MapLibre overlay for MapLibre 6 so the interleaved map remains usable.

- Updated dependencies [[`5d93591`](https://github.com/eigenergy/tellegen/commit/5d93591d74ecbba1b80fc1e0f9a9fc8e4de4cc31), [`653bf18`](https://github.com/eigenergy/tellegen/commit/653bf186dbc8528d95633364cc42e2529708542f)]:
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
