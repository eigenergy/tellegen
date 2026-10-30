---
'@tellegen/engine': patch
---

OpenStreetMap lines on shared towers (`voltage=400000;220000`) belong to the line of each level they carry. Before, such a way counted only at its highest level, so the lower-voltage line running on the same towers broke where it reached them, and its pieces ended away from any substation. Now each level gets its own copy of the way with its share of the circuits, and a substation that states no voltage takes every level of the shared-tower ways that end at it.
