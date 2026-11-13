---
"@tellegen/svelte": minor
---

Format every multiconductor quantity through one helper, `quantity(value, unit)` in `format.ts`: four significant figures with thousands separators for V, A, kW, kvar and ms, volts switching to kV at 1 kV, two decimals for degrees, one for percent (no space before `%`), four for pu, and `—` for a missing value. `quantityRange`, `voltsWithPu` and `degrees` build on it. The terminal results, overview, results table, multi-wire view, value labels, schematic text and map tooltips no longer round the same current three different ways. `formatPower` now always reports kW and kvar.
