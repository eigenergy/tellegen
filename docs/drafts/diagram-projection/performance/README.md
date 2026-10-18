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


## Engineering workflow

With external geometry/results staged, open
`/performance/?dataset=french-medium&layout=tidy-tree&routing=equipment&results=loading&flows=static&open=compact`.
The **Results** toolbar is above the diagram; layout and routing choices are under
**Layout, routing and dataset experiments** below the workbench.

1. Choose **Power flow** to focus a high-flow branch and animate P triangles and Q
   chevrons. Toggle P/Q independently, pause/resume, or change playback speed.
   The legend fixes separate P and Q scales to the whole result snapshot. Speed
   uses a square-root magnitude scale, not physical propagation; equal P/Q speeds
   need not mean equal numeric values. Tiny nonzero values have a readability floor.
2. Choose **Highest loading** and hover a wire to see its source ID, signed powers
   at **both** ends, current/rating loading and rating basis. Click to pin details.
   The French worst line is `75_LVBranch0306208`, 94.23642% loading.
3. Choose **Voltage violations (1)**, then **Voltage profile ↓**. The 30-bus path
   ends at `75_LVBus0623137`, where 3–n is 207.7769 V below its 207.8461 V limit.
   Separate MV and LV plots prevent mixing phase-phase and phase-neutral values.
4. Hover or focus a chart point to highlight its bus. Click/Enter centers and pins
   it; Left/Right and Previous/Next move along the same retained path. **Fit path**
   frames its highlighted routes. Source-coordinate mode explicitly reports that
   a display-tree profile is unavailable.

The profile shows volts and declared pair bounds versus **branch steps**, not
kilometres. It follows the display tree, not an inferred unique electrical path
through meshes. At most 256 path buses are plotted, with explicit truncation;
paths longer than 4,096 buses are rejected with a notice. The charts preserve
source IDs and never guess a nominal voltage base or neutral reference.

[Results toolbar and P/Q detail](evidence/french-engineering-workflow.png) and
[linked MV/LV voltage profile](evidence/french-linked-voltage-profile.png) show the
saved French solution. Earlier screenshots below document the first overlay experiment.

Reproduce the CPU guardrail with `node docs/drafts/diagram-projection/performance/measure-workflow.mjs`.
[CPU evidence](evidence/engineering-workflow-cpu.json) samples 1,000 paths and local
camera queries on both datasets. SAPN path/markup p95 was 0.051/0.247 ms, excluding
DOM insertion; glyph generation p95 was 0.005 ms. The maximum sampled path contained
216 buses. Bounded SVG is rebuilt only on selection; animation only updates local
GPU markers. The snapshot issue list and independent P/Q maxima are prepared once.

[Browser evidence](evidence/engineering-workflow-browser.json) records static and
animated loading/PQ with a pinned voltage profile on both datasets. Settings:
692 × 500 CSS canvas, DPR 2, dark theme, equipment routing, local labels off,
**wire picking on**, profile highlighting on. This differs from the older
wire-picking-off experiment, so the old numbers are not a controlled regression baseline.

| Dataset | Loading + paused P/Q | Loading + magnitude animation |
| --- | ---: | ---: |
| French, 540 buses, 15-bus profile | 9.2 ms p95 | 9.0 ms p95 |
| SAPN, 98,633 buses, 37-bus profile | 9.1 ms p95 | 9.1 ms p95 |

Each cell is one completed six-second camera sample. These are browser frame
intervals, not GPU timings or universal budgets. SAPN uses deliberately artificial
rendering stress values on its 98,633-bus topology; it is not a saved solution.
The new features are covered by 73 passing POC tests. Browser checks include
P/Q toggles, playback/pause, exact branch hover, linked keyboard navigation with
retained path/focus, both themes, and source-layout profile unavailability.

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
node --test docs/drafts/diagram-projection/performance/bmopf.test.mjs docs/drafts/diagram-projection/performance/schematic.test.mjs docs/drafts/diagram-projection/performance/tidy-tree.test.mjs docs/drafts/diagram-projection/performance/corridor-routing.test.mjs docs/drafts/diagram-projection/performance/circuit-inspection.test.mjs docs/drafts/diagram-projection/performance/crossing-order.test.mjs
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
voltage sources. Adapter v2 also supports two-endpoint switches with an explicit
scalar boolean `open_switch`. Nonempty unknown equipment tables, dangling buses,
unknown terminals, unequal line/switch terminal-map lengths and unsupported or
contradictory enabled/status values fail explicitly. Absent service status means
in service; missing switch position never means closed. Coordinates are required
by default; `allowMissingCoordinates` permits wholly coordinate-free sources for
schematic layouts only. Partial coordinate coverage fails explicitly.
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
Choose **Leaf order / Tidy tree / Source coordinates** on staged BMOPF datasets with coordinates (MVLVmeshed is schematic-only). Selection
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
entry. Their `-corridor` and `-crossing-aware` variants add separate corridor geometry files.
All seven geometry variants use the same
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
stack failure. `verify-bmopf.mjs` validates all seven geometry variants of all ten staged datasets,
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

## Non-tree circuit inspection

