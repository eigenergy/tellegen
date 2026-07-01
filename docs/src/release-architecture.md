# Release Architecture

The public framework surfaces are `@tellegen/engine` and `@tellegen/svelte`.

Stable release surfaces:

- `crates/tellegen` Rust API layer, including the serde request and response
  shapes in `src/api.rs`;
- `crates/tellegen-wasm` wasm adapter;
- `packages/engine` TypeScript package, generated TypeScript types, and browser
  wasm entry points;
- `packages/svelte` Svelte component package; and
- examples under `examples/`.

The hosted demo under `apps/web` is one consumer of the packages. It keeps
routes, SEO, credits, privacy, and deployment specific behavior.

## JavaScript Workspace

The repository uses npm workspaces with one root `package-lock.json` for:

- `packages/engine`;
- `packages/svelte`;
- `apps/web`;
- `examples/browser-minimal`; and
- `examples/svelte-minimal`.

Install JavaScript dependencies from the repository root:

```sh
npm ci
```

Root scripts define the package order:

- `npm run wasm` builds both wasm packages into `packages/engine`;
- `npm run build:engine` builds `@tellegen/engine`;
- `npm run build:svelte` builds `@tellegen/svelte`;
- `npm run build:example` builds both downstream examples;
- `npm run check:web`, `npm run build:web`, and `npm run smoke:web` gate the
  hosted demo;
- `npm run pack:engine` previews the engine npm package contents; and
- `npm run pack:svelte` previews the Svelte npm package contents.

## Versioning

`@tellegen/engine` and `@tellegen/svelte` start at `0.1.0`.

Before `1.0`, releases can refine public APIs while preserving the examples and
hosted demo behavior. After `1.0`, breaking public TypeScript, Svelte prop, or
Rust API changes require a semver major version.

Examples of breaking changes after `1.0`:

- removing or renaming public exports;
- removing or renaming request or response fields;
- changing enum tags, formulation ids, solve status tags, operand tags, or
  parameter tags;
- changing field units or meanings;
- tightening optional fields to required fields; and
- changing serialized request or response shapes.

Nonbreaking changes can ship in a minor version:

- adding optional fields;
- adding formulations, operands, parameters, statuses, or helper exports while
  preserving existing meanings; and
- adding component props with defaults.

Patch versions are for bug fixes and docs that do not change public APIs.

## Package Release

The package publish workflow is `.github/workflows/npm-publish.yml`. Manual runs
build wasm, build packages, run downstream import checks, run packed Svelte
consumer smoke tests, run `npm pack --dry-run`, and upload packed `.tgz`
artifacts for inspection.

Publishing is gated by tags named `engine-vX.Y.Z` for `@tellegen/engine` and
`svelte-vX.Y.Z` for `@tellegen/svelte`. The workflow checks that each tag
version matches its package metadata, then publishes with npm provenance.

Publishing requires either npm trusted publishing for this repository or an
`NPM_TOKEN` secret with publish access.

## CI Gates

CI installs JavaScript dependencies once from the root lockfile, builds
`packages/engine` before `packages/svelte`, builds the hosted demo, builds both
examples, installs the packed Svelte tarball into a temporary downstream
consumer, and runs a browser test against the hosted demo shell. The root
`ci:js` script covers JS checks, builds, the Svelte package dry run, hosted demo
smoke, and downstream smoke tests.
