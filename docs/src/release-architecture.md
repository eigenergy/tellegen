# Release Architecture

The public framework surface is `@tellegen/engine`.

Stable release surfaces:

- `crates/tellegen` Rust API layer, including the serde request and response shapes in `src/api.rs`;
- `crates/tellegen-wasm` wasm adapter;
- `packages/engine` TypeScript package, generated contracts, and browser wasm transport; and
- examples under `examples/`, starting with `examples/browser-minimal`.

The hosted demo under `apps/web` is one consumer of the framework. Its routes, app state, controller internals, map component wiring, and shell components are not the browser engine contract.

## Contract Versioning

`@tellegen/engine` exports `CONTRACT_VERSION`. It matches the package version. The generated TypeScript contracts also export `CONTRACT_SOURCE_SHA256`, which pins the Rust API source used for that build.

Breaking TypeScript or Rust contract changes require a semver major version once the package reaches `1.0`:

- removing or renaming request or response fields;
- changing enum tags, formulation ids, solve status tags, operand tags, or parameter tags;
- changing field units or meanings;
- tightening optional fields to required fields; and
- changing the transport method contract.

Nonbreaking changes can ship in a minor version:

- adding optional fields;
- adding formulations, operands, parameters, or statuses while preserving existing meanings; and
- adding new helper exports.

Patch versions are for bug fixes and docs that do not change serialized contracts.

## CI Gates

CI builds `packages/engine` before `apps/web`, checks generated contracts with `contracts:check`, builds the minimal downstream example, and runs a browser test against the hosted demo shell.
