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

## Aux coordinates

PowerWorld aux exports have used two coordinate layouts:

- ACTIVSg complete case exports repeat substation latitude and longitude on each
  bus row in `Latitude:1` and `Longitude:1`.
- Later exports can leave the bus latitude and longitude columns empty and
  reference the `Substation` table through `SubNumber`.

PowerIO promotes the bus columns (`Latitude:1`/`Longitude:1`) into the typed
`Bus.location` at parse; tellegen keeps the two fallbacks upstream does not
cover — bare `Latitude`/`Longitude` bus columns and the `Substation` table join
— so dropped files using either layout resolve when the data is present.

## Buses Sharing Coordinates

Multiple buses can share one substation coordinate. tellegen spreads each group
on a deterministic ring of about 400 m around the substation point, ordered by
bus id. The group remains visually associated with the substation at network
zoom, and individual buses remain hoverable at street zoom.

## Explicit Fallback

The embedded fallback ([deployment](deployment.md)) serves two PGLib cases
whose coordinates are synthetic and labeled as synthetic.

## Prepared generator costs

The Texas7k demo selects Tellegen's reusable convex quadratic/linear fitting
option. It fits each piecewise curve by least squares and retains a linear fit
when the quadratic fit is nonconvex or singular. Other demo cases keep their
declared costs. Model details retain the source and prepared cost rows and
report RMS and maximum errors at the source breakpoints.

All four configured cases must load and solve for the deployment health check
to pass. Unavailable cases remain in the catalogue with an explanation.

## Synthetic generation inputs

Generated grids read the populated places table shipped with
`tellegen-synth`: GeoNames `cities5000` (CC BY 4.0), 69,999 places of 5,000
people or more with name, position, population and country, derived by
`scripts/derive-populated-places.py`. Every place sizes and spreads demand; a
transmission-only grid sites its buses on the places of 50,000 people or more,
or on every place when a region has none that large. Generated buses are
labelled with coordinate provenance `synthetic`. OpenStreetMap mode reads the
Overpass answer the caller fetched and labels its buses `source`. Its
electrical parameters and loads are modelled, so its buses and lines are
mapped, not surveyed. See
[Synthetic Grid Generation](synthetic-generation.md).

### Data sources

Every table the generator ships names its source and the SHA-256 of the file
it was derived from, and the generation report repeats them under
`provenance`, so a host can show where each figure came from without a table
of its own.

| Table | Source | Licence | Report field |
| --- | --- | --- | --- |
| `data/populated_places.json` | GeoNames `cities5000` and `countryInfo` ([geonames.org](https://www.geonames.org/)) | [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/), attribution required | `provenance.places` (`source`, `source_sha256`, `count`, `sha256`) |
| `data/national_demand.json` | Ember, *Yearly Electricity Data*, through Our World in Data's energy dataset (demand and population) | CC BY 4.0 | `provenance.demand_table` (`source`, `licence`, `year`, `sha256`, `source_sha256`, `peak_factor`) |
| `data/country_areas.json` | Natural Earth v5.1.2, 1:50m admin-0 countries | Public domain | `provenance.demand_table` (`areas_sha256`, `areas_source_sha256`) |
| `data/voltage_ladders.json` | National transmission operators' and regulators' documents, the ENTSO-E members list and IEC 60038, cited per entry; copies no dataset | Compiled for tellegen | `provenance.voltage_table` (`version`, `sha256`) |
| `data/subnational_demand.json`, `data/subnational_fuel_mix.json` | EIA-861, EIA-860 and the US Census Bureau's state population estimates | Public domain (US government works) | `context.demand_by_subdivision`, `context.fuel_mix_basis` |
| OpenStreetMap mode | The Overpass answer the caller fetched (`power=*` features) | [ODbL 1.0](https://opendatacommons.org/licenses/odbl/1-0/), © OpenStreetMap contributors | `provenance.osm` (`document_sha256`, `elements`, `timestamp_osm_base`, `copyright`) |

The `country_areas.json` keys are Natural Earth's `ADM0_A3`, and the places
table carries the same code, so every lookup across the tables agrees.
