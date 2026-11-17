#!/usr/bin/env bash
set -euo pipefail
# Arguments: installed libklusolvex, header/build directory, output library.
library=$1
build_dir=$2
output=$3
mkdir -p "$build_dir"
base=https://raw.githubusercontent.com/DrTimothyAldenDavis/SuiteSparse/v5.6.0
for header in KLU/Include/klu.h AMD/Include/amd.h COLAMD/Include/colamd.h BTF/Include/btf.h SuiteSparse_config/SuiteSparse_config.h; do
  curl -fsSL "$base/$header" -o "$build_dir/${header##*/}"
done
source_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
if [[ $(uname -s) == Darwin ]]; then
  cc -O3 -dynamiclib -I"$build_dir" "$source_dir/klu_metrics.c" "$library" -o "$output"
else
  cc -O3 -shared -fPIC -I"$build_dir" "$source_dir/klu_metrics.c" "$library" -o "$output"
fi
