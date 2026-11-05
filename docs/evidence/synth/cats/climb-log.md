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
