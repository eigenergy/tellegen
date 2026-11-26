# Architecture

tellegen is a differentiable power flow and optimal power flow engine written
in Rust for native targets and WebAssembly. The public browser packages are
`@tellegen/engine`, `@tellegen/svelte`, and `@tellegen/webmcp`; the SvelteKit
hosted demo is one private consumer.

## Repository layout

A Cargo workspace and a web app, side by side.

- `crates/tellegen`: the engine. It parses a case through powerio and solves the requested supported formulation. The result carries the fields and sensitivities that formulation defines.
- `crates/tellegen-wasm`: the `wasm-bindgen` adapter that exposes the engine to the browser, built with `wasm-pack`.
- `crates/tellegen-server`: a native HTTP server that serves the bundled cases and the static app.
- `crates/tellegen-cli`: a command line front end over the engine's JSON API.
- `crates/benchmarks`: a private harness that runs the PGLib-OPF corpus for validation and timing.
- `packages/engine`: the public browser engine package, TypeScript types generated from Rust, and browser wasm transport.
- `packages/svelte`: the public Svelte component package for maps, panels, local case files, and browser solves.
- `packages/webmcp`: WebMCP tool definitions, validation, and registration lifecycle.
- `apps/web`: the private SvelteKit hosted demo that consumes `@tellegen/svelte`.
- `examples/browser-minimal`: a minimal downstream app that imports `@tellegen/engine` directly.
- `examples/svelte-minimal`: a minimal downstream app that imports `@tellegen/svelte`.

powerio owns parsing and the network and display formats; the engine and the app depend on it.

## The engine

The balanced-network interface supports these formulations:

- **DC power flow** and **DC OPF**: a B–θ linear/quadratic program;
- **AC power flow**: a polar Newton solve; and
- **SOCWR**: the Jabr second-order cone relaxation of AC OPF, in W-space,
  enabled by the `conic` feature.

The optional native `acopf` feature adds exact polar AC OPF through POUNCE.
It reports independently checked primal results, LMPs, and selected derivatives
with respect to demand and line ratings. Derivatives require a suitable active
set; the API reports when degeneracy makes them unavailable. The browser runs
this calculation in a separate WASI worker. Its module, source information and
license notices are released separately from the ordinary engine package.
The host must provide its URL and successfully probe it before offering AC OPF.

Multiconductor power flow is enabled by the default `mc-pf` feature. The
optional native `mc-opf` feature provides multiconductor optimization; the
ordinary browser, HTTP and Python adapters do not enable it.

The formulations share one result structure, but they do not claim the same
quantities. DC power flow returns angles and branch flows without prices,
optimized dispatch, or sensitivities. DC OPF adds dispatch, objective value,
LMPs, and KKT sensitivities. AC power flow returns voltages and nodal
injections with Newton sensitivities. SOCWR exposes the quantities and conic
KKT sensitivities defined by the relaxation. Formulations that implement the
`Differentiable` trait accept an output `Operand` and input `Parameter`;
the common driver solves the retained KKT or Newton system for the requested
forward or adjoint columns.

The default engine is Rust and compiles to WebAssembly, so the same code runs
natively and in the browser. The convex solves use Clarabel; the sensitivities
use faer. The Python wheel enables exact AC OPF. Native Rust callers enable the
`acopf` feature, and the browser loads the separate experimental WASI worker. The ordinary browser solver module
uses `wasm32-unknown-unknown`; the AC OPF module uses `wasm32-wasip1`.

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

`packages/engine` is the reusable package surface. It exports the generated
types, case and display parsing helpers, module based solves, capabilities,
the browser `Study`, and the browser wasm transport. It has no SvelteKit
dependency.

`packages/svelte` consumes `@tellegen/engine` and exports the map, panels, local
file flow, solve card, state provider, and full viewer as Svelte components.

`packages/webmcp` exports tool definitions, runtime validation, and registration
through an adapter that does not depend on Svelte or the engine package.

`apps/web` consumes the Svelte package and keeps demo concerns: routes, SEO,
credits, privacy, deployment, and bundled case pages. Embedded viewers have
neutral defaults: local files, no backend, no bundled cases, and no footer.

The public `Controller` class delegates case imports, balanced calculations,
distribution calculations and results, and calculation settings to separate
modules. Consumers keep using the same class methods; the provider owns their
lifetime and frees engine resources when it is disposed.

## In the browser

`@tellegen/engine` ships the ordinary wasm module built from
`crates/tellegen-wasm`. It supports DC power flow, DC OPF, AC power flow,
SOCWR, multiconductor power flow, generation, collection imports and dynamics.
Experimental dynamics tools remain disabled in the UI until the host or user
enables them. The hosted demo's server compute endpoints are an opt-in DC OPF
fallback (`TELLEGEN_SERVER_COMPUTE`).

The Svelte package uses `Study` for supported balanced calculations. `preview`
returns a first-order update for the quantities the calculation defines;
`commit` performs an exact re-solve and can return a selected sensitivity
column. DC power flow returns no sensitivities and rejects such requests.
Multiconductor power flow has its own retained session. AC OPF uses the
separate worker described above, which reuses the compiled module but creates
a fresh WASI instance for each calculation. Cancellation terminates that
worker. AC OPF case edits, retained Studies and browser solution export remain
unsupported.

Supported local case files run in the browser and are not uploaded.

## Sources

- Rust to WebAssembly: [wasm-bindgen](https://github.com/wasm-bindgen/wasm-bindgen), [wasm-pack](https://crates.io/crates/wasm-pack)
- Solvers and linear algebra: [Clarabel.rs](https://github.com/oxfordcontrol/Clarabel.rs), [faer](https://docs.rs/faer/latest/faer/)
- Convex relaxation: R. A. Jabr, "Radial distribution load flow using conic programming," IEEE Transactions on Power Systems, 21(3), 2006.
- Svelte: [`$state`](https://svelte.dev/docs/svelte/$state)
