---
"@tellegen/engine": patch
---

Generated grids convert kilometres to degrees with one scale, 111.195 km per degree from PowerIO's Earth radius, where some steps used 111.0. Place snapping, OSM clipping, cell margins, site offsets and generator areas move slightly, so a generated grid for a given request and seed differs from the one the previous release produced.
