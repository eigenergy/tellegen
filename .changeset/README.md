# Changesets

Pending release notes for `@tellegen/engine` and `@tellegen/svelte`. The
`tellegen` crate is not here. It releases through release-plz
(`release-plz.toml`), because the two surfaces version independently.

## When you need one

A pull request that changes `packages/engine` or `packages/svelte` needs a
changeset, or the change ships in no release. A pull request that changes only
`apps/web`, `examples/`, or the crate does not need one.

Run `npm run changeset` and commit the file it writes.

## Running the version step by hand

`npm run version:packages` needs a `GITHUB_TOKEN` in the environment. The
changelog generator calls the GitHub API to find the pull request for each
changeset. Without a token it fails with `Bad credentials` and changes no
files. Export a personal token with `public_repo` scope first.
