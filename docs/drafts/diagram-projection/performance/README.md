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
node --test docs/drafts/diagram-projection/performance/bmopf.test.mjs docs/drafts/diagram-projection/performance/schematic.test.mjs docs/drafts/diagram-projection/performance/tidy-tree.test.mjs docs/drafts/diagram-projection/performance/corridor-routing.test.mjs
node docs/drafts/diagram-projection/performance/verify-bmopf.mjs
node docs/drafts/diagram-projection/performance/build.mjs
```

The selector offers the three synthetic sizes, five individual SAPN cases,
three French representatives (smallest, median and largest by bus count), the Texas
pilot and **SAPN · combined collection**. `?dataset=french-medium&layout=schematic` opens the French starting case.
`?dataset=sapn-combined&layout=schematic` opens the collection;
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

In **Source coordinates** mode, geographic coordinates are projected into local
equirectangular metres, north up.
Edges connect endpoints with straight display segments; source route polylines are
not imported. This is a geographic-coordinate stress fixture without a basemap,
not an automatic schematic layout. The combined SAPN view translates each network
into a separate panel at the same metre scale. It adds no links, merges no buses,
and makes no assertion that a panel's source network is internally connected.
The sidecar retains original coordinates, each projection origin and panel offset.

Co-located source buses remain distinct and receive separate schematic positions.
Search reaches every source ID, and in the coordinate reference **Next
co-located bus** cycles the IDs at a selected position. Zero-length branches get a
small selectable display detour, with no invented electrical nodes. Multiple
coincident branches may still overlap. Line and transformer identity, ordered
terminal maps, state and detour status appear in the inspector; bus attachments
are listed there without adding load glyphs; schematic mode also marks declared source buses. Find accepts an exact raw ID
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

## Schematic layout mode

This experiment adds a **diagram projection parallel to Tellegen's existing
geospatial visualizer**. Its purpose is to explain electrical structure. Source
coordinates are a comparison reference, not the feature being improved here.
Choose **Leaf order / Tidy tree / Source coordinates** on any staged BMOPF dataset. Selection
keeps the same source identity across modes, and each mode remembers its camera.
Use **Trace to layout root** to highlight and frame a bus's tree path; the inspector
shows its depth, root and path length. **Fit selected source** frames its source panel.
Mesh line connections are cyan; transformers retain violet family styling and an
explicit non-tree role in the inspector when applicable. Enabled source buses and
fallback layout roots receive amber markers. No branch is removed to make a tree.

### First algorithm: source-rooted spanning forest

`schematic.mjs` is independent of source coordinates and of deck.gl:

1. Build adjacency from enabled lines and transformer *associations*, preserving
   their original direction, family, identity and terminal maps in shared metadata.
2. Start each enabled connected component at its first enabled voltage-source bus
   in canonical bus order. Components without such a source use their first
   canonical bus, explicitly labelled as a layout fallback. Multiple sources in
   one component remain source markers; one is chosen as the tree root.
3. Run deterministic BFS to record one parent edge and depth per bus. Canonical
   adapter ordering breaks ties. This is a display tree, not an energization or
   power-flow result; transformer associations do not become conductor continuity.
4. Assign non-overlapping subtree leaf rows and place each bus by depth from the
   root. Centre parents between their outermost children. Stack disconnected
   components and pack independent sources in separate panels. Spacing is fixed
   at 120 diagram units per depth and 24 per leaf row, not geographic metres.
5. Route tree edges orthogonally. Retain cycles, parallel edges and self loops as
   separately classified non-tree routes. Disabled links never parent a bus and
   retain a real gap. Endpoints are authoritative; crossings and shared routing
   trunks do not introduce electrical buses or junction records.

All traversals are iterative. Placement and routing use O(V + E) storage/work
apart from sorting the declared source-root candidates. There is no force
simulation, all-pairs optimization, recursive traversal or browser-time layout.
This is a replaceable radial-layout baseline, not a general-purpose crossing
minimizer. It does not aggregate buses, infer voltage levels, expand multi-wire
connectivity, or optimize transformer symbol/port placement.

The 540-bus French feeder has 539 tree edges, one source-rooted component and no
non-tree connections. Its schematic contains 1,179 route segments. The combined
SAPN fixture retains 98,633 buses, 98,628 tree edges, **2,655 non-tree connections**
and five components. Its schematic has 264,100 segments and a 10.65 MiB geometry
payload, including parent, depth, component and route-role arrays. The same
49.25 MiB reference JSON serves both projections. Layout plus routing took roughly
0.14 ms for the French case and 46.8 ms for SAPN in one warm Node staging process;
these observations exclude raw-file parsing, reference serialization and browser
loading and are not repeated performance guarantees.

The scale test exposes a **readability limit before a layout-computation limit**:
12,706 leaf rows in Golden Grove make its full-detail tree tall. All five sources
fit and remain searchable, but their overview cannot convey individual equipment.
Next evaluate feeder/transformer-group aggregation, an explicit overview-to-detail
navigation model, more compact subtree packing, and collision-aware cross-link
routing. Preserve an exact drill-down path and identities when adding aggregation.
Do not interpret a fast frame time as proof that a 100k-bus drawing is useful.

### Storage and validation

Staging writes `*.schematic.bin` and `*.tidy-tree.bin` beside the ignored coordinate
geometry and adds `layouts.schematic` and `layouts.tidy-tree` to each local manifest
entry. Their `-corridor` variants add separate fan-out/corridor geometry files.
All five geometry variants use the same
reference sidecar, bus/branch order, endpoints and state arrays. Layout metadata
records algorithm/version, spacing, root rule, component roots/counts/bounds,
non-tree counts and separate placement/routing timings. The source JSON, source
coordinates and PowerIO importer are unchanged.

The browser verifies the selected geometry hash and validates parent relationships,
component consistency, route roles, orthogonal segments and unique bus positions.
It reuses the current dataset's verified reference metadata when toggling modes;
exports record `layoutMode`, layout timings and `referenceCacheHit`. Compare cold
loads separately from mode switches. The camera normalizes deck.gl's per-axis zoom
into the gallery's scalar zoom so Fit and dataset changes cannot retain a stale
axis override. Each mode/hash has distinct base GPU layers.

`schematic.test.mjs` covers coordinate independence, source-root selection,
multiple sources, cycles, parallel edges, self loops, disabled links, disconnected
components, transformer associations and a **100,000-bus chain** without recursive
stack failure. `verify-bmopf.mjs` validates all five geometry variants of all ten staged datasets,
including every route's original endpoints and collection isolation. Neither test
suite claims electrical solvability or crossing-free geometry.

### Tidy-tree comparison

The **Tidy tree** option (`layout=tidy-tree`) is an independent iterative
Buchheim-style implementation in `tidy-tree.mjs`, informed by the algorithm study
of [Numair's DistributionNetworkPlots PR #4](https://github.com/frederikgeth/DistributionNetworkPlots.jl/pull/4).
No project code or dependency was copied. The original **Leaf order** option keeps
its `layout=schematic` URL and default behavior.

Both options use exactly the same enabled BFS forest, root selection, child order,
120-unit depth spacing, 24-unit lane gap, non-tree connections and orthogonal
routing. Tidy placement compacts adjacent subtree contours independently within
each component, then packs the existing source panels. It operates on the display
spanning tree even when the electrical graph is meshed; it does not classify the
whole collection as radial or remove ties. This deliberately isolates subtree
placement from graph selection and routing changes.

The spacing fix is in the **first-walk traversal order**: finish a child and
apportion it against previous siblings **before entering the next sibling**.
Running all children first and apportioning later can leave a later sibling using
an obsolete coordinate. A seven-bus regression exercises this exact failure;
200 deterministic uneven trees check rank separation and parent centering. Dense
arrays, constant-time ancestor membership and iterative passes keep tidy placement
O(V) in time and storage. Tests include 100k-bus chains and stars. Geometry uses
Float64 during placement and Float32 for rendering; comparisons allow 0.01 diagram
units of rounding in the nominal 24-unit gap.

Reproduce the aggregate comparison without committing raw or derived network data:

```sh
node docs/drafts/diagram-projection/performance/compare-layouts.mjs /absolute/path/to/BMOPFDraftData/output /tmp/layout-comparison.json
```

The script performs one warm-up per strategy and five interleaved measured runs,
checks identical endpoints, states and forest arrays, and records bounds, spacing,
segment counts and positive-length collinear overlap pairs between distinct
branches. Decode, validation and metric collection are outside the layout timer.
The committed [offline evidence](evidence/tidy-tree-comparison.json) contains
aggregate metrics and source hashes, without bus IDs or source coordinates.

| Dataset / metric | Leaf order | Tidy tree |
| --- | ---: | ---: |
| French · 540 buses / Drawing height (diagram units) | 4,612.0 | 2,452.3 |
| French · 540 buses / Routed segments | 1,179 | 1,171 |
| French · 540 buses / Collinear overlap pairs | 268 | 261 |
| French · 540 buses / Median layout + routing + packing (ms) | 0.653 | 0.293 |
| SAPN combined · 98,633 buses / Drawing height (diagram units) | 433,934.0 | 115,733.6 |
| SAPN combined · 98,633 buses / Routed segments | 264,100 | 257,138 |
| SAPN combined · 98,633 buses / Collinear overlap pairs | 98,746 | 93,554 |
| SAPN combined · 98,633 buses / Median layout + routing + packing (ms) | 25.882 | 36.025 |

Drawing width is unchanged. Height falls **46.8% for the French feeder** and
**73.3% for combined SAPN**. SAPN geometry falls from 10.65 to 10.47 MiB; reference
JSON remains 49.25 MiB. The comparison ran on Apple M4 Max / arm64 / Node v24.19.0.
Times are local five-run observations, sensitive to JIT and garbage collection,
not acceptance budgets or browser load times.

Positive-length collinear overlaps still include shared trunks, and transverse
crossings, labels and symbol collisions are not measured by that count. Tidy tree
improves packing; it does **not** provide collision-free equipment routing. The
French overview shows more structure, while SAPN still needs aggregation and
feeder/substation drill-down. A future multi-wire view must reserve terminal and
conductor widths before routing; this bus placement alone is not a multi-wire
layout.

The [browser smoke evidence](evidence/tidy-tree-browser-smoke.json) records one
six-second camera test per dataset/layout at a 652 × 500 CSS-pixel canvas, DPR 2,
with labels and wire picking off. P95 frame intervals were 8.9/9.2 ms for French
leaf-order/tidy-tree and 8.9/8.9 ms for SAPN. These refresh-rate-limited observations
show no obvious interaction regression; they do not establish a renderer speedup.
The French bus selection and 32-step root path were verified across modes, and
browser diagnostics reported no warnings or errors. Mixed metadata-cache states
make the recorded load times unsuitable as a direct cold-load comparison.

## Fan-out and corridor routing

Choose **Current routes / Fan-out + lanes** independently of **Leaf order / Tidy
tree**. `?dataset=french-medium&layout=tidy-tree&routing=corridor` opens the French
comparison; use `dataset=sapn-combined` for the collection. The original routing
remains the default, including for older URLs. Routing controls are unavailable
for source coordinates and synthetic precomputed fixtures. Selection survives
switches; each dataset/layout/routing combination retains its own camera.

`corridor-routing.mjs` runs during local staging:

1. Allocate distinct display ports, ordered by the opposite endpoint's position,
   on the left/right sides of each original bus. A short vertical **bus-owned
   bar** replaces overlapping branch lead-ins. Every port is part of that bus;
   no electrical node, terminal, conductor or connection is inferred.
2. Reserve busbar clearance within each depth rank. When necessary, scale the
   component's vertical placement uniformly, preserving subtree shape. Round bus
   centres to whole diagram units, and stagger left/right ports on quarter-unit
   rows so horizontal leads cannot share the same row from opposing sides.
3. Allocate vertical lanes in the gaps between depth columns. Upward tree fans,
   downward tree fans and non-tree links have separate lane banks. Allocate each
   tree fan atomically in destination order; allocating its edges independently
   can reverse sibling lanes when tracks are reused. Clear tracks are reused via
   heaps; corridors expand if the existing depth gap cannot hold their tracks.
4. Retain parallel circuits and self loops as distinct routes. Long or
   inter-component links use exterior horizontal tracks plus local escape lanes.
   Disabled links retain a real display gap and never determine the spanning tree.
5. Repack source panels around the routed extents. Independent source namespaces
   remain independent. Branch routes begin/end at their owned busbar ports rather
   than the bus-centre point; source endpoint indices and terminal maps stay exact.

Preprocessing targets O((V+E) log(V+E)) time and O(V+E) storage. No routing runs in
the browser frame loop. A third binary LineLayer draws and picks busbars as buses;
branch segment ownership/picking stays intact. `branchPorts`, `busbarSources` and
`busbarTargets` are local geometry arrays, not additions to the PowerIO schema.
Validation checks finite geometry, port membership on the correct endpoint bar,
route endpoint correspondence and enabled-route continuity. The test suite covers
parallel circuits, loops, disconnected/disabled links, high-degree buses, a 100k
chain, 100k concurrent interval requests, and 100 uneven radial fans with no
measured overlaps or proper crossings.

Reproduce the comparison (raw inputs stay external):

```sh
node docs/drafts/diagram-projection/performance/compare-routing.mjs /absolute/path/to/BMOPFDraftData/output /tmp/routing-comparison.json
```

The [offline evidence](evidence/corridor-routing-comparison.json) uses tidy-tree
placement with both routing strategies, one warm-up and five interleaved measured
runs on Apple M4 Max / Node v24.19.0. It records complete placement/routing/packing
time; diagnostics and validation are outside the timer.

| Dataset / metric | Current routes | Fan-out + lanes |
| --- | ---: | ---: |
| French · 540 buses / Collinear overlap pairs | 261 | 0 |
| French · 540 buses / Proper route crossings | 0 | 0 |
| French · 540 buses / Horizontal wires through busbar interiors | 0 | 0 |
| French · 540 buses / Wire segments | 1,171 | 1,617 |
| French · 540 buses / Geometry bytes | 53,300 | 91,684 |
| French · 540 buses / Median offline preparation (ms) | 0.68 | 1.65 |
| SAPN · 98,633 buses / Collinear overlap pairs | 93,554 | 0 |
| SAPN · 98,633 buses / Proper route crossings | 7,803 | 7,799 |
| SAPN · 98,633 buses / Horizontal wires through busbar interiors | 128 | 0 |
| SAPN · 98,633 buses / Wire segments | 257,138 | 303,849 |
| SAPN · 98,633 buses / Geometry bytes | 10,974,420 | 17,080,312 |
| SAPN · 98,633 buses / Median offline preparation (ms) | 33.73 | 126.41 |

The French drawing has unchanged width and approximately unchanged height. SAPN
width grows 0.3% and height 6.6%; wire segment count grows 18.2%, and binary geometry
(including busbars and ports) grows from 10.47 to 16.29 MiB. The shared 49.25 MiB
reference JSON is unchanged. Peak corridor demand is 7 lanes for French and 10
for SAPN. Local preparation remains outside interaction, but the additional
payload and third layer are real memory/upload costs.

Zero collinear overlap does not mean crossing-free or readable at every zoom.
The diagnostics count positive-length exact collinear sharing, strict-interior
perpendicular crossings, and horizontal wires through vertical bus footprints
(current routing uses a two-unit bus-centre footprint). They exclude endpoint
contacts; the footprints do not model equipment symbols or labels. The French
radial case has zero measured crossings. SAPN still has 7,799, so mesh selection,
crossing cues and local route ordering remain useful follow-up experiments.
Subpixel separation at overview scale still needs semantic aggregation; the
busbar ports here are branch attachments, not the future multi-wire terminals.

The [browser smoke evidence](evidence/corridor-routing-browser-smoke.json) records
one six-second camera run per dataset/routing with tidy-tree placement, a 652 × 500
CSS-pixel canvas, DPR 2, labels off and wire picking off. P95 frame intervals were
10.0 / 9.1 ms for French current/corridor routing, and 9.9 / 9.8 ms for SAPN.
The added busbar layer showed no obvious interaction regression in this local,
refresh-rate-limited run. These are not repeated acceptance benchmarks or proof
of a speedup. Selection, root tracing, independent layout controls and canvas bus
picking were checked; diagnostics reported no warnings or errors.

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
and scene preparation are synchronous and timed. Schematic placement and routing run offline during staging, separately from drawing.

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
baseline. Production layout quality, total memory, editing, and study updates
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

## Schematic comparison evidence

[evidence/schematic-browser-smoke.json](evidence/schematic-browser-smoke.json)
records one completed six-second camera run for each combination below, at a
652 × 500 CSS-pixel canvas, pixel ratio 2, local labels and wire picking off.

| Dataset | Layout | Load → draw | Reference cache | Pan/zoom p95 |
| --- | --- | ---: | --- | ---: |
| French, 540 buses | Source coordinates | 21.6 ms | Miss | 9.0 ms |
| French, 540 buses | Schematic | 22.6 ms | Hit | 9.3 ms |
| SAPN, 98,633 buses | Source coordinates | 332.5 ms | Hit | 8.8 ms |
| SAPN, 98,633 buses | Schematic | 365.5 ms | Miss | 9.2 ms |

These are smoke observations, not repeated acceptance results or a fair cold-load
comparison between modes. The source and schematic camera paths visit different
geometry; root markers/labels are part of the schematic. Browser scheduling and
refresh rate limit interpretation of the frame intervals. Layout quality and total
memory remain separate concerns. Re-run the protocol before comparing regressions,
particularly older evidence collected before the per-axis camera normalization fix.

Final-build browser checks verified the same French LV bus and terminal declarations
across modes, positive selection-feedback timing, a framed 32-step root path,
interactive zoom followed by Fit, and visible geometry in both SAPN projections.
No browser warnings or errors were reported during those checks.
