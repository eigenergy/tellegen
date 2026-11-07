---
"@tellegen/engine": patch
---

Multiconductor power flow refuses a feeder whose import is incomplete. If an OpenDSS include was missing or nested too deeply, or the import reported an error or an unsupported element, the case still opens for viewing, but it can't become a study or be solved, and `mc_pf_reason` says why. Before, such a partial feeder could reach the solver. Bus names are matched without regard to letter case, as PowerIO matches them, so a line written to `SOURCEBUS` finds the bus `SourceBus`, and the display spelling and exact terminal names are kept. Two buses whose names differ only in case are refused as duplicates.
