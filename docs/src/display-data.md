# Display Data

Case files and display files are separate inputs. MATPOWER `.m`, PSS/E `.raw`,
and PowerWorld `.aux` files describe the network. JSON files are accepted as
geographic files in this release. PowerWorld `.pwd` files describe a one-line
diagram. tellegen reads coordinates from case files when they exist and reads
diagram positions from display files when they are dropped.

PowerIO keeps display data separate from network parsing.
`powerio::parse_display` and `powerio::parse_display_file` return
`DisplayData::PowerWorld(PwdDisplay)` for `.pwd` inputs. `parse_file` does not
accept `.pwd`.

## Reading `.pwd`

`crates/tellegen-wasm/src/lib.rs` exports `parse_display(bytes, format)` over
`powerio::parse_display`. The frontend reads `.pwd` files with
`arrayBuffer()` and passes a `Uint8Array` to the WebAssembly module.

A dropped `.pwd` creates a local display entry with substation points only. It
does not create buses, branches, or a solvable case.

## Projecting `.pwd` coordinates

PowerWorld `.pwd` coordinates are diagram coordinates, not latitude and
longitude. TAMU-generated diagrams use Web Mercator scaled by one constant,
`K = 535.81608`, with both axes expressed in degrees:

```text
x = K * lon
y = K * mercdeg(lat)
mercdeg(lat) = (180 / pi) * ln(tan(pi / 4 + lat * pi / 360))
```

The inverse transform is PowerIO's `to_lonlat_from_pwd_mercator`; the wasm
`parse_display` export returns each substation with its projected `lon`/`lat`
alongside the raw diagram coordinates, so the frontend never reimplements the
constant. This places checked ACTIVSg diagrams within about 0.02 degrees of
their corresponding named cities. Hand-edited diagrams can differ, so tellegen
labels these positions as approximate.

## The canonical geographic document

PowerIO's standalone geographic document (`GeoLayer`) carries element
points and branch routes in one coordinate space, keyed by uid, external id,
name, or the branch endpoint pair, written as a GeoJSON `FeatureCollection`
with the `powerio_geo` foreign member (`.geo.json`). tellegen consumes it
rather than defining a separate format: dropped geographic sidecars parse
through its tolerant reader, applied coordinates land on the network
(`Bus.location`, `Branch.route`), and the case panel exports the current
layout as a `.geo.json` layer with provenance stamped.

On top of it, a dropped `.pwd` sibling fills missing case coordinates through
the `SubNum` join (`to_geo_layer_from_pwd` + `apply_substation_points`, projected
by `to_lonlat_from_pwd_mercator`), and layouts computed by tellegen export with
`synthetic`/`manual` provenance.
