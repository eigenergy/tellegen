---
"@tellegen/engine": minor
---

Generate state-level grids down to subtransmission, in Synthetic and
OpenStreetMap mode. A region whose places lie mostly in one US state takes
that state's demand (EIA-861 sales with an EIA-930 peak factor), capacity mix
(EIA-860) and voltage ladder; California's is 500/230/115/66 kV. A ladder that
reaches below 100 kV is built level by level: a meshed top level,
subtransmission pockets fed from step-downs with tap junctions, regulated
secondaries and nameplate sized by firm capacity credit. Synthetic California
is held to the California Test System at three seeds, and OpenStreetMap
California at its own run, in `docs/evidence/synth/cats/`.

OpenStreetMap mode keeps 60–70 kV lines where the ladder reaches them, splits
mapped lines at their junctions into tap buses, prunes spurs that end nowhere,
leaves out junctions beyond the clip buffer, adds step-downs where the mapped
network cannot carry its load, and regulates subtransmission buses beyond the
pocket reach of any step-down. Generation is faster in both modes: angle
solves use a sparse Cholesky factorisation, line ends snap through a grid
index, and containment tests read only the polygon edges that can cross the
point's latitude. OpenStreetMap California takes 1.1 s natively, from 5.1 s,
and 1.8 s in the wasm engine; Synthetic California, 7,361 buses, 1.3 s in the
wasm engine.

The report gains `BusKind` `"tap"`, `context.subdivision`,
`context.demand_by_subdivision`, `context.fuel_mix_basis`, and the
`"subdivision"` voltage basis. `limits.max_bus_count` defaults to 12,000,
from 3,000. A transmission-only synthetic grid sized from demand still stops at
3,000 buses; one that reaches subtransmission is sized up to the cap.
