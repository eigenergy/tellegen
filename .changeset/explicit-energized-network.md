---
"@tellegen/engine": minor
"@tellegen/svelte": minor
---

Detect source-less islands on import and show one context-sensitive solve button
in Network and Studies, with exclusions visible before solving and shared solver
settings. Scroll Studies to the operating-point overview after successful explicit
solves. Add explicit energized-network preparation for multiconductor power flow. Retain
the full input for inspection, identify excluded source-less islands and unsupplied
loads, and preserve and validate network scope in saved studies, solution modules,
and CSV exports. Use default voltage-change tolerance 1e-4 V, absolute KCL tolerance 1e-4 A,
and relative KCL tolerance 1e-5; other solver settings remain unchanged.
