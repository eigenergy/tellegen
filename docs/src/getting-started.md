# Getting Started

## Repository Layout

- `frontend/`: tellegen frontend
- `rust/`: tellegen backend and WebAssembly packages
- `reference/julia-backend/`: Julia PowerDiff.jl parity harness
- `scripts/`: data staging and docs build helpers
- `deploy/`: deployment compose files and proxy notes
- `docs/src/`: mdBook documentation source

## tellegen backend

```sh
cargo run --manifest-path rust/Cargo.toml --bin tellegen-server
```

Set `TELLEGEN_ALLOW_FALLBACK=1` to run without staged TAMU data:

```sh
TELLEGEN_ALLOW_FALLBACK=1 cargo run --manifest-path rust/Cargo.toml --bin tellegen-server
```

The Julia reference harness is kept for PowerDiff.jl parity checks:

```sh
julia --project=reference/julia-backend -e 'using Pkg; Pkg.instantiate()'
julia --project=reference/julia-backend reference/julia-backend/test/runtests.jl
```

## WebAssembly Module

```sh
cd frontend
npm run wasm
```

## tellegen frontend

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
three staged cases, the tellegen backend exits. For CI or local smoke checks
without the TAMU distributions, set `TELLEGEN_ALLOW_FALLBACK=1` to serve the
two pglib fallback cases with synthetic coordinates.

## Docs

Install mdBook, then build the public docs:

```sh
scripts/build-docs.sh
```

CI pins mdBook to `v0.5.3`. For local work, any recent mdBook `0.5.x` release
should render the book.
