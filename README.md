<p align="center">
  <img src="docs/assets/hero.svg" alt="tellegen reactive power flow visualization" width="100%">
</p>

# tellegen

Reactive visualization for power systems optimization. The name refers to
Tellegen's theorem and the adjoint sensitivity calculations.

**Live demo: [tellegen.dev](https://tellegen.dev)**

tellegen uses a gradient preview, exact commit interaction model. Perturbations
update the display from KKT sensitivity columns. Exact solves for DC OPF, AC
power flow, and the SOCWR relaxation run in the browser in WebAssembly; full AC
OPF is in progress. Case parsing uses
[powerio](https://github.com/eigenergy/powerio).

Full documentation is published with mdBook at
[eigenergy.github.io/tellegen](https://eigenergy.github.io/tellegen/). The
source lives in [docs/src/SUMMARY.md](docs/src/SUMMARY.md).

## Packages

```sh
npm install @tellegen/svelte   # map, panels, and solve card as Svelte components
npm install @tellegen/engine   # case parsing and wasm solves, framework agnostic
```

`@tellegen/engine` exports case parsing, browser wasm solving, `Study` preview
and commit calls, sensitivities, and generated TypeScript types.
`@tellegen/svelte` exports the map, panels, local file flow, and solve card as
Svelte components. Both packages are MIT licensed. Start with the
[framework quickstart](https://eigenergy.github.io/tellegen/framework-quickstart.html);
`examples/browser-minimal/` and `examples/svelte-minimal/` are working
integrations of each package.

## Demo Behavior

The bundled demo serves three TAMU ACTIVSg synthetic grids and the CATS
California Test System at their staged geographic coordinates. These are
synthetic grids on geographic footprints, not surveyed infrastructure:

| case | territory | buses | branches |
|---|---|---:|---:|
| ACTIVSg200 | central Illinois | 200 | 245 |
| ACTIVSg500 | South Carolina | 500 | 597 |
| ACTIVSg7000 | Texas | 6717 | 9140 |
| CATS | California | 8870 | 10823 |

Each case solves as DC OPF by default; a selector switches to the SOCWR relaxation,
solved in the browser in WebAssembly. Bus color shows locational marginal price. Selecting a bus shows the dLMP/dd column for a
demand perturbation at that bus. Moving the demand slider applies the local
sensitivity immediately; releasing it computes the exact solution in WebAssembly.

Dropped `.m`, `.raw`, and `.aux` files are parsed in the browser by the
WebAssembly build of powerio. Files with coordinates render on the map. Files
without coordinates can be placed by clicking the map, or paired with local
geographic files in `.csv`, `.json`, or `.geojson` form. A dropped PowerWorld
`.pwd` file is decoded as display data and rendered as approximate substation
positions. Parsed local case files solve in the browser and are not uploaded.

## Development

Prerequisites:

- Rust from [rust-toolchain.toml](rust-toolchain.toml), including `rustfmt`, `clippy`,
  and the `wasm32-unknown-unknown` target
- Node.js 22 or newer
- `wasm-pack` 0.15.x for the browser WebAssembly build
- mdBook 0.5.x for local documentation builds

tellegen backend:

```sh
TELLEGEN_ALLOW_FALLBACK=1 cargo run -p tellegen-server
```

WebAssembly module:

```sh
npm ci
npm run wasm
npm run build:engine
```

tellegen frontend demo:

```sh
npm ci
npm run wasm
npm run build:engine
npm run build:svelte
npm --workspace tellegen-frontend run dev
```

The Vite dev server proxies `/api` to `http://localhost:8000`. `apps/web`
resolves `@tellegen/svelte` through its built `dist/`, so `build:svelte` must
run before the dev server starts.

Framework package tarballs:

```sh
npm run pack:engine
npm run pack:svelte
```

`apps/web` is a private hosted demo that consumes the Svelte package. See
[docs/src/frontend-package.md](docs/src/frontend-package.md).

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

## Tests

The served sensitivity columns are KKT derivatives at the optimum. tellegen
backend tests cover the solver, the sensitivity columns, and the API:

```sh
cargo test --workspace
```

tellegen frontend checks:

```sh
npm run check
npm run build
npm run smoke:web
npm run test:downstream
```

## Repository layout

- `apps/web/`: private SvelteKit hosted demo
- `crates/`: Rust workspace — `tellegen` (engine), `tellegen-wasm` (WebAssembly), `tellegen-server` (HTTP), `tellegen-cli`, `benchmarks`
- `packages/engine/`: public `@tellegen/engine` browser package
- `packages/svelte/`: public `@tellegen/svelte` component package
- `examples/browser-minimal/`: minimal downstream Vite example
- `examples/svelte-minimal/`: minimal Svelte example using the component package
- `scripts/`: data staging and docs build helpers
- `deploy/`: deployment compose files and proxy notes
- `docs/src/`: mdBook documentation source

## Documentation

Install mdBook, then build the docs:

```sh
scripts/build-docs.sh
```

## API

- `GET /api/health`
- `GET /api/cases`
- `GET /api/cases/{id}/case`
- `GET /api/cases/{id}/network`
- `GET /api/cases/{id}/solution`
- `GET /api/cases/{id}/sensitivity/lmp/d/{bus}`
- `GET /api/cases/{id}/solve`

The sensitivity and tellegen backend solve endpoints accept `?d=bus:mw,bus:mw`,
where each value is a MW delta from the base case. The solve stream emits
`status`, `solution`, optional `sensitivity`, and `done` events.

The tellegen backend solve work is bounded by `TELLEGEN_SOLVER_CONCURRENCY`
(default `2`) and `TELLEGEN_SOLVER_TIMEOUT_SECS` (default `30`). Public solve
routes are also rate limited per client: 5 solve requests and 25 sensitivity
requests per 10 seconds by default. Tune with
`TELLEGEN_RATE_LIMIT_WINDOW_SECS`, `TELLEGEN_SOLVE_RATE_LIMIT_EVENTS`, and
`TELLEGEN_SENSITIVITY_RATE_LIMIT_EVENTS`.

## Deployment

Local development uses the build based `docker-compose.yml`:

```sh
docker compose up -d --build
```

Production deployment uses the image based compose file in
`deploy/docker-compose.prod.yml`. The GitHub Actions deploy workflow builds and
pushes `ghcr.io/eigenergy/tellegen:<sha>`, restarts the host container, and
checks both the local host health endpoint and the public demo URL. Required
secrets are documented in [docs/src/deployment.md](docs/src/deployment.md).

## Roadmap

The roadmap lives on the
[direction page](https://eigenergy.github.io/tellegen/direction.html): near
term, harden the framework package boundary; mid term, make the no-backend
deployment the default; long term, AC in the browser.

## License

The Rust crates are licensed under either of [Apache-2.0](crates/tellegen/LICENSE-APACHE)
or [MIT](crates/tellegen/LICENSE-MIT), at your option. The npm packages
`@tellegen/engine` and `@tellegen/svelte` and the web app under `apps/web/` are
[MIT](LICENSE). See
[docs/src/third-party-notices.md](docs/src/third-party-notices.md) for attributions.
