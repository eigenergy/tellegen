# Getting Started

## Repository Layout

- `apps/web/`: private SvelteKit hosted demo
- `crates/`: the tellegen engine and its wasm, server, CLI, and benchmark adapters
- `packages/engine/`: public `@tellegen/engine` browser package
- `packages/svelte/`: public `@tellegen/svelte` component package
- `examples/browser-minimal/`: minimal downstream Vite example
- `examples/svelte-minimal/`: minimal Svelte example using the component package
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
npm ci
npm run wasm
npm run build:engine
npm run build:svelte
```

## tellegen frontend demo

```sh
npm ci
npm run wasm
npm run build:engine
npm --workspace tellegen-frontend run dev
```

The Vite dev server proxies `/api` to `http://localhost:8000`.

## Framework Package

Preview the package contents before publishing:

```sh
npm run pack:engine
npm run pack:svelte
```

Use `@tellegen/svelte` for the map, panels, local file flow, and solve card. Use
`@tellegen/engine` for apps that want case parsing and browser solves without
the tellegen UI. The hosted app is a private demo workspace that consumes
`@tellegen/svelte` like another Svelte app would.

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
