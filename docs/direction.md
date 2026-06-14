# Direction

tellegen is the browser layer of powerio: a reactive frontend framework for
power systems. This note records where it is going and why. It is grounded in a
June 2026 review of the Rust/WebAssembly, Svelte, and Julia ecosystems and of
the power systems software landscape; key sources are listed at the end.

## The gap tellegen owns

No web-native, developer-facing framework for reactive power systems exists. The
closest tools each miss on one axis:

- Electrisim runs pandapower and OpenDSS in the browser, but the compute is
  server side and it reports no locational marginal prices.
- RTE's GridSuite (PowSyBl) is the most production-grade browser grid
  application in the field. It is closed operator software with no reactive
  price map.
- GridStatus.io has a polished nodal price map, but it shows ISO-published
  prices the user cannot recompute.
- PowerPlots.jl is hover-to-inspect, driven from Julia, not a client
  application.

tellegen's pitch is the one nobody ships: load a real case, click a bus, drag
demand, and watch prices and flows re-solve live, backed by exact KKT
sensitivities. PowerDiff already computes the sensitivity column that makes this
possible. tellegen is the framework that exposes it, and the demo is one
application built on it.

## Architecture: a hybrid that is already proven

The recommended design for "instant interaction with correct numbers" is to
ship the sensitivity matrices to the browser for an immediate matrix-vector
preview, fire the exact solve on the server in parallel, and reconcile. That is
exactly tellegen's gradient-preview-then-exact-commit loop. The core interaction
does not need rethinking. The work is sharpening the boundary and packaging it.

The boundary:

- **powerio** (Rust): parse, encode, the formats, the network data model. Where
  the canonical display format belongs ([display-format.md](display-format.md)).
- **tellegen** (browser): powerio's reactive frontend. Owns interaction, the
  fast math, and rendering. The Rust it needs is built in-repo against powerio.
- **Julia server** (Oxygen + PowerDiff): the exact solver and research-grade
  numerics, for as long as they resist WebAssembly. See the next section.

## Can the browser take the solver?

Yes. The constraint was never that the browser cannot compute. It is that
PowerDiff.jl is Julia (JuMP, Ipopt, SuiteSparse), and Julia has no production
WebAssembly path: the runtime port has been dead since 2021, and the one active
function-level compiler cannot reach BLAS or JuMP. The route to a client-side
solver is to write the numerics in Rust and compile to wasm, the way powerio
already is. The difficulty runs as a gradient:

- **DC OPF, a sparse LP/QP: ready today.** Clarabel.rs is a pure-Rust interior
  point solver (QDLDL factorization, no BLAS) that compiles to and runs in the
  browser; a 1000-variable QP has been solved in-browser with it. A 200 to 2000
  bus DC OPF is that size.
- **DC sensitivities, the dLMP/dd columns: linear algebra.** Once the DC OPF is
  solved, the sensitivity column is one linear solve against the KKT
  factorization for the active set. PowerDiff already worked out the algorithm;
  reimplementing the DC path in Rust is engineering, not research. This is
  tellegen's differentiator, and it can run client side.
- **AC power flow, Newton with sparse LU: feasible, greenfield.** faer provides
  sparse LU in Rust under wasm. There is no shipped precedent, and faer's wasm
  sparse path has an unresolved crash report, so validate it on real case
  matrices before relying on it.
- **AC OPF, a nonconvex program: the one genuine holdout.** There is no
  Ipopt-in-wasm. The options are a second-order cone relaxation through Clarabel
  (approximate, well studied for AC OPF) or a Rust nonlinear solver (thin
  ground). Until one matures, AC OPF is the reason to keep a server.

So the browser can own the entire DC pipeline: parse, solve, differentiate,
render, with no server. That is the move that turns tellegen into a frontend
framework rather than a frontend plus a Julia backend. The server then shrinks
to what genuinely resists wasm today, and PowerDiff.jl stays as the reference
implementation the Rust solver is validated against.

## What the research settles

A few calls are now well grounded, so they do not need relitigating:

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
- **`$state.raw` on large payloads is the documented best practice.** When
  tellegen is packaged as a library, scope shared reactive state through context
  rather than module singletons to stay SSR safe.
- **Keep the exact AC solver in Julia for now.** A long-running Oxygen process
  with `PrecompileTools.@compile_workload` on the OPF and KKT paths makes cold
  start a non-issue.

## Roadmap

**Near term: become a framework, not an application.** Build the Rust into the
tellegen crate against powerio (done). Package with `@sveltejs/package`: export
the map components, the parsed-case data model, the layer accessor and theme
props, and the wasm parser as a standalone function; peer-depend deck.gl and
maplibre; ship the wasm through `?url`. The demo becomes the reference consumer.
This is the highest-leverage step toward "framework."

**Mid term: move the DC pipeline into the browser.** Reimplement DC OPF (via
Clarabel.rs) and the dLMP/dd sensitivities in Rust, compiled to wasm. Start with
the sensitivity preview as a matrix-vector product fed by server-computed
matrices, then promote the full DC OPF client side so the whole DC experience
runs with no server. This is the answer to "can the browser take the solver" and
the path to a static, no-backend deployment.

**Long term: AC and the frontier.** AC power flow in the browser (faer plus
Newton) once faer's wasm sparse path is validated. AC OPF stays on the server
until a wasm nonlinear or cone-relaxation path is solid. WebGPU when deck.gl's
backend gains picking and basemap support. Synthetic grid generation pairs
naturally with in-browser compute.

The through line: powerio owns the formats and the math primitives; tellegen
owns the reactive browser experience and is the framework others build on; the
server holds the exact AC solver until the browser can take it. Every reasonable
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
