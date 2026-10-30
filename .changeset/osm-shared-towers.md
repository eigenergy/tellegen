---
'@tellegen/engine': patch
---

OpenStreetMap lines that share towers with another level (`voltage=400000;220000`) or change circuit count or kind along their route are stitched whole. Each level of a shared-tower way joins its own line with its share of the circuits, and a joined line keeps the fewest circuits of its ways. Before, such lines broke at every change, their pieces ended away from any substation and were dropped, and the island policy filled the gaps with synthetic ties: Lisbon's 220 kV network reached the Portuguese 400 kV grid through none of its mapped lines, and a Germany run needed 195 ties where it now needs 109.
