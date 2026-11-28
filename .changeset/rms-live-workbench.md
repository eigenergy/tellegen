---
"@tellegen/engine": patch
"@tellegen/svelte": patch
---

Experimental RMS tools in the browser. The engine's RMS worker gains a case's DYR records as named parameters and their edits, a unit's models (set, remove, a published unit), controller block graphs (template, check) and a live session that advances on demand and takes events at its current time (`BrowserRmsLive`); `BrowserRmsCase.linear` takes frequency responses (`bode`). Dynamics mode gains a live workbench under the map (plots of any bus, unit or line channel; bus and line faults, trips, closes, load steps and reference changes applied as they are picked; the engine's real-time factor), bus actions in the Network panel, unit selection in the Models tab and controller block diagrams, and Bode plots. A deployment can serve a model library (`examples/models/index.json`): its models open from the case menu and the command palette, and those with dynamic data also from the Dynamics panel while dynamics is turned on.
