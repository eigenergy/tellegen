---
'@tellegen/svelte': minor
---

Choosing a calculation in the Network panel now runs it, and the panel says when it is solving. Settings shows the options for that calculation in plain words, and no longer repeats the calculation or the DC line model. Menu > Settings brings Settings to the front when another panel covers it, and Escape closes it. The Log tab counts new messages instead of a button floating over the map. The price readout for a selected bus or line uses plain labels and decimals with units. In dark mode the price ramp keeps high prices bright, the diagram uses the dark basemap and readable limit labels, and the map attribution is legible. The diagram's summary sits beside the drawing, and its toolbar has one row of views above the drawing controls. On a phone the case menu fits the screen and the panel sheet has a handle to resize or close it. New exports: `PanelLayout.show(id)` and an optional `badge` on panels.