The gallery can inspect each enabled non-tree connection in a schematic BMOPF
layout. The searchable connection browser preserves the source namespace,
equipment family/kind and ID, including distinct parallel circuits. Search matches
source labels, equipment IDs and endpoint IDs; it is a literal, case-insensitive
substring filter applied on submit. The selector renders at most 40 connections;
Previous/Next navigates all matches, wrapping at the ends. Source-coordinate and
synthetic views do not have the required display spanning forest. A radial French
feeder correctly reports zero eligible connections.

Choose a connection to enable inspection. Cyan highlights its exact stored route
with a background-coloured halo through crossings. A/B markers identify its buses;
full IDs and terminal mappings remain in the inspector. The optional amber path
joins those endpoints through their lowest common ancestor in the display tree,
including bus-owned fan-out bars. It excludes the common route back to the root.
Fit connection includes the connection's bends and endpoint bars; Fit circuit also
includes the entire tree path. Dim context changes base-layer opacity without
changing geometry or dropping topology. Disabling inspection restores ordinary
branch selection. Selection identity survives changes of routing and layout.

This is a cycle in an equipment-association display graph, not evidence of
operating state, energization, power flow, or interwinding conductor continuity.
Disabled equipment is excluded from the circuit browser but remains in the
underlying drawing and ordinary inspector. Different display components have no
connecting tree path; self-loops have a zero-edge path; parallel equipment keeps
its own branch identity. Crossings never create connectivity.

Implementation: `circuit-inspection.mjs` follows parent/depth links to the common
ancestor in O(path length), without graph-wide search. Overlay coordinates are
views into resident geometry, collected through the existing branch-to-segment
index. Overlays are cached between selection changes, including during pan/zoom.
There are a constant number of overlay layers; no layer per branch and no global
crossing decorations. The enabled non-tree index takes O(E) preparation and 4 bytes
per eligible connection. Search scans only that list, and DOM results are bounded.
A pathological 100k-deep path remains O(path length), so it can still be expensive
to display in full; this POC does not aggregate the selected path.

Run the pure topology tests and external-fixture comparison:

```sh
node --test docs/drafts/diagram-projection/performance/*.test.mjs
node docs/drafts/diagram-projection/performance/compare-inspection.mjs
```

The comparison uses already staged local geometry; it does not copy raw data.
`evidence/circuit-inspection-comparison.json` checks every SAPN non-tree path for
ordered endpoint continuity, unique tree nodes and enabled tree edges. All 2,655
connections passed. The longest path has 145 tree branches; the largest overlay
has 438 wire segments (plus up to 146 busbars and two endpoint markers). The index
adds 10,620 bytes. Five timed passes after a warm-up measured 0.0065 ms p95 for
path finding plus wire-overlay construction on Node 24.19.0 / macOS arm64. This
microbenchmark excludes DOM work, busbar objects, drawing and GPU/library copies.

The browser report schema is now version 2: camera-run environments record
inspection state and route/path sizes; `selections` records selection-to-draw
callbacks, including inspection toggles. Those are browser delivery observations,
not GPU completion measurements. Use matching viewport, labels, picking and theme
when comparing inspection on/off. The camera trajectory remains unchanged, so
inspection results can be compared with the baseline; a whole-network camera test
does not continuously keep the selected circuit in view.

[evidence/circuit-inspection-browser-smoke.json](evidence/circuit-inspection-browser-smoke.json)
records three interleaved runs per mode on combined SAPN, using Tidy tree and
Fan-out + lanes, a 652 × 500 CSS-pixel canvas, DPR 2, dark theme, labels and picking
off. Inspection used the longest circuit (145 tree branches / 438 wire segments).

| Mode | Pan/zoom p95 across three runs | Median p95 |
| --- | --- | ---: |
| No selection | 8.9, 9.5, 9.4 ms | 9.4 ms |
| Circuit and tree path highlighted, context dimmed | 9.0, 9.2, 9.2 ms | 9.2 ms |

Forty-five consecutive connection selections, including a result-page boundary,
measured 10.9 ms median, 11.8 ms p95 and 14.4 ms maximum selection-to-draw callback.
No material camera regression appeared in this local smoke comparison; the small
difference is not evidence of a speedup. Refresh rate limits these observations,
and GPU/power context was not recorded. The browser's owned-array total increases
by the 10,620-byte index; selection objects, list strings, deck.gl and GPU copies
remain excluded. No base fixture bytes or source identities changed.

Functional checks covered page transitions and wraparound, empty results, both
routing modes and schematic placements, preserving equipment selection through
the source-coordinate reference, optional path/dimming controls, distinct fit
bounds, paper/dark themes, French radial root tracing and the synthetic 100k view.
No browser warnings or errors were reported. Bridge glyphs, global crossing
reduction, viewport aggregation and multi-wire inspection are outside this change.

## Crossing-aware lane ordering

**Crossing-aware lanes** is a third routing option, independent of Leaf order /
Tidy tree placement. Open
`?dataset=sapn-combined&layout=tidy-tree&routing=crossing-aware` and compare with
**Fan-out + lanes**. The original corridor option is unchanged. Source-coordinate
and synthetic views disable all topology-routing controls. The mutable local
catalog is fetched without caching so reloading after staging discovers new modes.
The existing circuit inspector works with all schematic routing variants.

