---
"@tellegen/svelte": minor
"@tellegen/engine": patch
---

Add a transmission operating-limits view with voltage magnitudes and branch utilization,
independent of the map display series. Show fixed voltage scales, lower/upper violation markers,
local utilization gauges with a 100% tick, and a persistent configurable loading-warning threshold.
Station summaries retain extrema and the worst bus identity, with links to select those buses.

Preserve solver formulation and status in compact results. Show result freshness, missing limits,
out-of-service and de-energized equipment, and the SOCWR relaxation explicitly. Keep nominal
voltage colours in their own view, and include operating gauges and context in PNG exports.

Keep the solved result when saving a study whose commit and snapshot differ only in the
ordering of JSON fields.
