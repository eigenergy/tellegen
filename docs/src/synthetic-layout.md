# Synthetic Layout

Most public OPF test cases do not include geographic coordinates. tellegen uses
synthetic topology layouts in two places: the explicit pglib dev fallback, and
local files parsed in the browser that the user places on the map. The tellegen
backend API marks fallback coordinates with `synthetic_coords: true`; local
files are labeled in the panel as synthetic layouts.

## Tree Short-Circuit for Radial Networks

Before the force pass, the browser layout counts independent cycles on the
deduplicated graph (open switches are already excluded). When the count is at
most `max(2, ceil(0.02 * n))`, the graph is a tree or nearly one — the shape of
a distribution feeder — and it is drawn as a tidy tree instead: depth grows
along one axis from the root, and each subtree occupies its own contiguous band
of leaf slots on the other, so the drawing is planar and the longest chain
renders as a straight trunk. The root is a source bus when the case identifies
one (multiconductor cases pass their sources as hints), else a BFS diameter
endpoint. Chords of a near-tree draw as plain segments between their placed
endpoints. Meshed networks fall through to the force pass below.

## Deterministic Seed

The layout starts from a golden angle spiral on the unit square. A deterministic
jitter of about `1e-4`, computed from bus index sines, breaks exact ties. No
random number generator is used.

## Force Refinement

tellegen refines the seed with a Fruchterman–Reingold force pass on the unit
square:

- repulsion `k^2 / d` between each bus pair, with `k = sqrt(1 / n)`;
- attraction `d^2 / k` along branches;
- capped displacement with a linearly cooled temperature.

The force pass is `O(iterations * n^2)`. In the tellegen backend it runs once
at boot and the resulting network payload is cached. In the browser, local
dropped files use the same deterministic force idea with a lighter seed and
iteration count, then scale into a small footprint around the user's chosen map
point. The fit into that footprint uses one uniform scale for both axes, so a
long feeder keeps its aspect ratio instead of stretching to a square. Once
placed, the local network solves in browser WebAssembly, and the layout is
stamped into the network payload (`Bus.location`, provenance `synthetic`), so
saved study packages and exports carry the placement and the panel can download
it as a `.geo.json` layer.

## Determinism

The pipeline uses source order, fixed jitter, and fixed iteration counts.
Identical input files produce identical coordinates across boots.

## Tests

Rust tests check that:

- every coordinate lies inside the bounding box;
- no two buses remain stacked in the test fixture.

## References

- T. M. J. Fruchterman and E. M. Reingold, "Graph drawing by force-directed
  placement," Software: Practice and Experience, 1991.
