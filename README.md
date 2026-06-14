# tellegen

Reactive visualization for power systems optimization. Named for Tellegen's theorem, the basis of adjoint sensitivity analysis in circuits.

The interaction model is gradient preview, exact commit: perturbations update the display instantly through KKT sensitivity columns computed by [PowerDiff.jl](https://github.com/grid-opt-alg-lab/PowerDiff.jl), and exact OPF re-solves stream in behind them. Case file parsing uses [powerio](https://github.com/eigenergy/powerio), on the server and in the browser.

## The demo

Three TAMU ACTIVSg synthetic grids share one map, each at the real substation coordinates from its aux export: ACTIVSg200 on central Illinois, ACTIVSg500 on South Carolina, ACTIVSg2000 across Texas ([docs/real-coordinates.md](docs/real-coordinates.md)). Each is an islanded DC OPF instance on the backend; the header switcher pans the shared view between them. All three have quadratic generator costs, which keeps dLMP/dd nonzero across the interior of the feasible region (linear cost cases have piecewise constant LMPs whose gradient is zero almost everywhere).

Bus colors are locational marginal prices on a sequential ramp. Click a bus and the map switches to its dLMP/dd column on a diverging ramp: one exact KKT column, no re-solve. Drag the demand slider and prices preview instantly along that gradient; release it and the exact solution streams back over SSE with the interior point iterations drawn live. The panel then scores the preview: predicted objective change next to the exact one.

Drop a case file (`.m`, `.raw`, `.aux`) anywhere on the window and powerio, compiled to WebAssembly, parses it in the browser. Files with substation coordinates land on the map; the file never uploads. A PowerWorld `.pwd` display file drops too, decoded to its substation points at approximate positions ([docs/display-format.md](docs/display-format.md)).

## Exact gradients

The sensitivities are exact derivatives of the KKT system at the optimum, not finite differences or fits. `backend/test/runtests.jl` holds that claim to numbers: dLMP/dd columns from PowerDiff match central finite differences of full re-solves to better than 0.1% relative error, and the residual is finite difference truncation, not gradient error.

```sh
julia --project=backend backend/test/runtests.jl
```

The objective prediction in the UI uses the envelope theorem (the LMP times the demand step) plus the second order term from the dLMP/dd diagonal, so the preview is exact through second order in the perturbation.

## Layout

- `backend/` Julia API server (Oxygen.jl + PowerDiff.jl); real coordinate ingestion in `src/coords.jl`
- `frontend/` SvelteKit 5 static app (MapLibre GL + deck.gl)
- `rust/` tellegen's Rust: powerio compiled to WebAssembly for in browser parsing
- `scripts/` data staging
- `deploy/` deployment guide and Caddy config
- `docs/` notes: [direction](docs/direction.md) and [ecosystem research](docs/research-notes.md); [real coordinates](docs/real-coordinates.md), [synthetic layout](docs/synthetic-layout.md), [display format](docs/display-format.md)

## Data

The TAMU distributions are downloaded by the operator and never vendored. With the distributions at `~/Datasets`:

```sh
scripts/stage-data.sh ~/Datasets
```

stages the six needed files (about 9 MB) into `data/`. Without staged data the backend falls back to pglib copies of the small cases, placed by the [synthetic layout](docs/synthetic-layout.md).

## Development

Backend (port 8000):

```sh
cd backend
julia --project=. bootstrap.jl
```

PowerIO.jl is in the General registry; PowerDiff.jl is not, so `backend/Project.toml` pins it through `[sources]` at a git rev. Resolve both:

```sh
julia --project=backend -e 'using Pkg; Pkg.instantiate()'
```

(Maintainers developing PowerIO.jl or PowerDiff.jl locally can `Pkg.develop` a path instead; the gitignored Manifest keeps that local.) PowerIO.jl 0.1.2 bundles the powerio v0.2.2 binary as a lazy artifact, so no separate Rust build is needed. To run against an unreleased powerio, build `powerio-capi` and set `POWERIO_CAPI=/path/to/libpowerio_capi.{dylib,so}`.

WASM module (required before the frontend builds; powerio comes from crates.io):

```sh
cargo install wasm-pack
cd rust && wasm-pack build --target web --out-dir ../frontend/src/lib/wasm-pkg
```

Frontend (port 5173, proxies `/api` to 8000):

```sh
cd frontend
npm install
npm run dev
```

## API

- `GET /api/health`
- `GET /api/cases`
- `GET /api/cases/{id}/network` buses, branches, coordinates (`synthetic_coords` flags manufactured ones)
- `GET /api/cases/{id}/solution` LMPs, flows, dispatch from the DC OPF
- `GET /api/cases/{id}/sensitivity/lmp/d/{bus}` dLMP/dd column at a bus
- `GET /api/cases/{id}/solve` SSE re-solve stream

Both of the last two accept `?d=bus:mw,bus:mw`, demand deltas in MW from the base case, so gradients and solves are taken at the client's operating point. The solve stream emits `status`, then `iteration` events as Ipopt walks in (iterate, objective, primal and dual infeasibility), then `solution`, then a refreshed `sensitivity` column when `?sens={bus}` is passed, then `done`. It is a GET because EventSource only speaks GET.

## Roadmap

- AC OPF sensitivities and voltage/reactive operands (PowerDiff.jl computes them today; the UI needs operand switching)
- solve and differentiate dropped cases, not only the bundled ones
- synthetic grid generation, so users can spawn networks onto any part of the map (powerio)
- a display format in powerio for case geometry, rendered in the browser through tellegen, so any case can carry positions ([docs/display-format.md](docs/display-format.md))
- energy burden and demographic overlays
- DER screening workflows

## License

MIT
