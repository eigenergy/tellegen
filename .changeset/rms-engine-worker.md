---
"@tellegen/engine": minor
---

Add RMS dynamic simulation in a dedicated worker: `openRmsCase` prepares a case with its PSS/E `.dyr` data once, and `BrowserRmsCase` inspects it, checks that it rests without events, runs events (faults, line faults, trips, load steps) and returns the display channels as one transferred `Float64Array` with a Rust screening of the run (verdict, centre-of-inertia frequency nadir and RoCoF, rotor angle separation, voltage recovery, bus frequencies). Stopping a run terminates the RMS worker only; static studies keep running. `rmsSupport`, `looksLikeDyr`, and the experimental `linear`, `rmsDynamize` and `rmsLibrary` come with it.

The web module now carries the `rms` and `rms-experimental` features. The RMS engine adds about 0.44 MB to the raw `tellegen_bg.wasm`, and the experimental tools add 0.36 MB more (76 KB, about 2%, after brotli). The experimental tools stay behind the Experimental setting, so they cost download size but run nothing until that setting is turned on.
