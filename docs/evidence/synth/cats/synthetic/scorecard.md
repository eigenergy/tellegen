# CATS scorecard: california (synthetic)

Pass rule: every critical check, and at least 80% of the scored metrics, in every run.

| Run | Verdict | Passed | Score | Critical failures |
| --- | --- | --- | --- | --- |
| seed 1 | pass | 49/53 (92%) | 22.25 | none |
| seed 7 | pass | 47/53 (89%) | 23.09 | none |
| seed 2026 | pass | 47/53 (89%) | 22.84 | none |

| Metric | CATS | seed 1 | seed 7 | seed 2026 | Tolerance |
| --- | --- | --- | --- | --- | --- |
| Peak load (MW) | 44,009 | 48,042 ✓ | 48,042 ✓ | 48,042 ✓ | ±15% |
| Nameplate excl. condensers and imports (MW) | 67,972 | 76,854 ✓ | 76,903 ✓ | 76,975 ✓ | ±20% |
| Buses after series reduction | 8,347 | 7,259 ✓ | 7,200 ✓ | 7,206 ✓ | ×0.7–1.4 |
| Sites (buses within 300 m) | 4,999 | 4,390 ✓ | 4,390 ✓ | 4,387 ✓ | ×0.7–1.4 |
| Bus share at 66 kV | 0.651 | 0.616 ✓ | 0.622 ✓ | 0.623 ✓ | ±0.06 |
| Bus share at 115 kV | 0.225 | 0.243 ✓ | 0.241 ✓ | 0.239 ✓ | ±0.06 |
| Bus share at 230 kV | 0.113 | 0.128 ✓ | 0.125 ✓ | 0.126 ✓ | ±0.06 |
| Bus share at 500 kV | 0.011 | 0.012 ✓ | 0.012 ✓ | 0.012 ✓ | ±0.06 |
| Circuit-km at 66 kV | 20,391 | 24,134 ✓ | 23,618 ✓ | 22,773 ✓ | ×0.67–1.5 |
| Circuit-km at 115 kV | 12,991 | 16,124 ✓ | 15,741 ✓ | 16,194 ✓ | ×0.67–1.5 |
| Circuit-km at 230 kV | 13,352 | 14,559 ✓ | 14,648 ✓ | 14,101 ✓ | ×0.67–1.5 |
| Circuit-km at 500 kV | 7,462 | 8,155 ✓ | 8,281 ✓ | 7,873 ✓ | ×0.67–1.5 |
| Transformers per site | 0.132 | 0.133 ✓ | 0.133 ✓ | 0.135 ✓ | ×0.67–1.5 |
| Generators excl. condensers | 2,123 | 1,223 ✓ | 1,222 ✓ | 1,222 ✓ | ×0.5–2.0 |
| Fuel capacity mix, total variation distance | – | 0.224 ✗ | 0.222 ✗ | 0.223 ✗ | ≤ 0.15 |
| Mean degree (reduced) | 2.403 | 2.209 ✓ | 2.203 ✓ | 2.200 ✓ | ±12% |
| Degree distribution, L1 distance | – | 0.245 ✓ | 0.254 ✗ | 0.246 ✓ | ≤ 0.25 |
| Lines per site, lv band | 1.177 | 0.970 ✗ | 0.964 ✗ | 0.960 ✗ | ±15% |
| Lines per site, mv band | 1.110 | 0.960 ✓ | 0.962 ✓ | 0.964 ✓ | ±15% |
| Lines per site, hv band | 1.172 | 1.096 ✓ | 1.087 ✓ | 1.084 ✓ | ±15% |
| Line length / MST, lv band | 1.435 | 1.208 ✓ | 1.178 ✓ | 1.185 ✓ | ±25% |
| Line length / MST, mv band | 1.510 | 1.224 ✓ | 1.242 ✓ | 1.276 ✓ | ±25% |
| Line length / MST, hv band | 2.619 | 1.890 ✗ | 1.916 ✗ | 1.868 ✗ | ±25% |
| Lines on the Delaunay triangulation, lv band | 0.874 | 0.889 ✓ | 0.894 ✓ | 0.892 ✓ | ±0.1 |
| Lines on the Delaunay triangulation, mv band | 0.830 | 0.883 ✓ | 0.884 ✓ | 0.882 ✓ | ±0.1 |
| Lines on the Delaunay triangulation, hv band | 0.710 | 0.777 ✓ | 0.766 ✓ | 0.765 ✓ | ±0.1 |
| Mean clustering coefficient | 0.033 | 0.044 ✓ | 0.048 ✓ | 0.048 ✓ | ±0.05 |
| Mean hop distance | 25.6 | 26.7 ✓ | 26.3 ✓ | 26.2 ✓ | ×0.67–1.5 |
| x per km inside CATS p10-p90 at 66 kV | – | 0.830 ✓ | 0.821 ✓ | 0.827 ✓ | ≥ 0.7 |
| X/R inside CATS p10-p90 at 66 kV | – | 1.000 ✓ | 1.000 ✓ | 1.000 ✓ | ≥ 0.7 |
| Rating inside CATS p10-p90 at 66 kV | – | 1.000 ✓ | 1.000 ✓ | 1.000 ✓ | ≥ 0.7 |
| x per km inside CATS p10-p90 at 115 kV | – | 0.946 ✓ | 0.945 ✓ | 0.950 ✓ | ≥ 0.7 |
| X/R inside CATS p10-p90 at 115 kV | – | 1.000 ✓ | 1.000 ✓ | 1.000 ✓ | ≥ 0.7 |
| Rating inside CATS p10-p90 at 115 kV | – | 0.993 ✓ | 0.991 ✓ | 0.989 ✓ | ≥ 0.7 |
| x per km inside CATS p10-p90 at 230 kV | – | 0.958 ✓ | 0.939 ✓ | 0.960 ✓ | ≥ 0.7 |
| X/R inside CATS p10-p90 at 230 kV | – | 1.000 ✓ | 1.000 ✓ | 1.000 ✓ | ≥ 0.7 |
| Rating inside CATS p10-p90 at 230 kV | – | 0.996 ✓ | 0.997 ✓ | 0.996 ✓ | ≥ 0.7 |
| x per km inside CATS p10-p90 at 500 kV | – | 0.798 ✓ | 0.750 ✓ | 0.760 ✓ | ≥ 0.7 |
| X/R inside CATS p10-p90 at 500 kV | – | 1.000 ✓ | 1.000 ✓ | 1.000 ✓ | ≥ 0.7 |
| Rating inside CATS p10-p90 at 500 kV | – | 0.990 ✓ | 0.962 ✓ | 0.981 ✓ | ≥ 0.7 |
| Transformer x on own base inside CATS p10-p90 | – | 1.000 ✓ | 1.000 ✓ | 1.000 ✓ | ≥ 0.8 |
| DC OPF branch loading, median | 0.285 | 0.143 ✓ | 0.143 ✓ | 0.126 ✗ | ±0.15 |
| DC OPF branch loading, p90 | 0.711 | 0.713 ✓ | 0.721 ✓ | 0.702 ✓ | ±0.15 |
| DC OPF binding fraction | 0.004 | 0.002 ✓ | 0.000 ✓ | 0 ✓ | ±0.05 |
| Site distance to CATS sites, symmetric mean (km) | – | 4.630 ✓ | 4.691 ✓ | 4.774 ✓ | ≤ 20.0 |
| Line-km per 25 km cell, Spearman | – | 0.575 ✗ | 0.580 ✗ | 0.598 ✗ | ≥ 0.6 |
| Load per 50 km cell, Spearman | – | 0.834 ✓ | 0.840 ✓ | 0.819 ✓ | ≥ 0.7 |
| Sited buses in 0.5° cells under 20,000 people, share | 0.181 | 0.166 ✓ | 0.162 ✓ | 0.160 ✓ | ±0.07 |
| Sited buses in the 10 most populous 0.5° cells, share | 0.350 | 0.349 ✓ | 0.355 ✓ | 0.366 ✓ | ±0.1 |
| Sited buses per 1,000 km², Los Angeles basin | 46.3 | 83.7 ✓ | 86.3 ✓ | 87.5 ✓ | ×0.5–2.0 |
| Sited buses per 1,000 km², Mojave Desert | 1.440 | 2.483 ✓ | 3.476 ✗ | 3.178 ✗ | ×0.5–2.0 |
| Sited buses per 1,000 km², Modoc and Lassen | 1.105 | 1.489 ✓ | 1.249 ✓ | 1.489 ✓ | ×0.5–2.0 |
| Sited bus density, Los Angeles basin over Modoc and Lassen | 41.9 | 56.2 ✓ | 69.1 ✓ | 58.8 ✓ | ×0.33–3.0 |
