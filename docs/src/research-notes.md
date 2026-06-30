# Ecosystem Notes, June 2026

These notes summarize the technical checks behind
[architecture.md](architecture.md). Version and maturity claims are point in
time.

## Rust to WebAssembly

### Toolchain

wasm-pack and wasm-bindgen are active under the `wasm-bindgen` GitHub
organization. The rustwasm organization was sunset in July 2025, but that did
not retire these tools.

| Tool | Version checked | Use in tellegen |
|---|---:|---|
| wasm-bindgen | 0.2.125, 2026-06-12 | Rust/JS bindings for browser wasm |
| wasm-pack | 0.15.0, 2026-05-15 | Build the npm style wasm package |
| trunk | 0.21.14 / 0.22.0 beta | Rust web app bundler; not needed here |
| cargo-component | 0.21.1, 2025-03 | Component Model tooling; not a browser DOM path |
| jco | 1.23.0, 2026-06-11 | Component to JS transpilation |

Browser code that interacts with JS and the DOM should continue to use
wasm-bindgen and wasm-pack. Component Model tooling is more relevant for server,
edge, and plugin targets.

Sources: [wasm-pack](https://crates.io/crates/wasm-pack),
[wasm-bindgen](https://github.com/wasm-bindgen/wasm-bindgen),
[rustwasm sunset](https://blog.rust-lang.org/inside-rust/2025/07/21/sunsetting-the-rustwasm-github-org/).

### Browser features

For numerical kernels, the relevant browser features are fixed width SIMD and
threads. SIMD is available across current major browsers. Threads require cross
origin isolation with `COOP: same-origin` and `COEP: require-corp`. GitHub Pages
cannot set those headers directly; hosted deployments should use a proxy or a
host that can set them.

Stay on 32 bit wasm memory. memory64 is not needed unless a single allocation
exceeds 4 GB, and available measurements report slower execution in current
engines.

Sources: [wasm SIMD](https://caniuse.com/wasm-simd),
[wasm threads](https://caniuse.com/wasm-threads),
[COOP/COEP](https://web.dev/articles/coop-coep),
[memory64 analysis](https://spidermonkey.dev/blog/2025/01/15/is-memory64-actually-worth-using.html).

## Browser numerical kernels

DC OPF is a sparse LP/QP. The current Rust path is Clarabel.rs, whose pure Rust
QDLDL backend has a WebAssembly target and user reported browser use. Clarabel
does not have wasm CI, so tellegen should add its own wasm smoke test before
depending on it for the demo.

| Solver | Rust only | Browser wasm | Notes |
|---|---:|---:|---|
| Clarabel.rs | yes | yes | Candidate for DC OPF |
| good_lp + microlp | yes | yes | LP path; backend choice matters |
| microlp | yes | yes | Maintained minilp fork |
| argmin | yes | yes | Experimental wasm support |
| basin | yes | yes | Young nonlinear least squares stack |
| OSQP wrapper | no | no clear path | C build path; avoid for browser wasm |

For sparse linear algebra, pure Rust libraries are the practical browser path.
faer has the broadest sparse factorization surface, but its wasm sparse path
needs validation on tellegen matrices. Clarabel's QDLDL factorization is the
near term KKT path for DC sensitivities.

| Library | Browser wasm | Sparse support |
|---|---:|---|
| faer | yes, needs validation | LU, LLT/LDLT, LBLT, QR |
| nalgebra | yes | dense only |
| nalgebra-sparse | yes | sparse Cholesky |
| Clarabel QDLDL | yes | symmetric quasidefinite LDLT |
| rsparse | likely | CSparse style LU and Cholesky |

Sources: [Clarabel issue 133](https://github.com/oxfordcontrol/Clarabel.rs/issues/133),
[faer](https://docs.rs/faer/latest/faer/),
[faer wasm sparse issue](https://github.com/sarah-quinones/faer-rs/issues/222),
[nalgebra wasm](https://www.nalgebra.org/docs/user_guide/wasm_and_embedded_targets/),
[Jangda et al.](https://ar5iv.labs.arxiv.org/html/1901.09056).

## tellegen frontend

### Svelte 5

Use `$derived` for computed values and `$effect` for effects such as network
calls and third party rendering. Use `$state.raw` for large immutable API
payloads so Svelte does not proxy every nested object. When packaging tellegen
as a library, put shared state behind context or explicit state objects rather
than module level globals.

Sources: [$state](https://svelte.dev/docs/svelte/$state),
[$effect](https://svelte.dev/docs/svelte/$effect),
[Svelte guidance](https://svelte.dev/docs/svelte/best-practices).

### Rendering

deck.gl and MapLibre are the current rendering stack. deck.gl documents 60 fps
for ScatterplotLayer sized near one million elements, with lower frame rates and
allocation limits at much larger sizes. The served grids are in the hundreds to
low thousands of buses. The relevant optimization path is binary typed array
attributes from wasm and `updateTriggers` for recoloring.

WebGPU support is not yet the default choice for this app because deck.gl's
WebGPU backend lacks picking and basemap interleaving needed here. luma.gl v9
keeps a later backend change possible.

Sources: [deck.gl performance](https://deck.gl/docs/developer-guide/performance),
[deck.gl WebGPU](https://deck.gl/docs/developer-guide/webgpu),
[WebGPU support](https://caniuse.com/webgpu).

### Packaging

The first reusable surface is `@tellegen/engine`, not a Svelte component
package. It exports case parsing, browser wasm solving, `Study` calls, and
generated contracts. A Svelte package can come later if map and panel components
become reusable outside the hosted demo layout. Do not import `$app/*` from
library code.

Ship the wasm asset with `new URL(..., import.meta.url)` or Vite `?url` so the
consumer fetches a file rather than inlining the module.

Sources: [Svelte packaging](https://svelte.dev/docs/kit/packaging),
[Vite assets](https://vite.dev/guide/features).

## tellegen backend

The tellegen backend uses Rust so the deployed runtime shares the parser,
solver, and sensitivity implementation with the browser WebAssembly module.
`axum` fits the API shape: JSON routes, shared immutable case state, SSE for
fallback solves, and static file serving through `tower-http`.

The engine is validated against the published PGLib reference solves (PowerModels.jl
with IPOPT) by the `benchmarks` crate; the Rust path is the production runtime.

## Julia to WebAssembly

PowerDiff.jl cannot be moved to the browser as written. It depends on JuMP,
Ipopt, and sparse linear algebra packages that do not have a supported Julia
browser WebAssembly path.

There are two separate efforts to track:

- whole runtime Julia through emscripten, represented by `julia-wasm`, with no
  recent active path in the checked sources;
- function level Julia to wasm compilation, represented by WasmTarget.jl and
  related experiments, with restrictions on dynamic dispatch, BLAS, allocation,
  and C or Fortran dependencies.

A browser solver for tellegen is therefore written in Rust, which has a mature wasm
compilation path, and validated against the published PGLib reference (PowerModels.jl).

Sources: [julia-wasm](https://github.com/Keno/julia-wasm),
[WasmTarget.jl](https://github.com/GroupTherapyOrg/WasmTarget.jl).

## Power systems software landscape

Most comparable tools are libraries, desktop applications, hosted data views, or
server backed simulation tools.

| Tool | Category | Browser interaction |
|---|---|---|
| pandapower | Python PF/OPF library | no browser app |
| PyPSA | Python linear PF and planning | dashboards, not live PF/OPF editing |
| PowerModels.jl, Sienna | Julia optimization libraries | programmatic |
| OpenDSS, GridLAB-D | distribution simulators | server or desktop workflows |
| MATPOWER | MATLAB/Octave PF/OPF | local MATLAB/Octave |
| GridCal / VeraGrid | Python desktop tool | static map export |
| GridStatus.io | ISO market data | published LMP maps, no recomputation |
| Electricity Maps | carbon and energy mix data | data view |
| GridSuite / PowSyBl | Java grid tools and browser apps | browser grid operations |
| ANDES / AGVis | Python dynamics and JS viewer | result inspection |

The gap relevant to tellegen is interactive recomputation: load a case, perturb
demand or generation, and inspect updated flows and prices. The shared
Rust/WebAssembly path runs DC OPF, AC power flow, and SOCWR in the browser, with
the tellegen backend providing bundled case data. The full nonlinear AC OPF stays
on the native roadmap because it needs threads.

## Open checks

- Add a hosted browser regression that solves a local wasm case end to end.
- Treat browser OPF landscape claims as public source observations, not proof
  that no private implementation exists.
