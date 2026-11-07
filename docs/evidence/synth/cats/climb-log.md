# Hill climb log

One run of `climb.py` (coordinate search over the `Tuning` constants) at seed 2026, on the
engine as it stood once the hierarchical design, the state tables and the subtransmission
line classes were in. Each evaluation generates California natively and scores it with
`compare.py`; a step is kept when the score falls. Two passes, 111 evaluations, 7.7 minutes.

| Eval | Step | Score | Passed |
| ---: | --- | ---: | ---: |
| 1 | start | 31.65 | 35/47 |
| 4 | pass 1: mw_per_bus = 7.0 | 29.81 | 38/47 |
| 15 | pass 1: mesh_ratio = 1.15 | 29.38 | 38/47 |
| 30 | pass 1: tap_at_site_share = 0.95 | 29.35 | 36/47 |
| 40 | pass 1: fill_bandwidth_km = 24.0 | 28.49 | 37/47 |
| 63 | pass 2: mw_per_bus = 10.0 | 25.55 | 37/47 |
| 70 | pass 2: pocket_ties = [0.35, 0.3, 0.25] | 23.51 | 39/47 |
| 71 | pass 2: mesh_ratio = 1.25 | 23.47 | 40/47 |

The kept values became the defaults in `crates/tellegen-synth/src/tuning.rs`:

```json
{
 "mw_per_bus": 10.0,
 "mesh_ratio": 1.25,
 "tap_at_site_share": 0.95,
 "fill_bandwidth_km": 24.0,
 "pocket_ties": [
  0.35,
  0.3,
  0.25
 ]
}
```

Structural changes made by hand from what the scorecard showed, before and after the climb:

- A layered (hierarchical) design for ladders that reach subtransmission: sites per level,
  a meshed top level, subtransmission pockets fed from step-downs, tap junctions, and pocket
  splitting by load and by reach.
- State tables (EIA-861 demand, EIA-860 fuel mix, EIA-930 peak factors) and the US-CA
  500/230/115/66 kV ladder, picked by the subdivision carrying the most demand.
- 66/69 kV and 115 kV line classes, regulated subtransmission secondaries (load tap changers
  with capacitor banks, modelled as condensers), and capacity sized by firm credit per fuel.
- Every later engine change was checked against both country sweeps, which are the hold-out
  (the climb never sees them).

## Siting: the fill's floor and exponent

