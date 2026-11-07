# CATS scorecard: california (synthetic)

Pass rule: every critical check, and at least 80% of the scored metrics, in every run.

| Run | Verdict | Passed | Score | Critical failures |
| --- | --- | --- | --- | --- |
| seed 1 | pass | 40/47 (85%) | 22.09 | none |
| seed 7 | pass | 40/47 (85%) | 21.81 | none |
| seed 2026 | pass | 39/47 (83%) | 21.02 | none |

| Metric | CATS | seed 1 | seed 7 | seed 2026 | Tolerance |
| --- | --- | --- | --- | --- | --- |
| Peak load (MW) | 44,009 | 48,042 ✓ | 48,042 ✓ | 48,042 ✓ | ±15% |
| Nameplate excl. condensers and imports (MW) | 67,972 | 77,194 ✓ | 76,925 ✓ | 76,879 ✓ | ±20% |
| Buses after series reduction | 8,347 | 7,389 ✓ | 7,321 ✓ | 7,361 ✓ | ×0.7–1.4 |
| Sites (buses within 300 m) | 4,999 | 4,403 ✓ | 4,394 ✓ | 4,396 ✓ | ×0.7–1.4 |
| Bus share at 66 kV | 0.651 | 0.616 ✓ | 0.618 ✓ | 0.620 ✓ | ±0.06 |
| Bus share at 115 kV | 0.225 | 0.242 ✓ | 0.241 ✓ | 0.241 ✓ | ±0.06 |
| Bus share at 230 kV | 0.113 | 0.130 ✓ | 0.129 ✓ | 0.126 ✓ | ±0.06 |
| Bus share at 500 kV | 0.011 | 0.011 ✓ | 0.012 ✓ | 0.012 ✓ | ±0.06 |
| Circuit-km at 66 kV | 20,391 | 30,636 ✗ | 31,692 ✗ | 31,000 ✗ | ×0.67–1.5 |
| Circuit-km at 115 kV | 12,991 | 21,037 ✗ | 20,966 ✗ | 19,920 ✗ | ×0.67–1.5 |
| Circuit-km at 230 kV | 13,352 | 16,289 ✓ | 15,692 ✓ | 15,833 ✓ | ×0.67–1.5 |
| Circuit-km at 500 kV | 7,462 | 8,187 ✓ | 8,811 ✓ | 8,561 ✓ | ×0.67–1.5 |
| Transformers per site | 0.132 | 0.144 ✓ | 0.142 ✓ | 0.141 ✓ | ×0.67–1.5 |
| Generators excl. condensers | 2,123 | 1,230 ✓ | 1,229 ✓ | 1,225 ✓ | ×0.5–2.0 |
| Fuel capacity mix, total variation distance | – | 0.222 ✗ | 0.220 ✗ | 0.223 ✗ | ≤ 0.15 |
| Mean degree (reduced) | 2.403 | 2.213 ✓ | 2.214 ✓ | 2.200 ✓ | ±12% |
| Degree distribution, L1 distance | – | 0.245 ✓ | 0.238 ✓ | 0.254 ✗ | ≤ 0.25 |
| Lines per site, lv band | 1.177 | 0.969 ✗ | 0.971 ✗ | 0.964 ✗ | ±15% |
| Lines per site, mv band | 1.110 | 0.964 ✓ | 0.965 ✓ | 0.970 ✓ | ±15% |
| Lines per site, hv band | 1.172 | 1.094 ✓ | 1.094 ✓ | 1.093 ✓ | ±15% |
| Line length / MST, lv band | 1.435 | 1.263 ✓ | 1.271 ✓ | 1.231 ✓ | ±25% |
| Line length / MST, mv band | 1.510 | 1.256 ✓ | 1.273 ✓ | 1.261 ✓ | ±25% |
| Line length / MST, hv band | 2.619 | 1.902 ✗ | 1.900 ✗ | 1.903 ✗ | ±25% |
| Lines on the Delaunay triangulation, lv band | 0.874 | 0.890 ✓ | 0.895 ✓ | 0.885 ✓ | ±0.1 |
| Lines on the Delaunay triangulation, mv band | 0.830 | 0.890 ✓ | 0.884 ✓ | 0.883 ✓ | ±0.1 |
| Lines on the Delaunay triangulation, hv band | 0.710 | 0.751 ✓ | 0.767 ✓ | 0.761 ✓ | ±0.1 |
| Mean clustering coefficient | 0.033 | 0.045 ✓ | 0.042 ✓ | 0.041 ✓ | ±0.05 |
| Mean hop distance | 25.6 | 28.7 ✓ | 25.3 ✓ | 27.0 ✓ | ×0.67–1.5 |
| x per km inside CATS p10-p90 at 66 kV | – | 0.836 ✓ | 0.827 ✓ | 0.824 ✓ | ≥ 0.7 |
| X/R inside CATS p10-p90 at 66 kV | – | 1.000 ✓ | 1.000 ✓ | 1.000 ✓ | ≥ 0.7 |
| Rating inside CATS p10-p90 at 66 kV | – | 1.000 ✓ | 1.000 ✓ | 1.000 ✓ | ≥ 0.7 |
| x per km inside CATS p10-p90 at 115 kV | – | 0.946 ✓ | 0.937 ✓ | 0.951 ✓ | ≥ 0.7 |
| X/R inside CATS p10-p90 at 115 kV | – | 1.000 ✓ | 1.000 ✓ | 1.000 ✓ | ≥ 0.7 |
| Rating inside CATS p10-p90 at 115 kV | – | 0.996 ✓ | 0.997 ✓ | 0.997 ✓ | ≥ 0.7 |
| x per km inside CATS p10-p90 at 230 kV | – | 0.928 ✓ | 0.939 ✓ | 0.940 ✓ | ≥ 0.7 |
| X/R inside CATS p10-p90 at 230 kV | – | 1.000 ✓ | 1.000 ✓ | 1.000 ✓ | ≥ 0.7 |
| Rating inside CATS p10-p90 at 230 kV | – | 0.996 ✓ | 1.000 ✓ | 0.997 ✓ | ≥ 0.7 |
| x per km inside CATS p10-p90 at 500 kV | – | 0.731 ✓ | 0.815 ✓ | 0.827 ✓ | ≥ 0.7 |
| X/R inside CATS p10-p90 at 500 kV | – | 1.000 ✓ | 1.000 ✓ | 1.000 ✓ | ≥ 0.7 |
| Rating inside CATS p10-p90 at 500 kV | – | 0.971 ✓ | 0.963 ✓ | 0.991 ✓ | ≥ 0.7 |
| Transformer x on own base inside CATS p10-p90 | – | 0.998 ✓ | 1.000 ✓ | 1.000 ✓ | ≥ 0.8 |
| DC OPF branch loading, median | 0.285 | 0.113 ✗ | 0.128 ✗ | 0.118 ✗ | ±0.15 |
| DC OPF branch loading, p90 | 0.711 | 0.673 ✓ | 0.672 ✓ | 0.694 ✓ | ±0.15 |
| DC OPF binding fraction | 0.004 | 0.003 ✓ | 0.002 ✓ | 0.001 ✓ | ±0.05 |
| Site distance to CATS sites, symmetric mean (km) | – | 6.383 ✓ | 6.555 ✓ | 6.395 ✓ | ≤ 20.0 |
| Line-km per 25 km cell, Spearman | – | 0.540 ✗ | 0.462 ✗ | 0.568 ✗ | ≥ 0.6 |
| Load per 50 km cell, Spearman | – | 0.828 ✓ | 0.817 ✓ | 0.845 ✓ | ≥ 0.7 |
