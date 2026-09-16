---
"@tellegen/engine": minor
"@tellegen/svelte": minor
---

Add retained fixed-point multiconductor power-flow sessions. Reuse the sparse
factorization and last converged voltage for automatic load P/Q edits, expose
the live session through the browser engine, and materialize portable snapshots
only when they are needed for save, export, or geography operations.
