# CATS scorecard: california (synthetic)

Pass rule: every critical check, and at least 80% of the scored metrics, in every run.

| Run | Verdict | Passed | Score | Critical failures |
| --- | --- | --- | --- | --- |
| seed 1 | pass | 41/47 (87%) | 21.91 | none |
| seed 7 | pass | 42/47 (89%) | 21.38 | none |
| seed 2026 | pass | 42/47 (89%) | 20.98 | none |

| Metric | CATS | seed 1 | seed 7 | seed 2026 | Tolerance |
| --- | --- | --- | --- | --- | --- |
| Peak load (MW) | 44,009 | 48,042 ✓ | 48,042 ✓ | 48,042 ✓ | ±15% |
| Nameplate excl. condensers and imports (MW) | 67,972 | 76,873 ✓ | 76,903 ✓ | 77,232 ✓ | ±20% |
| Buses after series reduction | 8,347 | 7,389 ✓ | 7,321 ✓ | 7,361 ✓ | ×0.7–1.4 |
| Sites (buses within 300 m) | 4,999 | 4,403 ✓ | 4,394 ✓ | 4,396 ✓ | ×0.7–1.4 |
| Bus share at 66 kV | 0.651 | 0.616 ✓ | 0.618 ✓ | 0.620 ✓ | ±0.06 |
| Bus share at 115 kV | 0.225 | 0.242 ✓ | 0.241 ✓ | 0.241 ✓ | ±0.06 |
| Bus share at 230 kV | 0.113 | 0.130 ✓ | 0.129 ✓ | 0.126 ✓ | ±0.06 |
| Bus share at 500 kV | 0.011 | 0.011 ✓ | 0.012 ✓ | 0.012 ✓ | ±0.06 |
| Circuit-km at 66 kV | 20,391 | 37,491 ✗ | 38,246 ✗ | 32,392 ✗ | ×0.67–1.5 |
| Circuit-km at 115 kV | 12,991 | 23,042 ✗ | 22,612 ✗ | 21,503 ✗ | ×0.67–1.5 |
| Circuit-km at 230 kV | 13,352 | 18,431 ✓ | 17,586 ✓ | 16,310 ✓ | ×0.67–1.5 |
| Circuit-km at 500 kV | 7,462 | 8,187 ✓ | 8,640 ✓ | 8,251 ✓ | ×0.67–1.5 |
| Transformers per site | 0.132 | 0.143 ✓ | 0.143 ✓ | 0.137 ✓ | ×0.67–1.5 |
| Generators excl. condensers | 2,123 | 1,230 ✓ | 1,229 ✓ | 1,225 ✓ | ×0.5–2.0 |
| Fuel capacity mix, total variation distance | – | 0.224 ✗ | 0.220 ✗ | 0.224 ✗ | ≤ 0.15 |
| Mean degree (reduced) | 2.403 | 2.223 ✓ | 2.223 ✓ | 2.208 ✓ | ±12% |
| Degree distribution, L1 distance | – | 0.234 ✓ | 0.232 ✓ | 0.248 ✓ | ≤ 0.25 |
| Lines per site, lv band | 1.177 | 0.976 ✗ | 0.976 ✗ | 0.968 ✗ | ±15% |
| Lines per site, mv band | 1.110 | 0.973 ✓ | 0.974 ✓ | 0.979 ✓ | ±15% |
| Lines per site, hv band | 1.172 | 1.101 ✓ | 1.109 ✓ | 1.103 ✓ | ±15% |
| Line length / MST, lv band | 1.435 | 1.417 ✓ | 1.441 ✓ | 1.297 ✓ | ±25% |
| Line length / MST, mv band | 1.510 | 1.351 ✓ | 1.347 ✓ | 1.360 ✓ | ±25% |
| Line length / MST, hv band | 2.619 | 2.018 ✓ | 2.047 ✓ | 1.980 ✓ | ±25% |
| Lines on the Delaunay triangulation, lv band | 0.874 | 0.891 ✓ | 0.896 ✓ | 0.885 ✓ | ±0.1 |
| Lines on the Delaunay triangulation, mv band | 0.830 | 0.889 ✓ | 0.885 ✓ | 0.882 ✓ | ±0.1 |
| Lines on the Delaunay triangulation, hv band | 0.710 | 0.750 ✓ | 0.761 ✓ | 0.758 ✓ | ±0.1 |
| Mean clustering coefficient | 0.033 | 0.045 ✓ | 0.042 ✓ | 0.040 ✓ | ±0.05 |
| Mean hop distance | 25.6 | 27.9 ✓ | 24.8 ✓ | 26.4 ✓ | ×0.67–1.5 |
| x per km inside CATS p10-p90 at 66 kV | – | 0.828 ✓ | 0.828 ✓ | 0.840 ✓ | ≥ 0.7 |
| X/R inside CATS p10-p90 at 66 kV | – | 1.000 ✓ | 1.000 ✓ | 1.000 ✓ | ≥ 0.7 |
| Rating inside CATS p10-p90 at 66 kV | – | 0.995 ✓ | 0.996 ✓ | 0.999 ✓ | ≥ 0.7 |
| x per km inside CATS p10-p90 at 115 kV | – | 0.940 ✓ | 0.938 ✓ | 0.945 ✓ | ≥ 0.7 |
| X/R inside CATS p10-p90 at 115 kV | – | 1.000 ✓ | 1.000 ✓ | 1.000 ✓ | ≥ 0.7 |
| Rating inside CATS p10-p90 at 115 kV | – | 0.991 ✓ | 0.991 ✓ | 0.990 ✓ | ≥ 0.7 |
| x per km inside CATS p10-p90 at 230 kV | – | 0.951 ✓ | 0.940 ✓ | 0.942 ✓ | ≥ 0.7 |
| X/R inside CATS p10-p90 at 230 kV | – | 1.000 ✓ | 1.000 ✓ | 1.000 ✓ | ≥ 0.7 |
| Rating inside CATS p10-p90 at 230 kV | – | 0.988 ✓ | 0.990 ✓ | 0.990 ✓ | ≥ 0.7 |
| x per km inside CATS p10-p90 at 500 kV | – | 0.760 ✓ | 0.821 ✓ | 0.755 ✓ | ≥ 0.7 |
| X/R inside CATS p10-p90 at 500 kV | – | 1.000 ✓ | 1.000 ✓ | 1.000 ✓ | ≥ 0.7 |
| Rating inside CATS p10-p90 at 500 kV | – | 0.971 ✓ | 0.962 ✓ | 0.991 ✓ | ≥ 0.7 |
| Transformer x on own base inside CATS p10-p90 | – | 0.998 ✓ | 1.000 ✓ | 1.000 ✓ | ≥ 0.8 |
| DC OPF branch loading, median | 0.285 | 0.127 ✗ | 0.144 ✓ | 0.093 ✗ | ±0.15 |
| DC OPF branch loading, p90 | 0.711 | 0.662 ✓ | 0.694 ✓ | 0.662 ✓ | ±0.15 |
| DC OPF binding fraction | 0.004 | 0.001 ✓ | 0.001 ✓ | 0.001 ✓ | ±0.05 |
| Site distance to CATS sites, symmetric mean (km) | – | 6.383 ✓ | 6.555 ✓ | 6.395 ✓ | ≤ 20.0 |
| Line-km per 25 km cell, Spearman | – | 0.525 ✗ | 0.470 ✗ | 0.602 ✓ | ≥ 0.6 |
| Load per 50 km cell, Spearman | – | 0.828 ✓ | 0.817 ✓ | 0.845 ✓ | ≥ 0.7 |