`crossing-order.mjs` runs inside corridor preprocessing, after interval colouring
and before assigning physical lane x coordinates:

1. Keep the exact bus positions, ports, interval-to-lane membership, lane counts,
   corridor widths and source-panel packing from the corridor baseline.
2. Build sorted left/right endpoint-row lists and vertical spans for each lane.
   For each pair of lanes, count strict interior crossings in both possible
   orders using binary searches over endpoint rows. This includes tree/mesh and
   mesh/mesh conflicts; touching endpoints and collinear sharing are excluded.
3. Insert mesh lanes among the existing tree lanes and other mesh lanes. Tree
   lanes retain their relative order, protecting the tree drawing. Accept only
   strict crossing reductions, with stable IDs and deterministic tie handling.
   Alternate mesh traversal direction for at most four passes; stop early if
   nothing changes. This is a bounded local heuristic, not a global optimum.
4. Fall back to original corridor ordering when there are more than 32 lanes or
   a corridor contains a disabled route or exterior escape. Those routes need a
   different cost model because of gaps and additional bends. Radial corridors
   need no ordering work. Fallback reasons are recorded, never silently omitted.

Moving a whole interval-coloured lane preserves its non-overlapping vertical
segments. Horizontal leads are reattached to the same original ports; branch
ownership, terminal maps, state and source identities never change. No crossing
introduces a junction. Tree routes may shift sideways along with their lanes;
their relative order and zero proper crossings are preserved. Only physical lane
order changes; no electrical or PowerIO API changes are involved.

With the lane cap K = 32, cost construction is O(K I log I) per eligible corridor
with I intervals, plus O(K²) working storage and O(I) endpoint/span storage. Four
insertion passes cost O(K²) each. There is no pairwise scan over all network edges,
no global crossing minimization and no ordering during camera movement. The base
renderer and circuit overlays consume the same binary schema and layer count.

### Offline comparison

Run after installing the usual local gallery dependencies:

```sh
node --test docs/drafts/diagram-projection/performance/*.test.mjs
node docs/drafts/diagram-projection/performance/compare-crossing-order.mjs /external/BMOPFDraftData/output /tmp/crossing-order.json
node docs/drafts/diagram-projection/performance/stage-bmopf.mjs /external/BMOPFDraftData/output
node docs/drafts/diagram-projection/performance/verify-bmopf.mjs
```

[evidence/crossing-order-comparison.json](evidence/crossing-order-comparison.json)
records one warm-up and five interleaved runs on Node 24.19.0 / Apple M4 Max.
Tidy-tree placement is used in both modes; decoding, validation and diagnostics
are outside the timer. The test also asserts identical bus positions, ports,
busbars, spanning forest, endpoint indices and states between both fixtures.

| Combined SAPN, 98,633 buses | Corridor | Crossing-aware |
| --- | ---: | ---: |
| Proper crossings | 7,799 | **5,139** |
| Tree / tree | 0 | **0** |
| Tree / non-tree | 7,529 | **4,967** |
| Non-tree / non-tree | 270 | **172** |
| Exact collinear overlap pairs | 0 | **0** |
| Horizontal wire / busbar interior crossings | 0 | **0** |
| Wire segments | 303,849 | 303,849 |
| Geometry bytes | 17,080,312 | 17,080,312 |
| Width × height, diagram units | 85,640 × 123,337 | 85,640 × 123,337 |
| Total Manhattan route length | 16,807,596.5 | 16,740,700.5 |
| Median placement + routing + packing | 127.5 ms | 163.2 ms |

The reduction is 2,660 crossings (**34.1%**), with unchanged footprint and geometry
size. 595 of 775 corridors are eligible; 408 improve after 495 accepted lane moves.
The remaining 180 are radial; no SAPN corridor hits the lane cap or exterior/gap
fallback. Peak lane demand stays at 10. The cost-model reduction exactly matches
the independent geometry sweep. The remaining 5,139 crossings are not claimed
unavoidable: this experiment fixes port ordering and bus placement, retains tree
lane order and uses a bounded local search.

The French radial feeder retains byte-identical geometry and zero crossings.
Its measured median preparation times are 0.70 / 0.79 ms. Tests cover 200 random
meshed drawings across both placements, deterministic geometry, cost-model versus
independent sweep agreement, identity/footprint preservation, parallel circuits,
self-loops, disabled/exterior fallback, a wide-corridor cap and a 100k-bus chain.
All 70 staged variants (seven per dataset) validate; raw source files remain
external and all derived fixtures remain ignored by Git. Geometry metrics count
strict crossing events, not distinct equipment pairs or screen-space readability.


### Browser comparison

[evidence/crossing-order-browser-smoke.json](evidence/crossing-order-browser-smoke.json)
records three interleaved camera runs for each routing option, using combined SAPN
and Tidy tree. Canvas: 652 × 500 CSS pixels, DPR 2, dark theme, labels/picking off,
no selection. Both modes use the same six-second camera trajectory.

