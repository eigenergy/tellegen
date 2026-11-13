---
"@tellegen/svelte": patch
---

Find a bus's neutral one way everywhere. The schematic's conductor marks (`conductorSet`), neutral earths and attachment ticks, and the overview's conductor bars (`conductorBars`) now call `neutralTerminal`: a declared neutral wins, a declared absence means none, and only a bus silent about it falls back to the aliases n, 4 and neutral. Before, the schematic honoured declared neutrals only and the bars assumed `n`.
