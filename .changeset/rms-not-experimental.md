---
"@tellegen/svelte": patch
"@tellegen/webmcp": patch
---

Dynamics (RMS simulation) is no longer experimental: the Dynamics mode, its runs and the `simulate_dynamics` agent tool are on by default. The RMS engine is checked against PowerSimulationsDynamics.jl in CI: every dynamic model it accepts on a single machine (ESAC1A also at its limits), and WECC 240 as a system. Settings → Experimental features now gates only the tools that are still being validated: small-signal modes and frequency responses, default dynamic data, the live view and unit editing.
