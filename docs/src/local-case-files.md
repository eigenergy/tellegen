# Local Case Files

Dropped case files stay in the browser. The current public demo does not upload
local `.m`, `.raw`, `.aux`, `.pwd`, `.csv`, `.json`, or `.geojson` files.

MATPOWER `.m`, PSS/E `.raw`, and PowerWorld `.aux` files describe network
topology. If a case file includes complete coordinates, tellegen draws it
directly. If coordinates are missing, tellegen creates a local synthetic layout
and asks the user to place it on the map. JSON files are treated as coordinate
sidecars in this release, not as network case files.

After a parsed local case has coordinates, either from the file, a sidecar, or
manual placement, tellegen solves the DC OPF in browser WebAssembly. Local case
files do not call the tellegen backend solve endpoints.

## Manual Placement

For no coordinate files such as a plain `case14.m`, tellegen computes a
deterministic topology layout from buses and in service branches. The user then
clicks the map to center that synthetic layout at the chosen location.

The placed local case can be moved later with the case panel move action. The
first version uses explicit click placement rather than drag movement.

After placement, the local case enters the same bus selection and demand slider
workflow as the bundled demo cases. The solve card reports the browser solve
time and does not show backend iterations.

## Coordinate Sidecars

Some case files contain network topology but keep map coordinates in separate
GIS files. tellegen accepts those files as local sidecars: drop the case file
with one or more `.csv`, `.json`, or `.geojson` files, or drop the sidecars
after selecting a parsed local case.

All files stay in the browser. The tellegen backend does not receive dropped
case files or sidecars.

## Bus Coordinates

The sidecar must identify buses by the same ids used in the case file. CSV and
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

tellegen requires coordinates for every bus before it draws a geographic local
case. If the sidecar is incomplete, the local case stays in manual placement
mode and the panel lists the first missing buses.

## Branch Paths

Branch geometry is optional. Without branch paths, tellegen draws straight
segments between placed buses.

CSV and JSON branch records can use:

| Meaning | Accepted fields |
| --- | --- |
| Branch id | `branch`, `branch_id`, `branch number`, `cats_id`, `id` |
| From bus | `f_bus`, `from`, `from_bus` |
| To bus | `t_bus`, `to`, `to_bus` |
| Endpoint coordinates | `Lat1`, `Lon1`, `Lat2`, `Lon2` and lower case variants |

GeoJSON `LineString` features are also accepted. The parser matches a path by
branch id when present, then by from/to bus ids. A `LineString` endpoint can
also provide bus coordinates for its `f_bus` and `t_bus` properties.

## Display Files

PowerWorld `.pwd` files are still treated as display overlays. They can show
substation symbols, but they are not assumed to map one to one onto buses in a
case file.
