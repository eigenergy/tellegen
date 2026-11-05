---
"@tellegen/engine": patch
"@tellegen/svelte": patch
---

Preserve source voltage bounds, AC endpoint flows, and utilization metadata through browser
and server result adapters. Branch results identify MW versus MVA loading, effective ratings,
stated versus synthesized bounds, and the limiting AC end. Legacy results remain readable.

Transmission result colours distinguish warnings and at-limit operation from overloads. Unknown
and synthesized equipment ratings are faded. Voltage colours use a fixed per-unit scale and
retain actual case-limit violations even when every bus has the same voltage.
