# Diagram performance gallery

A local deck.gl 9.4.0 / WebGL2 gallery for **1k, 10k and 100k buses**, using
precomputed schematic layouts, plus an optional locally staged BMOPF catalog. This isolates rendering and interaction from
layout computation. It complements the [electrical reference gallery](../gallery/index.html).

## Build and open

Install the repository's locked JavaScript dependencies first (`npm ci` from the
repository root), then use Node 22.18 or newer:

```sh
node docs/drafts/diagram-projection/performance/generate.mjs
node docs/drafts/diagram-projection/performance/verify.mjs
node docs/drafts/diagram-projection/performance/build.mjs
python3 -m http.server 8765 --bind 127.0.0.1 --directory docs/drafts/diagram-projection
```

Open http://127.0.0.1:8765/performance/ . The `?size=100000` query selects the
largest fixture. The original gallery remains at `/gallery/`.

An isolated worktree may reuse another installed checkout:

```sh
node docs/drafts/diagram-projection/performance/build.mjs /absolute/path/to/installed/tellegen
```

The build uses the workspace's deck.gl core/layers and Vite's installed Rolldown
bundler. No runtime CDN, account, geospatial basemap or network data service is
used. Generated `.bin` fixtures and `app.bundle.js` are ignored; their deterministic
sources, manifest hashes and build instructions are versioned. Rebuild after
source changes. `THIRD-PARTY-NOTICES.txt` contains the licenses of bundled packages;
they are not reimplemented here.

## Fixtures

| Buses | Branches | Open ties | Routed segments | Binary payload |
| ---: | ---: | ---: | ---: | ---: |
| 1,000 | 1,009 | 10 | 2,017 | 0.07 MiB |
| 10,000 | 10,099 | 100 | 20,206 | 0.74 MiB |
| 100,000 | 100,999 | 1,000 | 202,096 | 7.41 MiB |

Each 1,000-bus region contains a binary feeder tree and ten explicit open ties.
Region roots join a closed backbone. The closed graph has exactly N−1 branches
and is connected. IDs B000001…B100000 remain tied to source indices. Coordinates
and orthogonal line segments are computed by `generate.mjs`, never by the browser.
These are synthetic topology/performance fixtures, not PowerIO cases or solved
networks. They deliberately do not simulate transformers, poles or numerical
operating values. Crossings in this synthetic layout are not junctions; endpoints
are authoritative. The detailed electrical gallery remains the visual contract.

`verify.mjs` checks deterministic binary bytes and SHA-256, finite coordinates,
counts, endpoint-to-route mapping, and a connected acyclic closed graph distinct
from open ties. Binary sections use native typed arrays (little-endian on the
supported browser/build platforms); version 1 is a local fixture format, not a
portable PowerIO interchange format.

## External BMOPF datasets

Stage the supplied external collection with Node; it is never copied or symlinked
into this repository or the HTTP document root:

```sh
node docs/drafts/diagram-projection/performance/stage-bmopf.mjs /absolute/path/to/BMOPFDraftData/output
node --test docs/drafts/diagram-projection/performance/bmopf.test.mjs
node docs/drafts/diagram-projection/performance/verify-bmopf.mjs
node docs/drafts/diagram-projection/performance/build.mjs
```

The selector offers the three synthetic sizes, five individual SAPN cases,
three French representatives (smallest, median and largest by bus count), the Texas
pilot and **SAPN · combined collection**. `?dataset=sapn-combined` opens the collection;
legacy `?size=100000` links still work. Without local staging the synthetic gallery
remains available. All configured files must exist for staging to succeed.

| Source | Buses | Lines | Transformer objects |
| --- | ---: | ---: | ---: |
| SAPN Golden Grove | 25,658 | 25,634 | 621 |
| SAPN Norwood | 25,456 | 25,679 | 288 |
| SAPN Campbelltown | 27,239 | 27,803 | 220 |
| SAPN Hackham | 10,907 | 11,174 | 215 |
| SAPN Harrow | 9,373 | 9,532 | 117 |
| **SAPN combined** | **98,633** | **99,822** | **1,461** |
| French 44_MVFeeder2707 | 5 | 4 | 0 |
| French 75_MVFeeder0100 | 540 | 482 | 57 |
| French 53_MVFeeder0641 | 1,583 | 1,433 | 149 |
| Texas p1uhs0_1247 | 22,081 | 16,822 | 7,756 |

`bmopf-sources.json` records relative filenames, namespaces and source qualifications.
`bmopf-inventory.json` records SHA-256 of the exact input bytes and verified counts;
regeneration updates it, so review its diff when source data changes. Raw source
JSON and generated `local-data/` geometry/reference files are not committed. Keep
source licensing and attribution files with the external originals. No remote
uploads or background fetching are involved. Derived sidecars contain source IDs,
terminal information and original coordinates, so they remain local and ignored too.

