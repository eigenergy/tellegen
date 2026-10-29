# ---- rust: wasm module ----
# Pin the build and runtime to the same Debian release (trixie) so the server
# binary's glibc matches the runtime image; an unpinned rust:slim floats to the
# latest Debian and broke against debian:bookworm-slim (glibc 2.41 vs 2.36).
FROM rust:1-slim-trixie AS wasm
RUN apt-get update && apt-get install -y --no-install-recommends git curl ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# The tellegen crates depend on the crates.io powerio release, so cargo fetches
# the source itself; nothing to clone here.
RUN rustup target add wasm32-unknown-unknown
# Prebuilt wasm-pack binary (statically linked), verified against a pinned digest.
RUN curl -fsSL https://github.com/wasm-bindgen/wasm-pack/releases/download/v0.15.0/wasm-pack-v0.15.0-x86_64-unknown-linux-musl.tar.gz -o /tmp/wasm-pack.tar.gz \
    && echo 'c09f971ecaed9a2efc80fdcea7a00ef6b53c7fadc8c57d1f61b53a6aa66b668a  /tmp/wasm-pack.tar.gz' | sha256sum -c - \
    && tar -xzf /tmp/wasm-pack.tar.gz -C /usr/local/bin --strip-components=1 --wildcards '*/wasm-pack' \
    && rm -f /tmp/wasm-pack.tar.gz
# The whole Cargo workspace: wasm-pack builds the tellegen-wasm member, which
# depends on the tellegen engine and resolves against the root lockfile.
COPY Cargo.toml Cargo.lock /build/
COPY crates /build/crates
# The one wasm module: the `conic` feature carries the whole engine (DC optimal
# power flow, AC power flow, the SOCWR relaxation, the Study) with sensitivities,
# and `synth` adds synthetic grid generation sessions. It stays off relaxed SIMD
# at the default wasm target, so it validates on Safari. Mirrors the `wasm`
# script in packages/engine/package.json, including the places table the
# generator reads beside the module.
RUN wasm-pack build /build/crates/tellegen-wasm --target web --out-dir /out/wasm-pkg --out-name tellegen -- --features conic,synth \
    && mkdir -p /out/data \
    && cp /build/crates/tellegen-synth/data/populated_places.json /out/data/

# Separate reactor: keep POUNCE out of the ordinary wasm-bindgen module and
# native server. The website runs nonlinear AC OPF in a terminable worker.
FROM wasm AS acopf
RUN apt-get update && apt-get install -y --no-install-recommends python3 \
    && rm -rf /var/lib/apt/lists/*
RUN rustup target add wasm32-wasip1
COPY scripts/build-acopf-wasi.sh scripts/acopf-release.py /build/scripts/
COPY licenses /build/licenses
ARG TELLEGEN_SOURCE_REVISION=""
RUN bash /build/scripts/build-acopf-wasi.sh

# ---- frontend build ----
FROM node:22-slim AS frontend
WORKDIR /app
COPY package.json package-lock.json ./
COPY apps/web/package.json apps/web/package.json
COPY packages/engine/package.json packages/engine/package.json
COPY packages/svelte/package.json packages/svelte/package.json
COPY packages/webmcp/package.json packages/webmcp/package.json
COPY examples/browser-minimal/package.json examples/browser-minimal/package.json
COPY examples/svelte-minimal/package.json examples/svelte-minimal/package.json
RUN npm ci --ignore-scripts
COPY apps/web apps/web
COPY packages/engine packages/engine
COPY packages/svelte packages/svelte
COPY packages/webmcp packages/webmcp
COPY examples/browser-minimal examples/browser-minimal
COPY examples/svelte-minimal examples/svelte-minimal
# `types:check` (run by build:engine) reads the Rust records the generated
# TypeScript mirrors: tellegen's api.rs and the synth request/report/error.
COPY crates/tellegen/src crates/tellegen/src
COPY crates/tellegen-synth/src crates/tellegen-synth/src
COPY --from=wasm /out/wasm-pkg ./packages/engine/src/wasm-pkg
COPY --from=wasm /out/data ./packages/engine/src/data
COPY --from=acopf /build/target/experimental-acopf ./target/experimental-acopf
# Optional CARTO basemap key. It is public by design, because the browser
# sends it with every tile request, so restrict it to the deployment's hosts
# on CARTO's side. Empty leaves OpenFreeMap as the primary basemap.
ARG PUBLIC_CARTO_BASEMAP_KEY=""
RUN npm run build:engine && npm run build:web && npm run smoke:web

# ---- tellegen backend (cargo-chef: dependency compile is a cacheable layer) ----
FROM rust:1-slim-trixie AS chef
RUN apt-get update && apt-get install -y --no-install-recommends git ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && cargo install cargo-chef --locked
WORKDIR /build

FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS server
COPY --from=planner /build/recipe.json recipe.json
# Cook only the server binary's dependencies; this layer is reused across builds
# whenever Cargo.toml / Cargo.lock are unchanged, even when the crate source
# changes. Scoping to the bin keeps the benchmark dependencies out of the build
# entirely. `-p tellegen-server` selects the package explicitly so the bin resolves
# regardless of the workspace `default-members` set (which is scoped to the engine);
# `--locked` fails the build instead of silently editing Cargo.lock.
RUN cargo chef cook --release --recipe-path recipe.json --locked -p tellegen-server --bin tellegen-server
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo build --release --locked -p tellegen-server --bin tellegen-server

# ---- runtime ----
FROM debian:trixie-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid 10001 tellegen \
    && useradd --system --uid 10001 --gid tellegen --home-dir /app --shell /usr/sbin/nologin tellegen
WORKDIR /app
COPY --from=server /build/target/release/tellegen-server /usr/local/bin/tellegen-server
COPY --chown=10001:10001 --from=frontend /app/apps/web/build /app/frontend/build

ENV TELLEGEN_FRONTEND_BUILD=/app/frontend/build
ENV TELLEGEN_DATA=/app/data
EXPOSE 8000
USER 10001:10001
# The tellegen backend parses the staged cases and solves the base DC OPF at boot.
HEALTHCHECK --interval=30s --timeout=5s --start-period=120s --retries=5 \
    CMD curl -fsS http://localhost:8000/api/health | grep -q '"ok"' || exit 1

CMD ["tellegen-server"]