| Routing | Pan/zoom p95 across three runs | Median p95 |
| --- | --- | ---: |
| Fan-out + lanes | 9.4, 8.6, 9.0 ms | 9.0 ms |
| Crossing-aware lanes | 8.7, 8.9, 8.6 ms | 8.7 ms |

No camera regression was observed; these refresh-rate-limited local measurements
are not evidence of a speedup or a hardware-independent performance guarantee.
The browser still submits 303,849 wire segments and 98,633 busbars in the same
layers, with 19.37 MiB of owned arrays (excluding JS objects, library/GPU copies).
The added cost is offline preprocessing. Source references are reused on mode
switches; the recorded load times mix cold fetches and reference-cache hits.

Browser checks verified preservation of the same selected branch and 13-edge
connecting tree path, both schematic placements, source-coordinate disable/restore,
French radial geometry and synthetic 100k availability. No console warnings or
errors were observed. As before, screen-space clarity at 100k buses still needs
aggregation and local inspection; a lower global crossing count is only one metric.


## MVLVmeshed switch scenarios

The external `test/data/MVLVmeshed` case restores 38 switches and two lines to the
radial Australian MV21 + 33 LV feeder collection. The complete export has **3,409
buses, 3,098 lines, 33 transformer objects and 317 switches**. This reconstructed
mesh overlay is not an authoritative operating model. Attribution and limitations
remain in the external README/ANALYSIS and the catalog note (CC BY-NC-SA 4.0).

The pre-existing `output/combined/original/MV_LV_combined.json` is the radial base;
it is not this case. To export the full mesh using a working BMOPFTools environment:

```sh
julia --project=/path/to/BMOPFTools.jl --startup-file=no \
  docs/drafts/diagram-projection/performance/export-mvlv.jl /path/to/BMOPFDraftData
node docs/drafts/diagram-projection/performance/stage-bmopf.mjs /path/to/BMOPFDraftData/output
node docs/drafts/diagram-projection/performance/verify-bmopf.mjs
node docs/drafts/diagram-projection/performance/compare-switch-routing.mjs \
  /path/to/BMOPFDraftData/output /tmp/switch-routing.json
```

The exporter writes `output/MVLVmeshed/original/MVLVmeshed.json` externally, checks
expected equipment counts and records input DSS hashes. PowerIO's case-boundary
checks reject the nested master's sibling redirects if imported directly. The
helper creates a temporary include manifest at the common external `test/data`
root, validates each redirect remains inside it, and deletes that manifest after
import. Original DSS files stay unchanged. Tested with Julia 1.12.6, the local
BMOPFTools project and PowerIO 0.11.1; the data repository's older scripts Manifest
was incompatible with the current BMOPFTools checkout.

This export contains **zero open switches**, despite the master's 34 explicit
opening commands. The catalog therefore stages two named scenarios from the same
source bytes and namespace, with separate geometry/reference hashes:

| Scenario | Open / closed switches | Enabled components | Independent cycles |
|---|---:|---:|---:|
| Documented open ties | 34 / 283 | 1 | 6 |
| All switches closed | 0 / 317 | 1 | 40 |

Documented state applies the master's explicit `open line.<id> term=1` commands;
all other switches retain imported state. All-closed deliberately overrides every
switch position while retaining service status. Neither scenario infers position
from `_OPEN` or `_CLOSED` names. The bounded command reader rejects unknown IDs,
duplicates, ambiguous case-insensitive IDs and unsupported opening/closing forms.
The command file hash, override policy and count changed from source accompany each
scenario. These are complete named snapshots, not live switching controls.

Switch references keep `sourceOpenSwitch`, effective `openSwitch`, and
`outOfService`. The existing binary `states=1` / `disabled=true` means inactive
for layout: `openSwitch || outOfService`. It does not conflate switch position
with equipment service status in the inspector. Terminal maps remain ordered,
paired and attached to the exact BMOPF source IDs. Scalar state applies to the
whole equipment object; unknown position and independent pole states require the
future PowerIO semantic projection and fail explicitly in this bounded adapter.
Open switches have real route gaps and cannot enter the spanning forest or enabled
non-tree circuit index. They remain discoverable with **Find switch**, including
qualified `source/id` lookup. Closed switches are green, open/inactive routes amber;
these are equipment drawing styles, not phase/voltage/result encodings.

No buses in this export have coordinates. References retain `coordinates: null`,
the geographic projection is marked unavailable, and `toFixture` refuses to
fabricate a source view. Staging provides an actual tidy-tree base plus all six
schematic variants. Source coordinates is disabled; an incoming `layout=source`
URL falls back to Tidy tree. Changing scenario recomputes the forest offline;
source identities are stable but bus positions and selection may change. Stable
anchors across switching scenarios remain future work.

### Routing comparison

[evidence/switch-routing-comparison.json](evidence/switch-routing-comparison.json)
contains source hashes, counts, complete aggregate diagnostics and five interleaved
timing samples after one warm-up per routing strategy. All use Tidy tree. No raw
source topology, bus IDs or coordinates are committed in that report.

