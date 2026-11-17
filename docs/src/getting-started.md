# Run the demo locally

## Requirements

Install Rust using `rust-toolchain.toml`, Node.js 24, `just`, `wasm-pack`,
and Python 3.11 or newer. The build needs the Rust targets
`wasm32-unknown-unknown` and `wasm32-wasip1`.

## Build and run

From the repository root:

```sh
just dev-setup
just dev
```

`dev-setup` installs JavaScript dependencies and builds the browser engine,
AC OPF module, and packages. Run it after pulling changes that affect those
parts. `dev` starts the case server on port 8000 and the app at
<http://localhost:5173>. Ctrl-C stops the server it started.

Without staged data the server uses small example cases. To use the larger
demo cases, download their distributions and run:

```sh
scripts/stage-data.sh ~/Datasets
```

The source distributions are not included in this repository. See
[data sources](data-sources.md) for their origins and limitations.

## Repository folders

| Folder | Purpose |
| --- | --- |
| `crates/` | Rust engine, adapters and numerical benchmarks |
| `packages/` | Published browser engine, Svelte components, agent tools and analytics |
| `apps/web/` | Hosted demo using those packages |
| `examples/` | Small applications showing how to use the packages |
| `scripts/` | Build, release and data preparation commands |
| `validation/` | Numerical comparisons, measurements and their inputs |
| `.changeset/` | Pending release notes for JavaScript packages |
| `deploy/` | Server deployment files |
| `docs/` | This book and its assets |

`examples/` contains applications; `scripts/` contains repository commands.
Keep them separate so examples remain usable by package consumers.

## Check changes

Run `just ci` for the repository checks. See
[contributing](https://github.com/eigenergy/tellegen/blob/main/CONTRIBUTING.md)
for the PR rules and [releases](release-architecture.md) for publishing.

To build the book, install mdBook 0.5.3 and run:

```sh
scripts/build-docs.sh
```

For a new application rather than this demo, use the
[package quickstart](framework-quickstart.md).
