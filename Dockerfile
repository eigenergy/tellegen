# ---- rust: wasm module ----
# Pin the build and runtime to the same Debian release (trixie) so the server
# binary's glibc matches the runtime image; an unpinned rust:slim floats to the
# latest Debian and broke against debian:bookworm-slim (glibc 2.41 vs 2.36).
FROM rust:1-slim-trixie AS wasm
RUN apt-get update && apt-get install -y --no-install-recommends git curl ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# The tellegen crate depends on the crates.io powerio release, so cargo fetches
# the source itself; nothing to clone here.
RUN rustup target add wasm32-unknown-unknown
# Prebuilt wasm-pack binary (statically linked), verified against a pinned digest.
RUN curl -fsSL https://github.com/wasm-bindgen/wasm-pack/releases/download/v0.15.0/wasm-pack-v0.15.0-x86_64-unknown-linux-musl.tar.gz -o /tmp/wasm-pack.tar.gz \
    && echo 'c09f971ecaed9a2efc80fdcea7a00ef6b53c7fadc8c57d1f61b53a6aa66b668a  /tmp/wasm-pack.tar.gz' | sha256sum -c - \
    && tar -xzf /tmp/wasm-pack.tar.gz -C /usr/local/bin --strip-components=1 --wildcards '*/wasm-pack' \
    && rm -f /tmp/wasm-pack.tar.gz
COPY rust /build/rust
# The core wasm disables SIMD so Safari (no relaxed SIMD) can parse it; it carries
# no faer kernels. The sens wasm keeps the default target (its faer/pulp linalg
# emits relaxed SIMD regardless), so Safari falls back to server sensitivity.
RUN RUSTFLAGS="-C target-feature=-simd128,-relaxed-simd" \
    wasm-pack build /build/rust --target web --out-dir /out/wasm-pkg -- --no-default-features
RUN wasm-pack build /build/rust --target web --out-dir /out/wasm-sens-pkg --out-name tellegen_sens

# ---- frontend build ----
FROM node:22-slim AS frontend
WORKDIR /app
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci
COPY frontend/ ./
COPY --from=wasm /out/wasm-pkg ./src/lib/wasm-pkg
COPY --from=wasm /out/wasm-sens-pkg ./src/lib/wasm-sens-pkg
RUN npm run build && npm run smoke:build

# ---- tellegen backend (cargo-chef: dependency compile is a cacheable layer) ----
FROM rust:1-slim-trixie AS chef
RUN apt-get update && apt-get install -y --no-install-recommends git ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && cargo install cargo-chef --locked
WORKDIR /build

FROM chef AS planner
COPY rust .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS server
COPY --from=planner /build/recipe.json recipe.json
# Cook only the dependencies; this layer is reused across builds whenever
# Cargo.toml / Cargo.lock are unchanged, even when the crate source changes.
RUN cargo chef cook --release --recipe-path recipe.json
COPY rust .
RUN cargo build --release --bin tellegen-server

# ---- runtime ----
FROM debian:trixie-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=server /build/target/release/tellegen-server /usr/local/bin/tellegen-server
COPY --from=frontend /app/build /app/frontend/build

ENV TELLEGEN_FRONTEND_BUILD=/app/frontend/build
ENV TELLEGEN_DATA=/app/data
EXPOSE 8000
# The tellegen backend parses the staged cases and solves the base DC OPF at boot.
HEALTHCHECK --interval=30s --timeout=5s --start-period=120s --retries=5 \
    CMD curl -fsS http://localhost:8000/api/health | grep -q '"ok"' || exit 1

CMD ["tellegen-server"]
