# Geographic data and drawings

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

## Shared GeoJSON

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
