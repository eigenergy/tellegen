# Tellegen

Tellegen is a framework for power-system applications. It provides a Rust
numerical engine, browser packages, and Svelte components. The hosted
[demo](https://tellegen.dev) is one application built with those packages.

Use the demo to open a case, choose a calculation, inspect results, and change
loads or equipment settings. Supported calculations depend on the imported
model. A parsed file is not necessarily ready to solve; the app explains
unsupported models and missing data.

## Start here

- [Run the demo locally](introduction.md#getting-started).
- [Build an application](framework-quickstart.md) with the viewer or engine.
- [Load your files](local-case-files.md), including [ZIPs and folders](local-case-files.md#collection-import).
- [Choose a calculation](formulations.md) and read its assumptions.
- [Connect an agent](webmcp.md) to the browser, or use the [Python MCP server](python.md#mcp).

Browser calculations run in WebAssembly. Local case files stay in the browser
unless the host application explicitly sends them elsewhere. The demo server
provides bundled cases; applications using local files do not need it.

Sensitivity previews estimate nearby changes. Committing an edit runs the
calculation again. A preview is not a replacement for checking the new solution.

For package APIs, see the [engine reference](engine-api.md),
[Svelte components](framework-quickstart.md#frontend-package), [Python](python.md), and
[HTTP server](engine-api.md#http-api). The [architecture](architecture.md) explains which
parts belong to the framework and which belong to the demo.


<a id="getting-started"></a>

<a id="getting-started-run-the-demo-locally"></a>

## Run the demo locally

<a id="getting-started-requirements"></a>

### Requirements

Install Rust using `rust-toolchain.toml`, Node.js 24, `just`, `wasm-pack`,
and Python 3.11 or newer. The build needs the Rust targets
`wasm32-unknown-unknown` and `wasm32-wasip1`.

<a id="getting-started-build-and-run"></a>

### Build and run

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
[data sources](local-case-files.md#data-sources) for their origins and limitations.

<a id="getting-started-repository-folders"></a>

### Repository folders

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

<a id="getting-started-check-changes"></a>

### Check changes

Run `just ci` for the repository checks. See
[contributing](https://github.com/eigenergy/tellegen/blob/main/CONTRIBUTING.md)
for the PR rules and [releases](deployment.md#release-architecture) for publishing.

To build the book, install mdBook 0.5.3 and run:

```sh
scripts/build-docs.sh
```

For a new application rather than this demo, use the
[package quickstart](framework-quickstart.md).
