# Contributing

Thanks for helping. This page is the short version of what the repository
enforces; the [release architecture](docs/src/release-architecture.md) page
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

## Changesets

A pull request that changes `packages/engine`, `packages/svelte`, or
`packages/webmcp` needs a changeset, or the change ships in no package release:

```sh
npm run changeset
```

Commit the file it writes. Use a `minor` changeset for a new or incompatible
public API while a package is on `0.x`, and `patch` for compatible fixes. A
pull request that changes only `apps/web`, `examples/`, or the Rust crates
needs none: the `tellegen` crate's changelog is written by release-plz from
commit messages, so write those for a reader of the changelog.

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
