# Framework Packages

The reusable browser engine lives in `packages/engine` and is published as
`@tellegen/engine`. Start there for new applications.

`apps/web` is the hosted demo and Svelte UI package. It consumes
`@tellegen/engine`, adds the map, panels, local file placement, default case
loading, and demo specific state management. Those app internals are useful
examples, but they are not the engine contract.

## Which Package To Use

Use `@tellegen/engine` when an app needs:

- case parsing;
- display parsing;
- browser wasm solves;
- `Study` preview and commit;
- sensitivity requests; or
- generated TypeScript contracts.

Use `tellegen-frontend` only when an app wants to reuse the hosted demo map or
Svelte shell components. It peers on Svelte, deck.gl, and MapLibre.

## Engine Entry Point

```ts
import {
  browserWasmTransport,
  createStudy,
  formatOf,
  solveJson
} from "@tellegen/engine";
```

The engine package imports its wasm files through `?url`, so consuming apps need
a bundler with wasm asset support. Vite and SvelteKit work.

## Demo App Boundary

`apps/web/src/routes` is the hosted app. `apps/web/src/lib` contains the demo map,
controller, state, and components. These modules can change as the demo changes.
Downstream apps should not deep import from `apps/web/src/lib` or from generated
wasm folders.
