# Data Provenance

The demo cases are synthetic grids from TAMU ACTIVSg and CATS. They are
fictional networks built on geographic footprints. ACTIVSg200 and ACTIVSg500
coordinates come from PowerWorld aux exports. ACTIVSg7000 and CATS coordinates
come from GIS bus CSVs. These positions are not surveyed infrastructure
locations.

| case | territory | buses | branches | files |
|---|---|---:|---:|---|
| ACTIVSg200 | central Illinois | 200 | 245 | `case_ACTIVSg200.m` + `ACTIVSg200.aux` |
| ACTIVSg500 | South Carolina | 500 | 597 | `case_ACTIVSg500.m` + `ACTIVSg500.aux` |
| ACTIVSg7000 | Texas | 6717 | 9140 | `Texas7k_20210804.m` + `Texas7k_lat_long.csv` |
| CATS | California | 8870 | 10823 | `CaliforniaTestSystem.m` + `CATS_buses.csv` + `CATS_lines.json` |

The MATPOWER file feeds the DC OPF. For ACTIVSg200 and ACTIVSg500, the aux file
supplies the coordinates. For ACTIVSg7000, the bus coordinate CSV supplies
latitude and longitude. Operators download the ACTIVSg distributions from
[electricgrids.engr.tamu.edu](https://electricgrids.engr.tamu.edu/) and stage
them with `scripts/stage-data.sh`; the repository does not vendor them.

CATS comes from the
[WISPO POP CATS repository](https://github.com/WISPO-POP/CATS-CaliforniaTestSystem).
The server reads `CaliforniaTestSystem.m` for the network, `CATS_buses.csv`
for bus latitude and longitude, and `CATS_lines.json` for branch paths. The
staging script also copies `CATS_gens.csv` when present; the current map does
not render a separate generator geometry layer.

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

## Explicit Fallback

The embedded fallback ([deployment](deployment.md)) serves two PGLib cases
whose coordinates are synthetic and labeled as synthetic.
