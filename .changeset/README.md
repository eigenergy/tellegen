# Changesets

Pending release notes for `@tellegen/engine`, `@tellegen/svelte`, and
`@tellegen/webmcp`. The `tellegen` crate releases separately through
release-plz (`release-plz.toml`).

## When you need one

A pull request that changes `packages/engine`, `packages/svelte`, or
`packages/webmcp` needs a changeset, or the change ships in no package release.
A pull request that changes only `apps/web`, `examples/`, or the crate does not
need one.

Use a minor changeset for a new or incompatible public API while a package is
on a `0.x` version. Use a patch changeset for compatible fixes.

Run `npm run changeset` and commit the file it writes.

## Running the version step by hand

`npm run version:packages` needs a `GITHUB_TOKEN` in the environment. The
changelog generator calls the GitHub API to find the pull request for each
changeset. Without a token it fails with `Bad credentials` and changes no
files. Prefer an existing GitHub CLI login:

```sh
export GITHUB_TOKEN="$(gh auth token)"
```

Otherwise use a short-lived fine-grained token limited to this repository with
read-only pull-request access; repository metadata read access is implicit.
