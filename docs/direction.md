# Direction

tellegen is the browser interface for power systems cases parsed by powerio and
solved by PowerDiff.jl. This note records the intended boundary between Rust,
Svelte, and Julia as of June 2026.

## Current Boundary

- **powerio** parses, encodes, and owns the network and display formats.
- **tellegen Rust** builds browser WebAssembly against powerio. It owns browser
  parsing, display decoding, and browser numerical kernels as they are added.
- **tellegen frontend** owns interaction, rendering, and the gradient preview,
  exact commit loop.
- **Julia backend** owns exact OPF solves and PowerDiff.jl sensitivities until
  the corresponding browser numerical path exists.

This split keeps format support in powerio, interface code in tellegen, and the
reference solver in PowerDiff.jl.

## Placement Of Solver Work

The browser can run the DC path if the numerical code is written in Rust and
compiled to WebAssembly. PowerDiff.jl itself depends on JuMP, Ipopt, and sparse
linear algebra that do not have a supported Julia WebAssembly path.

The implementation order is:

1. Use server computed sensitivity matrices for browser matrix vector previews.
2. Port DC OPF to Rust/WebAssembly, with PowerDiff.jl as the reference.
3. Port dLMP/dd columns for the solved DC active set.
4. Keep AC OPF on the Julia server until a browser solver path is validated.

Clarabel.rs is the candidate for DC LP/QP solves because it is pure Rust and has
a WebAssembly path. DC sensitivities require a linear solve against the active
KKT system. AC power flow requires separate validation of sparse linear algebra
under WebAssembly; AC OPF remains a server calculation.

## Frontend And Packaging

The current application already uses the intended interaction model:
sensitivity preview in the browser, exact solve on the server, and
reconciliation when the solve returns.

The next packaging step is to turn the Svelte code into a library with
`@sveltejs/package`. The package should export map components, typed case data,
layer accessors, theme inputs, and the WebAssembly parser entry points. deck.gl,
MapLibre, and Svelte should remain peer dependencies.

When the package is introduced, shared reactive state should move behind context
or explicit object instances rather than module singletons so server side
rendering does not share state across requests.

## Research Summary

- Use wasm-pack and wasm-bindgen for browser WebAssembly. The Component Model
  does not replace wasm-bindgen for DOM and browser library integration.
- Use deck.gl and MapLibre on WebGL2. The current grid sizes are below deck.gl's
  documented performance limits. WebGPU can be reconsidered when deck.gl has
  picking and basemap interleaving on that backend.
- Use `$state.raw` for large immutable payloads in Svelte 5.
- Keep exact AC solves in Julia for now.

## Roadmap

1. Package the frontend library surface.
2. Implement DC OPF and DC sensitivities in Rust/WebAssembly.
3. Use PowerDiff.jl parity tests as the acceptance criterion for the Rust DC
   path.
4. Validate sparse Rust linear algebra in WebAssembly before starting browser AC
   power flow.
5. Keep AC OPF on the backend until a nonlinear or conic browser solver path is
   selected and tested.

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
