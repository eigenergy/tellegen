# Packages

The first JavaScript release publishes one package:

- `engine/` is `@tellegen/engine`, the browser engine package for case parsing,
  wasm solves, `Study` preview and commit, sensitivities, and generated
  TypeScript contracts.

`apps/web` and `examples/browser-minimal` are npm workspaces, but they are not
published packages. `apps/web` is the hosted demo. `examples/browser-minimal`
is the downstream import smoke test and integration reference.

Add another package under `packages/` only when its API is reusable outside the
current hosted demo layout.
