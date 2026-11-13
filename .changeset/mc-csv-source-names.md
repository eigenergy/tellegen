---
'@tellegen/svelte': patch
---

The bus CSV's Pairs column keeps the source terminal names (`1–n 2–n 3–n`) so an export round-trips with the case data; the results table shows the phase labels (`A–N`) from a separate `pairLabels` row field that is not exported.
