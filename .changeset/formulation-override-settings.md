---
'@tellegen/svelte': patch
---

Opening a module with a formulation override selects that calculation in the case's settings, and a Study the kit builds always has explicit settings. A Study built ahead of the case from a module that records settings for another calculation is still used; its solutions and the log no longer carry a `settings_ignored` warning for it.
