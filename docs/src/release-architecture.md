# Release Architecture

The public browser surfaces are `@tellegen/engine`, `@tellegen/svelte`,
`@tellegen/webmcp` and `@tellegen/analytics`.

Stable release surfaces:

- `crates/tellegen` Rust API layer, including the serde request and response
  shapes in `src/api.rs`;
- `crates/tellegen-wasm` wasm adapter;
- `packages/engine` TypeScript package, generated TypeScript types, and browser
  wasm entry points;
- `packages/svelte` Svelte component package;
- `packages/webmcp` WebMCP tool definitions and runtime validation without a UI
  framework dependency;
- `packages/analytics` allowlisted usage analytics with a first-party Umami
  Cloud sender, no dependencies;
- examples under `examples/`.

The hosted demo under `apps/web` is one consumer of the packages. It keeps
routes, SEO, credits, privacy, and deployment specific behavior.

## JavaScript Workspace

The repository uses npm workspaces with one root `package-lock.json` for:

- `packages/engine`;
- `packages/webmcp`;
- `packages/analytics`;
- `packages/svelte`;
- `apps/web`;
- `examples/browser-minimal`; and
- `examples/svelte-minimal`.

Install JavaScript dependencies from the repository root:

```sh
npm ci
```

Root scripts define the package order:

- `npm run wasm` builds the one wasm module (features `conic` and `synth`) into
  `packages/engine`;
- `npm run build:engine` builds `@tellegen/engine`;
- `npm run build:webmcp` builds `@tellegen/webmcp`;
- `npm run build:analytics` builds `@tellegen/analytics`;
- `npm run build:svelte` builds `@tellegen/svelte`;
- `npm run build:example` builds both downstream examples;
- `npm run check:web`, `npm run build:web`, and `npm run smoke:web` gate the
  hosted demo;
- `npm run pack:engine` previews the engine npm package contents;
- `npm run pack:webmcp` previews the WebMCP npm package contents;
- `npm run pack:analytics` previews the analytics npm package contents;
- `npm run pack:svelte` previews the Svelte npm package contents.

## Versioning

`@tellegen/engine`, `@tellegen/svelte`, `@tellegen/webmcp` and
`@tellegen/analytics` use independent package versions.

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

### Saved cases

Tellegen saves the materialized network as a stored PowerIO module. The module
keeps applicable records, severs source linkage invalidated by edits, and adds
descriptive edit history. Loading it starts a fresh runtime `Study` at the
saved point. Its history records transformations applied to the stored value.

Exact OPF results are stored as PowerIO solution modules containing the
instance that was solved.

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
3. an npm trusted publisher for `@tellegen/engine`, `@tellegen/svelte`,
   `@tellegen/webmcp` and `@tellegen/analytics`, each set to this repository,
   `release-npm.yml`, **and the `npm` environment**. The publisher binds to the workflow filename. If you
   rename the workflow, publishing stops until you change the publisher;
4. a GitHub App on this repository with `contents: write` and
   `pull-requests: write`. Put its id in the `RELEASE_PLZ_APP_ID` variable and
   its private key in the `release-bot` environment's
   `RELEASE_PLZ_APP_PRIVATE_KEY` secret. Restrict that environment to `main`.
   Both release
   bots use it so their version pull requests start CI. Pull requests opened by
   `GITHUB_TOKEN` do not start workflows.

Keep deployment credentials in the main-only `demo` environment. No deployment
or Release App credentials belong at repository scope; only the jobs bound to
their environments receive them. The temporary migration workflow is removed
after its environment-only aliases prove that all copied credentials match.

The current `main` ruleset requires pull requests, merge commits and CI checks;
it blocks force pushes and deletion. It does not require the branch to be
up to date before merging. It requires zero
approvals and does not require approval of the last push, so a sole maintainer
can release after reviewing the final changes and checks. The version
pull-request merge is the publication gate. If independent approval becomes
required, set at least one approval and dismiss stale approvals or require
approval of the last reviewable push. Do not give the Release App a ruleset bypass.

For npm and crates.io, bootstrap a missing package before configuring its
trusted publisher. PyPI supports a pending publisher that creates the project
on the first OIDC upload; its setup is described below.

Check all four npm publishers with `npm trust list @tellegen/<name>`. A package
previously published by hand still needs its own publisher; configuring the
engine does not configure WebMCP. After `npm login`, the command is
`npm trust github @tellegen/<name> --file release-npm.yml --repo eigenergy/tellegen --env npm --allow-publish`.
It requires npm's interactive two-factor confirmation. A partial publish can
be retried: already published versions are retained, and remaining packages
publish through the same workflow.

### Separate AC OPF download

