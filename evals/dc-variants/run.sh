#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
case_path=${1:?usage: run.sh CASE.m OUTPUT_DIRECTORY}
out=${2:?usage: run.sh CASE.m OUTPUT_DIRECTORY}
mkdir -p "$out"
out=$(cd "$out" && pwd)
case_path=$(python3 -c 'import pathlib,sys; print(pathlib.Path(sys.argv[1]).resolve())' "$case_path")
cd "$root"
cargo run --release --locked -q -p tellegen --example dc_variants -- "$case_path" > "$out/tellegen.json"
julia --project=evals/dc-variants evals/dc-variants/reference.jl "$case_path" "$out/tellegen.json" "$out/reference.json"
python3 evals/dc-variants/compare.py "$case_path" "$out/tellegen.json" "$out/reference.json" "$out/report.json"
