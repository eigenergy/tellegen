---
"@tellegen/svelte": minor
---

Add a Dynamics mode beside Steady state: attach a PSS/E `.dyr` file to a dropped `.raw` case (or open the bundled WECC 240 demo), pick faults, line faults, trips and load steps from the selected bus or branch, and run an RMS simulation in its own worker. The map plays the run back with bus voltages on a fixed scale and machine speed rings, a timeline dock scrubs and plays it with event markers, trace charts show frequency, rotor angles, voltages and loads, and a verdict card states the screening result row by row with its criteria. A validated WECC 240 scenario carries a badge only when every setting matches the published validation. An Experimental switch in Settings shows small-signal modes and default dynamics for a case without `.dyr` data.
