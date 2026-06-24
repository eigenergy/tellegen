# Port phasor into tellegen: final architecture

Status: **completed and superseded.** This port has shipped — the cargo workspace
(`crates/{tellegen,tellegen-wasm,tellegen-server,tellegen-cli,benchmarks}`, `apps/web`,
root `Cargo.toml`) exists, and `docs/src/architecture.md` documents the current layout
and dependency direction. Kept only as a historical record of the migration; do not
execute it as a plan. The one piece not yet done is the M7 feature split
(`conic-solve` / `conic-sens`); M0–M6 are in the tree.

Original intent: the source of truth for porting the
`~/Research/phasor` work into `~/Visualization/tellegen`.

Do not reopen the repo and crate architecture unless a concrete implementation
fact forces it. The goal is to move once, delete the copied solver, and keep the
engine usable as a normal Rust library.

## Decision

Use one monorepo with multiple Rust crates:

```text
tellegen/
  Cargo.toml
  Cargo.lock

  crates/
    tellegen/          # publishable engine crate, formerly phasor
    tellegen-wasm/     # browser adapter
    tellegen-server/   # native HTTP demo and fallback server
    benchmarks/        # validation and parity harness

  frontend/            # SvelteKit app and later Svelte package
  docs/                # mdBook source
  reference/
    julia-backend/     # PowerDiff.jl parity harness
  data/                # staged demo data, not published as a Rust crate
  deploy/
  scripts/
```

This is the settled shape:

```text
frontend -> tellegen-wasm -> tellegen -> powerio
server   -> tellegen      -> powerio
bench    -> tellegen      -> powerio
```

No dependency points back toward the frontend, wasm adapter, or server.

## Why this shape

Cargo workspaces are the Rust mechanism for managing multiple related packages
with one lockfile and shared build output. Large Rust projects use this shape
when one domain library has several target adapters: a core crate, then separate
CLI, server, wasm, desktop, or binding crates.

This is also the right dependency hygiene for tellegen. A Rust user who only
wants OPF, power flow, or sensitivities should not compile `wasm-bindgen`,
`axum`, `tokio`, Svelte glue, or deployment code.

References used for this decision:

- Cargo workspaces: <https://doc.rust-lang.org/cargo/reference/workspaces.html>
- Cargo features and when package splits are cleaner than feature combinations:
  <https://doc.rust-lang.org/cargo/reference/features.html>
- Ruffle: core engine plus separate web and desktop crates:
  <https://github.com/ruffle-rs/ruffle>
- Typst: core compiler crate plus CLI crate:
  <https://github.com/typst/typst>
- DataFusion: workspace with engine crates, CLI, tests, and benchmarks:
  <https://github.com/apache/datafusion>

## Root workspace

Create a root `Cargo.toml`:

```toml
[workspace]
members = [
  "crates/tellegen",
  "crates/tellegen-wasm",
  "crates/tellegen-server",
  "crates/benchmarks",
]
default-members = ["crates/tellegen"]
resolver = "2"

[workspace.package]
edition = "2021"
license = "Apache-2.0 OR MIT"
repository = "https://github.com/eigenergy/tellegen"

[workspace.dependencies]
powerio = "0.3"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

Keep `default-members = ["crates/tellegen"]` so a bare `cargo test`,
`cargo check`, or wasm compile gate can stay focused on the engine unless a
command explicitly selects another crate.

## Crate responsibilities

### `crates/tellegen`

The publishable engine crate. This crate is the ported and renamed version of
`~/Research/phasor`.

It owns:

- DC power flow
- DC OPF
- AC power flow
- SOCWR conic relaxation
- AC OPF backends
- sensitivity contracts and KKT or adjoint implementations
- generalized request and response structs
- `solve_json` and `capabilities_json`
- framework free interactive engine state
- typed edits and preview math
- framework free display coordinate helpers currently in `rust/src/geo.rs`

It must not depend on:

- `wasm-bindgen`
- `js-sys`
- `web-sys`
- `axum`
- `tokio`
- `tower-http`
- frontend TypeScript or Svelte files
- deck.gl or map payload shapes
- deployment paths or environment variables

Recommended module layout:

```text
crates/tellegen/src/
  lib.rs
  api.rs
  edit.rs
  preview.rs
  session.rs
  formulation.rs
  solve.rs
  geo.rs
  model/
    mod.rs
    dc.rs
    ac.rs
    cases.rs
  problem/
    mod.rs
    dc.rs
    pf_dc.rs
    pf_ac.rs
    conic.rs
    acopf.rs
    acopf_pounce.rs
  sens/
    mod.rs
    contract.rs
    dc.rs
    ac.rs
    conic.rs
    acopf.rs
