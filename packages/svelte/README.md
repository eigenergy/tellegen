# @tellegen/svelte

Svelte components for tellegen maps, case panels, local case files, and browser solves.

```sh
npm install @tellegen/svelte maplibre-gl @deck.gl/core @deck.gl/layers @deck.gl/maplibre
```

`maplibre-gl` and the three `@deck.gl` packages are peer dependencies: the map
shares their instances with the host through `extraLayers` and `onMapReady`, so
the app installs them and resolves one copy of each.

Import the component and stylesheet in your app:

```svelte
<script lang="ts">
  import { TellegenViewer } from "@tellegen/svelte";
  import "@tellegen/svelte/styles.css";
</script>

<TellegenViewer />
```

Use local files only by disabling bundled case loading:

```svelte
<script lang="ts">
  import { TellegenViewer } from "@tellegen/svelte";
  import "@tellegen/svelte/styles.css";
</script>

<TellegenViewer loadDefaultCases={false} showFooter={false} />
```

The viewer accepts at most 32 files in one ingestion batch. Each file must be
at most 128 MiB, and the aggregate batch must also be at most 128 MiB. Files
remain in the browser on this path.

`TellegenViewer` accepts:

- `apiBase`, default `/api`
- `acOpfWasmUrl`, default `null` (optional experimental AC OPF asset)
- `loadDefaultCases`, default `true`
- `docsHref`
- `orgHref`
- `orgLabel`
- `showFooter`, default `true`
- `theme`, `light` (default) or `dark`
- `cartoApiKey`, the basemap key (see [Basemap](#basemap))

`theme` and `cartoApiKey` follow their props. The others are read once, when
the viewer mounts; remount it to change them.

Use `TellegenProvider` and `TellegenShell` when state should survive route changes:

```svelte
<script lang="ts">
  import { TellegenProvider, TellegenShell } from "@tellegen/svelte";
  import "@tellegen/svelte/styles.css";
</script>

<TellegenProvider>
  <TellegenShell />
</TellegenProvider>
```

The provider owns the controller and its cases. Unmounting it disposes the
controller, which frees every case's browser Study and multiconductor session
in the engine worker, so a provider that should keep its cases across pages
belongs in a layout that stays mounted.

## Experimental AC OPF

A development host can pass `acOpfWasmUrl="/experimental-acopf/tellegen_acopf_wasi.wasm"`
to `TellegenViewer` or `TellegenProvider`. The asset must be built and served
separately; the selector enables AC OPF only after a worker probe succeeds.
See the [local setup and release gates](../../docs/src/acopf-pounce-decision.md#experimental-browser-abi).

Balanced networks and stored canonical AC OPF instances are supported at the
base operating point. Imported instances retain their objective and constraints.
Demand/rating editing, LMPs, sensitivities, and saving/exporting the AC OPF case
are unavailable. Geographic sidecars and layout downloads remain available.
Without the optional worker, imported AC OPF instances remain viewable but
cannot be solved. No server solve is substituted for AC OPF.

## Panel layout

The shell docks Network and Studies on the left, and Solver and Agent on the
right. The most recently opened panel gets more room in its dock; click another
heading to expand it. Drag a panel heading to detach it; use the corner handle to resize it.
Panel options dock it on either side, and **Reset layout** restores the default
positions. Focus a heading or resize handle and use arrow keys for keyboard
control; hold Shift for larger steps. Narrow windows show one active drawer.
Panel positions are stored in the browser, independently of case and Study data.

Custom panels can join the same layout inside `TellegenProvider`:

```svelte
<TellegenProvider>
  <TellegenShell />
  <PanelFrame id="notes" title="Notes" side="right" order={30}>
    <p>Case notes</p>
  </PanelFrame>
</TellegenProvider>
```

Import `PanelFrame` from `@tellegen/svelte`. Its optional `open` prop is bindable;
`width` sets its preferred width, and the `headerActions` snippet adds compact
controls beside its title.

### Custom shells

`TellegenShell` is one arrangement of exported parts. A host can mount its own
inside `TellegenProvider`:

```svelte
<!-- +layout.svelte -->
<script lang="ts">
  import { TellegenProvider } from "@tellegen/svelte";
  import "@tellegen/svelte/styles.css";
  import MyShell from "./MyShell.svelte";
</script>

<TellegenProvider loadDefaultCases={false} apiBase={null} theme="dark">
  <MyShell />
</TellegenProvider>
```

```svelte
<!-- MyShell.svelte: reads the controller from context, so it must be a child -->
<script lang="ts">
  import { PanelFrame, getController } from "@tellegen/svelte";
  import { TellegenMap } from "@tellegen/svelte/map";
  import { NotificationCenter, PanelHost, ShellEnvironment } from "@tellegen/svelte/components";

  const ctrl = getController();
</script>

<ShellEnvironment />
<TellegenMap
  theme="dark"
  projection="globe"
  onbusclick={ctrl.selectBus}
  onlocalbusclick={ctrl.selectLocalBus}
  onmultibusclick={ctrl.selectMultiBus}
  onplacecase={ctrl.placeCase}
  onmapclick={ctrl.clearSelection}
/>
<PanelHost showToolbar={false} />
<NotificationCenter />
<PanelFrame id="region" title="Region" side="left">...</PanelFrame>
```

The components show data, labels and controls, not host copy. A host that
wants a note under a local case's stats passes `ControlPanel` a `caseNote`
snippet, which receives the case; `DropZone` renders its children under the
drop prompt:

```svelte
<ControlPanel>
  {#snippet caseNote(lc)}
    <p>{lc.label} is a synthetic grid.</p>
  {/snippet}
</ControlPanel>
```

`ShellEnvironment` is renderless and keeps the compact-layout, pointer and
viewport state in step, as the default shell does. `PanelHost` reads
`--tellegen-dock-left`, `--tellegen-dock-right` and `--tellegen-dock-bottom`
from an ancestor for its dock insets (defaults 20px, 20px and 100px), so a
shell with its own rail or status bar moves the docks without restyling them.

When a panel closes, focus moves to its toolbar button. Without the toolbar,
focus goes back to the control that had focus when the panel opened. If there
is none, it goes to the panel's dock, a region named "Left panels" or "Right
panels". On narrow screens that region is the drawer, named "Panels".

A shell that shows `app.error` itself can also read `app.errorDetail`. When
it is set, `app.error` is a message written for people, and the detail holds
the technical text behind it. `NotificationCenter` shows that text in a
collapsed disclosure.

### Log

`app.log` keeps every error and engine finding of the session as structured
entries: severity (error, warning or note), the step (`Open`, `Coordinates`,
`Solve`, `Export`), who reported it (`PowerIO` or `tellegen`), the code, the
message, the file or case it belongs to, the element and buses it names, and
PowerIO's suggested action and details. The controller records what PowerIO
reported while reading a case or a geographic layer, what each solve reported
(islands, HVDC transfers), what each export's writer reported, and every
`app.error`. A finding reported again counts up in place.

`DiagnosticLogView` (from `@tellegen/svelte/components`) shows the log grouped
by step and file, with severity filters, Copy, Download (`tellegen.log` JSON)
and Clear; pass `onbus` to make the named buses selectable. The default shell
shows it in a "Log" panel (`LogPanel`). A host can add its own findings:

```ts
getAppState().log.record({
	severity: 'warning',
	stage: 'Fetch',
	source: 'Overpass',
	message: 'overpass-api.de answered 504; trying the next mirror'
});
```

## Basemap

CARTO's vector styles suit a network overlay best: quiet roads, flat land and
clear water. CARTO's basemap terms ask for an API key, which you can get from
[carto.com/basemaps/apikey](https://carto.com/basemaps/apikey). No account is
needed, and non-commercial use (personal projects, research, teaching,
non-profits) is free up to 5 million requests a month. The browser sends the
key with every style, tile, glyph and sprite request as `?key=`, so it is
public by design: restrict it to the hosts that serve your app in CARTO's key
dashboard, and use one key per project.

The default order depends on the key:

- **With `cartoApiKey`:** the map loads the CARTO style. That is
  `positron-nolabels` for the light theme and `dark-matter-nolabels` for the
  dark theme, or the labelled styles with `hideBasemapLabels={false}`. It
  falls back to the matching [OpenFreeMap](https://openfreemap.org/) style.
- **Without a key:** the map loads the OpenFreeMap style (`positron` or
  `dark`) and falls back to the keyless CARTO style.

The map switches to the fallback in three cases:

- the primary style cannot be fetched;
- its tiles answer 401, 403 or 429;
- tile fetches keep failing with nothing arriving.

Whenever an OpenFreeMap style renders, as primary or as fallback,
`openFreeMapEdits` tunes it towards CARTO's look. Road lines start at zoom 8.
In the dark theme, water and waterways take Dark Matter's colours and land
fills take one tone. The edits match layers by `source-layer`, not by id.

Label layers are hidden by default. The attribution control is always on and
shows the credits of the style on screen.

`TellegenMap` (also exported from `@tellegen/svelte/map`) accepts these props
besides its click callbacks:

- `mapStyle`: a style URL or inline `StyleSpecification`. Default: the CARTO
  style for `theme` with a key, else the OpenFreeMap style.
- `fallbackMapStyle`: the style used when the primary fails; `null` disables
  the switch. Default: whichever of the two is not the primary.
- `cartoApiKey`: the CARTO key. It is trimmed, and an empty, blank or
  absent key means none.
- `theme`: `'light'` (default) or `'dark'`. Picks the default styles and the
  overlay palette (`MAP_PALETTE`).
- `projection`: `'mercator'` (default) or `'globe'`. Re-applied after every
  style load, so a basemap switch keeps it.
- `initialCamera`: `{ center, zoom, bearing, pitch }`, read once per mount.
- `hideBasemapLabels`: default `true`.
- `extraLayers`: deck.gl layers drawn after tellegen's own, in the same
  overlay.
- `attribution`: `'compact'` (default) or `'full'`. There is no off switch.
- `onMapReady(map, overlay)`: called with the MapLibre map and the deck
  overlay once they exist. It may return a cleanup, and it fires again after
  a context-loss or basemap rebuild. If it throws, the map and its window
  and document listeners are still released when the map is torn down.
- `onBasemapFallback(reason)`: the switch to the fallback happened.
  `reason.from` and `reason.to` name the two providers.
- `onBasemapError(reason)`: no basemap can be shown. The network layers
  still draw. `reason.from` names the provider that failed last, and
  `reason.to` is `null`.

`TellegenProvider` and `TellegenViewer` also take `cartoApiKey`, and pass it,
together with `theme`, to the default shell's map. Both follow their props:
changing either rebuilds that map in place. A SvelteKit 3 host can
declare the key as an optional public build-time variable:

```ts
// src/env.ts
import { defineEnvVars } from "@sveltejs/kit/env";

export const variables = defineEnvVars({
  PUBLIC_CARTO_BASEMAP_KEY: { public: true, static: true, schema: (value) => value ?? "" }
});
```

```svelte
<script lang="ts">
  import { TellegenProvider } from "@tellegen/svelte";
  import { PUBLIC_CARTO_BASEMAP_KEY } from "$app/env/public";
</script>

<TellegenProvider cartoApiKey={PUBLIC_CARTO_BASEMAP_KEY}>...</TellegenProvider>
```

On SvelteKit 2, import it from `$env/static/public` instead; that module
needs the variable defined at build time, even if empty.

Changing `mapStyle`, `fallbackMapStyle`, `theme`, `hideBasemapLabels`,
`cartoApiKey` or `attribution` rebuilds the map in place and keeps the camera.

The helpers behind the defaults are exported too:

- `OPENFREEMAP_STYLES` and `CARTO_STYLES`;
- `cartoStyleUrl(url, apiKey)`;
- `defaultBasemap(theme, { cartoApiKey })` and `resolveBasemap(props)`;
- `attachBasemapFallback(map, spec, handlers)`;
- `isOpenFreeMapStyle(style)` and `openFreeMapEdits(style, theme)`.

`defaultBasemap` and `resolveBasemap` also say who serves each style, so a
host can word its credits and fallback notice without reading URLs. Their
`primaryProvider` and `fallbackProvider` are `'carto'`, `'openfreemap'`, or
`'custom'` for a style the host passed, and `fallbackProvider` is `null` when
the fallback is off. `defaultBasemap` is typed `DefaultBasemap`: two style
URLs, one CARTO and one OpenFreeMap, so its providers are never `'custom'` and
its fallback is never `null`. The fallback reasons use the same names in
`from` and `to`.

A Content Security Policy must allow `https://tiles.openfreemap.org` and
`https://*.cartocdn.com` in `connect-src` and `img-src`. The style, vector
tiles, fonts and sprites are all fetched. The engine underneath also needs
`script-src 'wasm-unsafe-eval'` (never `'unsafe-eval'`) and `worker-src
'self'`, since its workers are same-origin.

`@deck.gl/core`, `@deck.gl/layers`, `@deck.gl/maplibre` and `maplibre-gl`
are peer dependencies (`^9.4.0` for deck.gl, `^6.15.0` for maplibre-gl), so
the host's copies are the ones the map uses, including the deck.gl layers it
passes in `extraLayers` and the MapLibre map it gets from `onMapReady`. When
two deck.gl versions load, deck.gl throws "multiple versions detected"; if
another dependency pulls in a second copy, point both at one version with npm
`overrides` (or `resolutions` in pnpm and Yarn).

The map needs no Vite settings, in a build or in the dev server. MapLibre's
worker comes from a `?worker&url` import, which a production build bundles
with its shared chunk. Vite's dev server pre-bundles this package and cannot
pre-bundle that import, so the package's `./maplibre-worker` export has a
`development` condition that loads it at run time, outside the pre-bundle.
That export is internal to `TellegenMap`. A host that excluded
`@tellegen/svelte` from `optimizeDeps` to work around this can drop the
exclusion.

## Dark theme

`<TellegenProvider theme="dark">` (or `<TellegenViewer theme="dark">`) sets
`data-theme="dark"` on the document root while mounted, which switches every
token in `styles.css`. The default shell passes the theme to its map, so the
basemap and the overlay palette follow. A custom shell passes
`theme="dark"` to `TellegenMap` itself. A server-rendered host can put the
attribute in its template to avoid a light first paint. Text tokens keep at
least 7:1 against the panel on either ground.

## Opening a module from memory

A host that produces a balanced powerio module itself opens it without a file:

```ts
import { getController } from "@tellegen/svelte";

const ctrl = getController();
const local = await ctrl.openBalancedModule({ name: "synthetic grid", moduleJson });
// or, with the payload the engine's ingestCase already returned for it:
await ctrl.openBalancedModule({ name, payload, frame: false });
// swap it for a new result in place
await ctrl.replaceLocalCase(local, { name, payload });
```

Options: `geoLayer` (a `.geo.json` document, for a module without
coordinates; a module that carries `Bus.location` needs none),
`formulation`, `activate` (default `true`; `false` returns the prepared case
without adding it) and `frame` (default `true`). `replaceLocalCase` keeps the
case's slot in the header, its formulation and display mode, and the selected
bus when the new case has it. With `loadDefaultCases={false}` the controller
fetches nothing on `load()`; with `apiBase={null}` it has no API client at all
and every server path reports compute off.

## Engine Reexports

The package reexports the browser engine helpers, including `ingestCase`,
`classifyJson`, and `ingestJsonDrop`, for custom file imports. These APIs use
byte input: pass `Uint8Array` to `ingestCase`, await
`classifyJson(bytes)` and read its `{ kind, format }` result, and use
`ingestJsonDrop(bytes)` to classify and parse in one call. `isStudyPackageText`
was removed; stored PowerIO documents report `kind === "module"`.
`JsonDropKind` also includes `transmission`, `distribution`, `ambiguous`, and
`unknown`; BMOPF and PMD are distribution `format` values rather than separate
kinds. Every solvable ingest payload carries the retained module in
`module_json`; the viewer uses that generation-2 IR for Study construction and
geographic edits without keeping a second serialized network.

Drawing layers use a plain canvas with their own x/y coordinates. Saved Studies
retain the drawing center and scale separately from the geographic map camera.

## Release

Build and inspect the package from the repository root:

```sh
npm ci
npm run wasm
npm run build:engine
npm run build:svelte
npm run pack:svelte
npm run test:svelte-packed
```

`@tellegen/svelte` is published with `@tellegen/engine` in the first framework
release. The package ships only `dist`, the README, the MIT license text, and
package metadata. The packed smoke test installs the generated tarballs into a
temporary Svelte consumer and builds it so missing exports, styles, or wasm
assets fail before publish.

Unmounting `TellegenProvider` (including through `TellegenViewer`) disposes its
controller and terminates pending AC OPF probes and solves, including background
cases. Custom hosts that construct a `Controller` directly must call
`controller.dispose()` when their owning view is destroyed.

## Citing

If you use this package in published work, please cite the tellegen paper:
Samuel Talkington, Frederik Geth, Qian Zhang, Le Xie, and Skyler Liu. 2026.
*Interactive Power Flow in the Browser*. The Inaugural ACM Conference on Digital
Transformation (ACM DXConf 2026), Ann Arbor, MI, USA.
[doi:10.1145/3847238.3849038](https://doi.org/10.1145/3847238.3849038). The
[repository README](https://github.com/eigenergy/tellegen#citing) has the BibTeX
entry.
