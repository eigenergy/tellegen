---
"@tellegen/svelte": minor
---

Transmission diagrams explain their results. Each result view opens with a plain reading of
the result and the reason for it ("Every bus has the same price: no line limit binds, so the
cheapest generators can reach every bus") and links to the matching section of the docs.
Terms such as rating, binding, marginal relief and objective units explain themselves on hover,
and the notation moves under "Key and settings".

A case drawn in diagram coordinates now fits the canvas and can be zoomed into; the zoom limit
follows the drawing's units instead of capping it at 256 px. The diagram opens at the map's
view only when that shows the case at a useful size. Such cases also get a "Single-line
diagram" button beside "Fit diagram". When two of a case's main voltage levels share a hue
and a lower hue is unused, the lower level takes it (230 and 115 kV draw blue and cyan). While
the diagram is open, the network panel says its LMP, angle and demand colours apply to the map
and the diagram's Map display view.
