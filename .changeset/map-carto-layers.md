---
"@tellegen/svelte": patch
---

Add `cartoBasemap(theme, { cartoApiKey, hideLabels })`, a CARTO-only basemap: the keyed style with the keyless one as fallback, or the keyless style alone without a key. `TellegenMap` takes it through a new `basemap` prop, which keeps the provider names in fallback and failure reasons.

`TellegenMap` takes a `layers` prop (`branches`, `buses`, `substations`) that hides network layers without rebuilding them.

Export the map's legend data: the line loading thresholds and colours (`LOADING_ATTENTION`, `LOADING_AT_LIMIT`, `LOADING_BASE`, `LOADING_AMBER`, `LOADING_RED`, `lineLoadingKey`), and `busColorScale`, which gives the bus colour stops, domain, unit and clamped ends in the map's theme. The voltage legend gradient now follows the dark theme.
