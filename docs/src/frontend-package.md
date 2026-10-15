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

### Command palette and zen mode

Press Cmd+K (Ctrl+K on Windows and Linux) in the shell to open the command
palette. Type to filter, use the arrow keys to move, and press Enter to run.
Two commands ship with the shell:

- **Enter zen mode** hides the panel toolbar, every panel, the footer and the
  zoom buttons so only the map and header remain. The case strip collapses to
  the active case and reveals the rest on hover or focus, and the map
  attribution collapses to its (i) button. Escape, the **exit zen** link in
  the header, or running the command again restores the previous view. Zen
  mode lasts for the visit only.
- **Reset panel layout** returns detached or redocked panels to their default
  docks, the same as the toolbar button.

Components under `TellegenProvider` can add commands:

```ts
import { getCommandRegistry } from "@tellegen/svelte";

const commands = getCommandRegistry();
onMount(() =>
  commands.register({
    id: "fit-network",
    title: "Fit network to view",
    hint: "Reframe the map on the active case",
    keywords: ["zoom", "frame"],
    run: () => app.requestFrame("all"),
  }),
);
```

`register` returns an unregister function, and registering the same `id`
again replaces the earlier command. `title` and `hint` may be functions when
the label depends on state.

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