| Scenario / routes | Overlapping segment pairs | Proper crossings | Median offline ms | Segments |
|---|---:|---:|---:|---:|
| Documented / current | 4,769 | 708 | 2.15 | 8,562 |
| Documented / corridor | 0 | 2,089 | 6.03 | 10,440 |
| Documented / crossing-aware | 0 | 2,077 | 6.82 | 10,440 |
| All closed / current | 4,747 | 768 | 1.23 | 8,484 |
| All closed / corridor | 0 | 837 | 5.42 | 10,344 |
| All closed / crossing-aware | 0 | 759 | 6.24 | 10,344 |

Crossing counts include open routes. The current routing's lower counts do not
account for its thousands of shared collinear segments. Both corridor options
remove those overlaps and wire-through-busbar intersections. Crossing-aware lanes
preserves corridor bus positions, ports, footprint and segment count, reducing
all-closed crossings by **9.3%**. The documented scenario improves only **0.6%**:
41 corridors retain their ordering because of open routes or exterior escapes;
only three are eligible. This is a useful limit of the current heuristic, not a
reason to hide open equipment or treat it as conducting. A separate presentation
layer for inactive ties and stable scenario anchors are the next experiments.
Geometry remains under 0.57 MiB in both scenarios. Timing is a local CPU sample,
not an end-to-end latency guarantee.

`switch-scenarios.test.mjs` covers state validation, exact family identities,
terminal order, real gap geometry, exclusion from trees/circuit paths, service
status, scenario non-mutation, command ambiguity and missing-coordinate behavior.
All 41 POC tests passed; all 84 staged variants passed geometry/reference/hash
verification, including the unchanged combined SAPN collection.


Browser smoke evidence is in
[evidence/switch-browser-smoke.json](evidence/switch-browser-smoke.json). One
completed six-second camera run per scenario/route produced p95 frame intervals:

| Scenario | Current | Corridor | Crossing-aware |
|---|---:|---:|---:|
| Documented open ties | 10.1 ms | 8.9 ms | 9.9 ms |
| All switches closed | 8.8 ms | 9.6 ms | 9.3 ms |

One interrupted corridor run is retained but excluded. These are local visible-tab
smoke samples, not statistical throughput claims. Browser checks also covered the
same source switch across both states, exact-ID lookup, open-switch exclusion from
circuit inspection, closed non-tree switch endpoint/tree-path highlighting, routing
selection retention, coordinate-free bus inspection, source-view fallback, and
restoring the ordinary SAPN dataset controls.

A final 98,633-bus SAPN crossing-aware regression run completed at 9.6 ms p95;
its source-coordinate option remained available and switch lookup was disabled.


## Compact presentation of open switches

**Endpoint rings** is now the default open-switch presentation. `?open=full` keeps
the previous full-route view; `?open=compact` selects rings. This is independent of
layout, routing and electrical state. The toggle retains the camera, selection,
bus positions, ports and busbars. It never rebuilds the forest or changes a switch.

One hollow amber annotation appears at each bus with an open switch. Several open
switches sharing an endpoint share a ring; source identities stay separate in the
browser. Clicking a ring scopes the open-switch list to that bus and selects its
first tie. **All open switches** removes the scope. The list shows at most 40
entries at a time; Previous/Next wraps through all matches. Exact **Find switch**
lookup still reaches every source identity. A self-loop contributes only one ring
and shows an A / B endpoint label.

Selecting an open switch reveals just its complete original route, with the
existing gap, a contrasting halo and A / B labels. It never enables circuit/tree
path inspection for that switch. Selecting a bus in compact mode highlights its
other incident branches without restoring all incident open routes. Clear removes
the selected route. Markers are pickable independently of **Pick wires**; ordinary
wire picking maps compact segment indices back to the original source equipment.
No branches or endpoints are removed from the source references or topology.
Closed out-of-service switches and disabled non-switch equipment retain their
full inactive routes; this presentation targets explicit `openSwitch === true`.

`open-ties.mjs` prepares endpoint groups and a compact copy of the remaining segment
coordinates and pick-index map once per dataset/geometry load. Work is linear in
branches, segments and open endpoints. Original geometry remains resident for
selection and Full routes. Two cached wire layers swap visibility; toggle and
camera movement do not repartition geometry. Colors are prepared once and updated
on theme changes. Scenes with no open switches use the original layer and allocate
no extra presentation arrays. This trades bounded additional memory for predictable
interaction; it is not viewport culling, routing optimization or network LOD.
Both wire layers remain in the layer list; `visible` chooses which draws.
GPU allocations are not instrumented. CPU array accounting includes the compact coordinates, source
segment indices and colors, but excludes object/Map/Set overhead and GPU copies.

### MVLV comparison at fixed Tidy-tree layouts

[Aggregate evidence](evidence/open-ties-comparison.json) records one warm-up and five
preparation samples for every MVLV scenario/routing pair plus combined SAPN. Source
and staged-geometry hashes are checked; source geometry remains byte-identical.
Raw sources and staged fixtures stay external/ignored. Reproduce with:

```sh
node docs/drafts/diagram-projection/performance/compare-open-ties.mjs /tmp/open-ties.json
node --test docs/drafts/diagram-projection/performance/*.test.mjs
```

