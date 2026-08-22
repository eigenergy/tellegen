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

A saved study is a powerio `.pio.json` document, and it states the powerio
release that wrote it. powerio reads the lineage it belongs to: a 0.9 build
reads a 0.9 document. When powerio leaves a lineage behind, every study saved
under it stops loading.

tellegen cannot migrate such a file. The document is a snapshot of a case and an
edit log, so the user must save it again from the source case. All three call
sites that load a package pass powerio's rejection through unchanged; it names
the release that wrote the document and the lineage this build reads.

The tellegen block in the document (`study.app["tellegen"]`) has its own
`schema_version` and is checked separately.

## Releasing

Package versions and the crate version move independently, so two release bots
drive the pipeline. A release that touches one surface publishes that surface
only.

Nobody cuts a tag by hand. This repository stores no registry token. Both
registries authenticate with OIDC, and each exchange runs in a deployment
environment (`crates-io`, `npm`). Reviewing and merging the generated version
pull request is the final human publication gate. Publication after that merge
is unattended.

Each environment allows deployments from `main` only and has no required
reviewer. Set the same environment on the registry's trusted publisher, along
with the repository and workflow filename. The workflow also rejects non-main
refs; the publisher binding makes the registry enforce that identity.

### Enabling the pipeline

The release workflows do nothing until the `TELLEGEN_RELEASE_ENABLED` repository
variable is `true`. What they need lives outside the repository. Set the
variable after you make all of these:

1. a `crates-io` environment and an `npm` environment, each with a deployment
   branch rule limited to `main` and no required reviewers;
2. a crates.io trusted publisher for `tellegen`, set to this repository,
   `release-crate.yml`, **and the `crates-io` environment**;
3. an npm trusted publisher for `@tellegen/engine` and one for
   `@tellegen/svelte`, each set to this repository, `release-npm.yml`, **and
   the `npm` environment**. The publisher binds to the workflow filename. If
   you rename the workflow, publishing stops until you change the publisher;
4. a GitHub App on this repository with `contents: write` and
   `pull-requests: write`. Put its id in the `RELEASE_PLZ_APP_ID` variable and
   its private key in the `RELEASE_PLZ_APP_PRIVATE_KEY` secret. Both release
   bots use it so their version pull requests start CI. Pull requests opened by
   `GITHUB_TOKEN` do not start workflows.

Protect `main` with a repository ruleset that requires pull requests, at least
one approval, and the CI checks before merge, and blocks force pushes and
deletion. Do not give the Release App a ruleset bypass. Those protections make
the version pull-request merge the publication gate.

If a package or crate does not yet exist at its registry, publish its first
version manually before configuring the trusted publisher. Later releases use
this pipeline end to end.

Delete any `NPM_TOKEN` or `CARGO_REGISTRY_TOKEN` repository secrets. The
workflows do not read long-lived registry credentials.

### Packages

`@tellegen/engine` and `@tellegen/svelte` release through changesets.

A change to either package needs a changeset. Run `npm run changeset` and commit
the file it writes. See `.changeset/README.md`.

On a push to `main`, `.github/workflows/release-npm.yml` keeps a "Release
packages" pull request open. That pull request applies every pending changeset:
version bumps, changelog entries, the engine `CONTRACT_VERSION`, and the
lockfile. Merge it to run the gates and publish. Tags take the form
`@tellegen/<name>@X.Y.Z`.

The workflow selects version or publish mode before it requests privileged
permissions. Versioning uses the GitHub App. Publishing builds immutable
tarballs in an unprivileged job, then gives only the final npm job the `npm`
environment and OIDC permission. Merging the version pull request is therefore
the only manual release action.

`@tellegen/svelte` resolves `@tellegen/engine` from the registry, so a release
that moves both publishes the engine first.

### Crate

`tellegen` is the only crate that publishes to crates.io. `tellegen-wasm`,
`tellegen-server`, `tellegen-cli`, and `benchmarks` carry `publish = false`, and
the release-plz workspace defaults to `release = false`. A new crate needs an
explicit package opt-in.

On a push to `main`, `.github/workflows/release-crate.yml` keeps a pull request
open that bumps the version. Merge it to run the gates, continue the existing
`vX.Y.Z` tag series, make the GitHub release, and publish. The configuration
sets `release_always = false`, limiting publication to a merged release-plz pull
request. The unprivileged gate runs `cargo package --locked`; the OIDC-enabled
release then uses Cargo's `--no-verify` path so package build scripts never run
with registry authority. release-plz obtains its short-lived crates.io
credential directly from OIDC.

An emergency repair to a generated crate release must retain its
`release-plz-*` branch name; that is how `release_always = false` recognizes the
merged release commit.

### Inspecting an artifact

`.github/workflows/package-inspect.yml` is a manual run. It builds the artifacts
a release would upload, `cargo package` and both `npm pack` tarballs, and
attaches them to the run. It publishes nothing.

### Changelogs

Each surface keeps its own generated changelog:
`crates/tellegen/CHANGELOG.md`, `packages/engine/CHANGELOG.md`, and
`packages/svelte/CHANGELOG.md`. The root `CHANGELOG.md` is the curated view
across all three, and the only record for releases before the split.

## CI Gates

`.github/workflows/gates-rust.yml` and `.github/workflows/gates-js.yml` define
what green means. CI calls both on every pull request. The crate release calls
the Rust gates, and the npm release calls both. Add a gate to those files, not
to a caller.

The Rust gates are `cargo fmt --check`, clippy with warnings denied on the
shipping crates, `cargo-deny`, the EPL guard, the workspace tests, the engine's
`conic` path, `tellegen-wasm` built with `conic`, and `cargo package --locked`
for the published crate. The wasm test matters
because `tellegen-wasm` declares `default = []`, so the workspace run skips the
tests that assert an untrusted package or case rejects rather than panicking.

The JavaScript gates install once from the root lockfile, build
`packages/engine` before `packages/svelte`, build the hosted demo, install the
packed Svelte tarball into a temporary consumer, and run a browser test against
the demo shell. `just ci` runs everything locally.
