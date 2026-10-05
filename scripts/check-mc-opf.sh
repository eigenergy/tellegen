#!/usr/bin/env bash
# Test committed Tellegen against the companion PowerIO checkout without
# changing either checkout's Cargo.lock. All path patches stay temporary.
set -euo pipefail
if [[ $# != 1 ]]; then
    echo "Usage: $0 /path/to/powerio-with-mc-ivr-preparation" >&2
    exit 2
fi
powerio_checkout=$(cd "$1" && pwd)
tellegen_checkout=$(cd "$(dirname "$0")/.." && pwd)
scratch=$(mktemp -d "${TMPDIR:-/tmp}/tellegen-mc-opf.XXXXXX")
trap 'rm -rf "$scratch"' EXIT
git -C "$tellegen_checkout" archive HEAD | tar -x -C "$scratch"
python3 - "$powerio_checkout" "$scratch/powerio-local.toml" <<'PY'
import json, pathlib, sys
root, out = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
crates = ['powerio', 'powerio-core', 'powerio-dist', 'powerio-prob', 'powerio-matrix', 'powerio-tx']
out.write_text('[patch.crates-io]\n' + ''.join(
    f'{crate} = {{ path = {json.dumps(str(root / crate))} }}\n' for crate in crates))
PY
cd "$scratch"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$tellegen_checkout/target/mc-opf-review}"
cargo test -p tellegen --features mc-opf --config "$scratch/powerio-local.toml"
cargo test -p tellegen --no-default-features --features mc-opf --config "$scratch/powerio-local.toml"
cargo clippy -p tellegen --features mc-opf --all-targets --config "$scratch/powerio-local.toml" -- -D warnings
