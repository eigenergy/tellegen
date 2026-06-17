# Architecture

tellegen is the browser interface for power systems cases parsed by powerio and
solved in browser WebAssembly, with a Rust server fallback for bundled cases.
This chapter records the intended boundary between Rust, Svelte, and the Julia
reference harness as of June 2026.

## Current boundary

- **powerio** parses, encodes, and owns the network and display formats.
- **tellegen Rust** builds browser WebAssembly against powerio and the native
  HTTP server. It owns browser parsing, display decoding, bundled case loading,
  DC OPF, dLMP/dd columns, and server fallback solves.
- **tellegen frontend** owns interaction, rendering, and the gradient preview,
  exact commit loop.
- **Julia reference** owns PowerDiff.jl parity checks. It is not part of the
  production runtime.

This split keeps format support in powerio, interface code in tellegen, and the
reference checks in PowerDiff.jl. Local dropped case files solve only in the
browser and are not uploaded.

## Placement of solver work

The browser runs the DC path because the numerical code is written in Rust and
compiled to WebAssembly. PowerDiff.jl itself depends on JuMP, Ipopt, and sparse
linear algebra that do not have a supported Julia WebAssembly path.

The implemented order is:

1. Use server computed sensitivity matrices for browser matrix vector previews.
2. Port DC OPF to Rust/WebAssembly, with PowerDiff.jl as the reference.
3. Port dLMP/dd columns for the solved DC active set.
4. Move the bundled case API and server fallback to the native Rust server.

Clarabel.rs runs the DC LP/QP solves in the Rust crate. DC sensitivities use a
linear solve against the active KKT system. AC power flow requires separate
validation of sparse linear algebra under WebAssembly. AC OPF is not implemented
in the production server.

## Frontend and packaging

The current application already uses the intended interaction model:
sensitivity preview in the browser, exact solve in the browser, and bundled
case fallback to the server when WebAssembly solve fails.

The next packaging step is to turn the Svelte code into a library with
`@sveltejs/package`. The package should export map components, typed case data,
layer accessors, theme inputs, and the WebAssembly parser entry points. deck.gl,
MapLibre, and Svelte should remain peer dependencies.

When the package is introduced, shared reactive state should move behind context
or explicit object instances rather than module singletons so server side
rendering does not share state across requests.

## Research summary

- Use wasm-pack and wasm-bindgen for browser WebAssembly. The Component Model
  does not replace wasm-bindgen for DOM and browser library integration.
- Use deck.gl and MapLibre on WebGL2. The current grid sizes are below deck.gl's
  documented performance limits. WebGPU can be reconsidered when deck.gl has
  picking and basemap interleaving on that backend.
- Use `$state.raw` for large immutable payloads in Svelte 5.
- Keep PowerDiff.jl as a reference path, not as production infrastructure.

## Roadmap

1. Package the frontend library surface.
2. Keep PowerDiff.jl parity tests as the acceptance criterion for the Rust DC
   path.
3. Validate sparse Rust linear algebra in WebAssembly before starting browser AC
   power flow.
4. Choose a separate server side numerical path before adding AC OPF.

## Sources

- Rust to wasm: [wasm-bindgen](https://github.com/wasm-bindgen/wasm-bindgen),
  [wasm-pack](https://crates.io/crates/wasm-pack)
- Browser wasm features: [wasm SIMD](https://caniuse.com/wasm-simd),
  [wasm threads](https://caniuse.com/wasm-threads),
  [COOP/COEP](https://web.dev/articles/coop-coep)
- Solvers and linear algebra: [Clarabel.rs](https://github.com/oxfordcontrol/Clarabel.rs),
  [faer](https://docs.rs/faer/latest/faer/),
  [faer wasm sparse issue](https://github.com/sarah-quinones/faer-rs/issues/222)
- Rendering: [deck.gl performance](https://deck.gl/docs/developer-guide/performance),
  [deck.gl WebGPU status](https://deck.gl/docs/developer-guide/webgpu)
- Svelte: [$state](https://svelte.dev/docs/svelte/$state),
  [packaging](https://svelte.dev/docs/kit/packaging)
- Julia to wasm: [julia-wasm](https://github.com/Keno/julia-wasm),
  [WasmTarget.jl](https://github.com/GroupTherapyOrg/WasmTarget.jl)
