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
# The core wasm disables SIMD so Safari (no relaxed SIMD) can parse it, and drops
# default features so it carries no faer kernels. The full wasm enables the `conic`
# feature — the whole engine (DC optimal power flow, AC power flow, and the SOCWR
# relaxation) with sensitivities; it stays off relaxed SIMD at the default wasm target,
# so it validates on Safari too. `--out-name tellegen` keeps the core package's file
# names stable for the frontend's imports.
RUN RUSTFLAGS="-C target-feature=-simd128,-relaxed-simd" \
    wasm-pack build /build/crates/tellegen-wasm --target web --out-dir /out/wasm-pkg --out-name tellegen -- --no-default-features
RUN wasm-pack build /build/crates/tellegen-wasm --target web --out-dir /out/wasm-sens-pkg --out-name tellegen_sens -- --features conic

# ---- frontend build ----
FROM node:22-slim AS frontend
WORKDIR /app
COPY package.json package-lock.json ./
COPY apps/web/package.json apps/web/package.json
COPY packages/engine/package.json packages/engine/package.json
COPY examples/browser-minimal/package.json examples/browser-minimal/package.json
RUN npm ci --ignore-scripts
COPY apps/web apps/web
COPY packages/engine packages/engine
COPY examples/browser-minimal examples/browser-minimal
COPY crates/tellegen/src/api.rs crates/tellegen/src/api.rs
COPY --from=wasm /out/wasm-pkg ./packages/engine/src/wasm-pkg
COPY --from=wasm /out/wasm-sens-pkg ./packages/engine/src/wasm-sens-pkg
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
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=server /build/target/release/tellegen-server /usr/local/bin/tellegen-server
COPY --from=frontend /app/apps/web/build /app/frontend/build

ENV TELLEGEN_FRONTEND_BUILD=/app/frontend/build
ENV TELLEGEN_DATA=/app/data
EXPOSE 8000
# The tellegen backend parses the staged cases and solves the base DC OPF at boot.
HEALTHCHECK --interval=30s --timeout=5s --start-period=120s --retries=5 \
    CMD curl -fsS http://localhost:8000/api/health | grep -q '"ok"' || exit 1

CMD ["tellegen-server"]
