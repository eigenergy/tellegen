---
"@tellegen/svelte": patch
---

Stop showing an in-progress calculation after a failed solve, and distinguish
map initialization failures from case-file errors in notifications.

Reject a missing AC/SOCWR browser input instead of silently falling back to the
DC-only server solver, carrying forward the fail-closed boundary from the
experimental AC OPF integration.
