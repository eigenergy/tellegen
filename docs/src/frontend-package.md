# Framework Packages

The reusable browser engine lives in `packages/engine` and is published as
`@tellegen/engine`. Start there for new applications.

`apps/web` is the hosted demo. It is a private npm workspace that consumes
`@tellegen/engine`, adds the map, panels, local file placement, default case
loading, and demo state. Those internals are useful examples, but they are not
the package contract.

The first npm release includes only `@tellegen/engine`. A Svelte component
package can come later when there is a reusable component API that is not tied
to the current demo layout.

## Which Package To Use

Use `@tellegen/engine` when an app needs:

- case parsing;
- display parsing;
- browser wasm solves;
- `Study` preview and commit;
- sensitivity requests; or
- generated TypeScript contracts.

Use `apps/web` as source code reference when an app wants to copy a map or panel
pattern. Do not import it as a package.

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
