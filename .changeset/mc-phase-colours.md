---
"@tellegen/svelte": minor
---

Color terminals by phase wherever they are listed: the terminal and equipment results, the overview's pair and conductor bars, and the results table's pair cells now carry the same phase dots as the map strands, inspector chips and multi-wire view. The neutral is one theme-aware gray, `MAP_PALETTE.neutral`, through `phaseColor(terminal, theme)` and the new `phaseCss`. Source, generator and inverter badges take wine, mustard and teal so they no longer share the phase B and C hues. The caution rule beside a qualified result uses a new `--warn` token with a dark-theme value.
