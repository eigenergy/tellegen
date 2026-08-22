---
"@tellegen/engine": minor
"@tellegen/svelte": minor
---

A saved study now states the powerio release that wrote it, and a study saved by
an earlier build no longer loads. Open the source case and save the study again.
Case uploads are parsed as bytes, so a `.raw` or `.aux` exported in CP1252 is
refused rather than silently mangled.
