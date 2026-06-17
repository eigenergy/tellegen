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
# Prebuilt wasm-pack binary (statically linked) instead of compiling from source.
RUN curl -fsSL https://github.com/wasm-bindgen/wasm-pack/releases/download/v0.15.0/wasm-pack-v0.15.0-x86_64-unknown-linux-musl.tar.gz \
    | tar -xz -C /usr/local/bin --strip-components=1 --wildcards '*/wasm-pack'
COPY rust /build/rust
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

# ---- tellegen backend ----
FROM rust:1-slim-trixie AS server
RUN apt-get update && apt-get install -y --no-install-recommends git ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /build
COPY rust ./rust
RUN cargo build --manifest-path rust/Cargo.toml --release --bin tellegen-server

# ---- runtime ----
FROM debian:trixie-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=server /build/rust/target/release/tellegen-server /usr/local/bin/tellegen-server
COPY --from=frontend /app/build /app/frontend/build

ENV TELLEGEN_FRONTEND_BUILD=/app/frontend/build
ENV TELLEGEN_DATA=/app/data
EXPOSE 8000
# The tellegen backend parses the staged cases and solves the base DC OPF at boot.
HEALTHCHECK --interval=30s --timeout=5s --start-period=120s --retries=5 \
    CMD curl -fsS http://localhost:8000/api/health | grep -q '"ok"' || exit 1

CMD ["tellegen-server"]