| Scenario / routing | Full route crossings | Compact route crossings | Suppressed segments | Endpoint rings |
|---|---:|---:|---:|---:|
| Documented / current | 708 | 50 | 272 | 68 |
| Documented / corridor | 2,089 | 35 | 198 | 68 |
| Documented / crossing-aware | 2,077 | 23 | 198 | 68 |
| All closed / current | 768 | 768 | 0 | 0 |
| All closed / corridor | 837 | 837 | 0 | 0 |
| All closed / crossing-aware | 759 | 759 | 0 | 0 |

The crossing-aware documented overview shows **98.9% fewer route crossings**, with
all 34 open switches represented. This measures route geometry before selection,
not marker/label collisions or a topological/routing improvement. Selecting one tie
restores at most six segments in corridor modes (eight in current routing), so its
crossings become visible again. Current routing still has 4,707 overlapping segment
pairs in compact view; both corridor modes retain zero overlaps.

The crossing-aware compact copy adds **327,744 bytes (0.31 MiB)** of CPU arrays;
layout and source geometry stay resident and their bounds are unchanged. All-closed
and SAPN allocate zero extra arrays. Because layout still reserves corridors for
open routes, compact presentation does not reclaim their whitespace. The sparse
rings can also coincide on screen at extreme zoom or on co-located source buses;
the complete keyboard browser remains the unambiguous route to every switch.
Production work should evaluate screen-space clustering and stable layout anchors
as separate features rather than silently dropping identities.

Regression coverage includes explicit open versus out-of-service behavior, original
pick owners and coordinates in all three routing modes, parallel ties, self-loops,
selected route gap retention, unchanged source bytes/state/forest, all-closed and
synthetic fast paths, and a 100k-bus partition/remapping test. All 45 POC tests pass.


The final offline sample measured median compact preparation of 0.92 ms for
crossing-aware MVLV, 0.02 ms for all-closed, and 0.62 ms for combined SAPN. Browser
presentation-toggle updates took 0.6–0.8 ms of JavaScript (not a GPU completion
measurement). [Browser evidence](evidence/open-ties-browser-smoke.json) records the
viewport, scenario, routing, presentation, extra array storage, loads, and camera
runs. One six-second visible-tab sample per scenario/presentation gave:

| Scenario | Full routes p95 | Endpoint rings p95 |
|---|---:|---:|
| Documented open ties | 8.9 ms | 9.2 ms |
| All switches closed | 9.6 ms | 9.5 ms |

A selected open route completed at 9.6 ms p95 in the final telemetry build (an
initial smoke sample was 9.1 ms). These samples establish comparable interaction
cost on this machine, not a frame-rate improvement. Verification also covered
ring picking and bus scope, list wraparound, full/compact selection retention,
all three routing options, paper/dark themes, original identity and open-state
inspection, and disabling the circuit path for open switches.

The final combined SAPN camera run completed at 9.5 ms p95 with zero additional
presentation arrays; its 98,633-bus geometry and routing were unchanged.


## Equipment symbols and reserved routing space

Select **Equipment + clearance** in either schematic placement. Direct comparison:
`?dataset=mvlv-meshed-documented&layout=tidy-tree&routing=equipment&open=compact`.
The existing routing modes remain available without symbols. Re-stage external
BMOPF data and rebuild the gallery to obtain the new variants.

The compact geometry reuses the authored transformer circles, switch contacts and
blades, supply circle/wave and load triangle from `../symbols/symbols.mjs`.
A general **Y** block represents shunts: MVLV contains neutral conductance shunts,
so the capacitive specimen would misstate the imported model. No earth bond,
constant-power subtype or generator is inferred. Lines remain conductor bundles;
π equivalents and conductor-level detail are outside this equipment projection.

Transformer/switch bodies occupy real gaps in their branch route, with no opaque
background masks. Open switches use the actual blade gap; a closed out-of-service
switch retains a separate service gap. Attachments occupy stacked slots off their
original busbar, with junction dots and individual source-owned stems. A slot is
32 × 32 units, inline centres are 34 units from the bus port, ports and attachment
slots are separated by 40 units, and corridor leads reserve 68 units. A monotone
piecewise row expansion preserves original row order and alignment while adding
space only where needed. It avoids magnifying a whole feeder because of one tiny
row gap. Component and source-panel bounds include equipment extents.

All glyphs are precomputed binary line segments, grouped into one stroke layer,
one junction layer and one transparent picking layer. A selected glyph has a small
stroke overlay. No SVG/DOM node is created per device. Numeric owners point into
the unchanged branch/attachment sidecar; inspection shows original source keys,
terminal maps and service state. Exact ID, `source/id`, and the full JSON source
key are supported; ambiguous short IDs fail explicitly. Attachment selection uses
a cached owner index. Selecting a transformer does not create interwinding
conductor continuity. Compact open-tie presentation suppresses the open body and
route together; selecting that switch restores both. Sources with no open switches
reuse the full glyph buffers without a compact copy.

Reproduce the offline audit after staging:

