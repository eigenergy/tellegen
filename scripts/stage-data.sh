#!/usr/bin/env sh
# Stage the demo case files tellegen serves. ACTIVSg uses the MATPOWER OPF
# export plus the PowerWorld aux coordinate export. CATS uses the MATPOWER case
# plus its GIS bus coordinate CSV. Roughly 12 MB lands in the target; the rest
# of each distribution stays where it is.
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

cats=CATS-CaliforniaTestSystem
if [ ! -f "$src/$cats/MATPOWER/CaliforniaTestSystem.m" ] || [ ! -f "$src/$cats/GIS/CATS_buses.csv" ]; then
    echo "skip CATS: need both $src/$cats/MATPOWER/CaliforniaTestSystem.m and $src/$cats/GIS/CATS_buses.csv" >&2
else
    mkdir -p "$dst/CATS"
    cp "$src/$cats/MATPOWER/CaliforniaTestSystem.m" "$dst/CATS/"
    cp "$src/$cats/GIS/CATS_buses.csv" "$dst/CATS/"
    echo "staged CATS -> $dst/CATS"
fi
