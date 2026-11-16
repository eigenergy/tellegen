# CATS scorecard: california (synthetic)

Pass rule: every critical check, and at least 80% of the scored metrics, in every run.

| Run | Verdict | Passed | Score | Critical failures |
| --- | --- | --- | --- | --- |
| seed 1 | fail | 7/47 (15%) | 108.78 | none |
| seed 7 | fail | 7/47 (15%) | 108.96 | none |
| seed 2026 | fail | 7/47 (15%) | 110.65 | none |

| Metric | CATS | seed 1 | seed 7 | seed 2026 | Tolerance |
| --- | --- | --- | --- | --- | --- |
| Peak load (MW) | 44,009 | 80,207 ✗ | 80,207 ✗ | 80,207 ✗ | ±15% |
| Nameplate excl. condensers and imports (MW) | 67,972 | 94,644 ✗ | 94,644 ✗ | 94,644 ✗ | ±20% |
| Buses after series reduction | 8,347 | 535 ✗ | 535 ✗ | 535 ✗ | ×0.7–1.4 |
| Sites (buses within 300 m) | 4,999 | 535 ✗ | 535 ✗ | 535 ✗ | ×0.7–1.4 |
| Bus share at 66 kV | 0.651 | 0 ✗ | 0 ✗ | 0 ✗ | ±0.06 |
| Bus share at 115 kV | 0.225 | 0 ✗ | 0 ✗ | 0 ✗ | ±0.06 |
| Bus share at 230 kV | 0.113 | 0.563 ✗ | 0.555 ✗ | 0.555 ✗ | ±0.06 |
| Bus share at 500 kV | 0.011 | 0.437 ✗ | 0.445 ✗ | 0.445 ✗ | ±0.06 |
| Circuit-km at 66 kV | 20,391 | 0 ✗ | 0 ✗ | 0 ✗ | ×0.67–1.5 |
| Circuit-km at 115 kV | 12,991 | 0 ✗ | 0 ✗ | 0 ✗ | ×0.67–1.5 |
| Circuit-km at 230 kV | 13,352 | 5,937 ✗ | 5,856 ✗ | 5,776 ✗ | ×0.67–1.5 |
| Circuit-km at 500 kV | 7,462 | 7,654 ✓ | 6,433 ✓ | 7,186 ✓ | ×0.67–1.5 |
| Transformers per site | 0.132 | 0.750 ✗ | 0.781 ✗ | 0.740 ✗ | ×0.67–1.5 |
| Generators excl. condensers | 2,123 | 86 ✗ | 95 ✗ | 87 ✗ | ×0.5–2.0 |
| Fuel capacity mix, total variation distance | – | 0.324 ✗ | 0.259 ✗ | 0.336 ✗ | ≤ 0.15 |
| Mean degree (reduced) | 2.403 | 2.901 ✗ | 2.901 ✗ | 2.901 ✗ | ±12% |
| Degree distribution, L1 distance | – | 0.704 ✗ | 0.662 ✗ | 0.760 ✗ | ≤ 0.25 |
| Lines per site, lv band | 1.177 | – ✗ | – ✗ | – ✗ | ±15% |
| Lines per site, mv band | 1.110 | – ✗ | – ✗ | – ✗ | ±15% |
| Lines per site, hv band | 1.172 | 0.714 ✗ | 0.699 ✗ | 0.731 ✗ | ±15% |
| Line length / MST, lv band | 1.435 | – ✗ | – ✗ | – ✗ | ±25% |
| Line length / MST, mv band | 1.510 | – ✗ | – ✗ | – ✗ | ±25% |
| Line length / MST, hv band | 2.619 | 0.922 ✗ | 0.880 ✗ | 0.909 ✗ | ±25% |
| Lines on the Delaunay triangulation, lv band | 0.874 | – ✗ | – ✗ | – ✗ | ±0.1 |
| Lines on the Delaunay triangulation, mv band | 0.830 | – ✗ | – ✗ | – ✗ | ±0.1 |
| Lines on the Delaunay triangulation, hv band | 0.710 | 0.893 ✗ | 0.909 ✗ | 0.923 ✗ | ±0.1 |
| Mean clustering coefficient | 0.033 | 0.229 ✗ | 0.220 ✗ | 0.202 ✗ | ±0.05 |
| Mean hop distance | 25.6 | 15.8 ✗ | 17.0 ✗ | 15.6 ✗ | ×0.67–1.5 |
| x per km inside CATS p10-p90 at 66 kV | – | 0 ✗ | 0 ✗ | 0 ✗ | ≥ 0.7 |
| X/R inside CATS p10-p90 at 66 kV | – | 0 ✗ | 0 ✗ | 0 ✗ | ≥ 0.7 |
| Rating inside CATS p10-p90 at 66 kV | – | 0 ✗ | 0 ✗ | 0 ✗ | ≥ 0.7 |
| x per km inside CATS p10-p90 at 115 kV | – | 0 ✗ | 0 ✗ | 0 ✗ | ≥ 0.7 |
| X/R inside CATS p10-p90 at 115 kV | – | 0 ✗ | 0 ✗ | 0 ✗ | ≥ 0.7 |
| Rating inside CATS p10-p90 at 115 kV | – | 0 ✗ | 0 ✗ | 0 ✗ | ≥ 0.7 |
| x per km inside CATS p10-p90 at 230 kV | – | 0.936 ✓ | 0.927 ✓ | 0.955 ✓ | ≥ 0.7 |
| X/R inside CATS p10-p90 at 230 kV | – | 1.000 ✓ | 1.000 ✓ | 1.000 ✓ | ≥ 0.7 |
| Rating inside CATS p10-p90 at 230 kV | – | 1.000 ✓ | 1.000 ✓ | 1.000 ✓ | ≥ 0.7 |
| x per km inside CATS p10-p90 at 500 kV | – | 0.181 ✗ | 0.189 ✗ | 0.256 ✗ | ≥ 0.7 |
| X/R inside CATS p10-p90 at 500 kV | – | 0.582 ✗ | 0.544 ✗ | 0.456 ✗ | ≥ 0.7 |
| Rating inside CATS p10-p90 at 500 kV | – | 0.489 ✗ | 0.479 ✗ | 0.364 ✗ | ≥ 0.7 |
| Transformer x on own base inside CATS p10-p90 | – | 0.142 ✗ | 0.184 ✗ | 0.144 ✗ | ≥ 0.8 |
| DC OPF branch loading, median | 0.285 | 0.216 ✓ | 0.238 ✓ | 0.172 ✓ | ±0.15 |
| DC OPF branch loading, p90 | 0.711 | 0.688 ✓ | 0.767 ✓ | 0.639 ✓ | ±0.15 |
| DC OPF binding fraction | 0.004 | 0.005 ✓ | 0.007 ✓ | 0.001 ✓ | ±0.05 |
| Site distance to CATS sites, symmetric mean (km) | – | 42.8 ✗ | 43.1 ✗ | 44.0 ✗ | ≤ 20.0 |
| Line-km per 25 km cell, Spearman | – | -0.419 ✗ | -0.393 ✗ | -0.434 ✗ | ≥ 0.6 |
| Load per 50 km cell, Spearman | – | -0.295 ✗ | -0.300 ✗ | -0.302 ✗ | ≥ 0.7 |
