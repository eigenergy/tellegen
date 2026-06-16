# ---- rust: wasm module ----
FROM rust:slim AS wasm
RUN apt-get update && apt-get install -y --no-install-recommends git curl ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# The tellegen crate depends on the crates.io powerio release, so cargo fetches
# the source itself; nothing to clone here.
RUN rustup target add wasm32-unknown-unknown
RUN cargo install wasm-pack --locked
COPY rust /build/rust
RUN wasm-pack build /build/rust --target web --out-dir /out/wasm-pkg

# ---- frontend build ----
FROM node:22-slim AS frontend
WORKDIR /app
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci
COPY frontend/ ./
COPY --from=wasm /out/wasm-pkg ./src/lib/wasm-pkg
RUN npm run build

# ---- backend ----
FROM julia:1.11-bookworm
RUN apt-get update && apt-get install -y --no-install-recommends curl git \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app/backend

# Generic CPU target so an image built in CI runs on any host without
# recompiling (precompile cache misses otherwise).
ENV JULIA_CPU_TARGET="generic;haswell,clone_all"

# Dependency layer, keyed on Project.toml. PowerIO is registered (General);
# PowerDiff is unregistered, so Project.toml pins it through [sources] at a git
# rev. The local dev Manifest points at local clones and is intentionally not
# copied: this resolves fresh from Project.toml against the registry and the
# [sources] rev.
COPY backend/Project.toml ./
RUN julia --project=. -e "using Pkg; Pkg.Registry.update(); Pkg.instantiate(); Pkg.precompile()"

# Pull the lazy artifacts at build time so first boot does not hit the
# network: the pglib cases and the powerio binary (PowerIO.jl's Artifacts.toml
# tracks the powerio release, v0.2.2 today, which carries the aux bus extras
# where substation coordinates live).
RUN julia --project=. -e "using PowerDiff; PowerDiff.get_path(:pglib); \
    using PowerIO; PowerIO.library_available() || error(\"powerio artifact unavailable\")"

COPY backend/src ./src
COPY backend/bootstrap.jl ./
COPY --from=frontend /app/build /app/frontend/build

EXPOSE 8000
# Long start period: Julia loads JuMP and Ipopt, then solves the staged cases
# and warms their sensitivity caches; the 2000 bus case dominates.
HEALTHCHECK --interval=30s --timeout=5s --start-period=360s --retries=5 \
    CMD curl -fsS http://localhost:8000/api/health | grep -q '"ok"' || exit 1

CMD ["julia", "--project=.", "bootstrap.jl"]
