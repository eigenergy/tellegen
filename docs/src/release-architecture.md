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

### Saved studies and the `.pio.json` format version

A saved study is a powerio `.pio.json` document, and its format version is
powerio's, not tellegen's. powerio's reader accepts its own lineage and rejects
anything else, so a saved study can outlive the build that reads it: when
powerio narrows that lineage, every previously saved study stops loading.

tellegen cannot migrate such a file — the document is a snapshot of a case plus
an edit log, and the way back is to regenerate it, not to repair it. What
tellegen does is say so: a package from an unreadable lineage reports that the
source case should be opened and the study saved again, rather than being
reported as malformed. `crates/tellegen/src/package.rs` owns that wording for
all three surfaces that load a package.

The tellegen block inside the document (`study.app["tellegen"]`, carrying the
formulation and solve options) has its own `schema_version`, independent of the
envelope's, and is validated separately.

## Releasing

Package versions and the crate version move independently. A release that
touches only one surface publishes only that surface, which is why two release
bots drive the pipeline rather than one.

Nobody cuts a tag by hand, and no registry token is stored in this repository.
Both registries authenticate with OIDC: the workflow's own identity is exchanged
for a credential that lives for the length of the job. Each exchange sits inside
a GitHub deployment environment (`crates-io`, `npm`) whose protection rules —
required reviewers, and the rule limiting it to `main` — live in the repository
settings. Naming the environment in the workflow is only half of it; the
protection rule is what makes the approval real.

### Packages

`@tellegen/engine` and `@tellegen/svelte` release through changesets.

A change to either package ships with a changeset: run `npm run changeset`, say
what changed and how big the bump is, and commit the generated file alongside
the change. `.changeset/README.md` has the details.

On a push to `main`, `.github/workflows/release-npm.yml` keeps a "Release
packages" pull request open that consumes every pending changeset — version
bumps, changelog entries, the regenerated engine `CONTRACT_VERSION`, and a
refreshed lockfile. Merging that pull request runs the gates and publishes.
Tags are changesets' `@tellegen/<name>@X.Y.Z`.

`@tellegen/svelte` depends on `@tellegen/engine` by registry range, so a release
that moves both publishes the engine first. Nothing enforced that ordering when
each package had its own tag trigger.

### Crate

`tellegen` is the only crate that publishes to crates.io; `tellegen-wasm`,
`tellegen-server`, `tellegen-cli`, and `benchmarks` carry `publish = false`, and
`release-plz.toml` restates that so adding a publishable crate is a deliberate
edit.

On a push to `main`, `.github/workflows/release-crate.yml` keeps a version-bump
pull request open with the changelog entry written from the commits since the
last tag. Merging it runs the gates, tags `tellegen-vX.Y.Z`, cuts the GitHub
release, and publishes.

### Inspecting an artifact

`.github/workflows/package-inspect.yml` is a manual run that builds exactly what
a release would upload — `cargo package` and both `npm pack` tarballs — and
attaches them as workflow artifacts without publishing anything.

### Changelogs

Each surface keeps its own generated changelog:
`crates/tellegen/CHANGELOG.md`, `packages/engine/CHANGELOG.md`, and
`packages/svelte/CHANGELOG.md`. The root `CHANGELOG.md` is the curated view
across all three, and the only record for releases before the split.

## CI Gates

`.github/workflows/gates.yml` is the single definition of what green means. CI
calls it on every pull request, and both release workflows call it before they
publish — so the release path cannot quietly run a shorter list than the pull
request path did.

The Rust gates are `cargo fmt --check`, clippy with warnings denied on the
shipping crates, `cargo-deny`, the EPL guard (the EPL-2.0 `pounce` backend must
never enter a shipped wasm, server, or CLI build), the workspace tests, the
engine's `conic` path, and `tellegen-wasm` built with `conic` — the
configuration that actually ships, and the one that carries the untrusted-input
rejection tests, which `cargo test --workspace` skips because the adapter
declares `default = []`.

The JavaScript gates install once from the root lockfile, build
`packages/engine` before `packages/svelte`, build the hosted demo, install the
packed Svelte tarball into a temporary downstream consumer, and run a browser
test against the demo shell. The root `ci:js` script is the local equivalent;
`just ci` runs everything.
