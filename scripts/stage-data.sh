#!/usr/bin/env sh
# Stage the TAMU ACTIVSg files tellegen serves: the MATPOWER export for the
# OPF and the aux export for real substation coordinates. The source
# directory holds the distributions as downloaded from
# https://electricgrids.engr.tamu.edu/ (ACTIVSg200/, ACTIVSg500/,
# ACTIVSg2000/). Roughly 9 MB lands in the target; the rest of each
# distribution stays where it is.
#
# Usage: scripts/stage-data.sh <datasets dir> [target dir]
set -eu

src=${1:?usage: stage-data.sh <datasets dir> [target dir]}
dst=${2:-"$(cd "$(dirname "$0")/.." && pwd)/data"}

for c in ACTIVSg200 ACTIVSg500 ACTIVSg2000; do
    if [ ! -f "$src/$c/case_$c.m" ] || [ ! -f "$src/$c/$c.aux" ]; then
        echo "skip $c: need both $src/$c/case_$c.m and $src/$c/$c.aux" >&2
        continue
    fi
    mkdir -p "$dst/$c"
    cp "$src/$c/case_$c.m" "$src/$c/$c.aux" "$dst/$c/"
    echo "staged $c -> $dst/$c"
done
