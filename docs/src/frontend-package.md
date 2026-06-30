# Framework Packages

The reusable browser packages live under `packages/`.

Use `@tellegen/svelte` when a Svelte app wants the map, panels, local file flow,
and solve card as components.

Use `@tellegen/engine` when an app wants case parsing, browser WebAssembly
solves, studies, previews, and sensitivities without the tellegen UI.

`apps/web` is the hosted demo. It consumes `@tellegen/svelte` and keeps only
route level concerns such as SEO pages, `/credits`, and `/privacy`.

## Svelte UI Package

Install:

```sh
npm install @tellegen/svelte
```

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

`@tellegen/svelte` also exports lower level pieces for custom shells:

- `TellegenMap`
- `AppState`, `CaseState`, `LocalCase`, and `createAppState`
- `Controller` and `createController`
- `createApiClient`
- panels and controls from `@tellegen/svelte/components`
- colors, display helpers, formatting helpers, and public types

## Engine Package

Install:

```sh
npm install @tellegen/engine
```

Use the engine package when you want to build your own UI:

```ts
import { createStudy, formatOf, ingestCase, solveJson } from "@tellegen/engine";
```

The engine package imports its wasm files through `?url`, so consuming apps need
a bundler with wasm asset support. Vite and SvelteKit handle that path.

## Examples

- `examples/svelte-minimal` imports `@tellegen/svelte` and runs with
  `loadDefaultCases={false}` for local files only.
- `examples/browser-minimal` imports `@tellegen/engine` directly and has no map
  stack.

Downstream apps should not import from `apps/web/src/lib` or from generated wasm
folders.
