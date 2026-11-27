---
'@tellegen/svelte': patch
---

`app.sheetInset` is correct when `PanelHost` hides its tab strip: it counts the open panel's header above the sheet and is 0 when no panel is open.
