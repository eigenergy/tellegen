# Synthetic Layout

Most public OPF test cases do not include geographic coordinates. tellegen uses
synthetic topology layouts in two places: the explicit pglib dev fallback, and
local files parsed in the browser that the user places on the map. The tellegen
backend API marks fallback coordinates with `synthetic_coords: true`; local
files are labeled in the panel as synthetic layouts.

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
point. Once placed, the local network solves in browser WebAssembly.

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
