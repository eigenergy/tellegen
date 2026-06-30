# Getting Started

## Repository Layout

- `apps/web/`: `tellegen-frontend` package and SvelteKit demo
- `crates/`: the tellegen engine and its wasm, server, CLI, and benchmark adapters
- `scripts/`: data staging and docs build helpers
- `deploy/`: deployment compose files and proxy notes
- `docs/src/`: mdBook documentation source

## Prerequisites

- Rust from `rust-toolchain.toml`, including `rustfmt`, `clippy`, and the
  `wasm32-unknown-unknown` target
- Node.js 22 or newer
- `wasm-pack` 0.15.x for browser WebAssembly builds
- mdBook 0.5.x for local documentation builds

## tellegen backend

```sh
cargo run -p tellegen-server
```

Set `TELLEGEN_ALLOW_FALLBACK=1` to run without staged demo data:

```sh
TELLEGEN_ALLOW_FALLBACK=1 cargo run -p tellegen-server
```

## WebAssembly Module

```sh
cd apps/web
npm run wasm
```

## tellegen frontend demo

```sh
cd apps/web
npm install
npm run dev
```

The Vite dev server proxies `/api` to `http://localhost:8000`.

## Frontend Package

`apps/web` also builds the `tellegen-frontend` package:

```sh
cd apps/web
npm run package
```

`npm run build` packages `src/lib` first, then builds the demo. Another Svelte
app can consume the package through the export map documented in
[Frontend Package](frontend-package.md).

## Data

The ACTIVSg and CATS distributions are downloaded by the operator and are not
vendored. With the distributions under `~/Datasets`:

```sh
scripts/stage-data.sh ~/Datasets
```

The script stages any complete case pairs it finds into `data/`. The backend
serves the staged subset; if nothing is staged, it exits unless
`TELLEGEN_ALLOW_FALLBACK=1` is set. That fallback serves two pglib cases with
synthetic coordinates for CI and local smoke checks.

## Docs

Install mdBook, then build the public docs:

```sh
scripts/build-docs.sh
```

CI pins mdBook to `v0.5.3`. For local work, any recent mdBook `0.5.x` release
should render the book.
