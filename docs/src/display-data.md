# Display Data

Case files and display files are separate inputs. MATPOWER `.m`, PSS/E `.raw`,
and PowerWorld `.aux` files describe the network. JSON files are accepted as
geographic files in this release. PowerWorld `.pwd` files describe a one-line
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
longitude. TAMU-generated diagrams use Web Mercator scaled by one constant,
`K = 535.81608`, with both axes expressed in degrees:

```text
x = K * lon
y = K * mercdeg(lat)
mercdeg(lat) = (180 / pi) * ln(tan(pi / 4 + lat * pi / 360))
```

The viewer applies the inverse transform in `pwdToLngLat` in
`packages/svelte/src/lib/controller.svelte.ts`. This places checked ACTIVSg
diagrams within about 0.02 degrees of their corresponding named cities.
Hand-edited diagrams can differ, so tellegen labels these positions as approximate.

## Canonical display format (planned)

A canonical display format is planned in powerio as a `DisplayData` variant, so
the format is available to Rust, browser wasm, and Python bindings; tellegen
will consume and render it rather than define a separate file format. The open
design questions are whether it stores substation coordinates, bus coordinates,
branch routing, and style hints; whether coordinates are geographic or diagram
based; how elements are referenced (id, substation number, name, or a combined
key); and whether display data embeds in a case JSON file or stays a separate
geographic file.

Existing `.pwd` parsing gives powerio a migration path from PowerWorld
diagrams. Planned on top of the format: filling missing case coordinates from a
dropped `.pwd` sibling when the case has a bus-to-substation mapping, combining
a dropped case and its `.pwd` into one local entry, and exporting coordinates
computed by tellegen.
