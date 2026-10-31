#!/usr/bin/env bash
# Test committed Tellegen against the PowerIO locked in Cargo.lock by default. An
# optional companion checkout is patched only in a temporary archive, never in
# Cargo.lock.
set -euo pipefail
if [[ $# -gt 1 ]]; then
    echo "Usage: $0 [/path/to/powerio-companion]" >&2
    exit 2
fi
tellegen_checkout=$(cd "$(dirname "$0")/.." && pwd)
scratch=$(mktemp -d "${TMPDIR:-/tmp}/tellegen-mc-opf.XXXXXX")
trap 'rm -rf "$scratch"' EXIT
git -C "$tellegen_checkout" archive HEAD | tar -x -C "$scratch"
cargo_args=(--locked)
if [[ $# == 1 ]]; then
    powerio_checkout=$(cd "$1" && pwd)
    python3 - "$powerio_checkout" "$scratch/powerio-local.toml" <<'PATCHES'
import json, pathlib, sys
root, out = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
crates = ['powerio', 'powerio-core', 'powerio-dist', 'powerio-prob',
          'powerio-matrix', 'powerio-synth', 'powerio-tx', 'powerio-tnd']
out.write_text('[patch.crates-io]\n' + ''.join(
    f'{crate} = {{ path = {json.dumps(str(root / crate))} }}\n' for crate in crates))
PATCHES
    cargo_args=(--config "$scratch/powerio-local.toml")
fi
cd "$scratch"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$tellegen_checkout/target/mc-opf-review}"
if [[ $# == 1 ]]; then
    # Select the companion only in this archive, even when its version is newer.
    cargo update -p powerio -p powerio-core -p powerio-dist -p powerio-prob \
        -p powerio-matrix -p powerio-synth -p powerio-tx -p powerio-tnd "${cargo_args[@]}"
    cargo_args+=(--locked)
fi
cargo test -p tellegen --all-features "${cargo_args[@]}"
cargo test -p tellegen --no-default-features --features mc-opf "${cargo_args[@]}"
cargo clippy -p tellegen --all-features --all-targets "${cargo_args[@]}" -- -D warnings
