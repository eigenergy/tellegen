---
"@tellegen/engine": patch
"@tellegen/svelte": patch
---

Expose AC OPF recovery diagnostics from the POUNCE 0.13.0 worker and show the
total iteration count, including retries, in the solve card. Mark recovered
solves while preserving support for responses without recovery diagnostics.
