---
"@tellegen/engine": minor
---

`SolveRequest` accepts `start: "flat" | "case"` (`PowerFlowStart`). With
`"case"`, the AC power flow starts Newton from the case's stored bus voltages
(each bus's stored magnitude and its angle relative to the reference bus), and
falls back to the flat start if that state does not converge. Other
formulations refuse `"case"`. AC power flow now also accepts a reference bus
with a nonzero stated angle and reports every angle against it.
