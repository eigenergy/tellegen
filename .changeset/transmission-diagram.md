---
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
