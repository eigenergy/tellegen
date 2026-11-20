---
"@tellegen/engine": patch
"@tellegen/svelte": patch
---

The engine builds on PowerIO 0.12.4. AC power flow on a case with HVDC lines or on a PowerIO module solves for the voltage at a bus coded PV whose generators are all out of service, and reports the bus in a `TRANSFORM.INSTANCE.ASSUMPTION` diagnostic; it held that bus at its stated voltage before. `@tellegen/svelte` is rebuilt on the new engine.
