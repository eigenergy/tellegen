# tellegen

Reactive visualization for power systems optimization. The name refers to
Tellegen's theorem and the adjoint sensitivity calculations used by
PowerDiff.jl.

tellegen uses a gradient preview, exact commit interaction model. Perturbations
update the display from KKT sensitivity columns computed by
[PowerDiff.jl](https://github.com/grid-opt-alg-lab/PowerDiff.jl). Exact DC OPF
solutions stream back from the server. Case parsing uses
[powerio](https://github.com/eigenergy/powerio) in both Rust/WebAssembly and
Julia.

## Demo

The bundled demo serves three TAMU ACTIVSg synthetic grids at the geographic
coordinates stored in their PowerWorld aux exports. These are fictional grids
on geographic footprints, not surveyed infrastructure:

| case | territory | buses | branches |
|---|---|---:|---:|
| ACTIVSg200 | central Illinois | 200 | 245 |
| ACTIVSg500 | South Carolina | 500 | 597 |
| ACTIVSg2000 | Texas | 2000 | 3206 |

Each case is an islanded DC OPF instance. Bus color shows locational marginal
price. Selecting a bus shows the dLMP/dd column for a demand perturbation at
that bus. Moving the demand slider applies the local sensitivity immediately;
releasing it sends the perturbation to the server and streams Ipopt iterations
until the exact solution returns.

Dropped `.m`, `.raw`, and `.aux` files are parsed in the browser by the
WebAssembly build of powerio. Files with coordinates render on the map. Files
without coordinates can be placed by clicking the map, or paired with local
coordinate sidecars in `.csv`, `.json`, or `.geojson` form. A dropped PowerWorld
`.pwd` file is decoded as display data and rendered as approximate substation
positions. Dropped files are not uploaded.

## Sensitivities

The served sensitivity columns are KKT derivatives at the optimum. The backend
test suite compares dLMP/dd columns from PowerDiff.jl against central finite
differences of full re-solves:

```sh
julia --project=backend backend/test/runtests.jl
```

The objective preview uses the envelope theorem and the selected dLMP/dd
diagonal term, so the displayed prediction is second order in the demand step.

## Repository layout

- `backend/`: Julia API server, Oxygen.jl, PowerDiff.jl, TAMU coordinate ingestion
- `frontend/`: SvelteKit 5 static app, MapLibre GL, deck.gl
- `rust/`: tellegen Rust crate, compiled to WebAssembly for browser parsing
- `scripts/`: data staging
- `deploy/`: deployment compose files and proxy notes
- `docs/`: architecture and implementation notes

## Documentation

The docs index is [docs/README.md](docs/README.md). The release notes there
cover data provenance, fallback layout, display file handling, deployment, and
the caveats that should stay visible in public descriptions of the demo.

## Data

The TAMU distributions are downloaded by the operator and are not vendored. With
the distributions under `~/Datasets`:

```sh
scripts/stage-data.sh ~/Datasets
```

The script stages the six files used by the demo into `data/`. Without all
three staged cases, the backend exits. For CI or local smoke checks without the
TAMU distributions, set `TELLEGEN_ALLOW_FALLBACK=1` to serve the two pglib
fallback cases with synthetic coordinates.

## Development

Backend:

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

WebAssembly module:

```sh
cd frontend
npm run wasm
```

Frontend:

```sh
cd frontend
npm install
npm run dev
```

The Vite dev server proxies `/api` to `http://localhost:8000`.

## API

- `GET /api/health`
- `GET /api/cases`
- `GET /api/cases/{id}/network`
- `GET /api/cases/{id}/solution`
- `GET /api/cases/{id}/sensitivity/lmp/d/{bus}`
- `GET /api/cases/{id}/solve`

The sensitivity and solve endpoints accept `?d=bus:mw,bus:mw`, where each value
is a MW delta from the base case. The solve stream emits `status`, `iteration`,
`solution`, optional `sensitivity`, and `done` events.

## Deployment

Local development uses the build based `docker-compose.yml`:

```sh
docker compose up -d --build
```

Production deployment uses the image based compose file in
`deploy/docker-compose.prod.yml`. The GitHub Actions deploy workflow builds and
pushes `ghcr.io/eigenergy/tellegen:<sha>`, restarts the host container, and
checks both the local host health endpoint and the public demo URL. Required
secrets are documented in [deploy/DEPLOY.md](deploy/DEPLOY.md).

## Roadmap

- DC OPF and dLMP/dd sensitivities in Rust/WebAssembly
- library packaging with `@sveltejs/package`
- dropped case solving, not only browser parsing
- canonical display data in powerio
- AC operands and AC solver paths as the browser numerical stack matures

## License

MIT
