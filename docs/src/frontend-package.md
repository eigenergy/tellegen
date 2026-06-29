# Frontend Package

`apps/web` is both the public demo and the `tellegen-frontend` Svelte package.
The package exports the map, the demo shell components, the typed case and
solution data model, and the browser WebAssembly wrapper.

This follows the SvelteKit package convention: `src/lib` is the public package
surface, while `src/routes` is the demo or documentation app that consumes it.
That is why the reusable code lives under `apps/web/src/lib` instead of a
separate top-level `packages/` directory.

## Install

The package peers on Svelte, deck.gl, and MapLibre. A consuming Svelte app should
install them beside the package:

```sh
npm install tellegen-frontend svelte @deck.gl/core @deck.gl/layers @deck.gl/mapbox maplibre-gl
```

For local development from this repository, install the package by path:

```sh
npm install ../tellegen/apps/web
```

Use TypeScript `moduleResolution: "bundler"` in the consuming app. SvelteKit
projects created by the current Svelte CLI already use that setting.

## Entry Points

The package export map is:

- `tellegen-frontend` — root framework exports: `TellegenMap`, state, context,
  controller, display helpers, and wasm helpers.
- `tellegen-frontend/map` — map component plus color scales, display helpers,
  and `TellegenMapProps`.
- `tellegen-frontend/components` — the demo shell components used by the
  reference app.
- `tellegen-frontend/types` — type only imports for network, solution,
  sensitivity, local case, display, and wasm data.
- `tellegen-frontend/wasm` — parser, display reader, DC solver, and `Study`
  wrapper. This entry imports the bundled `.wasm` files through `?url`.
- `tellegen-frontend/styles.css` — tellegen CSS variables and shared component
  styles.

## Minimal SvelteKit Use

Import the package CSS once in the app shell:

```svelte
<script lang="ts">
	import 'tellegen-frontend/styles.css';
	import {
		TellegenMap,
		createAppState,
		createController,
		setAppState,
		setController
	} from 'tellegen-frontend';

	const app = createAppState();
	const ctrl = createController(app);

	setAppState(app);
	setController(ctrl);
</script>

<TellegenMap
	onbusclick={ctrl.selectBus}
	onlocalbusclick={ctrl.selectLocalBus}
	onplacecase={ctrl.placeLocalCase}
	onmapclick={ctrl.clearSelection}
/>
```

Call `ctrl.load()` after mount when the app wants bundled cases from the tellegen
backend. That controller path expects the backend API under `/api`, matching the
demo:

```ts
import { onMount } from 'svelte';

onMount(() => {
	if (!ctrl.casesLoaded) void ctrl.load();
});
```

Dropped local case files do not upload. They go through the browser wasm parser
and solver.

## Using the Wasm Entry Directly

Apps that only need parsing or solving can import the wasm wrapper without the
map or demo controller:

```ts
import {
	DEFAULT_FORMULATION,
	createStudy,
	formatOf,
	ingestCase,
	solveDc
} from 'tellegen-frontend/wasm';

const format = formatOf(file.name);
if (!format) throw new Error('unsupported case format');

const parsed = await ingestCase(await file.text(), format);
const base = await solveDc(parsed.name, parsed.network_json, {}, null);

const study = await createStudy(parsed.network_json, DEFAULT_FORMULATION);
try {
	const preview = study.preview({ 1: 25 });
	console.log(base.solution.objective, preview.objectiveDelta);
} finally {
	study.free();
}
```

The wasm wrapper loads on demand. The core DC package is the fallback path; the
sensitivity package carries the `Study` and the conic formulation.