```

`session.rs` is allowed in the engine only if it is plain Rust domain state. It
may parse or accept a `powerio::network::Network`, build reusable models, apply
typed `NetworkEdit` values, solve, and compute first order previews from
sensitivities. It may not know about web workers, HTTP, SSE, selected UI tabs,
map coloring, or frontend payload details.

`geo.rs` is allowed in the engine only because the current implementation is
framework free Rust over `powerio::Network`: coordinate extraction, stack
spreading, and synthetic layout. It must not grow deck.gl layer construction,
Svelte state, HTTP routes, or demo response payloads. If those concerns appear,
move them to the target adapter that needs them.

If the name `Session` keeps causing app boundary confusion, use `PreparedCase`
as the public engine type and reserve `WasmSession` for the wasm adapter.

### `crates/tellegen-wasm`

The browser adapter. It exports the engine to JavaScript.

It owns:

- `wasm-bindgen` exported functions and classes
- conversion from `JsValue`, strings, and typed arrays into engine calls
- `JsError` mapping
- panic hook setup
- optional `serde-wasm-bindgen`
- optional `js_sys::Float64Array` views for large numeric payloads
- worker friendly initialization helpers

It must not own:

- OPF math
- sensitivity formulas
- edit semantics
- first order preview logic
- connectivity validation
- server routes

Example shape:

```rust
use wasm_bindgen::prelude::*;

fn js_err(e: impl std::fmt::Display) -> JsError {
    JsError::new(&e.to_string())
}

#[wasm_bindgen]
pub fn solve_json(network_json: &str, request_json: &str) -> Result<String, JsError> {
    tellegen::solve_json(network_json, request_json).map_err(js_err)
}

#[wasm_bindgen]
pub fn capabilities_json() -> String {
    tellegen::capabilities_json()
}

