---
'@tellegen/engine': minor
'@tellegen/svelte': minor
---

A multiconductor session can hand over its complete result as columns. `BrowserMcPfSession.resultTables()` returns `McPfResultTables`: the terminals, equipment ports, and source reactions of `result()`, row for row, with each name stored once and every value in a typed array. The worker transfers the arrays rather than copying them, and `mcPfResultTables(result)` gives a stored `McPfResult` the same form.

The overview and the diagram now read a live session's operating point this way, so analysing a whole-city model no longer parses a 166 MiB JSON result into an object per terminal and port. `MulticonductorCase.overviewResult` holds either form.
