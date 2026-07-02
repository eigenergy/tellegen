# Architecture

tellegen is a differentiable power flow and optimal power flow engine, written in Rust and compiled to both native targets and WebAssembly. The public browser framework packages are `@tellegen/engine` and `@tellegen/svelte`; the SvelteKit hosted demo is one private consumer of them.

## Repository layout

A Cargo workspace and a web app, side by side.

- `crates/tellegen` — the engine. It parses a case (through powerio) and solves any formulation, returning a formulation-agnostic result and analytical sensitivities.
- `crates/tellegen-wasm` — the `wasm-bindgen` adapter that exposes the engine to the browser, built with `wasm-pack`.
- `crates/tellegen-server` — a native HTTP server that serves the bundled cases and the static app.
- `crates/tellegen-cli` — a command-line front end over the engine's JSON API.
- `crates/benchmarks` — a non-shipping harness that runs the PGLib-OPF corpus for validation and timing.
- `packages/engine` — the public browser engine package, generated TypeScript contracts, and browser wasm transport.
- `packages/svelte` — the public Svelte component package for maps, panels, local case files, and browser solves.
- `apps/web` — the private SvelteKit hosted demo that consumes `@tellegen/svelte`.
- `examples/browser-minimal` — a minimal downstream app that imports `@tellegen/engine` directly.
- `examples/svelte-minimal` — a minimal downstream app that imports `@tellegen/svelte`.

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

## Browser framework packages

`packages/engine` is the reusable package surface. It exports generated
contracts, case and display parsing helpers, stateless solve calls, capabilities,
the browser `Study`, and the browser wasm transport. It has no SvelteKit
dependency.

`packages/svelte` consumes `@tellegen/engine` and exports the map, panels, local
file flow, solve card, state provider, and full viewer as Svelte components.

`apps/web` consumes the Svelte package and keeps demo concerns: routes, SEO,
credits, privacy, deployment, and bundled case pages.

## In the browser

`@tellegen/engine` ships one wasm package built from `crates/tellegen-wasm`
(the `conic` feature): DC power flow, DC OPF, AC power flow, SOCWR, the
`Study`, and the sensitivity columns. A browser that cannot load it does not
solve; the hosted demo shows a notice, and the server's compute endpoints
exist as an opt-in fallback (`TELLEGEN_SERVER_COMPUTE`).

The Svelte package and the hosted app use the same `Study` loop: a drag calls `preview` (a first-order LMP and objective update, in WebAssembly, no server round-trip) and release calls `commit` (an exact re-solve that also returns the displayed sensitivity column). Every formulation solves in the browser; dropped-in case files solve there too and are never uploaded.

## Sources

- Rust to WebAssembly: [wasm-bindgen](https://github.com/wasm-bindgen/wasm-bindgen), [wasm-pack](https://crates.io/crates/wasm-pack)
- Solvers and linear algebra: [Clarabel.rs](https://github.com/oxfordcontrol/Clarabel.rs), [faer](https://docs.rs/faer/latest/faer/)
- Convex relaxation: R. A. Jabr, "Radial distribution load flow using conic programming," IEEE Transactions on Power Systems, 21(3), 2006.
- Svelte: [`$state`](https://svelte.dev/docs/svelte/$state)
