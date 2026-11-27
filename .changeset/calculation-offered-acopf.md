---
'@tellegen/svelte': patch
---

Calculation menus offer AC OPF when the host configured the solver asset, before it has loaded; it loads when chosen, and a failed load stops the offer. The calculation settings use the same rule. `FormulationSelector` takes `currentCase`, `id` and `variant="segmented"` props and no longer sets its own top margin.
