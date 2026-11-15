---
"@tellegen/webmcp": minor
---

A host can keep `simulate_dynamics` behind a setting. When the adapter provides `dynamicsAvailable()`, the tool is registered only while it returns true, and `onDynamicsAvailabilityChange` tells registration when to check again. Adapters without it register the tool as before.