The scorecard had no measure of where buses stand against where people live, so the climb
above never moved `fill_floor` or `fill_gamma`. On engine 0.5.1, MapToGrid measured 35% of
California's buses in 0.5° cells whose listed places hold fewer than 20,000 people, against
13% for CATS (tellegen#305). The siting group of `compare.py` now scores this. With it, a
grid search over `fill_floor` {0.06, 0.03, 0.02, 0.01} × `fill_gamma` {0.5, 0.75, 1.0} at
seeds 1, 7 and 2026 (`compare.py run --tuning`), on the engine with every other constant at
its default, gave its lowest score at the grid's edge, so the search went on to floors of
0.005 and 0.0025. Scores are over all 53 metrics; the siting columns are means over the
three seeds, measured as the scorecard does (sited buses). CATS: 0.181, 0.350, 46.3, 1.44,
1.10 and 41.9.

| fill_floor | fill_gamma | Score, mean (worst) | Score without siting | Passed of 53 (seeds 1, 7, 2026) | Critical checks met | Sparse share | Top-10 share | LA | Mojave | NE | LA / NE | AC voltage range (pu) |
| ---: | ---: | ---: | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| 0.06 | 0.5 | 31.56 (32.14) | 21.64 | 41/41/40 | 3 of 3 | 0.354 | 0.211 | 49.8 | 6.85 | 5.56 | 9.0 | 0.956–1.034 |
| 0.06 | 0.75 | 35.67 (42.09) | 25.23 | 42/41/41 | 2 of 3 | 0.370 | 0.235 | 56.5 | 7.02 | 6.64 | 8.5 | 0.881–1.026 |
| 0.06 | 1.0 | 33.00 (33.35) | 21.78 | 42/42/41 | 3 of 3 | 0.405 | 0.234 | 56.7 | 7.46 | 7.83 | 7.3 | 0.965–1.028 |
| 0.03 | 0.5 | 28.80 (29.07) | 21.13 | 44/43/43 | 3 of 3 | 0.297 | 0.238 | 55.7 | 5.83 | 3.63 | 15.4 | 0.957–1.038 |
| 0.03 | 0.75 | 29.14 (29.24) | 21.17 | 45/45/44 | 3 of 3 | 0.291 | 0.282 | 67.7 | 5.79 | 4.91 | 13.8 | 0.953–1.044 |
| 0.03 | 1.0 | 30.42 (30.51) | 21.53 | 44/44/44 | 3 of 3 | 0.323 | 0.298 | 73.5 | 6.06 | 6.15 | 12.0 | 0.910–1.028 |
| 0.02 | 0.5 | 27.69 (27.88) | 21.01 | 44/45/44 | 3 of 3 | 0.271 | 0.250 | 58.7 | 5.33 | 3.06 | 19.2 | 0.954–1.030 |
| 0.02 | 0.75 | 27.35 (27.59) | 21.09 | 46/47/46 | 3 of 3 | 0.251 | 0.308 | 73.9 | 4.97 | 3.68 | 20.1 | 0.944–1.042 |
| 0.02 | 1.0 | 28.25 (28.71) | 21.04 | 46/46/46 | 3 of 3 | 0.275 | 0.333 | 83.1 | 5.40 | 4.87 | 17.1 | 0.945–1.041 |
| 0.01 | 0.5 | 25.85 (26.23) | 20.93 | 47/47/48 | 3 of 3 | 0.243 | 0.264 | 61.3 | 4.70 | 1.95 | 31.5 | 0.975–1.060 |
| 0.01 | 0.75 | 23.99 (24.43) | 20.25 | 47/47/48 | 3 of 3 | 0.196 | 0.338 | 81.6 | 3.76 | 2.32 | 35.4 | 0.952–1.026 |
| 0.01 | 1.0 | 25.23 (25.59) | 20.24 | 46/46/46 | 3 of 3 | 0.202 | 0.388 | 98.1 | 3.92 | 3.19 | 31.2 | 0.949–1.043 |
| 0.005 | 0.5 | 24.04 (24.14) | 20.21 | 48/47/47 | 3 of 3 | 0.227 | 0.268 | 62.6 | 4.37 | 1.23 | 51.0 | 0.969–1.041 |
| 0.005 | 0.75 | 22.73 (23.09) | 19.75 | 49/47/47 | 3 of 3 | 0.163 | 0.357 | 85.8 | 3.05 | 1.41 | 61.3 | 0.953–1.031 |
| 0.005 | 1.0 | 24.29 (24.62) | 19.76 | 48/47/47 | 3 of 3 | 0.144 | 0.431 | 109.2 | 2.68 | 1.89 | 59.2 | 0.951–1.031 |
| 0.0025 | 0.5 | 23.96 (24.50) | 20.00 | 48/48/46 | 3 of 3 | 0.216 | 0.273 | 63.6 | 4.29 | 0.94 | 68.6 | 0.941–1.035 |
| 0.0025 | 0.75 | 23.22 (23.43) | 19.47 | 49/49/48 | 3 of 3 | 0.139 | 0.371 | 89.0 | 2.65 | 0.88 | 101.2 | 0.906–1.031 |
| 0.0025 | 1.0 | 24.16 (24.59) | 19.10 | 48/46/46 | 3 of 3 | 0.105 | 0.457 | 115.3 | 2.09 | 1.36 | 89.8 | 0.969–1.051 |

`fill_floor` 0.005 and `fill_gamma` 0.75 became the defaults: the lowest mean score, every
critical check met at every seed, and the AC voltages 0.953–1.031 pu. A lower floor also
shortens the subtransmission built across empty land: circuit-km at 66 and 115 kV fell from
1.5–1.6 × CATS (failing) to 1.1–1.25 × (passing). Site spacing did not collapse: over the
three seeds the nearest-neighbour distance between sited positions, distinct at 0.01°
(about 1 km, as MapToGrid measures it), fell from a median of 4.1 to 2.8 km and a 10th
percentile of 1.9 to 1.2 km; over all buses, taps at their substations included, the 10th
percentile stays at 0.2 km.

The fill is shared with the transmission design, so both country sweeps were rerun with the
old and the new defaults on the same specs (the old runs reproduce the committed sweeps'
verdicts and voltages exactly). At 1:50m, 53 of 54 runs stay valid: Australia converges at seed 1 and no longer
at seed 2026. At 1:110m, 47 of 51 become 48: the United States converges at seeds 1 and
2026, and Australia no longer at seed 1. Incomplete corridor plans fall from 43 to 35 and
from 38 to 36. Peak demand and bus counts do not change.
