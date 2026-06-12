# Synthetic layout

Most public OPF test cases carry no geographic coordinates. pglib strips them
even for cases that were designed with a service territory in mind. tellegen
still wants every network on a map, so `backend/src/layout.jl` manufactures
coordinates from the network topology alone. This note records how, and why
the obvious method fails on its own.

The demo serves TAMU ACTIVSg cases at the real substation coordinates in
their aux exports (`backend/src/coords.jl`), so this code path runs only when
no case data is staged and the server falls back to pglib cases. The API
marks manufactured coordinates with `synthetic_coords: true`; real
coordinates carry `false`.

## Spectral embedding

Build the graph Laplacian over in-service branches: `L = D - A`, where `A` is
the bus adjacency matrix and `D` the diagonal degree matrix. Parallel
branches collapse to one edge; weights are uniform.

The embedding takes the eigenvectors of the two smallest nonzero eigenvalues
of `L` as x and y. This is Hall's classic quadratic placement result: among
all unit-norm coordinate vectors orthogonal to the constant vector, the
second Laplacian eigenvector minimizes

```
sum over branches (i,j) of (x_i - x_j)^2
```

so electrically adjacent buses land near each other, and the third
eigenvector repeats the argument in an orthogonal direction. The result reads
like a one line diagram: the embedding orders buses by graph distance through
the network, which for a transmission grid is a rough proxy for electrical
distance.

For a connected network the zero eigenvalue is simple and the construction is
well defined. A disconnected case would put indicator vectors of components
into the low eigenvectors and collapse each component to a point; the force
pass below would still separate them, but no case tellegen serves needs
this.

## Why the raw embedding is not enough

Low Laplacian eigenvectors localize. On a network with long radial chains
hanging off a meshed core, the Fiedler vector concentrates its variation
along the dominant chain and is nearly constant everywhere else. The
embedding then degenerates: one stretched filament for the chain, and the
rest of the network compressed into a blob at the origin.

ACTIVSg500 is a textbook case. Its raw spectral layout renders as a thin arc
with a dense knot, nothing like the real network, which fills its South
Carolina footprint. The eigenvalues confirm the graph is connected
(one zero eigenvalue, then 0.0068, 0.0165, ...); the failure is purely the
localization of the low eigenvectors.

## Force refinement

The spectral coordinates survive only as the seed of a Fruchterman-Reingold
force layout on the unit square:

- repulsion `k^2 / d` between every bus pair, with `k = sqrt(1/n)` the ideal
  spacing,
- spring attraction `d^2 / k` along branches,
- displacement per step capped by a temperature that cools linearly over 250
  iterations.

The seed preserves the global ordering the eigenvectors found (the west end
of the network stays west); the forces equalize local density, so chains
unfold and the mesh spreads to fill its territory. A deterministic jitter of
about 1e-4 (sines of the bus index, no RNG) breaks exact ties in the seed so
stacked buses have a direction to separate along.

The pass is O(iterations * n^2): 0.4 s for 500 buses, a few seconds at 2000.
It runs once at boot and the result is cached in the case registry.

Finally each axis rescales independently into the case's bounding box with
4% padding, which stretches the unit-square layout over the service
territory the case was designed for.

## Determinism

The pipeline contains no randomness: dense symmetric eigensolve, fixed
jitter, fixed iteration count. Identical input files produce identical
layouts across boots, so the pre-serialized network payloads are stable.

## Tests

The `synthetic layout` testset in `backend/test/runtests.jl` pins the
properties that matter:

- every coordinate inside the bounding box,
- no two buses closer than 1e-5 degrees (no stacks survive),
- interquartile range above 20% of the box span on both axes, which fails
  exactly when the layout collapses into a filament.

## References

- K. M. Hall, "An r-dimensional quadratic placement algorithm,"
  Management Science, 1970.
- Y. Koren, "On spectral graph drawing," COCOON, 2003.
- T. M. J. Fruchterman and E. M. Reingold, "Graph drawing by force-directed
  placement," Software: Practice and Experience, 1991.
