---
'@tellegen/engine': patch
'@tellegen/svelte': patch
---

A generated grid reaches its first DC OPF result sooner. `BrowserSynthSession.ingestWithStudy()` starts the Study while the payload is prepared, and `openBalancedModule` takes that Study through its new `study` option.
