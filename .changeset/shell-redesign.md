---
"@tellegen/svelte": minor
---

New look and a simpler shell.

- Design tokens: cool grey panels (`--panel`, `--raised`), ink text, a cobalt accent (`--accent`) for interaction, focus and selection, `--red` only for a line at its limit and `--amber` only for one near it. New tokens `--radius-panel`, `--radius-control`, `--on-accent`, `--surface-hover`, `--font-sans`, `--font-condensed` and `--font-code`. The type scale is 12, 13, 15, 20 and 26 px. Every number uses tabular figures, and `--font-mono` now points at the sans face; only code uses `--font-code`. The dark theme defines the same tokens.
- Map colours: LMP and voltage magnitude use a magma segment, voltage angle a diverging brown to teal ramp (`angleColor`, `angleGradient`), and the selection is cobalt. `scalarColor(mode, t)` and `scalarGradient(mode)` pick the ramp for a display mode. The selection cue pops once instead of pulsing.
- Header: one case menu replaces the row of case tabs and the drop tile. It lists every case with a remove button, and has Open a file, Open folder and Restore default cases. The right side has Commands, Docs and a Menu with Settings, Reset layout, What's new, Credits and Privacy. The default `orgLabel` is "Eigenergy Group, University of Michigan" and appears in that menu.
- Panels: the tab strip and the left dock form one panel that shows one tab at a time. Agent, Log and Solver panels dock on the left by default. Closing the front tab closes the dock. A panel floats from its options menu ("Float over the map"). `PanelFrame` takes `tab` to list a right-docking panel in the strip; Dynamics does this. `PanelLayout.shown(panel)` reports whether a panel is on screen.
- The solver's result shows inside the Network panel. `SolveCard embedded` renders it there; without `embedded` it is still its own panel, docked right.
- The Steady state / Dynamics switch moved into the Dynamics panel and shows once the case has dynamic data. The map's own Single-line diagram button is gone: Show diagram in the Network panel is the one toggle. The shell no longer renders `AppFooter` or `RestoreDefaultsButton`; both stay exported.
- Error titles name what failed in a few words; the full message is under Details.
