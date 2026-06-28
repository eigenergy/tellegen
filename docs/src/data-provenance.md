# Data Provenance

The demo cases are synthetic grids from TAMU ACTIVSg and CATS. They are
fictional networks built on geographic footprints. ACTIVSg coordinates come
from PowerWorld aux exports; CATS coordinates come from its GIS bus CSV. These
positions are not surveyed infrastructure locations.

| case | territory | buses | branches | files |
|---|---|---:|---:|---|
| ACTIVSg200 | central Illinois | 200 | 245 | `case_ACTIVSg200.m` + `ACTIVSg200.aux` |
| ACTIVSg500 | South Carolina | 500 | 597 | `case_ACTIVSg500.m` + `ACTIVSg500.aux` |
| ACTIVSg2000 | Texas | 2000 | 3206 | `case_ACTIVSg2000.m` + `ACTIVSg2000.aux` |
| CATS | California | 8870 | 10823 | `CaliforniaTestSystem.m` + `CATS_buses.csv` |

The MATPOWER file feeds the DC OPF. For ACTIVSg, the aux file supplies the
coordinates. Both files come from the same distribution, so bus numbering
matches. Operators download the ACTIVSg distributions from
[electricgrids.engr.tamu.edu](https://electricgrids.engr.tamu.edu/) and stage
them with `scripts/stage-data.sh`; the repository does not vendor them.

CATS comes from the
[WISPO POP CATS repository](https://github.com/WISPO-POP/CATS-CaliforniaTestSystem).
The server reads `CaliforniaTestSystem.m` for the network and `CATS_buses.csv`
for bus latitude and longitude.

## Aux coordinate forms

PowerWorld aux exports have used two coordinate layouts:

- ACTIVSg complete case exports repeat substation latitude and longitude on each
  bus row in `Latitude:1` and `Longitude:1`.
- Later exports can leave the bus latitude and longitude columns empty and
  reference the `Substation` table through `SubNumber`.

The Rust parser reads the bus row form for the three served cases and also
performs the substation join, so dropped files of either form resolve when the
data is present.

## Buses Sharing Coordinates

Multiple buses can share one substation coordinate. tellegen spreads each group
on a deterministic ring of about 400 m around the substation point, ordered by
bus id. The group remains visually associated with the substation at network
zoom, and individual buses remain hoverable at street zoom.

## Demo size

CATS is the largest bundled case. Larger cases, such as Texas7k, parse but
require larger sensitivity matrices and longer solves than the small demo host
is intended to serve.

## Explicit Fallback

The backend serves whichever complete demo cases are staged. If no complete
case pair is staged, it exits unless `TELLEGEN_ALLOW_FALLBACK=1` is set. CI and
local smoke checks use that fallback to serve two pglib cases with synthetic
coordinates. Those fallback coordinates are labeled as synthetic.