### Adapter contract

This is a rendering-only adapter for the inspected BMOPF shapes, not the canonical
PowerIO importer or an electrical solver. Keyed buses and lines are supported,
as are two-endpoint `delta_wye`, `single_phase` and `center_tap` transformer tables
(the staged cases exercise the first two), and bus-attached loads, shunts and
voltage sources. Nonempty unknown equipment tables, switches, missing coordinates,
dangling buses, unknown terminals, unequal line terminal-map lengths and unsupported
or contradictory enabled/status values fail explicitly. Absent state means enabled.
Electrical parameters, ratings, results and full source metadata are not copied.

A reference sidecar preserves the **exact original IDs**, original longitude and
latitude, terminal order, grounded-terminal declarations, endpoint mappings,
transformer family and attachment references. Identities are tuples
`[source namespace, family, transformer kind or "", original ID]`. Dense indices
are local buffer addresses, never source identities. Transformer entries render
as violet **equipment associations**, not conductor continuity between windings.
Their count includes model legs and is not a count of physical transformers.
Neutral is not inferred from a terminal number; the source terminal and grounding
declarations remain authoritative for a later multi-wire projection.

Geographic coordinates are projected into local equirectangular metres, north up.
Edges connect endpoints with straight display segments; source route polylines are
not imported. This is a geographic-coordinate stress fixture without a basemap,
not an automatic schematic layout. The combined SAPN view translates each network
into a separate panel at the same metre scale. It adds no links, merges no buses,
and makes no assertion that a panel's source network is internally connected.
The sidecar retains original coordinates, each projection origin and panel offset.

Co-located buses remain separate. Search reaches every source ID, and **Next
co-located bus** cycles the IDs at a selected position. Zero-length branches get a
small selectable display detour, with no invented electrical nodes. Multiple
coincident branches may still overlap. Line and transformer identity, ordered
terminal maps, state and detour status appear in the inspector; bus attachments
are listed there without adding load/source glyphs. Find accepts an exact raw ID
when unique, or `source/URI-encoded-ID`; duplicate raw IDs require qualification.
Long canvas labels are shortened, while the inspector retains the full ID.

The browser validates geometry and sidecar hashes, counts, endpoint correspondence
and namespaces before rendering. Dataset changes clear selection and replace base
GPU layers by dataset identity/hash, including when two cases have equal bus counts.
Staging writes the catalog only after all inputs adapt successfully. A concurrent
reader during file replacement may receive a hash mismatch; reload after staging.

### Measurements and verification

For BMOPF, exported loads/runs include dataset identity, geometry/reference hashes,
source hashes, adapter version, placement and reference JSON bytes. Decode/validation
time includes SHA-256, JSON parsing and reference checks. Preparation includes CSR
adjacency, spatial buckets, identity lookups and co-location/attachment indexes.
Offline `stagingMs` measures reading/adaptation (or collection assembly), geometry
packing and reference serialization, excluding output writes. It is not browser
load time and is not directly comparable between a single source and a collection.
CPU-array bytes still exclude parsed JSON, Maps, library copies and GPU memory.

The combined SAPN payload is approximately **4.76 MiB geometry + 49.25 MiB reference
JSON**. Metadata can dominate load and heap cost even when drawing is inexpensive.
The next experiment should compare eager JSON with compact columnar references,
lazy inspector metadata and worker preparation, while retaining full identities.

`bmopf.test.mjs` exercises identity collisions, ordering, grounding, disabled gaps,
coincident geometry, collection isolation, determinism and explicit input failures.
`verify-bmopf.mjs` checks every staged file's hashes, source/attachment references,
branch-to-segment routes and collection totals. This validates a rendering projection;
it does not establish electrical model validity or an operational asset register.
SAPN cases retain their provisional qualifications, including Norwood's rating hold.

## Interaction

- Select a BMOPF dataset or switch **1k / 10k / 100k**. A load aborts any previous fetch.
- Drag/scroll, use **Fit network**, or open **Feeder detail**.
- Find a bus by number or `B` ID; selection uses CSR adjacency and an overlay.
- **Local labels** enables spatially queried labels at detail zoom, capped at 200.
  The selected bus is labelled even with automatic labels off.
- **Pick wires** enables GPU picking for segments. Every segment resolves to its
  source branch. Open ties retain a geometric gap and explicit inspector state.
- Use **Paper / Dark** to compare the existing visual tokens.
- Run the camera test or stop it. Export measurements as JSON for comparison.

