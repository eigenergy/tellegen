#!/usr/bin/env sh
# Stage the demo case files tellegen serves. ACTIVSg200 and ACTIVSg500 use the
# MATPOWER OPF export plus the PowerWorld aux coordinate export. Texas 7k and
# CATS use the MATPOWER case plus GIS files. The rest of each distribution stays
# where it is.
#
# Usage: scripts/stage-data.sh <datasets dir> [target dir]
set -eu

src=${1:?usage: stage-data.sh <datasets dir> [target dir]}
dst=${2:-"$(cd "$(dirname "$0")/.." && pwd)/data"}

for c in ACTIVSg200 ACTIVSg500; do
    if [ ! -f "$src/$c/case_$c.m" ] || [ ! -f "$src/$c/$c.aux" ]; then
        echo "skip $c: need both $src/$c/case_$c.m and $src/$c/$c.aux" >&2
        continue
    fi
    mkdir -p "$dst/$c"
    cp "$src/$c/case_$c.m" "$src/$c/$c.aux" "$dst/$c/"
    echo "staged $c -> $dst/$c"
done

texas7k=texas7k_equity
if [ ! -f "$src/$texas7k/Texas7k_20210804.m" ] || [ ! -f "$src/$texas7k/Texas7k_lat_long.csv" ]; then
    echo "skip ACTIVSg7000: need both $src/$texas7k/Texas7k_20210804.m and $src/$texas7k/Texas7k_lat_long.csv" >&2
else
    mkdir -p "$dst/ACTIVSg7000"
    cp "$src/$texas7k/Texas7k_20210804.m" "$dst/ACTIVSg7000/"
    cp "$src/$texas7k/Texas7k_lat_long.csv" "$dst/ACTIVSg7000/"
    echo "staged ACTIVSg7000 -> $dst/ACTIVSg7000"
fi

cats=CATS-CaliforniaTestSystem
if [ ! -f "$src/$cats/MATPOWER/CaliforniaTestSystem.m" ] || [ ! -f "$src/$cats/GIS/CATS_buses.csv" ]; then
    echo "skip CATS: need both $src/$cats/MATPOWER/CaliforniaTestSystem.m and $src/$cats/GIS/CATS_buses.csv" >&2
else
    mkdir -p "$dst/CATS"
    cp "$src/$cats/MATPOWER/CaliforniaTestSystem.m" "$dst/CATS/"
    cp "$src/$cats/GIS/CATS_buses.csv" "$dst/CATS/"
    if [ -f "$src/$cats/GIS/CATS_lines.json" ]; then
        cp "$src/$cats/GIS/CATS_lines.json" "$dst/CATS/"
    fi
    if [ -f "$src/$cats/GIS/CATS_gens.csv" ]; then
        cp "$src/$cats/GIS/CATS_gens.csv" "$dst/CATS/"
    fi
    echo "staged CATS -> $dst/CATS"
fi
