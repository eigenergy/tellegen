---
'@tellegen/svelte': patch
---

At low zoom the map draws one bus marker per crowded spot, so a state-sized grid no longer covers the map in overlapping markers. In each spot it keeps a generator before a substation before any other bus, as recorded in a grid made by tellegen-synth, such as one from MapToGrid, and then the bus with the most generation and demand. Zooming in shows the rest, and panning leaves the markers as they are. A selected bus always shows, with its selection ring, and every bus is still a bus of the case: the bus picker finds it by number and a line's tooltip names it. `TellegenMap` takes `declutterMarkers={false}` to draw every marker at every zoom, as before. `LocalCase` gains `busKinds`, each bus's kind in a generated grid.
