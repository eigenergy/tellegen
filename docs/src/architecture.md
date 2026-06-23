# Architecture

tellegen is a differentiable optimal power flow and power flow engine, written in
Rust and compiled to both native targets and WebAssembly, with a SvelteKit web app
that runs the engine in the browser. This chapter records the realized architecture
of the repository.

## Repository layout

The repository is a Cargo workspace and a web app side by side.

- `crates/tellegen` — the engine. It parses a case (through powerio) and solves any of
  five formulations, returning a formulation-agnostic result and analytical sensitivities.
- `crates/tellegen-wasm` — the `wasm-bindgen` adapter that exposes the engine to the
  browser, built with `wasm-pack`.
- `crates/tellegen-server` — a native HTTP server that serves the bundled cases and the
  static single-page app.
- `crates/tellegen-cli` — a command-line front end over the engine's stateless JSON API.
- `crates/benchmarks` — a non-shipping harness that runs the PGLib-OPF corpus for
  validation and timing. It is the only crate that enables the optional EPL-licensed
  AC-OPF backend (see [Licensing](#native-solving-and-licensing)).
- `apps/web` — the SvelteKit application, built as a static single-page app.
- `packages/` — reserved for a shared TypeScript wrapper over the wasm packages, to be
  populated once a second consumer (a desktop app) exists.

powerio owns parsing, encoding, and the network and display formats; the engine and the
app depend on it rather than re-implementing format support.

## The engine

`crates/tellegen` solves five formulations through one interface:

- **DC power flow** and **DC OPF** — a B–θ linear/quadratic program;
- **AC power flow** — a polar Newton solve;
- the **SOCWR** relaxation — the Jabr second-order-cone relaxation of AC OPF in W-space; and
- the **full nonlinear AC OPF** — a polar interior-point program.

Every formulation returns the same result shape — locational marginal prices, voltages,
branch flows, and generator dispatch — and exposes analytical **sensitivities** of any
output (an `Operand`) with respect to any input (a `Parameter`) through a single
implicit-differentiation contract, `Differentiable`. Each solved formulation builds its
own KKT or Newton system; the sensitivity driver solves that system, forward or adjoint,
for the requested columns. Adding a formulation, operand, or parameter is a matter of
implementing the contract, not of special-casing the callers.

Every formulation is pure Rust and compiles to WebAssembly, so the same code runs
natively and in the browser — including the full nonlinear AC OPF, whose interior-point
backend (the `interiors` crate) is pure Rust. The browser solves all five formulations;
nothing is offloaded to a server.

## The two API faces

The engine exposes its numerical core behind two faces that share one driver and one set
of result types.

- **Stateless** — `solve_json(network, request)` and `capabilities_json()`. Each call is
  independent: it parses the network, solves, and returns. This is the face for one-shot
  callers — the HTTP server, the CLI, fixtures, and the initial case load.
- **Stateful** — the `Study`. It builds the model once and holds it. `commit` applies a
  set of `NetworkEdit`s and re-solves exactly — the source of truth — optionally returning
  requested sensitivity columns in the same solve. `preview` returns a first-order
  linearization at the committed point with no re-solve. The build-once handle is the
  reactive hot path: a demand drag previews and commits without re-parsing the network
  every frame.

Because both faces share the driver and types, the server, the browser, and the CLI speak
one contract.

## In the browser

`crates/tellegen-wasm` is built in two tiers:

- a **full** package, built with the `acopf` feature (`faer`, `simd128`, no relaxed-SIMD),
  which carries all five formulations, the `Study`, and the analytical sensitivity columns,
  and loads on current browsers, Safari 16.4+ included; and
- a **core** package, built `--no-default-features` with `simd128` and `relaxed-simd`
  disabled — a smaller DC-only fallback that loads on any WebAssembly-capable browser.

The app's reactive loop uses the `Study`: a drag calls `preview` (a first-order LMP and
objective update, in WebAssembly, with no server round-trip) and release calls `commit`
(an exact re-solve that also returns the displayed sensitivity column). DC OPF, SOCWR, and
the full nonlinear AC OPF all solve in the browser; case files dropped into the app solve
in the browser and are never uploaded.

## The AC-OPF backends and licensing

The full nonlinear AC OPF has two backends: a default interior-point program (the
`interiors` crate, pure Rust) under the engine's own Apache-2.0/MIT license — this is the
one compiled into the browser — and an optional faster backend behind the `acopf-pounce`
feature that links EPL-2.0 dependencies. Only the `benchmarks` crate enables
`acopf-pounce`; the shipped engine, wasm adapter, server, and CLI remain Apache-2.0/MIT,
and CI fails the build if the EPL dependencies ever appear in them. The attribution and the
per-feature licensing are recorded in `crates/tellegen/NOTICE`.

## Sources

- Rust to WebAssembly:
  [wasm-bindgen](https://github.com/wasm-bindgen/wasm-bindgen),
  [wasm-pack](https://crates.io/crates/wasm-pack)
- Browser wasm features:
  [wasm SIMD](https://caniuse.com/wasm-simd),
  [COOP/COEP](https://web.dev/articles/coop-coep)
- Solvers and linear algebra:
  [Clarabel.rs](https://github.com/oxfordcontrol/Clarabel.rs),
  [faer](https://docs.rs/faer/latest/faer/)
- Convex relaxation: R. A. Jabr, "Radial distribution load flow using conic
  programming," IEEE Transactions on Power Systems, 21(3), 2006.
- Svelte: [`$state`](https://svelte.dev/docs/svelte/$state)