Each stable crate release also builds the isolated POUNCE WASI reactor from
that exact tag and attaches it to the existing GitHub release. The download
contains `tellegen_acopf_wasi.wasm`, `POUNCE-EPL-2.0.txt`, `POUNCE-NOTICE.txt`,
`POUNCE-SOURCE.json` and `ACOPF-SHA256SUMS.txt`. It remains separate from the MIT
engine packages. The manifest records the reactor ABI, engine version, full
source revision, source archive and POUNCE crate checksums. This preserves the
license and corresponding-source information distributed with the website.

Consumers pin a release URL and reactor hash, rather than the changing demo
URL. The publisher never replaces existing release assets: a retry verifies
identical bytes and uploads only missing files. A source or toolchain change
needs a new release. To backfill an existing stable tag, dispatch
`release-acopf.yml` on `main` with `tag=vX.Y.Z`; it accepts only an existing
release whose tagged commit is part of `main`.

Delete any `NPM_TOKEN` or `CARGO_REGISTRY_TOKEN` repository secrets. The
workflows do not read long-lived registry credentials.

### Packages

`@tellegen/engine`, `@tellegen/svelte`, `@tellegen/webmcp` and
`@tellegen/analytics` release through changesets.

A change to any of these packages needs a changeset. Run `npm run changeset`
and commit the file it writes. See `.changeset/README.md`.

On a push to `main`, `.github/workflows/release-npm.yml` keeps a "Release
packages" pull request open. That pull request applies every pending changeset:
version bumps, changelog entries, the engine `ENGINE_VERSION`, and the
lockfile. Merge it to run the gates and publish. Tags take the form
`@tellegen/<name>@X.Y.Z`.

The workflow selects version or publish mode before it requests privileged
permissions. Only publish mode runs the Rust and JavaScript gates; version mode
refreshes the pull request, whose CI runs them. Versioning runs without the GitHub App, validates an exact output
allowlist, and passes a SHA-256-verified patch to a fresh runner. That runner
revalidates and commits the patch before minting the App token; only `git` and
`gh` run afterward. Publishing builds immutable tarballs in an unprivileged
job, then gives only the final npm job the `npm` environment and OIDC
permission. Merging the version pull request is therefore the only manual
release action.

`@tellegen/svelte` resolves `@tellegen/engine` from the registry, so a release
that moves both publishes the engine first.

### Crate

`tellegen` is the only crate that publishes to crates.io. `tellegen-wasm`,
`tellegen-server`, `tellegen-cli`, `tellegen-py`, and `benchmarks` carry
`publish = false`, and the release-plz workspace defaults to `release = false`.
A new crate needs an explicit package opt-in.

On a push to `main`, `.github/workflows/release-crate.yml` keeps a pull request
open that bumps the version. Merge it to run the gates, continue the existing
`vX.Y.Z` tag series, make the GitHub release, and publish. The configuration
sets `release_always = false`, limiting publication to a merged release-plz pull
request. The unprivileged gate runs `cargo package --locked --features sensitivity,mc-pf,conic,acopf,mc-opf,schema`, so every
shipping numerical feature builds against crates.io; the OIDC-enabled
release skips build verification because those unprivileged gates have already
verified the package against that exact commit. release-plz obtains its
short-lived crates.io credential directly from OIDC.

The explicit feature list excludes `powerio-012`, which requires selecting
matching 0.12 dependencies and belongs to the separate PowerIO Candidate gate.
The release API comparison also uses `--only-explicit-features` with that list:
`cargo-semver-checks` otherwise enables non-default features heuristically.
`release-plz` itself has `semver_check = false` because this read-only gate owns
API comparison. Ordinary Cargo publication and docs builds use default features.

The workflow's first job decides whether the push can publish: its engine
version has no `v<version>` tag yet, and the push writes
`crates/tellegen/CHANGELOG.md`, which only the release pull request does. Only
then do the Rust gates and the publish job run. Any other push, including every
push between a version raised by hand and its release, only refreshes the
version pull request, which runs CI itself. A manual run, or a tag list or diff
that cannot be read, plans a publish and leaves the decision to release-plz.
While a release publishes, the version update waits for the gates, as it always
has.

The crate update uses the same sealed-patch privilege boundary as package
versioning. After `release-plz update`, an unprivileged step
(`.github/scripts/sync_python_version.py`) sets the Python wheel's package
version and its `Cargo.lock` entry to the engine's, without resolving or
updating any dependency, and `cargo metadata --locked` checks the result. The
patch then holds one of three path sets: the engine changelog alone (a version
already raised by hand on `main`); the engine manifest, its changelog, and the
lockfile; or those three and `crates/tellegen-py/Cargo.toml`, in which only the
package version may differ from the checkout. In every case the validator
requires both manifests and both lock entries to carry the engine's version,
before the patch is sealed and again after it is applied, before the App token
is minted. A wheel already on the new version needs no edit, and an update with
no release stays a no-op.

