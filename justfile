# tellegen monorepo task runner — `just` lists recipes, `just ci` runs the gates.
#
# The Rust side is a Cargo workspace (crates/*); the web app (apps/web) is a
# standalone npm package for now (packages/ is reserved for a future shared
# @tellegen/engine). This file is the single place that knows how the pieces fit
# together across the two languages.

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

# ---- WebAssembly + web (apps/web) ----

# Build both wasm packages (core + sensitivity) into apps/web/src/lib.
wasm:
    cd apps/web && npm run wasm

# Build the web app (expects `just wasm` first).
web-build:
    cd apps/web && npm run build

# Type-check + svelte-check the web app.
web-check:
    cd apps/web && npm run check

# CI gate: fail if the web app is unformatted.
web-lint:
    cd apps/web && npm run lint

# CI gate: smoke-check the static build output.
web-smoke:
    cd apps/web && npm run smoke:build

# ---- aggregate ----

# Everything CI enforces, in order.
ci: fmt-check clippy deny epl-guard test wasm web-lint web-check web-build web-smoke
