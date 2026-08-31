# Packages

- `engine/` is `@tellegen/engine`, the browser engine package for case parsing,
  wasm solves, `Study` preview and commit, sensitivities, and generated
  TypeScript types.
- `svelte/` is `@tellegen/svelte`, the Svelte component package for the map,
  panels, local file flow, and solve card.
- `webmcp/` is `@tellegen/webmcp`, the framework-independent WebMCP contract,
  tool definitions, registration lifecycle, and headless test surface.

`apps/web`, `examples/browser-minimal`, and `examples/svelte-minimal` are npm
workspaces, but they are not published packages. `apps/web` is the hosted demo.
The examples are downstream import checks and integration references.

The browser package builds independently. The UI build order is
`@tellegen/engine` first, then `@tellegen/svelte`; the Svelte package depends on
the engine package and reexports its browser helpers where the UI needs them.
