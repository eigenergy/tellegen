---
'@tellegen/engine': patch
---

OpenStreetMap lines on shared towers (`voltage=400000;220000`) belong to the line of each level they carry. Before, such a way counted only at its highest level, so the lower-voltage line running on the same towers broke where it reached them, and its pieces ended away from any substation. Now each level gets its own copy of the way, carrying the circuits the tag lists at that level: `voltage=380000;110000;110000` with `circuits=3` is one 380 kV and two 110 kV circuits, and the levels never carry more circuits than the way. A substation that states no voltage takes every level of the shared-tower ways that end at it.
