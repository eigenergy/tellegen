---
"@tellegen/svelte": minor
---

Add a command palette, opened with Cmd+K (Ctrl+K) or the header's Commands button. It runs the calculation, switches cases, opens and closes panels, changes the theme, fits the network to the view, resets the panel layout, and toggles zen mode, which hides the panels until Escape and is saved with the display preferences. Components register their own commands with `getCommandRegistry().register()`, or live lists with `registerSource()`.
