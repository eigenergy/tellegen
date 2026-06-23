# Display Data

Case files and display files are separate inputs. MATPOWER `.m`, PSS/E `.raw`,
and PowerWorld `.aux` files describe the network. JSON files are accepted as
geographic files in this release. PowerWorld `.pwd` files describe a one line
diagram. tellegen reads coordinates from case files when they exist and reads
diagram positions from display files when they are dropped.

powerio 0.2.2 added a display API separate from network parsing.
`parse_display_bytes` and `parse_display_file` return
`DisplayData::PowerWorld(PwdDisplay)` for `.pwd` inputs. `parse_file` does not
accept `.pwd`.

## Reading `.pwd`

`crates/tellegen-wasm/src/lib.rs` exports `parse_display(bytes, format)` over
`powerio::parse_display_bytes`. The frontend reads `.pwd` files with
`arrayBuffer()` and passes a `Uint8Array` to the WebAssembly module.

A dropped `.pwd` creates a local display entry with substation points only. It
does not create buses, branches, or a solvable case.

## Projecting `.pwd` coordinates

PowerWorld `.pwd` coordinates are diagram coordinates, not latitude and
longitude. TAMU generated diagrams use Web Mercator scaled by one constant,
`K = 535.81608`, with both axes expressed in degrees:

```text
x = K * lon
y = K * mercdeg(lat)
mercdeg(lat) = (180 / pi) * ln(tan(pi / 4 + lat * pi / 360))
```

The frontend applies the inverse transform in `pwdToLngLat` in
`frontend/src/routes/+page.svelte`. This places the checked ACTIVSg200 and
ACTIVSg2000 diagrams within about 0.02 degrees of their corresponding named
cities. Hand edited diagrams can differ, so tellegen labels these positions as
approximate.

## Canonical display format

A canonical display format belongs in powerio as a `DisplayData` variant. That
keeps the format available to Rust, browser wasm, Python, and Julia bindings.
tellegen should consume and render the format rather than define a separate file
format.

The format should decide:

- whether it stores substation coordinates, bus coordinates, branch routing, and
  style hints;
- whether coordinates are geographic or diagram based;
- whether elements are referenced by id, substation number, name, or a combined
  key;
- whether display data is embedded in a case JSON file or stored as a separate geographic file.

Existing `.pwd` parsing gives powerio a migration path from PowerWorld diagrams.

## Deferred tellegen work

- Fill missing case coordinates from a dropped `.pwd` sibling when the case has
  a bus to substation mapping.
- Combine a dropped case and corresponding `.pwd` into one local entry.
- Export coordinates computed by tellegen in the canonical display format.
