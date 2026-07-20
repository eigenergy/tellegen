<p align="center">
  <img src="docs/assets/hero.svg" alt="tellegen reactive power flow visualization" width="100%">
</p>

# tellegen

Reactive visualization for power systems optimization. Demand and rating edits
preview through KKT sensitivity columns and commit as exact solves, entirely in
the browser: DC OPF, AC power flow, and the SOCWR relaxation run in
WebAssembly. Case parsing uses
[powerio](https://github.com/eigenergy/powerio). The name is Tellegen's
theorem, the reciprocity result behind adjoint sensitivities.

Live demo: [tellegen.dev](https://tellegen.dev). Documentation:
[eigenergy.github.io/tellegen](https://eigenergy.github.io/tellegen/).

## Packages

```sh
npm install @tellegen/engine   # case parsing and wasm solves, framework agnostic
npm install @tellegen/svelte   # map, panels, and solve card as Svelte components
```

`@tellegen/engine` exports case parsing, browser solving, the `Study` preview
and commit calls, sensitivities, and generated TypeScript types.
`@tellegen/svelte` exports the map, panels, local file flow, and solve card.
Start with the
[framework quickstart](https://eigenergy.github.io/tellegen/framework-quickstart.html);
`examples/browser-minimal/` and `examples/svelte-minimal/` are working
integrations of each package. The Rust engine is the
[tellegen](https://crates.io/crates/tellegen) crate.

## Demo

The demo serves three TAMU ACTIVSg synthetic grids and the CATS California
Test System. These are synthetic networks on geographic footprints, not
surveyed infrastructure:

| case | territory | buses | branches |
|---|---|---:|---:|
| ACTIVSg200 | central Illinois | 200 | 245 |
| ACTIVSg500 | South Carolina | 500 | 597 |
| ACTIVSg7000 | Texas | 6717 | 9140 |
| CATS | California | 8870 | 10823 |

Bus color is locational marginal price. Selecting a bus shows ∂LMP/∂demand at
that bus; selecting a binding line shows ∂LMP/∂rating. Dragging a slider
applies the sensitivity column live; releasing it re-solves exactly in
WebAssembly. A selector switches the formulation between DC OPF and SOCWR.

Dropped `.m`, `.raw`, and `.aux` case files parse in the browser and never
upload. Files with coordinates render in place; files without can be placed by
clicking the map or paired with `.csv`, `.json`, or `.geojson` geography
(powerio's GeoLayer reader; branch routes render as polylines). A PowerWorld
`.pwd` file renders as approximate substation positions, or fills a
coordinate-less sibling case through its substation numbers. Saved studies and
exports carry the placement, and the layout downloads as a `.geo.json` layer.

## Development

Prerequisites: Rust from [rust-toolchain.toml](rust-toolchain.toml) with
`rustfmt`, `clippy`, and the `wasm32-unknown-unknown` target; Node.js 22 or
newer; `wasm-pack` 0.15.x; mdBook 0.5.x for docs.

```sh
# backend with the embedded fallback cases
TELLEGEN_ALLOW_FALLBACK=1 cargo run -p tellegen-server

# frontend demo (the dev server proxies /api to localhost:8000)
npm ci
npm run wasm
npm run build:engine
npm run build:svelte
npm --workspace tellegen-frontend run dev
```

Tests:

```sh
cargo test --workspace
npm run check && npm run build && npm run smoke:web && npm run test:downstream
```

Case data comes from the operator, not the repository. With the ACTIVSg and
CATS distributions under `~/Datasets`, `scripts/stage-data.sh ~/Datasets`
stages the complete cases into `data/`; the server serves whatever is staged.

## Repository layout

- `crates/`: Rust workspace — `tellegen` (engine), `tellegen-wasm` (WebAssembly), `tellegen-server` (HTTP), `tellegen-cli`, `benchmarks`
- `packages/engine/`: `@tellegen/engine` browser package
- `packages/svelte/`: `@tellegen/svelte` component package
- `apps/web/`: the hosted demo, a SvelteKit consumer of the Svelte package
- `examples/`: minimal Vite and Svelte integrations of each package
- `docs/src/`: mdBook source; `scripts/build-docs.sh` builds it

The [HTTP API](https://eigenergy.github.io/tellegen/http-api.html),
[deployment](https://eigenergy.github.io/tellegen/deployment.html), and
[roadmap](https://eigenergy.github.io/tellegen/direction.html) pages cover the
server surface, hosting, and where the project is going.

## License

[MIT](LICENSE). See
[docs/src/third-party-notices.md](docs/src/third-party-notices.md) for
attributions.
