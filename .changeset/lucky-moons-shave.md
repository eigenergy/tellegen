---
"@tellegen/engine": patch
---

Take powerio 0.8.1, which hardens the text writers. The engine embeds powerio
in its wasm, so the export path this package exposes carried the earlier
behavior: a bus name that held a `\n` or `\r` ended its record in a psse, pslf,
powerworld, or OpenDSS write, and the rest of the name parsed as new records.
Case names come from the dropped file. See powerio's
[v0.8.1 release notes](https://github.com/eigenergy/powerio/releases/tag/v0.8.1).
