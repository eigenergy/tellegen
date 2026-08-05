# Changesets

This directory holds the pending release notes for `@tellegen/engine` and
`@tellegen/svelte`. The `tellegen` crate is not here — it releases through
release-plz (`release-plz.toml`), because the two surfaces version
independently.

## Adding one

Run `npm run changeset` and answer the prompts: which packages changed, whether
the change is major/minor/patch, and a one-line summary. That writes a small
markdown file here; commit it with the change it describes.

A pull request that touches `packages/engine` or `packages/svelte` without a
changeset ships nothing, because nothing tells the release which version to
cut. A pull request that touches only `apps/web`, `examples/`, or the crate
does not need one.

## What happens next

On merge to `main`, `release-npm.yml` keeps a "Version Packages" pull request
open that consumes every pending changeset: it bumps the versions, folds the
summaries into each package's `CHANGELOG.md`, regenerates the engine's
`CONTRACT_VERSION`, and refreshes the lockfile. Merging *that* pull request runs
the gates and publishes.

`@tellegen/svelte` depends on `@tellegen/engine` by registry range, so a release
that moves both publishes the engine first — that ordering is why publishing
goes through changesets rather than two independent tag triggers.
