---
"@tellegen/engine": minor
---

Add RMS dynamic simulation in a dedicated worker: `openRmsCase` prepares a case with its PSS/E `.dyr` data once, and `BrowserRmsCase` inspects it, checks that it rests without events, runs events (faults, line faults, trips, load steps) and returns the display channels as one transferred `Float64Array` with a Rust screening of the run (verdict, centre-of-inertia frequency nadir and RoCoF, rotor angle separation, voltage recovery, bus frequencies). Stopping a run terminates the RMS worker only; static studies keep running. `rmsSupport`, `looksLikeDyr`, and the experimental `linear`, `rmsDynamize` and `rmsLibrary` come with it.
