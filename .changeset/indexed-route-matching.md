---
'@tellegen/svelte': patch
---

Match a multiconductor case's routed lines to its edges through an index built once, instead of scanning every edge for every route. On a 57,757-bus city case the scan took over a minute of the load. Which route lands on which edge does not change.
