---
'@tellegen/engine': patch
---

The multiconductor power flow imports OpenDSS cases with disabled lines and closed fuses, settles voltages where very short lines meet ordinary ones, and checks each transformer and load once per import. The multiconductor OPF result fills PowerIO's per coil generator dispatch. Requires PowerIO 0.12.3.
