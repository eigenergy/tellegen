# Framework Packages

The reusable browser packages live under `packages/`. The first framework
release publishes both `@tellegen/engine` and `@tellegen/svelte`.

Use `@tellegen/svelte` when a Svelte app wants the map, panels, local file flow,
and solve card as components. Use `@tellegen/engine` when an app wants case
parsing, browser WebAssembly solves, studies, previews, and sensitivities
without the tellegen UI.

`apps/web` is the hosted demo. It consumes `@tellegen/svelte` and keeps only
route level concerns such as SEO pages, `/credits`, and `/privacy`.

## Svelte UI Package

Install:

```sh
npm install @tellegen/svelte maplibre-gl @deck.gl/core @deck.gl/layers @deck.gl/maplibre
```

`maplibre-gl` and the three `@deck.gl` packages are peer dependencies: the map
shares their instances with the host through `extraLayers` and `onMapReady`, so
the app installs them and resolves one copy of each.

Render the full viewer:

```svelte
<script lang="ts">
  import { TellegenViewer } from "@tellegen/svelte";
  import "@tellegen/svelte/styles.css";
</script>

<TellegenViewer />
```

The default viewer loads bundled cases from `/api`, parses dropped local case
files in the browser, and runs supported local solves in WebAssembly.

`TellegenViewer` accepts:

- `apiBase`, default `/api`
- `loadDefaultCases`, default `true`
- `docsHref`
- `orgHref`
- `orgLabel`
- `showFooter`, default `true`
- `theme`, `light` (default) or `dark`
- `cartoApiKey`, the basemap key (see [Basemap](#basemap))

`theme` and `cartoApiKey` follow their props. The others are read once, when
the viewer mounts; remount it to change them.

Use a different backend base path like this:

```svelte
<TellegenViewer apiBase="/tellegen/api" />
```

Use local files only by disabling bundled case loading:

```svelte
<script lang="ts">
  import { TellegenViewer } from "@tellegen/svelte";
  import "@tellegen/svelte/styles.css";
</script>

<TellegenViewer loadDefaultCases={false} showFooter={false} />
```

For apps where state should survive route changes, mount the provider in a
persistent layout and render the shell on the page:

```svelte
<script lang="ts">
  import { TellegenProvider } from "@tellegen/svelte";
  import "@tellegen/svelte/styles.css";

  let { children } = $props();
</script>

<TellegenProvider>
  {@render children()}
</TellegenProvider>
```

```svelte
<script lang="ts">
  import { TellegenShell } from "@tellegen/svelte";
</script>

<TellegenShell />
```

The provider owns the controller and its cases. Unmounting it disposes the
controller, which frees every case's browser Study and multiconductor session
in the engine worker, so a provider that should keep its cases across pages
belongs in a layout that stays mounted.

`@tellegen/svelte` also exports lower level pieces for custom shells:

- `TellegenMap`, with basemap, theme, projection, camera, extra-layer and
  `onMapReady` props (see below)
- `AppState`, `CaseState`, `LocalCase`, and `createAppState`
- `Controller` and `createController`, including `openBalancedModule`
- `createApiClient`
- `PanelHost`, `NotificationCenter`, `ShellEnvironment`, panels and controls
  from `@tellegen/svelte/components`
- basemap helpers, colors, display helpers, formatting helpers, and public types

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

### Basemap

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

### Dark theme

`<TellegenProvider theme="dark">` (or `<TellegenViewer theme="dark">`) sets
`data-theme="dark"` on the document root while mounted, which switches every
token in `styles.css`. The default shell passes the theme to its map, so the
basemap and the overlay palette follow. A custom shell passes
`theme="dark"` to `TellegenMap` itself. A server-rendered host can put the
attribute in its template to avoid a light first paint. Text tokens keep at
least 7:1 against the panel on either ground.

### Opening a module from memory

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

## Engine Package

Install:

```sh
npm install @tellegen/engine
```

Use the engine package when you want to build your own UI:

```ts
import { createStudy, formatOf, ingestCase, solveModule } from "@tellegen/engine";
```

`ingestCase` and solvable JSON ingestion return a retained PowerIO module in
`module_json`. Both `createStudy` and `solveModule` accept that generation-2
PowerIO IR module. Geographic transforms update and return the same module.

The engine package resolves its wasm files relative to the package module. Apps
must serve package asset files from `node_modules`; Vite and SvelteKit handle
that path.

## Examples

- `examples/svelte-minimal` imports `@tellegen/svelte` and runs with
  `loadDefaultCases={false}` for local files only.
- `examples/browser-minimal` imports `@tellegen/engine` directly and has no map
  stack.

Downstream apps should not import from `apps/web/src/lib` or from generated wasm
folders.
