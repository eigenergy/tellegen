---
"@tellegen/svelte": minor
---

Read terminals as phases everywhere a person sees them. `phaseLabel(terminal, neutral?)` maps OpenDSS node numbers 1/2/3/4 and named a/b/c/n to A/B/C/N (and the ground reference to G), passing other names through; `pairLabel`, `pairVoltageLabel` and `wiresLabel` build on it. The legend, inspector chips, terminal results, overview bars and issues, multi-wire view, map tooltip and schematic text use them, and every conductor count reads `3 phases + N`. The bus table's voltage columns are named after the pair their rows share (`V AN`, `V AB`, `V AG`) and filled by phase, so they no longer read as sequence voltages V1/V2/V3. Agent tools and JSON exports keep the source terminal names.
