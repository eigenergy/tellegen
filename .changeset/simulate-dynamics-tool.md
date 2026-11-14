---
"@tellegen/webmcp": minor
---

Add the `simulate_dynamics` tool: an RMS simulation of the displayed case after up to 16 events (bus faults, line faults cleared by opening the line, branch and unit trips, load steps), returning the screening verdict, the centre-of-inertia frequency nadir and RoCoF, rotor angle separation, voltage recovery and up to four short traces within the output budget. Ambiguous or unknown element ids are refused with the candidates. `import_case` accepts `.dyr` text, which attaches to the displayed case.
