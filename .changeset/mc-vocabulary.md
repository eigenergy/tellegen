---
"@tellegen/svelte": minor
---

Name the calculation one way: "Multiconductor power flow", as the WebMCP tool `solve_multiconductor_pf` does, on the results heading and region, the unavailable notice and the build and case reasons, with the case kind "Distribution case". The interface says "energized", shows one "Calculating…", one unavailable wording and one excluded-island sentence (`EXCLUDED_ISLAND_BUS`). The solve controls shared by the inspector and the study workspace (power flow settings, run or cancel, invalid-option note and "Why unavailable") are one new component, `McSolveControls`, which takes extra row buttons as an `actions` snippet. `MC_PF_NAME`, the phase-label helpers and `quantity` are exported from the package root.
