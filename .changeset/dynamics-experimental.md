---
"@tellegen/svelte": minor
---

Dynamics (RMS) simulation is now experimental and off by default. With Settings → Experimental features off, the shell shows no Steady state / Dynamics switch, no Dynamics panel, playback dock or map legend, and no dynamics commands. A dropped `.dyr` file, or one an agent imports, is refused with a message that says how to turn it on, and `simulate_dynamics` is not offered to agents. `AppState.dynamicsEnabled` reports the setting, and `app.mode` reads as steady state while it is off. `UserPreferences.subscribe` calls a listener after every preference change.
