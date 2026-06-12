# ---- rust: powerio C library + wasm module ----
FROM rust:slim AS rust
RUN apt-get update && apt-get install -y --no-install-recommends git curl ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# PowerIO.jl's bundled binary artifact lags powerio main, and the wasm module
# wraps powerio directly, so both build here from source. The wasm crate
# points at a sibling checkout (../../../Research/powerio); recreate that
# layout. Override the source with --build-arg POWERIO_URL.
ARG POWERIO_URL=https://github.com/eigenergy/powerio.git
RUN git clone --depth 1 "$POWERIO_URL" /Research/powerio
RUN cargo build --release -p powerio-capi --manifest-path /Research/powerio/Cargo.toml
RUN cargo install wasm-pack --locked
COPY wasm /Visualization/tellegen/wasm
RUN wasm-pack build /Visualization/tellegen/wasm --target web --out-dir /out/wasm-pkg

# ---- frontend build ----
FROM node:22-slim AS frontend
WORKDIR /app
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci
COPY frontend/ ./
COPY --from=rust /out/wasm-pkg ./src/lib/wasm-pkg
RUN npm run build

# ---- backend ----
FROM julia:1.11-bookworm
RUN apt-get update && apt-get install -y --no-install-recommends curl git \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app/backend

# Generic CPU target so an image built in CI runs on any host without
# recompiling (precompile cache misses otherwise).
ENV JULIA_CPU_TARGET="generic;haswell,clone_all"

# Dependency layer, keyed on Project.toml only. PowerDiff and PowerIO are not
# registered yet, so they come from git; the local dev Manifest points at
# local paths and is intentionally not copied.
ARG POWERIO_JL_URL=https://github.com/eigenergy/PowerIO.jl.git
ARG POWERDIFF_URL=https://github.com/grid-opt-alg-lab/PowerDiff.jl.git
COPY backend/Project.toml ./
RUN julia -e "using Pkg; Pkg.activate(\".\"); \
    Pkg.add(url=\"$POWERIO_JL_URL\"); \
    Pkg.add(url=\"$POWERDIFF_URL\"); \
    Pkg.instantiate(); Pkg.precompile()"

# Pull the pglib artifact at build time so first boot does not hit the network.
RUN julia --project=. -e "using PowerDiff; PowerDiff.get_path(:pglib)"

# The source-built powerio C library, overriding PowerIO.jl's bundled binary
# (aux bus extras, where substation coordinates live, need current powerio).
COPY --from=rust /Research/powerio/target/release/libpowerio_capi.so /usr/local/lib/
ENV POWERIO_CAPI=/usr/local/lib/libpowerio_capi.so

COPY backend/src ./src
COPY backend/bootstrap.jl ./
COPY --from=frontend /app/build /app/frontend/build

EXPOSE 8000
# Long start period: Julia loads JuMP and Ipopt, then solves the staged cases
# and warms their sensitivity caches; the 2000 bus case dominates.
HEALTHCHECK --interval=30s --timeout=5s --start-period=360s --retries=5 \
    CMD curl -fsS http://localhost:8000/api/health | grep -q '"ok"' || exit 1

CMD ["julia", "--project=.", "bootstrap.jl"]
