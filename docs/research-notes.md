# Ecosystem notes, June 2026

A point-in-time snapshot of the Rust/WebAssembly, Svelte, and Julia ecosystems
and the power systems software landscape, gathered to inform
[direction.md](direction.md). Versions and project status churn; treat dates and
maturity claims as of mid-2026. Items that could not be fully verified are
flagged at the end.

## Rust to WebAssembly

### Toolchain

The premise that wasm-pack stagnated is outdated. It went quiet for about 14
months, then shipped v0.14.0 (Jan 2026) and v0.15.0 (May 2026) under a new
maintainer. The "looking for maintainers" issue people cite is from 2020. The
rustwasm GitHub org was sunset in July 2025, but wasm-bindgen and wasm-pack moved
to a live `github.com/wasm-bindgen` org.

| Tool | Latest | Status | For |
|---|---|---|---|
| wasm-bindgen | 0.2.125 (2026-06-12) | Active (daxpedda, guybedford) | The Rust/JS/DOM binding layer (web-sys, js-sys) |
| wasm-pack | 0.15.0 (2026-05-15) | Active again | Wraps wasm-bindgen + wasm-opt, emits an npm package |
| trunk | 0.21.14 / 0.22.0-beta | Maintained, slow | Bundler for Rust web apps (Leptos/Yew/Dioxus), not npm libraries |
| cargo-component | 0.21.1 (2025-03) | Transitional | Component Model / WIT; superseded by the `wasm32-wasip2` target |
| jco | 1.23.0 (2026-06-11) | Active (Bytecode Alliance) | Transpiles wasm components to ES modules |

