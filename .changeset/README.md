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

## Prereleases

While `.changeset/pre.json` exists, the repository is in Changesets pre mode.
The 0.4.0 cycle entered pre mode with the `next` tag so that MapToGrid could
build against `@tellegen/engine` and `@tellegen/svelte` from npm before the
stable release, and left it before any prerelease was published.

Changesets work as usual in pre mode. The version pull request differs in three
ways:

- A bumped package gets a prerelease version. A minor changeset on 0.3.0 gives
  `0.4.0-next.0`, and the next version pull request gives `0.4.0-next.1`.
- Each consumed changeset moves into `.changeset/pre/` instead of being
  deleted, so the stable release can still read it. `pre.json` stays as it is.
- Merging it publishes under the `next` dist-tag, and `latest` stays on the
  last stable version. The publish plan takes the dist-tag from `pre.json`. It
  switches a package to `latest` only when every version the package has
  published is a `next` prerelease. All three packages have stable releases,
  so this does not apply to them.

To enter pre mode, run the command below and commit the `.changeset/pre.json`
it writes:

```sh
npx changeset pre enter next
```

### Exiting before the stable release

Exit pre mode before 0.4.0 is released:

```sh
npx changeset pre exit
```

This sets `"mode": "exit"` in `.changeset/pre.json`; merge that change in its
own pull request. The next version pull request does three things:

- It versions the packages as stable (0.4.0).
- It writes their changelog from every changeset in `.changeset/pre/`.
- It deletes those files and `pre.json`.

Merging it publishes to `latest`.
