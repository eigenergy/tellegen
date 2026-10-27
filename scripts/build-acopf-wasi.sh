#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
asset_dir="$repo_root/target/experimental-acopf"

cargo build \
  --manifest-path "$repo_root/Cargo.toml" \
  --locked \
  --release \
  --target wasm32-wasip1 \
  -p tellegen-acopf-wasi \
  --features acopf

mkdir -p "$asset_dir"
install -m 0644 \
  "$repo_root/target/wasm32-wasip1/release/tellegen_acopf_wasi.wasm" \
  "$asset_dir/tellegen_acopf_wasi.wasm"

echo "built development-only AC OPF asset: $asset_dir/tellegen_acopf_wasi.wasm"
