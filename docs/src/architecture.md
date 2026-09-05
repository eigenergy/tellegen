# Architecture

tellegen is a differentiable power flow and optimal power flow engine written
in Rust for native targets and WebAssembly. The public browser packages are
`@tellegen/engine`, `@tellegen/svelte`, and `@tellegen/webmcp`; the SvelteKit
hosted demo is one private consumer.

## Repository layout

A Cargo workspace and a web app, side by side.

- `crates/tellegen`: the engine. It parses a case through powerio and solves the requested supported formulation. One result envelope carries the fields and sensitivities that formulation defines.
- `crates/tellegen-wasm`: the `wasm-bindgen` adapter that exposes the engine to the browser, built with `wasm-pack`.
- `crates/tellegen-server`: a native HTTP server that serves the bundled cases and the static app.
- `crates/tellegen-cli`: a command line front end over the engine's JSON API.
- `crates/benchmarks`: a private harness that runs the PGLib-OPF corpus for validation and timing.
- `packages/engine`: the public browser engine package, generated TypeScript contracts, and browser wasm transport.
- `packages/svelte`: the public Svelte component package for maps, panels, local case files, and browser solves.
- `packages/webmcp`: WebMCP tool definitions, validation, and registration lifecycle.
- `apps/web`: the private SvelteKit hosted demo that consumes `@tellegen/svelte`.
- `examples/browser-minimal`: a minimal downstream app that imports `@tellegen/engine` directly.
- `examples/svelte-minimal`: a minimal downstream app that imports `@tellegen/svelte`.

powerio owns parsing and the network and display formats; the engine and the app depend on it.

## The engine

`crates/tellegen` solves four formulations through one interface:

- **DC power flow** and **DC OPF**: a B–θ linear/quadratic program;
- **AC power flow**: a polar Newton solve; and
- **SOCWR**: the Jabr second-order cone relaxation of AC OPF, in W-space.

The formulations share one result envelope, but they do not claim the same
quantities. DC power flow returns angles and branch flows without prices,
optimized dispatch, or sensitivities. DC OPF adds dispatch, objective value,
LMPs, and KKT sensitivities. AC power flow returns voltages and nodal
injections with Newton sensitivities. SOCWR exposes the quantities and conic
KKT sensitivities defined by the relaxation. Formulations that implement the
`Differentiable` contract accept an output `Operand` and input `Parameter`;
the common driver solves the retained KKT or Newton system for the requested
forward or adjoint columns.

The engine is Rust and compiles to WebAssembly, so the same code runs natively
and in the browser. The convex solves use Clarabel; the sensitivities use faer.
The full nonlinear AC OPF is on the
[desktop and mobile roadmap](tauri-roadmap.md). Its planned interior point
solver uses threads; the current browser solver build is single threaded.

## The two API faces

One numerical core, two faces that share a driver and a result type:

- **Stateless**: `solve_module(module, request)` and `capabilities_json()`. Each
  call reads a PowerIO module, solves its declared problem instance, and
  returns.
- **Stateful**: the `Study`. It starts from a PowerIO module and builds the
  private solver workspace once. `commit` applies a set of `NetworkEdit`s and
  re-solves exactly, optionally returning the requested sensitivity columns in
  the same solve; `preview` returns a first order update at the committed point
  with no re-solve.

## Browser packages

`packages/engine` is the reusable package surface. It exports generated
contracts, case and display parsing helpers, module based solves, capabilities,
the browser `Study`, and the browser wasm transport. It has no SvelteKit
dependency.

`packages/svelte` consumes `@tellegen/engine` and exports the map, panels, local
file flow, solve card, state provider, and full viewer as Svelte components.

`packages/webmcp` exports tool definitions, runtime validation, and registration
through an adapter that does not depend on Svelte or the engine package.

`apps/web` consumes the Svelte package and keeps demo concerns: routes, SEO,
credits, privacy, deployment, and bundled case pages.

## In the browser

`@tellegen/engine` ships one wasm package built from `crates/tellegen-wasm`
(the `conic` feature): DC power flow, DC OPF, AC power flow, SOCWR, the
`Study`, and the sensitivity columns. A browser that cannot load it does not
solve; the hosted demo shows a notice, and the server's compute endpoints
exist as an opt-in fallback (`TELLEGEN_SERVER_COMPUTE`).

The Svelte package and the hosted app use `Study` for DC OPF, AC power flow,
and SOCWR. `preview` returns a first order update for the quantities the chosen
formulation defines; `commit` performs an exact re-solve and can return the
displayed sensitivity column. DC power flow uses the stateless solve path, and
full nonlinear AC OPF is unavailable. Supported solvable case files run in the
browser and are not uploaded.

## Sources

- Rust to WebAssembly: [wasm-bindgen](https://github.com/wasm-bindgen/wasm-bindgen), [wasm-pack](https://crates.io/crates/wasm-pack)
- Solvers and linear algebra: [Clarabel.rs](https://github.com/oxfordcontrol/Clarabel.rs), [faer](https://docs.rs/faer/latest/faer/)
- Convex relaxation: R. A. Jabr, "Radial distribution load flow using conic programming," IEEE Transactions on Power Systems, 21(3), 2006.
- Svelte: [`$state`](https://svelte.dev/docs/svelte/$state)
