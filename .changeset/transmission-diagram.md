---
"@tellegen/engine": minor
"@tellegen/svelte": minor
---

Balanced cases open as a single-line diagram. **Show diagram** in the network
panel replaces the map with a canvas that draws the case on its own positions
(geographic, with the map's basemap underneath, or drawing coordinates). Lines
are drawn by voltage level, transformers sit on their branch once legible with
each winding in its side's level colour, and the largest generators and the transformers stay findable as
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
buses glide into their substations when the layout switches. Lines route
through the channels of a Delaunay triangulation of the substations, spaced
apart and clear of the boxes they pass, and step around any box still in the
way. ACTIVSg200 lays out in under a millisecond with no overlapping
substations, 86 % of them at their true position and no line crossing another
substation; a 70,000-bus synthetic grid takes about 0.35 s natively.

A case without coordinates gets a schematic laid out by its connections (sparse
stress on the substation graph, `seeding: "stress"`), in place of the synthetic
positions it is placed with on the map: ACTIVSg2000 lays out in about 30 ms,
ACTIVSg70k in a little over a second.

The key names the case's own voltage levels ("500 kV", "230 kV") rather than
bands, up to six entries; past that, adjacent levels merge into ranges, low and
closely spaced levels first, so the highest levels stay distinct longest. Each
level combines a hue by tier (magenta from 300 kV, blue from 100 kV, cyan from
35 kV, grey below), a darker shade for a higher level within its tier, and a
width by rank. Hovering a level in the key highlights it; clicking keeps it
highlighted. Zoomed out, levels below the top three fade. With results shown,
chevrons on branches point the way active power flows. The schematic names each
substation at its box and labels busbars with their voltage close in.
