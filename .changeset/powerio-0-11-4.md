---
"@tellegen/engine": patch
---

Build against PowerIO 0.11.4. A saved multiconductor Study whose PowerIO
solution carries more than 65,536 terminal values reads back instead of
failing with `READ.MODULE.INVALID`, within the 128 MiB input boundary.
