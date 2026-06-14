# Synthetic Layout

Most public OPF test cases do not include geographic coordinates. When staged
TAMU data is absent, tellegen serves pglib fallback cases and manufactures
coordinates from topology. The API marks these coordinates with
`synthetic_coords: true`.

## Spectral Seed

The layout starts from the graph Laplacian over in service branches:
`L = D - A`, where `A` is the bus adjacency matrix and `D` is the diagonal degree
matrix. Parallel branches collapse to one unweighted edge.

The x and y coordinates are the eigenvectors of the two smallest nonzero
eigenvalues. This follows Hall's quadratic placement result: among unit norm
coordinate vectors orthogonal to the constant vector, the second Laplacian
eigenvector minimizes

```text
sum over branches (i, j) of (x_i - x_j)^2
```

Adjacent buses therefore land near each other in the seed layout.

## Force Refinement

Low Laplacian eigenvectors can localize on long radial structures and compress a
meshed core. tellegen uses the spectral coordinates only as the seed for a
Fruchterman Reingold force pass on the unit square:

- repulsion `k^2 / d` between each bus pair, with `k = sqrt(1 / n)`;
- attraction `d^2 / k` along branches;
- capped displacement with a linearly cooled temperature over 250 iterations.

A deterministic jitter of about `1e-4`, computed from bus index sines, breaks
exact ties. No random number generator is used.

The force pass is `O(iterations * n^2)`. It runs once at boot and the resulting
network payload is cached. Each axis is then rescaled independently into the
case bounding box with 4% padding.

## Determinism

The pipeline uses a dense symmetric eigensolve, fixed jitter, and a fixed
iteration count. Identical input files produce identical coordinates across
boots.

## Tests

`backend/test/runtests.jl` checks that:

- every coordinate lies inside the bounding box;
- no two buses remain closer than `1e-5` degrees;
- the interquartile range is at least 20% of the box span on both axes.

## References

- K. M. Hall, "An r-dimensional quadratic placement algorithm,"
  Management Science, 1970.
- Y. Koren, "On spectral graph drawing," COCOON, 2003.
- T. M. J. Fruchterman and E. M. Reingold, "Graph drawing by force-directed
  placement," Software: Practice and Experience, 1991.
