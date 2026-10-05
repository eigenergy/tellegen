---
"@tellegen/engine": minor
"@tellegen/svelte": minor
---

Multiconductor power-flow sessions answer a load edit with a constant-size
summary instead of the complete terminal and equipment result, in one worker
round trip. `BrowserMcPfSession.replaceLoadPowers` now resolves to
`McPfSummary`; session creation returns the initial summary; detail is
fetched on demand with `detail()` (one bus and a page of equipment ports) and
`terminalVoltages()`/`terminalCurrents()` (transferred `Float64Array`s), and
`result()` still returns the full result for export. Sessions report the
engine, transfer, and parse time of each edit and the engine's live and peak
heap. In `@tellegen/svelte`, `MulticonductorCase.result` is now the summary,
`mcFullResult` holds a stored full result when no session does, and
`McResults` accepts either with an optional `detail` loader.
