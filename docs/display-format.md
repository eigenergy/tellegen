# Display data in tellegen

Case formats divide on whether they carry geometry. PowerWorld `.aux` exports
keep substation latitude and longitude ([real-coordinates.md](real-coordinates.md));
PowerWorld `.pwd` files hold the one-line diagram. MATPOWER `.m`, PSS/E `.raw`,
and PowerModels and egret JSON carry topology and electrical data with no
positions. tellegen reads coordinates when a file supplies them and falls back
to a [synthetic layout](synthetic-layout.md) otherwise.

powerio v0.2.2 added a display API separate from network parsing.
`parse_display_bytes` and `parse_display_file` return
`DisplayData::PowerWorld(PwdDisplay)`, substation symbols at diagram
coordinates. `DisplayFormat` and `DisplayData` are `#[non_exhaustive]`, one
variant each, leaving room for further formats. `parse_file` no longer accepts
a `.pwd`.

## Reading `.pwd`

`wasm/src/lib.rs` exports `parse_display(bytes, format)` over
`powerio::parse_display_bytes`. A `.pwd` is binary, so the frontend reads it
with `arrayBuffer()` and passes a `Uint8Array`. Dropping a `.pwd` adds a local
entry with substation points only: no buses, no branches, nothing to solve.
The points render in a cooler hue than a parsed case, labeled to mark the
positions approximate.

## Projecting `.pwd` coordinates

A `.pwd` stores diagram positions. TAMU's generated layouts are Web Mercator
scaled by one constant `K = 535.81608`, both axes in degrees:

```
x = K * lon
y = K * mercdeg(lat),   mercdeg(lat) = (180/pi) * ln(tan(pi/4 + lat*pi/360))
```

So `lon = x/K`, and latitude is the inverse gudermannian of `y/K` after the
degree to radian conversion (`frontend/src/routes/+page.svelte`,
`pwdToLngLat`). Decoding the bundled `ACTIVSg200.pwd` (central Illinois, 111
substations) and a Texas `ACTIVSg2000.pwd` (1250 substations) places both
within about 0.02 degrees of the named cities. Symbols moved by hand, and some
older exports, drift from the constant, so tellegen labels the positions
approximate. A display format that stored the projection with the coordinates
would remove the constant from tellegen.

## A canonical display format

tellegen is the browser layer of powerio. powerio parses and encodes case and
display formats; tellegen renders them reactively and depends on powerio. The
open question is where the boundary sits: what a display format must encode,
and which side owns it.

The format belongs in powerio, as a `DisplayData` variant beside `PowerWorld`.
A format defined there reaches every powerio binding: Rust, the browser through
tellegen, Python, and Julia through PowerIO.jl. powerio already reads `.pwd`,
so the first writer can transcode an existing diagram. tellegen consumes the
format and renders it, building the Rust it needs against powerio. There is no
separate published crate.

Open questions for the format:

- Contents. Substation and bus coordinates at minimum; optionally branch
  routing, one-line diagram geometry, and style hints.
- Coordinate model. Latitude and longitude, or diagram coordinates with the
  projection stored alongside. The `.pwd` constant above is the case for
  storing the projection.
- Element references. Bus id, substation number, or name. Ids are exact and
  break across renumbering; names survive renumbering and collide.
- Carrier. A sidecar paired with the case, the way `.aux` and `.pwd` pair
  today, or display fields embedded in a case JSON.
- Migration. powerio reads `.pwd`, giving existing PowerWorld layouts a path
  into the new format.

## Deferred in tellegen

- Fill a case's missing coordinates from a dropped `.pwd` sibling through the
  bus to substation mapping (`SubNum` in a PowerWorld aux). The TAMU auxes
  self-place, so this covers auxes with `SubNum` but no latitude or longitude.
- Fold a dropped case and its `.pwd` into one entry instead of two drops.
- Write the coordinates tellegen computed, real or synthetic, in the canonical
  format, so a bare `.m` re-drops already placed.
