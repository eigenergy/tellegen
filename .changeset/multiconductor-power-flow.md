---
"@tellegen/engine": minor
"@tellegen/svelte": minor
---

Add multiconductor AC power flow for supported BMOPF networks and typed PowerIO
inputs, including explicit neutral conductors and finite-leakage transformers.
Inspect terminal voltages, currents, source powers, convergence, and KCL residuals.
Save and reopen distribution studies with their input and solution modules.

Use published PowerIO 0.11.1 dependencies. Portable electrical modules continue
to use PowerIO IR generation 2.

Apply the OpenDSS load-voltage envelope and report voltage validity separately
from convergence. Support fixed-tap single-phase autotransformers, including
ANSI Type A/B ratios, winding impedance, and excitation shunts.

Retain electrical results when attaching geography to saved distribution studies.
Share cancellable calculations between browser controls and WebMCP, expose
terminal results to agent queries, and add compact notifications and
privacy-filtered usage events.
