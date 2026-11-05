---
"@tellegen/engine": minor
"@tellegen/svelte": minor
---

Balanced cases open as a single-line diagram. **Show diagram** in the network
panel replaces the map with a canvas that draws the case on its own positions
(geographic, with the map's basemap underneath, or drawing coordinates). Lines
are shaded and weighted by voltage level, transformers sit on their branch once
legible, and the largest generators and the transformers stay findable as
landmarks when zoomed out. Once solved, **Colour by: Results** colours buses by
the map's current display (LMP, angle, voltage or demand) and branches by
loading, with loading labels where a branch is near its rating. Selection, hover
cards, search framing, keyboard navigation and PNG export work as in the
multiconductor diagram, and closing the diagram returns the map to the view it
showed.

The multiconductor schematic now shares its camera fitting and basemap code
with the new diagram; its behaviour is unchanged.

**Schematic** switches the diagram to a substation overview laid out in the
engine (`txSchematicLayout` in `@tellegen/engine`), after Birchfield and
Overbye (2018). Each substation is a box with one busbar per bus, highest
voltage first, and its transformers drawn between the busbars; lines leave
along their own row from the side of the box facing the far substation.
Substations take the free spot nearest their geographic position, and dense
areas are drawn smaller, so the overview stays registered to the basemap and
buses glide into their substations when the layout switches. ACTIVSg200 lays
out in under a millisecond with no overlapping substations and 86 % of them at
their true position; a 70,000-bus synthetic grid takes about 60 ms.
