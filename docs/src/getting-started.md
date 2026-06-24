# Getting Started

## Repository Layout

- `apps/web/`: tellegen web app (SvelteKit)
- `crates/`: the tellegen engine and its wasm, server, CLI, and benchmark adapters
- `scripts/`: data staging and docs build helpers
- `deploy/`: deployment compose files and proxy notes
- `docs/src/`: mdBook documentation source

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

## tellegen frontend

```sh
cd apps/web
npm install
npm run dev
```

The Vite dev server proxies `/api` to `http://localhost:8000`.

## Data

The ACTIVSg and CATS distributions are downloaded by the operator and are not
vendored. With the distributions under `~/Datasets`:

```sh
scripts/stage-data.sh ~/Datasets
```

The script stages the eight files used by the demo into `data/`. Without all
four staged cases, the tellegen backend exits. For CI or local smoke checks
without the staged distributions, set `TELLEGEN_ALLOW_FALLBACK=1` to serve the
two pglib fallback cases with synthetic coordinates.

## Docs

Install mdBook, then build the public docs:

```sh
scripts/build-docs.sh
```

CI pins mdBook to `v0.5.3`. For local work, any recent mdBook `0.5.x` release
should render the book.
