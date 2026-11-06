---
'@tellegen/svelte': patch
---

The map no longer draws a marker for a line tap in a grid made by tellegen-synth, such as one from MapToGrid. A tap is a point where a line is tapped, with no load or machine, and its line already shows it on the map. On a state-sized grid every tap used to get its own marker, so the markers overlapped and covered the network. Taps are read from the `kind` that tellegen-synth records on each bus row of the module, and other cases draw every bus as before. A tap is still a bus of the case: the bus picker finds it by number, a line's tooltip names it, and a selected tap gets its marker and the selection ring back. `LocalCase` gains `tapBuses`, the tap bus ids, and `markerBuses`, the buses the map draws as markers.
