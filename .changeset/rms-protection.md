---
"@tellegen/engine": patch
"@tellegen/svelte": patch
---

Protection relays on RMS runs, new excitation models and faster RMS steps. The engine's RMS worker gains protection calls (settings by rule with their audit, the standard tests, the relays at the operating point, static fault studies, runs with relays acting) and `BrowserRmsLive.setProtection`; a live session takes the case options of the runs; on cross-origin isolated pages an optional threaded SIMD build of the RMS engine is used when it is present. Dynamics mode gains a Protection tab and the relays of an element in its actions (experimental), relay operations on the map and in the live view, run options for the solver's speed-ups and held limits, and the excitation limiters in the unit editor.
