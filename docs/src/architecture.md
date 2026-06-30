# Architecture

tellegen is a differentiable power flow and optimal power flow engine, written in Rust and compiled to both native targets and WebAssembly, with a SvelteKit app that runs the engine in the browser.

## Repository layout

A Cargo workspace and a web app, side by side.

- `crates/tellegen` — the engine. It parses a case (through powerio) and solves any formulation, returning a formulation-agnostic result and analytical sensitivities.
- `crates/tellegen-wasm` — the `wasm-bindgen` adapter that exposes the engine to the browser, built with `wasm-pack`.
- `crates/tellegen-server` — a native HTTP server that serves the bundled cases and the static app.
- `crates/tellegen-cli` — a command-line front end over the engine's JSON API.
- `crates/benchmarks` — a non-shipping harness that runs the PGLib-OPF corpus for validation and timing.
- `apps/web` — the `tellegen-frontend` Svelte package and its static demo app.

powerio owns parsing and the network and display formats; the engine and the app depend on it.

## The engine

`crates/tellegen` solves four formulations through one interface:

- **DC power flow** and **DC OPF** — a B–θ linear/quadratic program;
- **AC power flow** — a polar Newton solve; and
- **SOCWR** — the Jabr second-order cone relaxation of AC OPF, in W-space.

Every formulation returns the same result shape — locational marginal prices, voltages, branch flows, and dispatch — and exposes analytical **sensitivities** of any output (an `Operand`) with respect to any input (a `Parameter`) through one implicit-differentiation contract, `Differentiable`. Each solved formulation builds its KKT or Newton system; the sensitivity driver solves that system, forward or adjoint, for the requested columns. Adding a formulation, operand, or parameter means implementing the contract, not special-casing the callers.

The whole engine is pure Rust and compiles to WebAssembly, so the same code runs natively and in the browser. The convex solves use Clarabel; the sensitivities use faer. The full nonlinear AC OPF (an interior-point program) is on the [desktop and mobile roadmap](tauri-roadmap.md): it parallelizes across threads, which the browser does not have.

## The two API faces

One numerical core, two faces that share a driver and a result type:

- **Stateless** — `solve_json(network, request)` and `capabilities_json()`. Each call parses, solves, and returns. This is the face for one-shot callers: the HTTP server, the CLI, fixtures, and the initial case load.
- **Stateful** — the `Study`. It builds the model once. `commit` applies a set of `NetworkEdit`s and re-solves exactly, optionally returning the requested sensitivity columns in the same solve; `preview` returns a first-order update at the committed point with no re-solve. This is the reactive hot path: a demand drag previews and commits without re-parsing the network every frame.

## Frontend package

`apps/web/src/lib` is the reusable package surface. It exports the map, the demo
components, the app state and controller, typed API shapes, and the browser wasm
wrapper through `@sveltejs/package`. `apps/web/src/routes` is the reference demo
that consumes those library modules.

The package peers on Svelte, deck.gl, and MapLibre. Its wasm wrapper imports the
generated wasm files with `?url`, so consuming Vite and SvelteKit apps can serve
the `.wasm` assets through their normal asset pipeline.

## In the browser

`crates/tellegen-wasm` ships two packages:

- a **full** package (the `conic` feature) carrying DC power flow, DC OPF, AC power flow, SOCWR, the `Study`, and the sensitivity columns; and
- a **core** package (`--no-default-features`, SIMD disabled) — a smaller DC-only fallback that loads on any WebAssembly-capable browser.

The app's reactive loop runs on the `Study`: a drag calls `preview` (a first-order LMP and objective update, in WebAssembly, no server round-trip) and release calls `commit` (an exact re-solve that also returns the displayed sensitivity column). Every formulation solves in the browser; dropped-in case files solve there too and are never uploaded.

## Sources

- Rust to WebAssembly: [wasm-bindgen](https://github.com/wasm-bindgen/wasm-bindgen), [wasm-pack](https://crates.io/crates/wasm-pack)
- Solvers and linear algebra: [Clarabel.rs](https://github.com/oxfordcontrol/Clarabel.rs), [faer](https://docs.rs/faer/latest/faer/)
- Convex relaxation: R. A. Jabr, "Radial distribution load flow using conic programming," IEEE Transactions on Power Systems, 21(3), 2006.
- Svelte: [`$state`](https://svelte.dev/docs/svelte/$state)