#[wasm_bindgen]
pub struct WasmSession {
    inner: tellegen::PreparedCase,
}
```

This crate is the build input for `wasm-pack`, not the engine.

### `crates/tellegen-server`

The native HTTP adapter for the demo and fallback path.

It owns:

- `axum` routes
- `tokio` runtime use
- SSE stream setup
- staged bundled case loading
- static frontend serving
- health checks
- solver concurrency limits
- timeout and cancellation policy
- environment variable handling
- server specific response wrappers

It calls `tellegen` for parsing, solving, sensitivities, and previews.

It must not duplicate solver code. The current `rust/src/dc/*` copy must be
deleted during the port.

### `crates/benchmarks`

The validation harness. It is not a shipping crate.

It owns:

- PGLib or case corpus runs
- parity tables
- timing reports
- finite difference checks
- PowerDiff.jl comparison drivers when useful

It depends on `tellegen` by path. It should not be a default workspace member.

## Engine features

Start by preserving the current phasor feature behavior so the port is small.
Then split solve and sensitivity features only after the app builds.

Initial `crates/tellegen/Cargo.toml` should be close to current phasor:

```toml
[package]
name = "tellegen"
version = "0.1.0"
edition.workspace = true
license.workspace = true
repository.workspace = true
description = "Differentiable power flow, optimal power flow, and sensitivities in Rust."
readme = "README.md"

[dependencies]
powerio.workspace = true
clarabel = "0.11.1"
faer = { version = "0.24.1", default-features = false, features = ["std", "linalg", "sparse", "sparse-linalg"], optional = true }
num-complex = { version = "0.4", default-features = false, optional = true }
interiors = { version = "0.1", optional = true, features = ["step-control"] }
sparsetools = { version = "0.2", optional = true }
spsolve = { version = "0.1", optional = true, features = ["rlu"] }
pounce-nlp = { version = "0.6.0", optional = true }
pounce-algorithm = { version = "0.6.0", optional = true }
pounce-common = { version = "0.6.0", optional = true }
getrandom = { version = "0.2", features = ["js"], optional = true }
serde.workspace = true
serde_json.workspace = true

[features]
default = ["sensitivity"]
sensitivity = ["dep:faer", "dep:num-complex"]
conic = ["sensitivity"]
acopf = ["conic", "dep:interiors", "dep:sparsetools", "dep:spsolve", "dep:getrandom"]
acopf-pounce = ["acopf", "dep:pounce-nlp", "dep:pounce-algorithm", "dep:pounce-common"]
```

After the migrated app works, split SOCWR solve from SOCWR sensitivity:

```toml
ac-model = ["dep:num-complex"]
sensitivity = ["dep:faer", "ac-model"]
conic-solve = ["ac-model"]
conic-sens = ["conic-solve", "sensitivity"]
conic = ["conic-sens"] # compatibility alias for one release
```

The reason: SOCWR solve uses Clarabel and does not need faer. SOCWR sensitivity
does need the sparse linear algebra path. Splitting these lets the safe browser
core build run SOCWR solve without dragging in sensitivity kernels.

## WASM target policy

Use `wasm32-unknown-unknown`. Do not design around `wasm64`.

`wasm64` is not about `f64`; `wasm32` already supports 64 bit floats. `wasm64`
means 64 bit linear memory indexes and larger pointers. It is only useful when a
single module needs more than 4 GB of linear memory. At that point, tellegen
should use a native or server path for the solve.

`tellegen-wasm` should support two builds at first:

- core build: no default features, no faer
- sensitivity build: default features

The frontend can load the core module first and the sensitivity module only when
needed. After `conic-solve` exists, the core build can include SOCWR solve.

## TypeScript contract

Generate TypeScript types from Rust public request and response structs, but do
it from the adapter or a build task, not by moving UI types into the engine.

Recommended path:

```text
crates/tellegen/src/api.rs                 # Rust structs
crates/tellegen-wasm/build.rs or xtask     # type generation
frontend/src/lib/generated/tellegen-api.ts # generated file
```

Do not hand maintain frontend copies of `SolveRequest`, `SolveResponse`,
`Operand`, `Parameter`, or capability payloads once generation is in place.

## Migration phases

### M0: freeze source facts

Before editing, capture current state:

```sh
git -C ~/Visualization/tellegen status --short
git -C ~/Research/phasor status --short
```

Do not revert unrelated user changes. `~/Visualization/tellegen` currently has
frontend and docs edits; preserve them.

### M1: create the workspace

In `~/Visualization/tellegen`:

1. Add root `Cargo.toml`.
2. Create `crates/tellegen/` from `~/Research/phasor/src` and
   `~/Research/phasor/Cargo.toml`.
3. Move the current `rust/src/geo.rs` into `crates/tellegen/src/geo.rs`.
4. Create `crates/tellegen-wasm/` from the current `rust/src/lib.rs`, removing
   direct solver modules and turning exports into wrappers around `tellegen`.
5. Create `crates/tellegen-server/` from the current `rust/src/server.rs` and
   `rust/src/bin/tellegen-server.rs`.
6. Add `crates/benchmarks/` from `~/Research/phasor/benchmarks`.
7. Keep one root `Cargo.lock`.

The end of M1 should have a valid Cargo workspace even if the frontend still
points at old wasm package paths.

### M2: port the phasor engine

Move these from `~/Research/phasor/src` into `crates/tellegen/src`:

```text
api.rs
formulation.rs
solve.rs
lib.rs
model/
problem/
sens/
```

Rename crate references from `phasor` to `tellegen`.

Update package metadata:

- package name: `tellegen`
- repository: `https://github.com/eigenergy/tellegen`
- license: `Apache-2.0 OR MIT`
- powerio: `0.3`
- faer: `0.24.1`

Delete the old copied DC solver after the new engine API is wired:

```text
crates/tellegen-wasm/src/dc/
```

Do not preserve two solver implementations.

### M3: rebuild wasm as a thin adapter

`crates/tellegen-wasm` should depend on `tellegen` by path:

```toml
[dependencies]
tellegen = { path = "../tellegen", default-features = false }
wasm-bindgen = "0.2"
serde.workspace = true
serde_json.workspace = true
```

For the sensitivity package:

```toml
tellegen = { path = "../tellegen" }
```

If one crate cannot express both wasm builds cleanly, use feature flags in
`tellegen-wasm`:

```toml
[features]
default = []
sensitivity = ["tellegen/sensitivity"]
conic = ["tellegen/conic"]
```

Keep wasm exports stable for the frontend first:

- `parse_case`
- `ingest_case`
- `parse_display`
- `solve_dc` as a compatibility wrapper over generalized `solve_json`
- `solve_json`
- `capabilities_json`

Then migrate frontend code to the generalized names.

### M4: rebuild server as a thin adapter

Move server code into `crates/tellegen-server`.

The server may keep its current HTTP shapes during the first port, but all
solves and sensitivities must call `tellegen`.

Existing routes can stay:

```text
GET /api/health
GET /api/cases
GET /api/cases/{id}/case
GET /api/cases/{id}/network
GET /api/cases/{id}/solution
GET /api/cases/{id}/sensitivity/lmp/d/{bus}
GET /api/cases/{id}/solve
```

After parity is restored, add generalized solve endpoints if the frontend needs
them.

### M5: frontend path updates

Update `frontend/package.json` wasm scripts:

```json
{
  "wasm:core": "RUSTFLAGS=\"-C target-feature=-simd128,-relaxed-simd\" wasm-pack build ../crates/tellegen-wasm --target web --out-dir ../frontend/src/lib/wasm-pkg -- --no-default-features",
  "wasm:sens": "wasm-pack build ../crates/tellegen-wasm --target web --out-dir ../frontend/src/lib/wasm-sens-pkg --out-name tellegen_sens -- --features sensitivity"
}
```

Adjust paths as needed if npm commands run from `frontend/`.

Do not rewrite the Svelte state model during the port. First make the current UI
work on the new engine. Then introduce generalized `(formulation, watched
operand, edited parameter)` state.

### M6: add the engine session

Add `PreparedCase` or `Session` only after M1 through M5 pass.

Minimum engine API:

```rust
pub struct PreparedCase {
    network: powerio::network::Network,
    dc: Option<DcNetwork>,
    ac: Option<AcNetwork>,
}

impl PreparedCase {
    pub fn new(network: powerio::network::Network) -> Result<Self, Error>;
    pub fn solve(&mut self, request: &SolveRequest) -> Result<SolveResponse, Error>;
    pub fn apply(&mut self, edit: NetworkEdit) -> Result<(), Error>;
    pub fn preview(&mut self, request: &PreviewRequest) -> Result<PreviewResponse, Error>;
}
```

Rules:

- continuous edits reuse model state where valid
- discrete edits rebuild affected model state
- topology edits validate islands and reference buses with powerio
- preview is a local first order linearization
- commit and exact resolve is the source of truth
- served unit scaling lives in Rust, not duplicated in TypeScript

### M7: split conic solve from conic sensitivity

Do this after the port, not during it.

Acceptance for this phase:

```sh
cargo build -p tellegen --target wasm32-unknown-unknown --no-default-features --features conic-solve
```

and a browser wasm smoke test that solves a SOCWR case without faer.

## Acceptance checks

After M1 through M5:

```sh
cargo test -p tellegen
cargo test -p tellegen-server
cargo build -p tellegen --target wasm32-unknown-unknown --no-default-features
cargo build -p tellegen-wasm --target wasm32-unknown-unknown --no-default-features
cargo build -p tellegen-wasm --target wasm32-unknown-unknown --features sensitivity
cd frontend && npm run wasm
cd frontend && npm run check
cd frontend && npm run build
cd frontend && npm run smoke:build
```

If staged TAMU data is not present, use the existing fallback mode for server
tests:

```sh
TELLEGEN_ALLOW_FALLBACK=1 cargo test -p tellegen-server
```

## Non goals during the port

Do not do these in the first port:

- redesign the frontend
- change the map interaction model
- add Tauri
- split `tellegen-session` into a fourth crate
- rename every public API in one pass
- replace `wasm-pack`
- move powerio into the monorepo
- port PowerDiff.jl
- optimize binary typed array transport before JSON parity works

## Public artifact hygiene

Commit messages, PR descriptions, docs, and generated files should describe the
technical migration only. Do not include private discussion history, agent
disagreement, or personal reasons for the repo choice.

## Final rule

When in doubt, keep the dependency direction clean:

```text
target adapter -> engine -> powerio
```

If a piece of code imports `wasm-bindgen`, it is not engine code.
If a piece of code imports `axum` or `tokio`, it is not engine code.
If a piece of code computes an OPF result, a sensitivity, a typed edit, or a
first order preview without knowing the caller, it belongs in the engine.
