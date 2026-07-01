# tellegen monorepo task runner — `just` lists recipes, `just ci` runs the gates.
#
# The Rust side is a Cargo workspace (crates/*). The JavaScript side is an npm
# workspace covering packages/engine, packages/svelte, apps/web,
# examples/browser-minimal, and examples/svelte-minimal. This file is the
# single place that knows how the pieces fit together across the two languages.

# List recipes.
default:
    @just --list

# ---- Rust ----

# Run the workspace test suite (engine, adapters, benchmarks).
test:
    cargo test --workspace

# Format the whole workspace.
fmt:
    cargo fmt --all

# CI gate: fail if anything is unformatted.
fmt-check:
    cargo fmt --all --check

# CI gate: clippy with warnings denied on the shipping crates.
clippy:
    cargo clippy -p tellegen -p tellegen-wasm -p tellegen-server -p tellegen-cli --all-targets -- -D warnings

# CI gate: licenses, advisories, bans, sources.
deny:
    cargo deny check

# CI gate: the EPL-2.0 pounce backend must never enter a shipped (wasm/server/cli) build.
epl-guard:
    #!/usr/bin/env bash
    set -euo pipefail
    for p in "tellegen-wasm --target wasm32-unknown-unknown" tellegen-server tellegen-cli; do
        if cargo tree -p $p 2>/dev/null | grep -qi pounce; then
            echo "EPL pounce backend leaked into: $p" >&2
            exit 1
        fi
    done
    echo "ok: no EPL pounce in wasm / server / cli"

# ---- JavaScript workspace ----

# Build both wasm packages (core + sensitivity) into packages/engine.
wasm:
    npm run wasm

# Type-check the engine package.
engine-check:
    npm run check:engine

# Build the engine package.
engine-build:
    npm run build:engine

# Type-check the Svelte component package.
svelte-check:
    npm run check:svelte

# CI gate: unit tests for the Svelte package's api/color helpers.
svelte-test:
    npm run test:svelte

# Build the Svelte component package.
svelte-build:
    npm run build:svelte

# CI gate: install the packed tarballs into a temporary consumer and build it.
svelte-packed:
    npm run test:svelte-packed

# Build the minimal downstream example.
example-build:
    npm run build:example

# CI gate: package-level import smoke test.
js-import:
    npm run test:import

# Build the web app (expects `just wasm` and `just engine-build` first).
web-build:
    npm run build:web

# Type-check + svelte-check the web app.
web-check:
    npm run check:web

# CI gate: fail if the web app is unformatted.
web-lint:
    npm run lint:web

# CI gate: smoke-check the static build output.
web-smoke:
    npm run smoke:web

# CI gate: browser coverage for the hosted demo local file flow.
web-browser:
    npm run test:browser

# ---- aggregate ----

# Everything CI enforces locally, in order.
ci: fmt-check clippy deny epl-guard test wasm engine-check engine-build js-import web-lint svelte-check svelte-test web-check svelte-packed web-build web-smoke web-browser
