# Local Case Files

Dropped case files stay in the browser. The current public demo does not upload
local `.m`, `.raw`, `.aux`, `.epc`, `.pwb`, `.dss`, `.pwd`, `.csv`, `.json`,
or `.geojson` files.

MATPOWER `.m`, PSS/E `.raw`, PowerWorld `.aux`/`.pwb`, and PSLF `.epc` files
describe balanced network topology; OpenDSS `.dss` opens the multiconductor
viewer. If a balanced case includes complete coordinates, tellegen draws it
directly. Otherwise it creates a local synthetic layout and asks the user to
place it on the map. PowerIO classifies dropped JSON by content. A stored
PowerIO IR (`pio-ir`) document opens by its value type: a balanced network or
DC OPF instance becomes a case, while supported multiconductor networks,
instances, and solutions open in the viewer. AC PF and AC OPF instance modules
retain their declared calculation and original problem definition. AC PF is
available in the ordinary browser build; AC OPF solving requires the
[experimental worker](formulations.md#acopf-pounce-decision-browser).
Without that worker, AC OPF instances can be viewed but cannot be solved.
Transmission exchange JSON opens as a balanced case, and BMOPF or
PowerModelsDistribution documents open the multiconductor viewer. The retired
bare model-JSON shape is unrecognized; regenerate it from source data as
`pio-ir`. Tellegen does not define a separate saved case format. Other
unrecognized JSON falls through to the geographic reader.

After a parsed local case has coordinates, either from the file, a geographic
file, or manual placement, tellegen solves the selected formulation in browser
WebAssembly. Local case files do not call the tellegen backend solve endpoints.

Builds with PowerIO collection support also offer ZIP and folder imports. The
chooser preserves shared paths, lets you select independent cases, and retains
completed imports if another selection fails. Imported collection cases keep
their saved calculation settings and wait for an explicit calculation. Switching
tabs does not start a solve. The standard build hides these optional
controls; individual case files continue to open normally.

## Manual Placement

For case files with no coordinates, such as a plain `case14.m`, tellegen
computes a deterministic topology layout from buses and in-service branches. The user then
clicks the map to center that synthetic layout at the chosen location.

The placed local case can be moved later with the case panel move action. The
first version uses explicit click placement rather than drag movement.

After placement, the local case enters the same bus selection and demand slider
workflow as the bundled demo cases. The solve card reports the browser solve
time and does not show backend iterations.

## Geographic Files

Some case files contain network topology but keep map coordinates in separate
GIS files. tellegen accepts those files as local geographic files: drop the case file
with one or more `.csv`, `.json`, or `.geojson` files, or drop the geographic files
after selecting a parsed local case.

All files stay in the browser. The tellegen backend does not receive dropped
case files or geographic files.

Parsing is powerio's `GeoLayer` tolerant reader, running in WebAssembly: it
accepts headerless OpenDSS buscoords CSV, CSV and JSON records with the aliased
field names below, and GeoJSON `Point`/`LineString` features, and it rejects
input carrying no usable coordinates. Applied coordinates land on the network
itself (`Bus.location`, `Branch.route`), so a saved PowerIO case module or an
exported case carries the placement on screen. Loading that module starts a
fresh runtime `Study` at the saved point. Its history records transformations
applied to the stored value. The case panel can also download the current
layout as a `.geo.json` layer that PowerIO and Tellegen read back.

## Bus Coordinates

The geographic file must identify buses by the same ids used in the case file. CSV and
JSON records can use these field names:

| Meaning   | Accepted fields                                        |
| --------- | ------------------------------------------------------ |
| Bus id    | `bus_i`, `bus`, `bus_id`, `bus number`, `number`, `id` |
| Latitude  | `lat`, `latitude`, `y`                                 |
| Longitude | `lon`, `lng`, `longitude`, `x`                         |

Example:

```csv
bus_i,Lat,Lon
1,37.77243572,-122.2429162
2,37.77848161,-121.6259513
```

A geographic file does not need to cover every bus: matched buses place, and
the panel reports the matched and unmatched counts. Buses left without
coordinates are omitted from the map with a warning; a file that matches
nothing is rejected and the case stays placeable. Records can also match by
powerio row uid (`buses:3`) or by case insensitive bus name.

## Branch Paths

Branch geometry is optional. Without branch paths, tellegen draws straight
segments between placed buses.

CSV and JSON branch records can use:

| Meaning              | Accepted fields                                         |
| -------------------- | ------------------------------------------------------- |
| Branch id            | `branch`, `branch_id`, `branch number`, `cats_id`, `id` |
| From bus             | `f_bus`, `from`, `from_bus`, `bus_from`                             |
| To bus               | `t_bus`, `to`, `to_bus`, `bus_to`                                 |
| Endpoint coordinates | `Lat1`, `Lon1`, `Lat2`, `Lon2` and lowercase variants   |

GeoJSON `LineString` features are also accepted. The reader matches a route by
uid or branch id when present, then by the unordered from/to bus pair. A
`LineString` endpoint can also provide bus coordinates for its `f_bus` and
`t_bus` properties. Applied routes land in `Branch.route` and render as
polylines instead of straight segments.

## Piecewise Costs

A dropped case with MATPOWER model 1 piecewise linear generator costs solves
the declared convex curve exactly ([formulations](formulations.md)). Malformed
rows and rows with decreasing segment slopes are rejected before a solve.

## Display Files

PowerWorld `.pwd` files supply drawing positions and branch paths. Drop one
with its case or attach it to the explicitly selected case. Tellegen reports
matched buses, matched routes, and unmatched objects. A file with no supported
equipment positions returns an error.

Drawings use a plain canvas with pan, zoom, and fit controls. Geographic files
use the map. When both are supplied, **Map** and **Diagram** select the view;
neither replaces the other. A later attachment to a demo case creates a local
copy and preserves its current edits and result.

Supported New England PWB files include geographic bus locations. Their RAW
revision 33 companions lack those records and need a matching geographic
sidecar. Arbitrary drawing positions are never treated as latitude/longitude.


<a id="collection-import"></a>

<a id="collection-import-zip-and-folder-case-collections"></a>

## ZIP and folder case collections

The published browser package includes ZIP and folder imports. Custom builds can
omit the `powerio-collections` feature; hosts can query `collectionSupported()`
before offering these controls.

Drop a ZIP onto Tellegen, choose it with **Open a case file**, or use **Open
folder**. Folder drops preserve relative paths. Each ZIP has its own namespace:
a missing OpenDSS include cannot be supplied by another uploaded archive.

The chooser lists candidate paths and formats before parsing. Select the cases
you want and choose **Import selected cases**. Several masters may share the
same topology or profile files, and a MATPOWER or JSON comparison case remains
independently selectable. Identical circuit names do not hide their source paths.
Import does not run every discovered case. Close the chooser to inspect a case
and choose its calculation.

Successful imports remain available when another selection fails or you cancel
the remaining work. Missing dependencies are reported separately from malformed
cases and solver restrictions. Distribution cases show their existing snapshot
power-flow capability and the reason when they are view-only. Transport success
does not enable unsupported controls, load laws or transformer models.

Use **Choose a nonstandard entry or document group** for an unusual master
suffix or a format that needs an explicit choice. XML is not assumed to be one
CGMES network: choose XIIDM for independent networks, or explicitly group the
profiles belonging to one CGMES case. GO problem/solution grouping is likewise
explicit. For a ZIP inside a ZIP, choose **Open ZIP** beside its path. The inner
archive appears as a separate collection; its cases cannot read files from its
parent or siblings. Each archive is opened only when requested, up to two ZIP
layers. Format-owned native ZIPs can be selected with their reader
format. Unsupported structural values, such as time series in a snapshot-only
view, are reported rather than flattened.

Files are read locally. PowerIO owns discovery, dependency resolution and
electrical parsing; no OpenDSS commands are executed by the browser and no
missing file is fetched from the host or network. Geographic sidecars are not
attached to every case in a mixed archive. Import first, select the intended
case, and attach its geographic file using the existing sidecar workflow.

The chooser limits one batch to 4096 members and 64 MiB of uploaded/expanded
file bytes, at most 32 ZIP uploads and 32 selected imports per operation. ZIP
members have a 200:1 expansion-ratio limit. PowerIO also limits inspection and
the work per operation. Parent and child archives share the limits on retained
file counts and expanded bytes.
Decoded electrical models may use more memory than their source files. Parsing
is serial. Cancellation terminates the collection worker; on a synchronous
fallback it discards the result when the current operation returns.

While the chooser is open, **Download original ZIP** returns the exact uploaded
bytes. **Export selected source ZIP** makes a new archive with an explicit scope:
selected members alone, or the entire permitted project (which can include
other cases). A single master by itself need not be a complete project. Closing
the chooser releases source collections. Imported PowerIO modules retain
structured source origin and diagnostics but do not embed the archive; saving
and restoring a module does not restore an original-ZIP download.

<a id="collection-import-opendss-snapshot-power-flow"></a>

### OpenDSS snapshot power flow

Qualified OpenDSS imports can run **Studies → Run power flow** after import.
Existing typed module and BMOPF solving remain available under the same
validation rules.
The initial profile covers normalized finite sources with grounded references,
explicit conductor impedances, bounded ZIP and exponential loads (including
OpenDSS voltage bands and ZIP dropout), and supported finite-leakage transformers.
An explicit fourth neutral conductor is supported. Source-defined load bounds
always take precedence over the optional low- and high-voltage fallback; disabling that option
does not remove the source model's own voltage bounds.

Passive definitions, profiles and monitoring records remain in the module.
This is a fixed equipment snapshot: daily/yearly simulation, control execution,
and an OpenDSS script's sequence of solves are not run. For retained regulator
controls, **Use written equipment settings** creates a separate case using the
written taps; it retains the original case and records the choice when saved.
Regulator and capacitor controls are never executed. Missing control targets,
unrelated invalid records and unsupported stepped-capacitor settings still
prevent a calculation.

Finite two- and three-winding transformers support the qualified connections,
taps, neutral settings and small grounding admittance. Unresolved geometry or
dependencies, floating source references, generator/IBR injections,
voltage-dependent loads without a nominal voltage, and unqualified transformer
settings remain view-only with a reason. Convergence and operating voltage-limit compliance are
reported separately. A runnable model can still fail to converge.

<a id="data-sources"></a>

<a id="data-sources-data-sources"></a>

## Data sources

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

<a id="data-sources-aux-coordinates"></a>

### Aux coordinates

PowerWorld aux exports have used two coordinate layouts:

- ACTIVSg complete case exports repeat substation latitude and longitude on each
  bus row in `Latitude:1` and `Longitude:1`.
- Later exports can leave the bus latitude and longitude columns empty and
  reference the `Substation` table through `SubNumber`.

PowerIO promotes the bus columns (`Latitude:1`/`Longitude:1`) into the typed
`Bus.location` at parse; tellegen keeps the two fallbacks upstream does not
cover — bare `Latitude`/`Longitude` bus columns and the `Substation` table join
— so dropped files using either layout resolve when the data is present.

<a id="data-sources-buses-sharing-coordinates"></a>

### Buses Sharing Coordinates

Multiple buses can share one substation coordinate. tellegen spreads each group
on a deterministic ring of about 400 m around the substation point, ordered by
bus id. The group remains visually associated with the substation at network
zoom, and individual buses remain hoverable at street zoom.

<a id="data-sources-explicit-fallback"></a>

### Explicit Fallback

The embedded fallback ([deployment](deployment.md)) serves two PGLib cases
whose coordinates are synthetic and labeled as synthetic.

<a id="data-sources-prepared-generator-costs"></a>

### Prepared generator costs

The Texas7k demo selects Tellegen's reusable convex quadratic/linear fitting
option. It fits each piecewise curve by least squares and retains a linear fit
when the quadratic fit is nonconvex or singular. Other demo cases keep their
declared costs. Model details retain the source and prepared cost rows and
report RMS and maximum errors at the source breakpoints.

All four configured cases must load and solve for the deployment health check
to pass. Unavailable cases remain in the catalogue with an explanation.

<a id="data-sources-synthetic-generation-inputs"></a>

### Synthetic generation inputs

Generated grids read the populated places table shipped with
`tellegen-synth`: GeoNames `cities5000` (CC BY 4.0), 69,999 places of 5,000
people or more with name, position, population and country, derived by
`scripts/derive-populated-places.py`. Every place sizes and spreads demand; a
transmission-only grid sites its buses on the places of 50,000 people or more,
or on every place when a region has none that large. Generated buses are
labelled with coordinate source information `synthetic`. OpenStreetMap mode reads the
Overpass answer the caller fetched and labels its buses `source`. Its
electrical parameters and loads are modelled, so its buses and lines are
mapped, not surveyed. See
[Synthetic Grid Generation](synthetic-generation.md).

<a id="data-sources-data-sources-1"></a>

#### Data sources

Every table the generator ships names its source and the SHA-256 of the file
it was derived from. The generation report repeats these source details, so a
host can show where each figure came from without maintaining a table of its own.

| Table | Source | Licence | Reported source details |
| --- | --- | --- | --- |
| `data/populated_places.json` | GeoNames `cities5000` and `countryInfo` ([geonames.org](https://www.geonames.org/)) | [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/), attribution required | Place source, original and compiled file hashes, and count |
| `data/national_demand.json` | Ember, *Yearly Electricity Data*, through Our World in Data's energy dataset (demand and population) | CC BY 4.0 | Demand source, licence, year, file hashes, and peak factor |
| `data/country_areas.json` | Natural Earth v5.1.2, 1:50m admin-0 countries | Public domain | Original and compiled area file hashes |
| `data/voltage_ladders.json` | National transmission operators' and regulators' documents, the ENTSO-E members list and IEC 60038, cited per entry; copies no dataset | Compiled for tellegen | Voltage table version and file hash |
| `data/subnational_demand.json`, `data/subnational_fuel_mix.json` | EIA-861, EIA-860 and the US Census Bureau's state population estimates | Public domain (US government works) | `context.demand_by_subdivision`, `context.fuel_mix_basis` |
| OpenStreetMap mode | The Overpass answer the caller fetched (`power=*` features) | [ODbL 1.0](https://opendatacommons.org/licenses/odbl/1-0/), © OpenStreetMap contributors | OpenStreetMap document hash, element count, timestamp, and copyright |

The `country_areas.json` keys are Natural Earth's `ADM0_A3`, and the places
table carries the same code, so every lookup across the tables agrees.

<a id="display-data"></a>

<a id="display-data-geographic-data-and-drawings"></a>

## Geographic data and drawings

Electrical case files describe equipment and operating values. Geographic and
drawing files describe where to display that equipment. Tellegen uses PowerIO's
`GeoLayer` for both, keeping the coordinate system explicit.

GeoJSON, CSV, and AUX files can supply geographic bus positions and line routes.
PowerWorld PWB files can also contain geographic bus locations. PowerWorld PWD
files supply drawing positions and polylines. A drawing appears on a plain
canvas; geographic coordinates appear on the map.

Drop companion files together, or select a case before attaching another file.
The attachment report lists matched equipment and unmatched objects. A later
attachment to a demo creates a local copy, preserving current demand changes,
line capacities, and the current result. No unrelated recent case is selected.

When a case has both geographic data and a drawing, use **Map** or **Diagram**
to choose the view. Save a Study to retain both layers, line paths, and the
current camera position. Drawing coordinates stay in their source units.

<a id="display-data-shared-geojson"></a>

### Shared GeoJSON

A GeoLayer uses a FeatureCollection with Point and LineString features. The
`powerio_geo` member records geographic, projected, drawing, or unknown
coordinates. Equipment matches by stable identity, source ID, name, or branch
endpoints. BMOPFTools `bus_from` and `bus_to` names are accepted.

PowerIO reads the proposed BMOPFTools embedded `bus[id].geo` and `line[id].geo`
values. Explicit BMOPF output retains geometry under `extras.geojson`, which
is permitted by the currently identified draft schema. This supports edited
coordinates without claiming that the Task Force has approved new schema fields.

**Download geography** exports geographic positions and routes. **Download
drawing** exports the selected drawing as a GeoLayer. Both can be reopened by
other PowerIO consumers.

<a id="antimeridian-routes"></a>

<a id="antimeridian-routes-routes-across-the-antimeridian"></a>

## Routes across the antimeridian

Geographic route coordinates are stored with longitudes in [-180, 180].
`tellegen-synth` writes every bus location and branch route vertex that way.
A route that crosses ±180 is not split at the seam, so two consecutive
vertices can sit almost 360° apart: a line in Chukotka may run from 179.6 to
-179.7.

A map consumer draws the short hop between consecutive vertices. When two
longitudes differ by more than 180°, move the second by a whole multiple of
360° until they differ by 180° or less, and measure the next vertex from the
moved value. Joined as stored, that segment runs the long way round, and a
route near ±180 appears to span the whole world.

`@tellegen/svelte` does this in its route layers on the flat map and on the
globe, in the selected-branch outline, and when it frames a selected branch.
The moved longitudes are for drawing only. Saved modules and exported GeoJSON
keep the stored values.
