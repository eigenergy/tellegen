---
"@tellegen/svelte": patch
---

A branch route that crosses ±180 draws as one short path on the flat map and on the globe, instead of a line across the whole map. `TellegenMap` takes the short hop between consecutive route vertices in its route layers, and the selected-branch outline, the selected-bus marker and the framing of a selected branch follow a network across the seam. Stored and exported coordinates are unchanged: route longitudes stay in [-180, 180], so a host drawing routes in its own layers (`extraLayers`) takes the short hop itself.
