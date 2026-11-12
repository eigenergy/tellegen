---
"@tellegen/engine": patch
---

The engine reads and writes cases with PowerIO 0.11.11, up from 0.11.9. The two patch releases in between are conversion fixes: XIIDM output gives every piece of equipment a terminal or leaves it out and says so, Egret time-series output writes `null` for an unlimited branch rating instead of a `0` Egret reads as a zero limit, CGMES output states a generated phase tap changer's reactance on the right base, canonical pandapower output declares package profile 3.2.1 with the split ZIP-load columns, CGMES active limits take precedence over equipment defaults and transformer current limits use their own terminal voltage, and OpenDSS decks find include files whose names differ only in letter case. No API the engine calls changes, and PowerIO IR version 2 is unchanged.
