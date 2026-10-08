---
"@tellegen/engine": minor
---

DC OPF requests accept `limits` (`LimitOptions`):
- `angle_difference: false` drops the angle-difference rows.
- `thermal` keeps `"all"`, `"rated"`, or `{ branches: [...] }` thermal limits.
- `lazy` enforces thermal limits incrementally until none is violated.

`SolveResponse.limit_rounds` (`LimitRound[]`) reports each solve's program size,
iterations, and violations. The program also no longer carries rows for
inactive limits or pinned shedding, which lets PGLib's `case78484` converge.
