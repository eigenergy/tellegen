# Coordinate Sidecars

Some case files contain network topology but keep map coordinates in separate
GIS files. tellegen accepts those files as local sidecars: drop the case file
with one or more `.csv`, `.json`, or `.geojson` files, or drop the sidecars
after selecting a parsed local case.

All files stay in the browser. The server does not receive dropped case files
or sidecars.

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
substation symbols, but they are not assumed to map one-to-one onto buses in a
case file.
