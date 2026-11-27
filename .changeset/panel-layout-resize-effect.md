---
'@tellegen/svelte': patch
---

`PanelLayout.resizeViewport` can be called from an effect: it no longer subscribes the effect to the layout, and it leaves floating panels, `compact` and the drawer alone when they do not change. Calling it from an effect while a panel floats no longer throws `effect_update_depth_exceeded`.