Decision rule: a browser library that touches the DOM uses wasm-bindgen +
wasm-pack. Server, edge, and plugin code uses the Component Model
(`wasm32-wasip2`), reaching JS through jco. The Component Model has no browser
DOM host and does not replace wasm-bindgen for tellegen's use. Sources:
[crates.io wasm-pack](https://crates.io/crates/wasm-pack),
[wasm-bindgen](https://github.com/wasm-bindgen/wasm-bindgen),
[rustwasm sunset](https://blog.rust-lang.org/inside-rust/2025/07/21/sunsetting-the-rustwasm-github-org/).

### Features and browser support

For numeric kernels only fixed-width SIMD and threads move the needle, and both
ship everywhere. The friction is a deployment-headers problem on threads.

| Feature | Chrome | Firefox | Safari | Production? | Relevance |
|---|---|---|---|---|---|
| simd128 | 91 | 89 | 16.4 | Yes, universal | Critical (vectorization) |
| Threads / SharedArrayBuffer | 74 | 79 | 14.1 | Yes, needs COOP/COEP | Critical (parallelism) |
| Bulk memory | 75 | 79 | 15 | Yes | Fast memcpy |
| Relaxed SIMD | 114 | 146 | none | No, Safari blocks | Optional FMA path; needs a simd128 fallback |
| memory64 | 133 | 134 | none | No, and slower | Avoid unless one allocation exceeds 4 GB |
| Tail calls | 112 | 121 | 18.2 | Yes (Baseline Dec 2024) | Irrelevant to numerics |
| WasmGC | 119 | 120 | 18.2 | Yes | Managed languages only; Rust uses linear memory |

Two findings matter for a static site. Threads need cross origin isolation via
two response headers (`COOP: same-origin`, `COEP: require-corp`); GitHub Pages
cannot set custom headers, so use
[coi-serviceworker](https://blog.tomayac.com/2025/03/08/setting-coop-coep-headers-on-static-hosting-like-github-pages/)
to synthesize them client side (or set headers directly on Cloudflare/Vercel/
Netlify/nginx). And memory64 is a regression, not an upgrade: SpiderMonkey
measured 64-bit wasm 10% to over 2x slower than 32-bit because 32-bit lets the
engine elide bounds checks. Stay on memory32. Sources:
[caniuse wasm-simd](https://caniuse.com/wasm-simd),
[wasm-threads](https://caniuse.com/wasm-threads),
[COOP/COEP](https://web.dev/articles/coop-coep),
[Memory64 analysis](https://spidermonkey.dev/blog/2025/01/15/is-memory64-actually-worth-using.html).

### In-browser convex optimization

DC OPF is a sparse LP/QP and has a production-ready path. AC power flow is a
sparse Newton iteration; every piece compiles, but nobody has shipped it.

| Solver | Pure Rust? | Browser wasm? | Maturity |
|---|---|---|---|
| Clarabel.rs (LP/QP/SOCP, IPM) | Yes (QDLDL path) | Yes, user-confirmed | Mature. Best fit for DC OPF. |
| good_lp + microlp or Clarabel | Yes (those backends) | Yes | Mature. CBC/HiGHS/SCIP backends do not compile to wasm. |
| microlp (LP/MILP) | Yes | Yes | Maintained fork of archived minilp |
| argmin (Newton/BFGS/L-BFGS) | Yes | Yes, needs the wasm-bindgen feature, self-labeled experimental | For an AC Newton loop |
| basin (Gauss-Newton, LM, TR) | Yes | Yes, wasm-first | Young |
| OSQP (osqp.rs) | No, wraps C OSQP | Unlikely (C build script); emscripten port abandoned 2021 | Avoid for this path |

Clarabel.rs has had wasm support since v0.9.0 (Jan 2024); its default linear
solver is QDLDL, pure Rust with no BLAS, and a user reported solving a
1000-variable box QP in the browser
([issue #133](https://github.com/oxfordcontrol/Clarabel.rs/issues/133)), the same
shape as a small DC OPF. Ready-made conic solvers compiled through emscripten
also exist: [scs.wasm](https://github.com/DominikPeters/scs.wasm) (npm
`scs-solver`) and clp-wasm. A native Rust power flow library exists
([rustpower](https://github.com/chengts95/rustpower), Newton-Raphson, reads
pandapower JSON) but with no evidence of a wasm build. Flag: Clarabel has no
wasm job in CI; its wasm support rests on a source-level target table plus the
one confirmed user build.

### Linear algebra in wasm

There is no clean `wasm32-unknown-unknown` BLAS or LAPACK; every browser BLAS
routes through emscripten and runs on one thread without SIMD, which defeats the
purpose. Pure Rust is the realistic path.

| Library | wasm? | Sparse factorization |
|---|---|---|
| faer | Yes (with caveat below) | Sparse LU, LLT/LDLT Cholesky, LBLT, QR. Most complete. |
| nalgebra | Yes, proven (ships in Rapier) | Dense only |
| nalgebra-sparse | Yes | Sparse Cholesky, no sparse LU |
| Clarabel QDLDL | Yes | Symmetric quasidefinite LDLT, fits OPF KKT systems |
| rsparse | Pure Rust, should build | CSparse-style LU and Cholesky |

Expect roughly 1.5x native on average and up to 2.5x peak (Jangda et al., USENIX
ATC 2019), a conservative envelope that predates stable wasm SIMD; dimforge
reports 2026 wasm physics builds run 2 to 5x faster than their 2024 builds,
mostly from SIMD. faer caveat to validate before relying on it:
[faer-rs#222](https://github.com/sarah-quinones/faer-rs/issues/222) reports a
sparse Cholesky crash on wasm32, reproduced in Firefox and through wasm-pack,
closed with no visible fix commit. faer has no wasm CI. Sources:
[faer](https://docs.rs/faer/latest/faer/),
[nalgebra wasm](https://www.nalgebra.org/docs/user_guide/wasm_and_embedded_targets/),
[Jangda et al.](https://ar5iv.labs.arxiv.org/html/1901.09056).

### Real Rust to wasm numeric projects

Proof that heavy compute runs in the browser this way:
[Rapier](https://rapier.rs/) (dimforge physics, npm `@dimforge/rapier3d`),
[candle](https://github.com/huggingface/candle) (Hugging Face ML inference, live
Whisper/LLaMA2/YOLO demos),
[ten-minute-physics-rs](https://github.com/lucas-schuermann/ten-minute-physics-rs)
(XPBD, about 3x the JS original),
[Photon](https://silvia-odwyer.github.io/photon/) (image processing). For dense
work, GPU compute through wgpu + WebGPU (Bevy, CubeCL, Burn-wgpu) is an
alternative to CPU wasm. Note for contrast: in-browser SPICE (EEcircuit) is C
through emscripten, Figma is C++, and Fornjot (Rust CAD) is native, so none are
Rust-to-wasm exemplars.

## Frontend: Svelte 5 and visualization

### Runes

Compute a value with `$derived`; perform a side effect (canvas, network, a third
party library) with `$effect`. The docs warn against updating state inside
effects. Use `$state.raw` for large immutable payloads: plain `$state` deep
proxies arrays and objects so it can react to mutation, and that proxy cost is
what janks on large data; raw state reacts only to reassignment. tellegen's
existing `$state.raw` on API payloads is the documented pattern. For new code use
runes, and share reactivity through a class with `$state` fields rather than
stores. One hard rule: a module cannot export a reassignable `$state` primitive
(importers will not see the rebinding); export an object or class instance, or
getters. For SSR safety the docs prefer context over module-level globals, which
otherwise leak state between requests. Sources:
[$state](https://svelte.dev/docs/svelte/$state),
[$effect](https://svelte.dev/docs/svelte/$effect),
[best practices](https://svelte.dev/docs/svelte/best-practices).

### Large network rendering (deck.gl + MapLibre)

The official performance guide: ScatterplotLayer and similar render at 60fps to
about 1M items, dropping to 10 to 20fps near 10M; Chrome caps a single
allocation at 1 GB, which crashes buffer generation between 10M and 100M items,
past which you split across layers. Levers, all first party: hand deck.gl
precomputed binary typed-array attributes (the highest-throughput path, and the
natural target for a wasm pipeline), use `updateTriggers` to recolor without
rebuilding positions, and stream with async iterables. Power grids run at ten
thousand to a hundred thousand elements, one to two orders of magnitude under the
60fps envelope, so deck.gl + MapLibre on WebGL2 is the right stack with large
headroom; the bottleneck is the wasm-to-buffer marshalling, not the GPU.
Alternatives win only off the geographic case:
[cosmos.gl](https://openjsf.org/blog/introducing-cosmos-gl) for GPU graph layout
of a million-plus abstract nodes, [Sigma.js](https://www.sigmajs.org/) +
graphology for abstract network analysis, regl/PixiJS/raw WebGPU as lower-level
escape hatches. Source:
[deck.gl performance](https://deck.gl/docs/developer-guide/performance).

### WebGPU

Chrome and Edge shipped at v113; Safari 26 (Sept 2025); Firefox on Windows 141
and macOS ARM64 145, with Linux, Android, and Intel Macs in progress. Global
support is about 82%. It is not Baseline as of June 2026, contrary to several
blog posts. deck.gl's WebGPU backend is "not production ready": the layers
tellegen uses are ported, but picking and basemap interleaving are not, and both
are required here. luma.gl v9 makes a later switch a backend flip. Ship WebGL2
now. Sources: [caniuse webgpu](https://caniuse.com/webgpu),
[deck.gl WebGPU](https://deck.gl/docs/developer-guide/webgpu).

### Packaging as a library

Use [`@sveltejs/package`](https://svelte.dev/docs/kit/packaging): `src/lib` is the
public surface, it emits `.d.ts` and copies non-JS files (the wasm) verbatim into
`dist`. In `package.json` set `exports` with `svelte` and `types` conditions,
`files: ["dist"]`, `sideEffects` for CSS, and `peerDependencies` for `svelte`,
`deck.gl`, `maplibre-gl` so a downstream app dedupes one copy; bundle only your
glue plus the wasm. Do not import `$app/*` in library code (it couples consumers
to SvelteKit); use `esm-env` for environment checks and take URLs and data as
props or context. Stay SSR safe by guarding deck.gl and MapLibre behind
`onMount` or dynamic import. The dominant theming pattern is a headless core with
CSS-variable or Tailwind theming (Bits UI, shadcn-svelte). Ship the wasm with
`new URL('./core_bg.wasm', import.meta.url)` or Vite `?url` so it is emitted as a
fetched asset rather than inlined; `vite-plugin-wasm` is optional and forcing it
on downstream apps is avoidable. Sources:
[packaging](https://svelte.dev/docs/kit/packaging),
[Vite features](https://vite.dev/guide/features).

## Backend: Julia web and wasm

### Web frameworks

[Oxygen.jl](https://github.com/OxygenFramework/Oxygen.jl) (v1.10.2) is a
FastAPI-style micro framework on HTTP.jl with OpenAPI docs, multithreading,
websockets, and SSE; it fits a thin API in front of a solver, with the caveat
that it is essentially a single-maintainer project.
[Genie.jl](https://github.com/GenieFramework/Genie.jl) (v6.0.0) plus
GenieFramework (Stipple reactive UI, Genie Builder on JuliaHub) is the full-stack
option, worth its weight only for a server-rendered reactive UI, which tellegen
does not need given SvelteKit.
[HTTP.jl](https://github.com/JuliaWeb/HTTP.jl) (v2.2.0) is the shared foundation.
The known weak spot is concurrency throughput: a 2023 community benchmark put
HTTP.jl far behind FastAPI and Fastify under load (at 256 connections, 244.96ms
vs FastAPI's 3.73ms), which matters only for many users issuing many small
requests, not a few users each triggering one heavy solve. The circulating "10x
faster than Flask" claim is a single cherry-picked compute endpoint; do not rely
on it.

### Startup and static compilation

Interactive latency is largely solved for a long-running server, and the wins
landed in Julia 1.9 and 1.10, not 1.12. Package images (1.9) cache native code to
disk (the highlights report first-execution speedups of 137x for CSV, 46x for
DataFrames vs 1.7); 1.10 cut load time more than 2x. For a long-running server
you pay compilation once at boot. Bake hot paths in with PrecompileTools
`@compile_workload` (it also caches callees reached through runtime dispatch,
which matters for JuMP and solver code), and optionally a PackageCompiler
sysimage. `juliac` and `--trim` (1.12, Oct 2025) produce small native binaries
but are experimental; the 1.12 NEWS says "not all code is expected to work," and
a Dec 2025 hands-on review concluded "definitely don't use trimming in production
code," with breakage on stdout, exceptions, and any dynamic dispatch. Maturity is
"within the next few years." This is irrelevant to a long-running tellegen
server. Sources:
[1.9 highlights](https://julialang.org/blog/2023/04/julia-1.9-highlights/),
[PrecompileTools](https://julialang.github.io/PrecompileTools.jl/stable/),
[1.12 review](https://viralinstruction.com/posts/aoc2025/).

### Julia to wasm

There is no official supported target. Two approaches, not to be conflated.
Shipping the whole runtime through emscripten ([Keno/julia-wasm](https://github.com/Keno/julia-wasm))
is dead, last commit Nov 2021; the blocker is that Julia's value is its JIT, so
you would ship the compiler into the browser. This is why Pyodide works for
Python (an interpreter, no JIT to port) and not for Julia. Compiling individual
type-stable functions with no runtime is the only active path, and it inherits
every static-compilation restriction (no GC from libjulia, no dynamic dispatch,
no BLAS or C dependencies): WebAssemblyCompiler.jl (stalled Feb 2024, no
multidimensional arrays, no BLAS),
[WasmTarget.jl](https://github.com/GroupTherapyOrg/WasmTarget.jl) (v0.3.8 June
2026, about 14 stars, emits WasmGC from typed IR, no runtime dispatch), and
StaticCompiler.jl (native, not wasm). WasmGC now ships in all browsers and Wasm
3.0 was finalized Sept 2025, which is what lets the function-level compilers lean
on browser GC.

Bottom line: a hand-written, type-stable, allocation-light kernel with no BLAS
can compile to wasm today, as an early adopter of a 14-star project. PowerDiff.jl
as it exists (JuMP builds a model, Ipopt solves it, sparse linear algebra for the
KKT sensitivities) running unmodified in the browser is not feasible for years,
if ever, because JuMP, Ipopt, and SuiteSparse are C and Fortran dependencies no
Julia wasm tool wires up. A client-side interior point OPF comes from compiling
C or Rust to wasm, the way Pyodide ships SciPy, not from porting Julia.

## Power systems software landscape

Almost everything is a library, a desktop GUI, or a read-only data dashboard.
Browser-native apps that load a grid, run a solver, and inspect results are rare.

| Tool | What it is | Interactive web sim/viz? |
|---|---|---|
| pandapower | Python PF/OPF/short-circuit library | No (library; community GUIs are stale desktop Qt) |
| PyPSA, -Eur, -Earth | Python linear PF + capacity expansion | Partial (model.energy is browser but not PF; Dash dashboards) |
| PowerModels.jl, Sienna (NREL) | Julia OPF + operations libraries | No (programmatic) |
| OpenDSS (EPRI) | Distribution simulator, COM app + ports | No official browser port |
| GridLAB-D (PNNL) / Arras | Distribution simulator CLI | Yes, via GLOW (a web GUI over the CLI) |
| MATPOWER | MATLAB/Octave PF/OPF | No (needs local MATLAB/Octave) |
| PowerWorld + SimAuto/ESA | Commercial Windows desktop | No (desktop; ESA is Python automation) |
| GridCal / VeraGrid | Python tool, Qt desktop | No (exports static Leaflet maps) |
| GridStatus.io | Real-time ISO market and grid data | No sim; clickable nodal LMP maps of ISO data |
| Electricity Maps | Real-time carbon and energy mix map | No sim; clickable inter-region flows |
| Breakthrough Energy PowerSimData/REISE | Production-cost sim on an 80k-bus US grid | No; the public map is data viz, server-side sim |
| NREL HELICS/reV/REopt | Co-sim engine + geospatial/market models | Mixed; REopt has a hosted optimizer, not transmission PF |
| PowSyBl (RTE) / GridSuite | Java grid library + browser apps | Yes; GridSuite is in production at RTE, demo.gridsuite.org |
| ANDES + CURENT LTB / AGVis | Python PF/dynamics + JS geo-viz of results | Partial; viewer of sim results, not click-to-perturb |

### Closest to tellegen's pitch, and the gap

No public tool today loads a real case, lets you click a bus, drag demand, and
watches flows, voltages, and prices re-solve and re-render live in the browser.
The nearest analogues each miss one axis: [Electrisim](https://electrisim.com/)
runs pandapower and OpenDSS in the browser but server side, the loop is
build-submit-view, and it reports no LMPs; [GridSuite](https://www.gridsuite.org/)
is the most production-grade browser grid app but closed operator software with
no reactive price map; [GridStatus.io](https://www.gridstatus.io/) has the
polished nodal price map but shows ISO-published prices you cannot recompute;
[PowerPlots.jl](https://wispo-pop.github.io/PowerPlots.jl/dev/) is hover-to-inspect
driven from Julia. Falstad CircuitJS1 proves real-time reactive physics runs
fully in browser, but for electronics. The open lane is the combination of a real
solver, real coordinates, client-side recompute, and a live LMP/voltage/flow
render, with the perturb-and-recompute loop as the primitive. tellegen's exact
differentiator is the live LMP map driven by the user's own demand and generation
edits, backed by exact KKT sensitivities, which is what PowerDiff.jl already
computes and nobody exposes interactively.

### Compute placement verdict

Keep the heavy exact solver in Julia on the server for now, and run a hybrid: the
browser owns interaction and cheap math (a PTDF or sensitivity matrix-vector
preview), the server owns the exact solve, reconciled in parallel. A DC OPF
solves in single-digit to low-tens of milliseconds server side, and a same
continent round trip adds about 20 to 80ms, fast enough to feel live. The Julia
to wasm timeline does not support moving PowerDiff.jl into the browser; if a fully
client-side solver is wanted, write it in Rust (see [direction.md](direction.md)
and issue #2), not by porting Julia.

## Flags and unverified items

- faer-rs#222 (sparse Cholesky on wasm) is closed with no visible fix; validate
  faer's sparse path on real case matrices before relying on it.
- Clarabel has no wasm CI; argmin's wasm support is self-labeled experimental.
- No public precedent exists for in-browser power flow or OPF via Rust to wasm.
- The 1.5 to 3x native multiplier is a 2019 figure, conservative and pre-SIMD.
- WebGPU is not Baseline as of June 2026 (Firefox Linux/Android/Intel-Mac and
  Safari desktop gaps remain), contrary to some 2026 blog posts.
- "Does not exist" claims about client-side OPF rest on absence of evidence in
  public search, not proof of absence; vendor-internal or unpublished tools
  cannot be ruled out.
- Oxygen and Genie adoption is measured in GitHub stars only; no download counts
  or named production users were confirmed.
- WasmTarget.jl metrics are churning fast; v0.3.8 is confirmed from the repo, but
  its real capability on matrix linear algebra is unproven.
