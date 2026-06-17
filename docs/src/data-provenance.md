# Data Provenance

The demo cases are TAMU ACTIVSg synthetic grids. They are fictional networks
built on geographic footprints and include latitude and longitude fields in the
PowerWorld aux export. These positions come from the TAMU datasets; they are
not surveyed infrastructure locations.

| case | territory | buses | branches | files |
|---|---|---:|---:|---|
| ACTIVSg200 | central Illinois | 200 | 245 | `case_ACTIVSg200.m` + `ACTIVSg200.aux` |
| ACTIVSg500 | South Carolina | 500 | 597 | `case_ACTIVSg500.m` + `ACTIVSg500.aux` |
| ACTIVSg2000 | Texas | 2000 | 3206 | `case_ACTIVSg2000.m` + `ACTIVSg2000.aux` |

The MATPOWER file feeds the DC OPF. The aux file supplies the coordinates. Both
files come from the same distribution, so bus numbering matches. Operators
download the distributions from
[electricgrids.engr.tamu.edu](https://electricgrids.engr.tamu.edu/) and stage
them with `scripts/stage-data.sh`; the repository does not vendor them.

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

ACTIVSg2000 is the largest bundled case. On the current demo host it takes about
1.4 s per exact solve and uses a 32 MB dense sensitivity cache. Larger cases,
such as Texas7k, parse but require larger sensitivity matrices and longer
solves than the small demo host is intended to serve.

## Explicit Fallback

Production expects all three TAMU cases to be staged. If they are missing, the
server exits. CI and local smoke checks can set `TELLEGEN_ALLOW_FALLBACK=1` to
serve two pglib cases with synthetic coordinates. Those fallback coordinates are
labeled as synthetic.
