# Real substation coordinates

The demo cases are TAMU ACTIVSg synthetic grids: fictional networks built on
real service territories, with substation latitude and longitude included.
tellegen serves three of them, sized to run comfortably on a small VPS:

| case | territory | buses | branches | files |
|---|---|---|---|---|
| ACTIVSg200 | central Illinois | 200 | 245 | `case_ACTIVSg200.m` + `ACTIVSg200.aux` |
| ACTIVSg500 | South Carolina | 500 | 597 | `case_ACTIVSg500.m` + `ACTIVSg500.aux` |
| ACTIVSg2000 | Texas | 2000 | 3206 | `case_ACTIVSg2000.m` + `ACTIVSg2000.aux` |

The MATPOWER export feeds the DC OPF; the PowerWorld aux export supplies the
coordinates. Both come from the same distribution, so bus numbering matches.
The operator downloads the distributions from
[electricgrids.engr.tamu.edu](https://electricgrids.engr.tamu.edu/) and runs
`scripts/stage-data.sh`; the repository never vendors them.

## Where coordinates live in an aux file

PowerWorld exports have written substation coordinates two ways:

- 2018-era complete case exports (the ACTIVSg distributions) repeat the
  substation's latitude and longitude on every bus row, in the `Latitude:1`
  and `Longitude:1` columns.
- Later exports (for example the 2022 Hawaii40 distribution) leave the bus
  columns empty and reference the `Substation` table through `SubNumber`.

powerio parses the aux and keeps these columns in bus `extras`. The backend
(`backend/src/coords.jl`) reads the bus row form, which covers every bus in
the three served cases. The browser parser (`wasm/`) additionally performs
the substation join, so dropped files of either generation resolve.

## Co-located buses

Buses at one substation share its coordinate exactly (the 200 bus case has
several buses per substation). Each co-located group is spread on a ring of
about 400 m around the substation point, ordered by bus id, so every bus
stays individually hoverable at street zoom while the group still reads as
one substation at network zoom. The spread is deterministic; identical input
yields identical layouts.

## Sizing

ACTIVSg2000 is the practical ceiling for the demo host: about 1.4 s per
exact re-solve and a 32 MB dense sensitivity cache. Texas7k parses fine but
a 7000 column KKT sensitivity matrix and multi-second solves want more
machine than a small VPS.
