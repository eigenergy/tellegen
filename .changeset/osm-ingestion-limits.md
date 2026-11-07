---
"@tellegen/engine": patch
---

A synthetic grid built from OpenStreetMap data refuses an Overpass answer that is too large before it parses it. `limits.max_osm_document_bytes` caps the answer's size (64 MiB by default) and `limits.max_osm_elements` caps the elements across its documents (500,000 by default); either one over its cap fails as an OSM error naming the limit. The request also refuses `osm` distances that are not finite and positive (`voltage_floor_kv`, `cluster_radius_m`, `snap_km`, `hvdc_snap_km`, `plant_snap_km`), and a negative or non-finite `clip_buffer_km`, as `invalid_request`.
