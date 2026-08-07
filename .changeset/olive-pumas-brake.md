---
"@tellegen/engine": minor
"@tellegen/svelte": minor
---

Count capacitor banks in a multiconductor case. powerio 0.8.0 reads a BMOPF
capacitor as its own type; before, it went to the untyped table and showed
nowhere. `IngestedDistCase` gains an optional `n_capacitor`, and the
multiconductor panel shows it beside the IBR and shunt counts. A `.dss` or PMD
capacitor still reads as a shunt and still counts in `n_shunt`.
