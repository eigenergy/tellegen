---
"@tellegen/engine": minor
---

Add RMS dynamic simulation in its own worker: `openRmsCase` prepares a case with its `.dyr` data, and `BrowserRmsCase` runs events and returns the traces with a screening verdict. The web module grows by about 0.8 MB before compression.
