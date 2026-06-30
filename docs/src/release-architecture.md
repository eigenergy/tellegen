# Release Architecture

The public framework surface is `@tellegen/engine`.

Stable release surfaces:

- `crates/tellegen` Rust API layer, including the serde request and response shapes in `src/api.rs`;
- `crates/tellegen-wasm` wasm adapter;
- `packages/engine` TypeScript package, generated contracts, and browser wasm transport; and
- examples under `examples/`, starting with `examples/browser-minimal`.

The hosted demo under `apps/web` is one consumer of the framework. Its routes, app state, controller internals, map component wiring, and shell components are not the browser engine contract.

## JavaScript Workspace

The repository uses npm workspaces with one root `package-lock.json` for:

- `packages/engine`;
- `apps/web`; and
- `examples/browser-minimal`.

Install JavaScript dependencies from the repository root:

```sh
npm ci
```

Root scripts define the package order:

- `npm run wasm` builds both wasm packages into `packages/engine`;
- `npm run build:engine` builds `@tellegen/engine`;
- `npm run test:import` builds the minimal downstream example through the engine package import smoke test;
- `npm run check:web`, `npm run build:web`, and `npm run smoke:web` gate the hosted demo; and
- `npm run pack:engine` previews the npm package contents without publishing.

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

## Package Release

Only `@tellegen/engine` is published for the first framework release. `apps/web`
is private and remains a hosted demo consumer.

The package publish workflow is `.github/workflows/npm-publish.yml`. Manual runs
build wasm, build the engine package, run the downstream import smoke test, run
`npm pack --dry-run`, and upload the packed `.tgz` artifact for inspection.

Publishing is gated by tags named `engine-vX.Y.Z`. The workflow checks that the
tag version matches `packages/engine/package.json`, then publishes with:

```sh
npm --workspace @tellegen/engine publish --provenance --access public
```

The workflow grants `id-token: write` so npm can attach provenance from GitHub
Actions. Publishing requires either npm trusted publishing for this repository
or an `NPM_TOKEN` secret with publish access.

## CI Gates

CI installs JavaScript dependencies once from the root lockfile, builds
`packages/engine` before `apps/web`, checks generated contracts with
`contracts:check`, builds the minimal downstream example, and runs a browser
test against the hosted demo shell.
