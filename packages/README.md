# Packages

- `engine/` is `@tellegen/engine`, the browser engine package for case parsing,
  wasm solves, `Study` preview and commit, sensitivities, and generated
  TypeScript types.
- `svelte/` is `@tellegen/svelte`, the Svelte component package for the map,
  panels, local file flow, and solve card.
- `webmcp/` is `@tellegen/webmcp`, the WebMCP contract, tool definitions,
  registration lifecycle, and test surface without a UI framework dependency.

`apps/web`, `examples/browser-minimal`, and `examples/svelte-minimal` are npm
workspaces, but they are not published packages. `apps/web` is the hosted demo.
The examples are downstream import checks and integration references.

Each browser package builds independently. `@tellegen/svelte` builds after
`@tellegen/engine` because it depends on the engine package and reexports its
browser helpers where the UI needs them.
