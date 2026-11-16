---
"@tellegen/engine": minor
"@tellegen/svelte": patch
"@tellegen/webmcp": patch
---

The engine reads and writes cases with PowerIO 0.12.0, so saved files carry version 0.12.0 and PowerIO 0.11 readers refuse them; files from earlier releases still open. An unrated branch is now saved as an infinite rating, and the multiconductor solvers follow PowerIO 0.12 for inverter power factor signs and type-A regulator taps.
