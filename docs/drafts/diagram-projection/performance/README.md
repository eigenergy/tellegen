# Diagram performance gallery

A local deck.gl 9.4.0 / WebGL2 gallery for **1k, 10k and 100k buses**, using
precomputed schematic layouts. This isolates rendering and interaction from
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

## Interaction

- Switch **1k / 10k / 100k**. A load aborts any previous fetch.
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
browser device-memory hint, viewport, pixel ratios, fixture hash, options and theme.
Pixel ratio is capped at 2. Repeat each condition at least three times after a
warm-up; report distributions, not the best run. The current gallery measures:

- **Load → first draw callback:** fetch + typed-array decode + preparation +
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
