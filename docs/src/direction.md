# Direction

tellegen is the browser layer of powerio: a reactive frontend framework for
power systems. This page records where the project is going and why, grounded
in a June 2026 review of the Rust/WebAssembly, Svelte, and Julia ecosystems and
of the power systems software landscape; sources are listed at the end.
[Architecture](architecture.md) describes the boundary as shipped; this page
describes the intent it serves.

## The gap tellegen owns

No web native, developer facing framework for reactive power systems shows up
in the sources checked (listed below; public sources, so a private
implementation could exist). The closest tools each miss on one axis:

- Electrisim runs pandapower and OpenDSS in the browser, but the compute is
  server side and it reports no locational marginal prices.
- RTE's GridSuite (PowSyBl) is the most mature browser grid
  application in the field. It is closed operator software with no reactive
  price map.
- GridStatus.io has a polished nodal price map, but it shows ISO-published
  prices the user cannot recompute.
- PowerPlots.jl is hover-to-inspect, driven from Julia, not a client
  application.

tellegen's pitch is the one none of them ship: load a real case, click a bus,
drag demand, and watch prices and flows re-solve live, backed by exact KKT
sensitivities. [PowerDiff.jl](https://github.com/grid-opt-alg-lab/PowerDiff.jl),
a Julia package for differentiable power system analysis, worked out the
sensitivity column that makes this possible. tellegen is the framework that exposes it, and the demo is one
application built on it.

## Architecture: a hybrid that is already proven

The interaction model for "instant interaction with correct numbers" is to use
the sensitivity matrices for an immediate matrix-vector preview in the browser,
fire the exact solve on release, and reconcile. That is tellegen's gradient
preview, exact commit loop. The core interaction does not need rethinking. The
work is sharpening the boundary and packaging it.

The boundary:

- **powerio** (Rust): parse, encode, the formats, the network data model. Where
  the canonical display format belongs ([display-data.md](display-data.md)).
- **tellegen** (browser): powerio's reactive frontend. Owns interaction, the
  fast math, and rendering. The Rust it needs is built in this repository against powerio
  and compiled to WebAssembly.
- **tellegen backend** (Rust): the same numerical core as the browser,
  compiled native. It hosts the bundled cases; its compute endpoints can serve
  browsers that cannot run the WebAssembly path and ship disabled behind
  `TELLEGEN_SERVER_COMPUTE`. PowerDiff.jl is kept only as a reference harness
  for parity checks, not as production infrastructure.

## Can the browser take the solver?

Yes, and for the DC pipeline it already has. The constraint was never that the
browser cannot compute. It was that PowerDiff.jl is Julia (JuMP, Ipopt,
SuiteSparse), and Julia has no production WebAssembly path: the runtime port has
been dead since 2021, and the one active function level compiler cannot reach
BLAS or JuMP. The route to a client side solver is to write the numerics in Rust
and compile to wasm, the way powerio already is. The difficulty runs as a
gradient:

- **DC OPF, a sparse LP/QP: shipped.** Clarabel.rs is a pure Rust interior
  point solver (QDLDL factorization, no BLAS) that compiles to and runs in the
  browser. A 200 to 7000 bus DC OPF is within its range, and it now runs
  client side as the exact commit.
- **DC sensitivities, the dLMP/dd columns: shipped.** Once the DC OPF is solved,
  the sensitivity column is one linear solve against the KKT factorization for
  the active set. PowerDiff worked out the algorithm; the DC path is now
  reimplemented in Rust (faer) and runs client side. This is tellegen's
  differentiator.
- **AC power flow, Newton with sparse LU: feasible, greenfield.** faer provides
  sparse LU in Rust under wasm. There is no shipped precedent, and faer's wasm
  sparse path has an unresolved crash report, so validate it on real case
  matrices before relying on it.
- **AC OPF, a nonconvex program: the one genuine holdout.** There is no
  Ipopt in wasm. The options are a second-order cone relaxation through Clarabel
  (approximate, well studied for AC OPF) or a Rust nonlinear solver (thin
  ground). Until one matures, AC OPF is the reason to keep a backend.

So the browser owns the entire DC pipeline: parse, solve, differentiate, render,
with no server. That move has landed
([issue #2](https://github.com/eigenergy/tellegen/issues/2)), and it is what turns tellegen
into a frontend framework rather than a frontend plus a separate solver service.
The backend has shrunk to what genuinely resists wasm today plus bundled-case
hosting, and PowerDiff.jl stays as the reference implementation the Rust solver
is validated against.

## What the research settles

A few calls are settled:

- **Keep wasm-pack and wasm-bindgen.** Both shipped releases in 2026
  (wasm-bindgen 0.2.125 in June). The "stagnant" reputation is stale. The
  WebAssembly Component Model targets server and edge and has no browser DOM
  host, so it does not apply here.
- **Keep deck.gl and MapLibre on WebGL2.** deck.gl holds 60fps to about a
  million elements; grids run at ten thousand to a hundred thousand, so there
  are one to two orders of magnitude of headroom. The lever is the
  wasm-to-binary-buffer path plus `updateTriggers`, not a new renderer. WebGPU
  is not ready in deck.gl (no picking, no basemap interleave), and adopting it
  later is a backend flip, not a rewrite.
- **`$state.raw` on large payloads is the documented best practice.** tellegen
  is packaged as a library, so shared reactive state is scoped through context
  rather than module singletons to stay SSR safe.
- **Keep the exact AC solver out of the browser for now.** AC OPF stays in the
  tellegen backend until a wasm nonlinear or cone relaxation path is solid. The
  Julia PowerDiff.jl harness is retained only for parity, not for serving.

## Roadmap

**Near term: become a framework, not an application.** The Rust core is built
against powerio and compiles to both wasm and native (done). `@tellegen/engine`
is the first framework package: it exports case parsing, browser wasm solving,
`Study` preview and commit calls, sensitivities, and generated contracts. The
hosted demo is the reference consumer. The remaining near term work is hardening
that package boundary as outside apps use it.

**Mid term: the DC pipeline in the browser (landed).** DC OPF (via Clarabel.rs)
and the dLMP/dd sensitivities are reimplemented in Rust and compiled to wasm, so
the whole DC experience runs with no server; the backend is the fallback for
engines that cannot run the wasm path. The Safari sensitivity gap is closed
([issue #8](https://github.com/eigenergy/tellegen/issues/8)); the remaining
work is prebaking the bundled cases as static assets and making the no-backend
deployment the default.

**Long term: AC and the frontier.** AC power flow in the browser (faer plus
Newton) once faer's wasm sparse path is validated. AC OPF stays in the backend
until a wasm nonlinear or cone relaxation path is solid. WebGPU when deck.gl's
backend gains picking and basemap support. Synthetic grid generation pairs
naturally with in-browser compute.

The through line: powerio owns the formats and the math primitives; tellegen
owns the reactive browser experience and is the framework others build on; the
backend holds the exact AC solver until the browser can take it. Every reasonable
"ultimate framework" move sits on that spine.

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
