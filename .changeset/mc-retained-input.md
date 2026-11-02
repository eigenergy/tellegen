---
'@tellegen/engine': minor
'@tellegen/svelte': minor
---

A multiconductor case can stay parsed in the engine instead of crossing the worker boundary as PowerIO IR text. `ingestJsonDrop` and `ingestDistCaseBytes` take `{ retain: true }` and then return the parsed module as `payload.input`, a `BrowserMcInput`, in place of `module_json`. `createMcPfSession` accepts that input, `BrowserMcPfSession.input()` returns the edited input in the same form, and `BrowserMcInput.moduleJson()` writes the IR text only when it is asked for. Without `retain`, both ingest calls return `module_json` as before.

The Svelte controller imports multiconductor cases this way, so AC power flow on a whole-city model no longer fails with "input exceeds 128 MiB limit". For a 57,757-bus model, the pretty-printed IR was 187.6 MB. `MulticonductorCase.moduleJson` is now `MulticonductorCase.mcInput`, which holds either a `BrowserMcInput` or IR text.
