#!/usr/bin/env bash
# Write the JSON schemas that the TypeScript types, result validators and
# Python MCP models are generated from. The Rust records are the source.
#
#   scripts/schemas.sh           regenerate the committed schemas
#   scripts/schemas.sh --check   fail if a committed schema differs
#
# Follow a regeneration with `npm run types`.
set -euo pipefail
cd "$(dirname "$0")/.."
out=packages/engine/src/generated
check=0
[ "${1:-}" = "--check" ] && check=1

# name | package | example | features
schemas="
api|tellegen|api_schema|schema,conic,mc-pf
settings|tellegen|settings_schema|schema,conic,mc-pf
study|tellegen|study_schema|schema,conic,mc-pf
synth|tellegen-synth|synth_schema|schema
rms|tellegen|rms_schema|schema,rms-experimental
http|tellegen-server|http_schema|schema
wasm-adapter|tellegen-wasm|adapter_schema|schema,conic,synth,powerio-collections
rms-adapter|tellegen-wasm|rms_adapter_schema|schema,rms-experimental
"

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
stale=0
for row in $schemas; do
  IFS='|' read -r name package example features <<<"$row"
  cargo run --quiet --locked -p "$package" --example "$example" --features "$features" >"$tmp/$name.json"
  if [ "$check" = 1 ]; then
    if ! diff -u "$out/$name.schema.json" "$tmp/$name.json"; then
      echo "::error::$out/$name.schema.json differs from the Rust records. Run scripts/schemas.sh, then npm run types." >&2
      stale=1
    fi
  else
    cp "$tmp/$name.json" "$out/$name.schema.json"
  fi
done
exit "$stale"
