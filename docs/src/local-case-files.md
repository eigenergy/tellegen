# Local Case Files

Dropped case files stay in the browser. The current public demo does not upload
local `.m`, `.raw`, `.aux`, `.pwd`, `.csv`, `.json`, or `.geojson` files.

MATPOWER `.m`, PSS/E `.raw`, and PowerWorld `.aux` files describe network
topology. If a case file includes complete coordinates, tellegen draws it
directly. If coordinates are missing, tellegen creates a local synthetic layout
and asks the user to place it on the map. Dropped JSON is content sniffed: a
`.pio.json` study package restores, a BMOPF or PowerModelsDistribution document
opens the multiconductor viewer, and anything else is read as a geographic
file.

After a parsed local case has coordinates, either from the file, a geographic file, or
manual placement, tellegen solves the DC OPF in browser WebAssembly. Local case
files do not call the tellegen backend solve endpoints.

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
itself (`Bus.location`, `Branch.route`), so a saved study package or an
exported case carries exactly the placement on screen. The case panel can also
download the current layout as a `.geo.json` layer — canonical GeoJSON with
provenance stamped (`synthetic` or `manual` for layouts) that powerio and
tellegen read back.

## Bus Coordinates

The geographic file must identify buses by the same ids used in the case file. CSV and
JSON records can use these field names:

| Meaning | Accepted fields |
| --- | --- |
| Bus id | `bus_i`, `bus`, `bus_id`, `bus number`, `number`, `id` |
| Latitude | `lat`, `latitude`, `y` |
| Longitude | `lon`, `lng`, `longitude`, `x` |

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

| Meaning | Accepted fields |
| --- | --- |
| Branch id | `branch`, `branch_id`, `branch number`, `cats_id`, `id` |
| From bus | `f_bus`, `from`, `from_bus` |
| To bus | `t_bus`, `to`, `to_bus` |
| Endpoint coordinates | `Lat1`, `Lon1`, `Lat2`, `Lon2` and lowercase variants |

GeoJSON `LineString` features are also accepted. The reader matches a route by
uid or branch id when present, then by the unordered from/to bus pair. A
`LineString` endpoint can also provide bus coordinates for its `f_bus` and
`t_bus` properties. Applied routes land in `Branch.route` and render as
polylines instead of straight segments.

## Piecewise Costs

A dropped case with MATPOWER model 1 piecewise linear generator costs solves
against a least squares quadratic fit of its breakpoints
([formulations](formulations.md)), so its objective and prices differ from a
solver that carries the piecewise curve exactly.

## Display Files

PowerWorld `.pwd` files are display overlays. Dropped alone, they show
substation symbols at approximate projected positions. Dropped alongside a
case file that has no coordinates, the substation points join onto buses
through the `SubNum` field on the bus rows and fill the case's positions; a
`.pwd` whose numbers match nothing stays a separate overlay entry.
