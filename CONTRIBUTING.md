# Contributing

Thanks for helping. This page is the short version of what the repository
enforces; the [release architecture](docs/src/deployment.md#release-architecture) page
explains the machinery behind it.

## Before opening a pull request

Run the gates locally:

```sh
just ci
```

That recipe runs, in order, everything `gates-rust.yml` and `gates-js.yml`
enforce: formatting, clippy with warnings denied, cargo-deny, the EPL guard,
the workspace tests and each feature-specific test run, the wasm adapter, crate
packaging, then the JavaScript checks, package smoke tests, the web build, the
browser tests, and `npm audit`. Individual recipes are listed by `just`.

The Python binding has its own gate (`gates-python.yml`); see
[docs/src/python.md](docs/src/python.md) for how to build and test the wheel.

CI runs a gate only when the change can affect it
(`.github/scripts/ci-scope.py`), on pull requests and pushes to `main` alike.
The Python gate runs for a change to the engine, the binding, the workspace
manifests, a Python file, or the gate's configuration, and builds the macOS
wheels only when the change reaches the wheel itself. The Rust and JavaScript
gates and the image build skip only changes known not to reach them, such as
documentation for all three, or the web app for the Rust gates. The release
workflows always run every gate.

## Rules CI checks on every pull request

`.github/scripts/pr-rules.py` fails a pull request that breaks one of these.
A maintainer can lift a rule with a label.

- **Releases are patches.** A changeset declares `patch`, and no commit
  subject, commit body or pull request title marks a breaking change (`feat!:`,
  `BREAKING CHANGE:`). Additions are patches too: a new export, option or
  field that leaves existing code working. A change that breaks existing code
  needs the `release:minor` label, so talk to a maintainer first.
- **At most 1,500 added lines**, not counting lockfiles, changelogs, generated
  files and test data. Split a larger change into pull requests that each do
  one thing. The `large-pr` label lifts this.
- **An owner reviews it** (`.github/CODEOWNERS`).

## What reviewers look for

- **Reuse before adding.** Look for an existing helper, component or solver
  before writing one. Number and unit formatting is in
  `packages/svelte/src/lib/format.ts`, colour scales in `colors.ts`, time
  series charts in `components/dynamics/TraceChart.svelte`, the list of
  calculations in `@tellegen/engine`. In the engine, case parsing, admittance
  matrices and branch flows come from PowerIO, and sensitivities go through
  `crates/tellegen/src/sens`.
- **The library does not depend on the demo.** `packages/*` is what other
  people build apps with; `apps/web` is one app built with it. Anything a
  second app would need belongs in a package, and anything specific to
  tellegen.dev belongs in `apps/web`.
- **Experimental features stay off by default** and behind their feature flag
  or setting until a maintainer turns them on.
- **Docs describe the code as it is.** Update the page under `docs/src/` in the
  same pull request, in plain words, without version numbers typed by hand.

## Changesets

A pull request that changes `packages/engine`, `packages/svelte`,
`packages/webmcp` or `packages/analytics` needs a changeset, or the change
ships in no package release:

```sh
npm run changeset
```

Commit the file it writes, with `patch` as the bump. A pull request that
changes only `apps/web`, `examples/`, or the Rust crates needs none: the
`tellegen` crate's changelog lists each merged pull request under its title, so
write the title for a reader of the changelog. Start it with `feat:` for
something new or `fix:` for a fix to file it under Added or Fixed; anything
else goes under Changed.

## Merging

Every pull request is merged with a merge commit. Do not squash and do not
rebase-merge: the generated release pull requests are recognised by their
release commit, and contributors' commits and trailers stay as they wrote them.

## Releases

Nobody tags by hand. Two bots open version pull requests on every push to
`main`: release-plz for the `tellegen` crate (`release-plz-main`) and
changesets for the npm packages (`changeset-release/main`). Reviewing and
merging those pull requests is the publication step; publication to crates.io
and npm then runs unattended through OIDC trusted publishing.

## Reporting problems

Use the issue templates. Security reports go through GitHub's private
vulnerability reporting, described in [SECURITY.md](SECURITY.md).
