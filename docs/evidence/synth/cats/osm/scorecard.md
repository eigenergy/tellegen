# CATS scorecard: california (osm)

Pass rule: every critical check, and at least 80% of the scored metrics, in every run; a metric the mapped data itself misses (the Map column, measured on the Overpass document inside the region with no engine involved) is map-limited and not scored.

| Run | Verdict | Passed | Score | Critical failures |
| --- | --- | --- | --- | --- |
| seed 2026 | pass | 34/40 (85%) | 25.60 | none |

| Metric | CATS | Map | seed 2026 | Tolerance |
| --- | --- | --- | --- | --- |
| Peak load (MW) | 44,009 | – | 48,042 ✓ | ±15% |
| Nameplate excl. condensers and imports (MW) | 67,972 | 102,391 map-limited | 118,641 (not scored) | ±20% |
| Buses after series reduction | 8,347 | – | 3,317 ✗ | ×0.75–1.33 |
| Sites (buses within 300 m) | 4,999 | 3,948 | 2,835 ✗ | ×0.75–1.33 |
| Bus share at 66 kV | 0.651 | 0.451 map-limited | 0.424 (not scored) | ±0.06 |
| Bus share at 115 kV | 0.225 | 0.298 map-limited | 0.292 (not scored) | ±0.06 |
| Bus share at 230 kV | 0.113 | 0.224 map-limited | 0.251 (not scored) | ±0.06 |
| Bus share at 500 kV | 0.011 | 0.027 | 0.033 ✓ | ±0.06 |
| Circuit-km at 66 kV | 20,391 | 11,454 map-limited | 14,092 (not scored) | ×0.8–1.25 |
| Circuit-km at 115 kV | 12,991 | 14,273 | 15,954 ✓ | ×0.8–1.25 |
| Circuit-km at 230 kV | 13,352 | 21,979 map-limited | 23,492 (not scored) | ×0.8–1.25 |
| Circuit-km at 500 kV | 7,462 | 7,882 | 6,307 ✓ | ×0.8–1.25 |
| Transformers per site | 0.132 | – | 0.393 ✗ | ×0.67–1.5 |
| Generators excl. condensers | 2,123 | – | 1,267 ✓ | ×0.5–2.0 |
| Fuel capacity mix, total variation distance | – | 0.183 map-limited | 0.177 (not scored) | ≤ 0.15 |
| Mean degree (reduced) | 2.403 | – | 2.743 ✗ | ±12% |
| Degree distribution, L1 distance | – | – | 0.697 ✗ | ≤ 0.2 |
| Lines per site, lv band | 1.177 | – | 1.178 ✓ | ±15% |
| Lines per site, mv band | 1.110 | – | 1.040 ✓ | ±15% |
| Lines per site, hv band | 1.172 | – | 1.083 ✓ | ±15% |
| Line length / MST, lv band | 1.435 | – | 1.176 ✓ | ±20% |
| Line length / MST, mv band | 1.510 | – | 1.300 ✓ | ±20% |
| Line length / MST, hv band | 2.619 | – | 2.144 ✓ | ±20% |
| Lines on the Delaunay triangulation, lv band | 0.874 | – | 0.873 ✓ | ±0.1 |
| Lines on the Delaunay triangulation, mv band | 0.830 | – | 0.815 ✓ | ±0.1 |
| Lines on the Delaunay triangulation, hv band | 0.710 | – | 0.726 ✓ | ±0.1 |
| Mean clustering coefficient | 0.033 | – | 0.073 ✓ | ±0.05 |
| Mean hop distance | 25.6 | – | 19.5 ✓ | ×0.67–1.5 |
| x per km inside CATS p10-p90 at 66 kV | – | – | 0.812 ✓ | ≥ 0.7 |
| X/R inside CATS p10-p90 at 66 kV | – | – | 1.000 ✓ | ≥ 0.7 |
| Rating inside CATS p10-p90 at 66 kV | – | – | 0.999 ✓ | ≥ 0.7 |
| x per km inside CATS p10-p90 at 115 kV | – | – | 0.935 ✓ | ≥ 0.7 |
| X/R inside CATS p10-p90 at 115 kV | – | – | 1.000 ✓ | ≥ 0.7 |
| Rating inside CATS p10-p90 at 115 kV | – | – | 0.990 ✓ | ≥ 0.7 |
| x per km inside CATS p10-p90 at 230 kV | – | – | 0.959 ✓ | ≥ 0.7 |
| X/R inside CATS p10-p90 at 230 kV | – | – | 1.000 ✓ | ≥ 0.7 |
| Rating inside CATS p10-p90 at 230 kV | – | – | 0.991 ✓ | ≥ 0.7 |
| x per km inside CATS p10-p90 at 500 kV | – | – | 0.728 ✓ | ≥ 0.7 |
| X/R inside CATS p10-p90 at 500 kV | – | – | 0.981 ✓ | ≥ 0.7 |
| Rating inside CATS p10-p90 at 500 kV | – | – | 0.962 ✓ | ≥ 0.7 |
| Transformer x on own base inside CATS p10-p90 | – | – | 0.897 ✓ | ≥ 0.8 |
| DC OPF branch loading, median | 0.285 | – | 0.274 ✓ | ±0.15 |
| DC OPF branch loading, p90 | 0.711 | – | 0.698 ✓ | ±0.15 |
| DC OPF binding fraction | 0.004 | – | 0.006 ✓ | ±0.05 |
| Site distance to CATS sites, symmetric mean (km) | – | – | 2.151 ✓ | ≤ 5.0 |
| Line-km per 25 km cell, Spearman | – | 0.863 | 0.830 ✗ | ≥ 0.85 |
| Load per 50 km cell, Spearman | – | – | 0.792 ✓ | ≥ 0.7 |

Map: 1,194 power=plant features inside the region tag no output and are not in its nameplate (the engine estimates them); its sites are every power=substation inside the region, an upper bound.
