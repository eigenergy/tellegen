# Getting Started

## Repository Layout

- `backend/`: Julia API server, Oxygen.jl, PowerDiff.jl, TAMU coordinate ingestion
- `frontend/`: SvelteKit 5 static app, MapLibre GL, deck.gl
- `rust/`: tellegen Rust crate, compiled to WebAssembly for browser parsing and DC solves
- `scripts/`: data staging and docs build helpers
- `deploy/`: deployment compose files and proxy notes
- `docs/src/`: mdBook documentation source

## Backend

```sh
cd backend
julia --project=. bootstrap.jl
```

PowerIO.jl is in the General registry. PowerDiff.jl is not registered, so
`backend/Project.toml` pins it through `[sources]` at a git revision:

```sh
julia --project=backend -e 'using Pkg; Pkg.instantiate()'
```

Maintainers developing PowerIO.jl or PowerDiff.jl locally can use
`Pkg.develop`; `backend/Manifest.toml` is ignored so local paths are not
committed. PowerIO.jl 0.1.2 bundles the powerio 0.2.2 binary as a lazy artifact.
To test an unreleased powerio build, build `powerio-capi` and set
`POWERIO_CAPI=/path/to/libpowerio_capi.{dylib,so}`.

## WebAssembly Module

```sh
cd frontend
npm run wasm
```

## Frontend

```sh
cd frontend
npm install
npm run dev
```

The Vite dev server proxies `/api` to `http://localhost:8000`.

## Data

The TAMU distributions are downloaded by the operator and are not vendored.
With the distributions under `~/Datasets`:

```sh
scripts/stage-data.sh ~/Datasets
```

The script stages the six files used by the demo into `data/`. Without all
three staged cases, the backend exits. For CI or local smoke checks without the
TAMU distributions, set `TELLEGEN_ALLOW_FALLBACK=1` to serve the two pglib
fallback cases with synthetic coordinates.

## Docs

Install mdBook, then build the public docs:

```sh
scripts/build-docs.sh
```

CI pins mdBook to `v0.5.3`. For local work, any recent mdBook `0.5.x` release
should render the book.
