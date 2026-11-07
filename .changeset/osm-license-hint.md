---
"@tellegen/engine": patch
---

A synthetic grid built from OpenStreetMap data records the data's licence as an identifier. `OsmProvenance` in the generation report gains `licence`, the SPDX identifier of the licence that the Overpass attribution notice names, when tellegen recognizes it: `ODbL-1.0` for OpenStreetMap's usual notice. The notice itself stays in `copyright`, word for word.
