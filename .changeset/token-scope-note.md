---
'@tellegen/svelte': patch
---

The theme tokens are declared on `.tellegen`, not `:root`, since 0.10.1: restyle them by overriding them on `.tellegen`. The docks start 8px below `app.headerInset` when the tab strip is hidden. The framework guide now says both.