After a crate release succeeds, `release-crate.yml` dispatches
`release-python.yml` on main with the published tag. It resolves the tag to an
immutable commit, confirms it belongs to main and has a published GitHub
release, and runs the Python gates against that source. The gates check the
installed extension's type stubs, test Python 3.11 and 3.13, build five platform
wheels, and build and solve a case from the source archive. Test fixtures are
excluded from the source archive; the package includes its MIT licence and
all solver build inputs.

Only the final, checkout-free job has `id-token: write`. Its `pypi` environment
is restricted to main, and its trusted publisher is `eigenergy/tellegen`,
workflow `release-python.yml`, environment `pypi`. PyPI does not support a
reusable publishing workflow, so the crate workflow dispatches this standalone
workflow instead of calling it. A pending publisher configured at
<https://pypi.org/manage/account/publishing/> creates the project on its first
OIDC upload; no bootstrap API token or manual first upload is necessary.

To retry an interrupted publication, dispatch `release-python.yml` on main
with the same stable tag. Existing distributions are skipped; published
versions and their source commits are never replaced. Add the public install
command to the README and Python guide only after the first upload is verified.

Its fixed `release-plz-main` branch is intentional: with
`release_always = false`, release-plz recognizes a release commit by the
`release-plz-` head prefix. Merge this generated pull request with a normal
merge commit, not squash, so its release commit remains unambiguous if another
change reaches `main` nearby.

An emergency repair to a generated crate release must retain its
`release-plz-*` branch name; that is how `release_always = false` recognizes the
merged release commit.

### Inspecting an artifact

`.github/workflows/package-inspect.yml` is a manual run. It builds the artifacts
a release would upload, `cargo package` and all four `npm pack` tarballs, and
attaches them to the run. It publishes nothing.

### Changelogs

Each surface keeps its own generated changelog:
`crates/tellegen/CHANGELOG.md`, `packages/engine/CHANGELOG.md`,
`packages/svelte/CHANGELOG.md`, `packages/webmcp/CHANGELOG.md` and
`packages/analytics/CHANGELOG.md`. The root `CHANGELOG.md` is the archive of
the releases before the split. Nobody edits a changelog by hand, including on a
release pull request.

The crate changelog lists the pull requests merged since the previous release,
one entry each, under its title. release-plz writes a section for the new
version from commit subjects, but on this repository most of those read "Merge
pull request #N", and it walks the history along one parent at a time, which
can miss a pull request. So the crate release workflow then runs
`.github/scripts/write-crate-changelog.py`, which replaces that section's body.
It reads the first-parent merge commits since the last `v*` tag, keeps those
that changed `crates/tellegen/` or a root `Cargo.toml` entry the crate builds
with, and skips Dependabot, release-plz and changesets merges and version-only
bumps. A title starting `feat:` files the entry under Added, `fix:` under Fixed,
and `!:` under Breaking; anything else goes under Changed. Write pull request
titles for a reader of the changelog. The section itself must stay, because
`release-plz release` takes the GitHub release body from it, so the
`commit_parsers` in `release-plz.toml` keep merge commits.

## CI Gates

`.github/workflows/gates-rust.yml` and `.github/workflows/gates-js.yml` define
what green means. CI calls both on every pull request and every push to `main`.
The crate release calls the Rust gates, and the npm release calls both before
it publishes. Add a gate to those files, not to a caller.

CI runs only the gates a change can affect. Its scope job runs
`.github/scripts/ci-scope.py` on the changed paths: a pull request's against
the branch it merges into, a push's against the last `main` commit whose CI
passed. The Rust and JavaScript gates and the image build run unless every
changed path is one the script names as unable to affect them; a path it does
not name runs them. A version bump that only moves `tellegen` and `tellegen-py`
in their manifests and `Cargo.lock` runs only the Rust gates. A missing base, a
failed or empty diff, or a change to the scoping itself runs every gate. A gate
that does not apply still reports its required check, as skipped. The release
workflows call the gates without this scope, so a release always runs them in
full. `.github/scripts/test-ci-scope.py` pins the rules, and checks them
against the files the gates and the Dockerfile read. Change both when a gate
starts reading a new path.

The Rust gates are `cargo fmt --check`, clippy with warnings denied on the
shipping crates, `cargo-deny`, the EPL guard, the workspace tests, the engine's
`conic` path, `tellegen-wasm` built with `conic` and `synth`, and
`cargo package --locked --features sensitivity,mc-pf,conic,acopf,mc-opf,schema` for the published crate. The wasm test matters
because `tellegen-wasm` declares `default = []`, so the workspace run skips the
tests that assert an untrusted package or case rejects rather than panicking.

The JavaScript gates install once from the root lockfile, check and pack
`packages/webmcp` and `packages/analytics`, build and check `packages/engine`
before `packages/svelte`,
run the engine import smoke test, check the Svelte package through a packed
temporary consumer, build the hosted demo, and run its browser tests. `just ci`
runs everything locally.
