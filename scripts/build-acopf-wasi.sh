#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
asset_dir="$repo_root/target/experimental-acopf"

"${PYTHON:-python3}" "$repo_root/scripts/acopf-release.py"

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

install -m 0644 "$repo_root/licenses/POUNCE-EPL-2.0.txt" "$asset_dir/POUNCE-EPL-2.0.txt"
install -m 0644 "$repo_root/licenses/POUNCE-NOTICE.txt" "$asset_dir/POUNCE-NOTICE.txt"
"${PYTHON:-python3}" "$repo_root/scripts/acopf-release.py" --asset "$asset_dir/tellegen_acopf_wasi.wasm"

echo "built development-only AC OPF asset: $asset_dir/tellegen_acopf_wasi.wasm"
