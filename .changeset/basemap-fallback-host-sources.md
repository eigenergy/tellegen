---
'@tellegen/svelte': patch
---

A source the host added to `TellegenMap`'s map counted as part of the basemap: if its tiles answered 401, 403 or 429, or kept failing to load, the map switched to the fallback basemap although the primary was fine, and the switch removed the host's own sources and layers. Now only the sources of the basemap style itself can trigger the switch, and a host source's errors are logged to the console. A real switch still replaces the whole style, so a host that adds sources and layers in `onMapReady` should add them on every `style.load`; the README shows how. `BasemapMapLike` gains an optional `getStyle`, which `attachBasemapFallback` reads to tell the basemap's sources from the host's.
