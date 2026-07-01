# Svelte Minimal Example

This example is the integration reference for `@tellegen/svelte`. It mounts the
full `TellegenViewer` (map, panels, local file flow, solve card) in a plain
Vite + Svelte 5 app, not SvelteKit.

With `loadDefaultCases` off and no backend, the viewer starts empty: drop a
MATPOWER `.m`, PSS/E `.raw`, or `.aux` case file onto the page and it parses
and solves in browser WebAssembly. Nothing is uploaded.

Run it from the repository root:

```sh
npm ci
npm run wasm
npm run build:engine
npm run build:svelte
npm --workspace @tellegen/example-svelte-minimal run dev
```

`build:svelte` must run first: the example resolves `@tellegen/svelte` through
the package's built `dist/`.
