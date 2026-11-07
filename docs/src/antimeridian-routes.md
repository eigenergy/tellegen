# Routes across the antimeridian

Geographic route coordinates are stored with longitudes in [-180, 180].
`tellegen-synth` writes every bus location and branch route vertex that way.
A route that crosses ±180 is not split at the seam, so two consecutive
vertices can sit almost 360° apart: a line in Chukotka may run from 179.6 to
-179.7.

A map consumer draws the short hop between consecutive vertices. When two
longitudes differ by more than 180°, move the second by a whole multiple of
360° until they differ by 180° or less, and measure the next vertex from the
moved value. Joined as stored, that segment runs the long way round, and a
route near ±180 appears to span the whole world.

`@tellegen/svelte` does this in its route layers on the flat map and on the
globe, in the selected-branch outline, and when it frames a selected branch.
The moved longitudes are for drawing only. Saved modules and exported GeoJSON
keep the stored values.
