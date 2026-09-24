# Direction

Where the project is going and why. [Architecture](architecture.md) describes
the boundary as shipped; this page describes the intent behind it. The
ecosystem claims date from a June 2026 review; sources at the end.

## Landscape

The sources below show no web native, developer facing framework for reactive
power systems (public sources; a private implementation could exist). The
closest tools:

- Electrisim runs pandapower and OpenDSS from the browser; the compute is
  server side, and it reports no locational marginal prices.
- RTE's GridSuite (PowSyBl) is the most mature browser grid application in the
  field: closed operator software without a reactive price map.
- GridStatus.io has a polished nodal price map of ISO published prices the
  user cannot recompute.
- PowerPlots.jl is hover to inspect, driven from Julia.

None of them let a user load a case, click a bus, drag demand, and watch
prices and flows re-solve live against exact KKT sensitivities. tellegen
ships that interaction.
[PowerDiff.jl](https://github.com/grid-opt-alg-lab/PowerDiff.jl) worked out
the sensitivity columns behind it; tellegen exposes them as a framework, and
the demo is one application built on it.

## The boundary

Sensitivity columns give an immediate matrix vector preview during a drag;
the exact solve fires on release and reconciles the preview. Around that
loop:

- **powerio** (Rust): parsing, encoding, the formats, the network data model,
  and the canonical display format ([display-data.md](display-data.md)).
- **tellegen** (browser): interaction, the fast math, rendering. Its Rust is
  built in this repository against powerio and compiled to WebAssembly.
- **tellegen backend** (Rust): the same numerical core compiled native. It
  hosts the bundled cases; its compute endpoints can serve browsers without a
  working WebAssembly path and ship disabled behind
  `TELLEGEN_SERVER_COMPUTE`, a single switch. Per endpoint control and
  authentication come before AC OPF ships as a server product.

PowerDiff.jl remains the reference harness for parity checks.

## Browser solver status

Julia has no production WebAssembly path: the runtime port has been dormant
since 2021, and the function level compilers cannot reach BLAS or JuMP. The
numerics are therefore written in Rust and compiled to wasm.

- **DC OPF (sparse LP/QP): shipped.** Clarabel.rs, a pure Rust interior point
  solver with no BLAS dependency, solves 200 to 7000 bus cases in the browser
  as the exact commit.
- **DC sensitivities (the dLMP/dd columns): shipped.** One linear solve
  against the KKT factorization at the active set, reimplemented in Rust on
  faer.
- **AC power flow (Newton with sparse LU): shipped.** The browser package runs
  the same polar Newton formulation as the native engine and exposes exact
  commit plus Newton sensitivities.
- **AC OPF (nonconvex): native development capability.** The non-default
  `acopf` feature now compiles and solves PowerIO's exact polar model through
  POUNCE, validates the primal solution independently, and emits a portable
  solution. It is not enabled by a shipping adapter. Browser delivery still
  needs a dedicated WASI worker, while native distribution awaits solver,
  license, and release gates.

The browser owns the whole DC pipeline: parse, solve, differentiate, render
([issue #2](https://github.com/eigenergy/tellegen/issues/2)).

## Decisions

- **wasm-pack and wasm-bindgen stay.** Both shipped releases in 2026
  (wasm-bindgen 0.2.125 in June). The WebAssembly Component Model targets
  server and edge with no browser DOM host, so it does not apply here.
- **deck.gl and MapLibre on WebGL2 stay.** deck.gl holds 60 fps to about a
  million elements; grids run at ten thousand to a hundred thousand. WebGPU
  in deck.gl lacks picking and basemap interleave today and can be adopted
  later without touching the rendering code.
- **`$state.raw` on large payloads.** Shared reactive state is scoped through
  context, so the packages stay SSR safe.

## Roadmap

Near term: harden the framework package boundary as outside apps consume
`@tellegen/engine` and `@tellegen/svelte`. The hosted demo is the reference
consumer.

Mid term: prebake the bundled cases as static assets and make the no backend
deployment the default. The DC pipeline in the browser has landed, including
the Safari sensitivity gap
([issue #8](https://github.com/eigenergy/tellegen/issues/8)).

Long term: graduate the native AC OPF feature after its solver and distribution
gates, then add browser AC OPF through a dedicated WASI worker with hard
cancellation. WebGPU follows when deck.gl's backend gains picking and basemap
support. Synthetic grid generation pairs with in-browser compute.

## Sources

- Rust to wasm toolchain: [wasm-bindgen](https://github.com/wasm-bindgen/wasm-bindgen),
  [wasm-pack](https://crates.io/crates/wasm-pack) (both active in 2026)
- wasm features and browser support: [caniuse wasm-simd](https://caniuse.com/wasm-simd),
  [wasm-threads](https://caniuse.com/wasm-threads),
  [COOP/COEP](https://web.dev/articles/coop-coep)
- In-browser solving: [Clarabel.rs](https://github.com/oxfordcontrol/Clarabel.rs)
  (wasm support, [issue #133](https://github.com/oxfordcontrol/Clarabel.rs/issues/133)),
  [faer](https://docs.rs/faer/latest/faer/)
  ([wasm sparse caveat](https://github.com/sarah-quinones/faer-rs/issues/222))
- Rendering: [deck.gl performance](https://deck.gl/docs/developer-guide/performance),
  [deck.gl WebGPU status](https://deck.gl/docs/developer-guide/webgpu)
- Svelte: [$state](https://svelte.dev/docs/svelte/$state),
  [packaging](https://svelte.dev/docs/kit/packaging)
- Julia to wasm status: [julia-wasm (dormant)](https://github.com/Keno/julia-wasm),
  [WasmTarget.jl (early)](https://github.com/GroupTherapyOrg/WasmTarget.jl)
- Landscape: [GridSuite](https://www.gridsuite.org/),
  [GridStatus](https://www.gridstatus.io/), [Electrisim](https://electrisim.com/),
  [PowerPlots.jl](https://wispo-pop.github.io/PowerPlots.jl/dev/)
