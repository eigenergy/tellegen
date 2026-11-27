---
'@tellegen/svelte': patch
---

`TellegenProvider` follows the document root's `data-theme` when it has no `theme` prop, including later changes. `.tellegen` sets `color-scheme` for its theme, so native controls match. The log, the diagram's limit labels and the map controls read the provider's theme instead of the root's, and the voltage legend uses the dark ramp in the dark theme. Until it mounts, the provider leaves `data-theme` off its element unless the host passed `theme`, so a server-rendered page paints dark from the first frame when its root has `data-theme="dark"` or the operating system prefers dark.