## Measurement protocol

Run from a visible tab, at a fixed viewport, pixel ratio and power mode. Record
machine model, OS, browser version and GPU externally with the exported JSON.
The export includes browser user agent, reported logical core count, optional
browser device-memory hint, viewport, pixel ratios, dataset and source hashes, options and theme.
Pixel ratio is capped at 2. Repeat each condition at least three times after a
warm-up; report distributions, not the best run. The current gallery measures:

- **Load → first draw callback:** fetch + decode/validation (including BMOPF JSON) + preparation +
  submission-to-callback. It excludes initial script download, renderer/device
  initialization and font loading. Record a first visit separately from switching
  sizes. The callback is not a GPU completion fence.
- **Pan/zoom:** a repeatable six-second overview/detail/overview path. p50, p95 and
  maximum intervals come from deck.gl draw callbacks. They include browser
  scheduling and CPU work and are not isolated GPU execution times. Refresh rate
  limits the interpretation of reciprocal FPS. Raw intervals are exported.
- **Selection feedback:** event handler to next draw callback. This excludes OS
  input delivery, GPU pick readback before the callback, and display presentation.
- **Owned CPU arrays:** fixture storage, colours, adjacency and branch/segment
  index. This excludes spatial-index objects, JS/library overhead, internal
  copies, shader/font caches, driver allocations and GPU memory. It is not total
  memory or peak memory. The browser does not expose a portable complete GPU
  memory counter.

Runs interrupted by tab hiding, resizing, input, errors or Stop are recorded as
incomplete and excluded from timing summaries. Changing size starts a fresh timing
display. Previous measurements remain in the exported session report.

## Deliberate baseline limits

Both base layers submit **all buses and all route segments** at every scale.
Clipping happens on the GPU; CPU viewport culling, partition caches and semantic
aggregation are not implemented. This is intentionally a full-geometry stress
baseline, not a claim that 100k detailed labels are readable at once. Fixture and theme changes retire the base layers and re-upload their buffers;
appearance-only GPU patches remain future work. CPU label
queries use a spatial grid; labels have a budget and remain optional. Adjacency
and scene preparation are synchronous and timed. No automatic layout is run.

Next experiments should add viewport chunks and semantic level of detail, compare
GPU glyph instancing against this simple bus layer, move expensive preparation to
a worker, and measure realistic dense/meshed datasets and operating-value updates.
Bulk result overlays, multi-wire detail and incremental topology edits are not
part of this first benchmark. The production requirements are in the
[design document](../design.md#scale-requirement-100000-buses).

## Initial browser evidence

[evidence/browser-smoke.json](evidence/browser-smoke.json) preserves one completed
camera run per size from this implementation, after checking that switching sizes
actually replaces the GPU geometry. At a 652 × 500 CSS-pixel canvas and pixel ratio
2, with labels and wire picking off, observed p95 draw-callback intervals were
9.3 ms (1k), 9.2 ms (10k), and 9.4 ms (100k). These are single-session smoke
measurements in the Codex in-app browser, not repeated acceptance benchmarks or
GPU execution times. They are refresh-rate-limited and do not establish headroom.
Machine/GPU model and power mode were not recorded, so do not compare these figures
as a hardware-independent result. The JSON records the browser-reported context.

Functional browser checks also covered bounds-checked search through B100000,
wire/source selection, both themes, bounded labels, cancelled-run exclusion and
JSON export. The latest label-spacing and theme-lifecycle refinements preserve
the baseline geometry; rerun the protocol before treating results as a regression
baseline. Production automatic layout, total memory, editing, and study updates
remain unmeasured.

## BMOPF browser smoke evidence

[evidence/bmopf-browser-smoke.json](evidence/bmopf-browser-smoke.json) records a
completed combined-SAPN run and functional checks. At 652 × 500 CSS pixels, pixel
ratio 2, labels and wire picking off, the final-build smoke run measured 325.6 ms
load-to-draw (43.5 ms fetch, 155.9 ms decode/validation, 90.3 ms preparation,
35.9 ms submit-to-draw) and 9.7 ms p95 draw-callback intervals. This is one local
session with unrecorded GPU/power context, not an acceptance result or a memory
budget. Fetch timing varied across reloads. The committed evidence omits projection
coordinates; local measurement exports retain complete projection provenance.

Browser checks covered switching real and synthetic geometry, exact/qualified bus
search, French line picking, Texas internal-bus cycling and transformer-family
inspection, both themes, Unicode labels and measurement export. Final diagnostics
showed no new console errors or missing-glyph warnings after correcting the theme
handler to listen only on theme buttons and enabling automatic glyph collection.
