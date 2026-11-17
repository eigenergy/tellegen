#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
asset_dir="$repo_root/target/experimental-acopf"

"${PYTHON:-python3}" "$repo_root/scripts/acopf-release.py"

# The `release-acopf` profile builds this one module for speed (`opt-level = 3`
# with LTO); the main browser module keeps the size-first `release` profile.
# Fixed-width SIMD is part of every browser and Node version that can run the
# worker. The flag is scoped to this target so no other build sees it.
CARGO_TARGET_WASM32_WASIP1_RUSTFLAGS="${CARGO_TARGET_WASM32_WASIP1_RUSTFLAGS:-} -C target-feature=+simd128" \
cargo build \
  --manifest-path "$repo_root/Cargo.toml" \
  --locked \
  --profile release-acopf \
  --target wasm32-wasip1 \
  -p tellegen-acopf-wasi \
  --features acopf

mkdir -p "$asset_dir"
install -m 0644 \
  "$repo_root/target/wasm32-wasip1/release-acopf/tellegen_acopf_wasi.wasm" \
  "$asset_dir/tellegen_acopf_wasi.wasm"

install -m 0644 "$repo_root/licenses/POUNCE-EPL-2.0.txt" "$asset_dir/POUNCE-EPL-2.0.txt"
install -m 0644 "$repo_root/licenses/POUNCE-NOTICE.txt" "$asset_dir/POUNCE-NOTICE.txt"
"${PYTHON:-python3}" "$repo_root/scripts/acopf-release.py" --asset "$asset_dir/tellegen_acopf_wasi.wasm"

echo "built AC OPF asset: $asset_dir/tellegen_acopf_wasi.wasm"