```sh
node docs/drafts/diagram-projection/performance/compare-equipment.mjs /tmp/equipment.json
node --test docs/drafts/diagram-projection/performance/*.test.mjs
```

[Aggregate evidence](evidence/equipment-comparison.json) contains hashes, five
samples after one warm-up, source/forest invariants and clearance checks. Layout
times include routing, glyph tessellation and packing, excluding parsing and
validation. Preparation covers open-tie and glyph caches, excluding the rest of
browser preparation. Samples are local diagnostics, not cross-machine budgets.

| Dataset | Routing | Symbols | Layout median ms | Cache median ms | Packed MiB | Bounds width × height |
|---|---|---:|---:|---:|---:|---:|
| mvlv-meshed-documented | crossing-aware | 0 | 8.9 | 0.87 | 0.56 | 16840 × 15488 |
| mvlv-meshed-documented | equipment | 2,894 | 13.8 | 5.08 | 1.58 | 23994 × 66583 |
| mvlv-meshed-all-closed | crossing-aware | 0 | 5.9 | 0.02 | 0.56 | 13960 × 12439 |
| mvlv-meshed-all-closed | equipment | 2,894 | 11.2 | 0.45 | 1.58 | 20106 × 53801 |
| sapn-combined | crossing-aware | 0 | 216.4 | 1.45 | 16.29 | 85640 × 123337 |
| sapn-combined | equipment | 80,123 | 266.0 | 12.66 | 38.56 | 133786 × 602129 |

Both MVLV scenarios have 350 inline devices and 2,544 attachments (2,894 symbols,
32,222 strokes). Combined SAPN has 80,123 symbols and 666,147 strokes, alongside
305,310 wire segments and 98,633 buses. All three equipment fixtures have **zero
foreign base-route intersections with reserved symbol boxes**. The offline audit
uses axis sweeps; it excludes text, glyph/stem collisions, screen-space crowding
and pixel line widths. It is not a claim that the mesh has no route crossings.
Source reference hashes, endpoints, states, parents and tree roles are unchanged.
The raw source JSON stays external and generated buffers stay ignored.

The main cost is visual footprint: equipment expands MVLV height about 4.3× and
combined SAPN height about 4.9× relative to crossing-aware routing. Port spacing is
conservative even for line-only ports. Adaptive port sizing and equipment/label
LOD are follow-up work. Every symbol is still submitted at overview; there is no
CPU viewport culling or aggregation. Packed/owned array totals exclude JS maps,
label objects, deck.gl copies and GPU allocations. Compact MVLV adds a glyph cache;
SAPN uses the zero-copy glyph path plus color arrays. Fifty regression tests pass,
including foreign-route intrusion detection, dense attachments, self-loops,
parallel circuits, source identity/state preservation and pathological row gaps.


[Browser comparison](evidence/equipment-browser-comparison.json) records one
six-second visible-tab sample per row at 1190 × 560 CSS pixels, DPR 2, dark theme,
compact open switches, automatic labels and wire picking off. Equipment picking
remains enabled. All samples completed without backgrounding.

| Dataset | Crossing-aware p95 | Equipment p95 |
|---|---:|---:|
| MVLV documented | 10.1 ms | 10.2 ms |
| MVLV all closed | Not sampled this comparison | 10.3 ms |
| Combined SAPN | 10.4 ms | 10.6 ms |

These runs support comparable camera interaction on this machine, not a guaranteed
frame rate or GPU timing result. SAPN equipment owns 44.18 MiB of CPU arrays. Its
observed load-to-first-draw was 1.17 s, including 263 ms browser preparation;
loading the baseline with a reused sidecar took 450 ms. Those load paths have
different cache states and are not a controlled overhead comparison. Full browser
preparation includes identity/search indexes and colors beyond the offline cache
benchmark. Visual checks covered dark/paper themes, transformer and supply bodies,
load and general-shunt attachments, direct glyph picking, exact source identities,
and restoring an open switch body in compact presentation. Selection IDs sit
above attachment glyphs; general label collision avoidance remains future work.

## Results overlay experiment

Stage the optional sidecars **after** staging BMOPF geometry:

```sh
node docs/drafts/diagram-projection/performance/stage-results.mjs /absolute/path/to/BMOPFDraftData/output
node docs/drafts/diagram-projection/performance/build.mjs /path/to/installed/tellegen
node docs/drafts/diagram-projection/performance/measure-results.mjs
```

`stage-results.mjs` expects the external French validation files at
`FrenchPowerGrids/validation/tellegen_pf/{summary.json,build_provenance.json,results/}`.
It verifies the exact input SHA-256, a passed validation record and convergence.
It publishes content-hashed, derived result sidecars and then a manifest, all under
ignored `local-data/`. Raw models and solver dumps are never copied into Git.
Re-stage after changing inputs. Mismatching result identities/hashes are rejected;
the browser still permits topology inspection with an explicit unavailable status.
The overlay uses existing Tellegen dependencies.

