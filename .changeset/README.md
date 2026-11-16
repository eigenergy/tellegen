# Changesets

This folder holds the pending release notes for the npm packages
(`@tellegen/engine`, `@tellegen/svelte`, `@tellegen/webmcp`,
`@tellegen/analytics`). The release tool,
[changesets](https://github.com/changesets/changesets), reads it and requires
it to be here. The `tellegen` crate releases separately through release-plz
(`release-plz.toml`).

## When you need one

A pull request that changes `packages/engine`, `packages/svelte`,
`packages/webmcp` or `packages/analytics` needs a changeset, or the change
ships in no package release. A pull request that changes only `apps/web`,
`examples/` or the crates does not need one.

Run `npm run changeset` and commit the file it writes.

## Which bump

Write `patch`. That covers fixes and additions: a new export, prop or option
that leaves existing code working is a patch.

A `minor` bump is for a change that breaks existing code, and a maintainer
decides when one ships. CI fails a pull request whose changeset says `minor` or
`major` until a maintainer adds the `release:minor` label.

## Running the version step by hand

`npm run version:packages` needs a `GITHUB_TOKEN` in the environment. The
changelog generator calls the GitHub API to find the pull request for each
changeset. Without a token it fails with `Bad credentials` and changes no
files. Prefer an existing GitHub CLI login:

```sh
export GITHUB_TOKEN="$(gh auth token)"
```

Otherwise use a short-lived fine-grained token limited to this repository with
read-only pull-request access.