Open `performance/?dataset=french-medium&layout=tidy-tree&routing=equipment&results=loading&flows=static`.
The controls offer voltage-limit colours, loading colours and independent P/Q
markers (off/static/motion). **Highest loading** selects a real branch and
shows its source ID, two end powers and rating basis; **Voltage violations**
selects the first bus outside its declared terminal-pair limits. Static direction
markers are the default in this link. Theme and routing remain independently
selectable; use local detail to see markers and exact inspector values.

French `75_MVFeeder0100` uses a saved Tellegen multiconductor solution: 540 buses,
539 branches, 16 iterations, one bus outside declared voltage limits and maximum
line loading 94.23642%. End P/Q are signed into equipment, summed over all terminals
including neutral, and reported in kW/kvar. Voltages use complex phase-neutral or
phase-phase differences with explicit bounds in V. Line loading is the maximum
conductor current/rating over both ends. Transformer loading is explicitly an
aggregate end-MVA/rating measure, **not a worst-phase or winding thermal check**.
The [design](../design.md#results-overlays)
explains sign conventions, references, model boundaries and provenance.

For `dataset=sapn-combined`, results are **artificial rendering stress values** on
the real topology. There is no saved SAPN solve in this experiment. Banner,
canvas watermark, inspector and exported measurements carry this distinction.
Other datasets have no result sidecar; values are never fabricated as a fallback.

At detail zoom a spatial query examines at most 4,096 candidates and emits at most
256 end anchors / 512 glyphs. P is a filled triangle; Q is a chevron on the other
side of the route. The initial experiment encoded direction only; the engineering workflow above now uses magnitude-dependent speed. No particles cross equipment
bodies, no layout runs on interaction and static colour buffers stay resident.
The browser initially respects reduced motion, pauses hidden-tab animation and
stops animating when there are no visible markers. Local annotations omit short
segments and overview detail; the inspector remains authoritative. The first/last
route-segment anchors are a deliberate POC limitation, not full-path animation.
General screen-space text collision avoidance remains future work.

[CPU evidence](evidence/results-comparison.json) records five parse/bind/prepare
samples and 1,000 deterministic local/overview queries on both datasets. The SAPN
sidecar is 34.61 MiB uncompressed; median parsing/binding/preparation in the recorded
run were approximately 124/97/154 ms. Local query p95 was 0.013 ms and bounded
glyph generation p95 0.005 ms. Added colour arrays are 1.54 MiB. These exclude
JSON objects, the 202,566 end-anchor objects, spatial maps, GPU and library copies.
The 512-glyph dense-case cap is tested separately; the sampled real layouts are
sparse enough that at most 54 glyphs were emitted by the CPU camera queries.
Production priorities are packed scalar snapshots, on-demand result fetch/detail,
worker parsing/preparation and avoiding reparse on layout changes.

[Browser evidence](evidence/results-browser-comparison.json) compares overlays-off,
static loading/PQ and animated loading/PQ on the same six-second camera path.
All modes retain the parsed snapshot, so the off sample measures incremental
rendering cost, not cold-start/memory cost without results. Frame intervals are
browser draw delivery, not GPU timer queries. Run with the tab visible, keep the
viewport/settings fixed, then **Export measurements**; reports include result
hash, solved/stress kind, modes, peak marker count and query timing.

Recorded browser comparison (692 × 500 CSS pixels, DPR 2, dark theme, tidy-tree /
equipment routing, labels and wire picking off):

| Dataset | Overlays off | Loading + static P/Q | Loading + moving P/Q | Voltage + static P/Q |
| --- | ---: | ---: | ---: | ---: |
| French medium (solved) | 9.1 ms | 9.3 ms | 9.1 ms | Not sampled |
| Combined SAPN (artificial stress) | 10.1 ms | 10.4 ms | 12.8 ms | 9.7 ms |

Values are p95 frame intervals from one completed six-second sample per cell.
The earlier SAPN baseline used a different viewport; it remains in the raw report
but is explicitly excluded from the comparison. Timing differences at this scale
are not evidence that animation or voltage is faster. The camera runs reached
38 local glyphs on French and 18 on SAPN; the pathological 512-glyph cap is covered
by a separate CPU test, not claimed as a GPU benchmark. SAPN result preparation
was about 147 ms in the browser, within 508 ms of total dataset preparation;
load-to-first-draw was 895 ms with local cached-file conditions. This is not a
controlled cold-load comparison with older equipment-only measurements.

Sixty-three POC tests pass, including stale/source identity, namespace collisions,
neutral voltage/power/current semantics, partial ports, missing ratings, independent
end signs, inactive/zero flows, limit precision and bounded dense annotation work.
The deterministic synthetic fixture and four-wire projection checks also pass.
Browser QA covered both themes, voltage violation inspection, loading/PQ inspection,
source-coordinate ↔ tidy-tree selection/result preservation, and no console errors.

[French loading/PQ detail](evidence/french-flow-overlay.png) and
[French voltage violation](evidence/french-voltage-overlay.png) are local-detail
screenshots. The selected route continues off screen; both end totals remain in
the inspector. The voltage bus is `75_LVBus0623137`, whose `3–n` value is
207.7769 V against the declared minimum of 207.8461 V. A selected bus/route retains
its selection accent; the separate red `! V` cue preserves its violation status.
