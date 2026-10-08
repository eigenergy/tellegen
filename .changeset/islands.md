---
"@tellegen/engine": minor
---

Solves accept networks with several islands. Every formulation solves each
supplied island against its own reference, so each island clears at its own
price. `SolveResponse.diagnostics` (`SolveDiagnostic[]`) reports how the solve
treated the islands. An island with no in-service generator is left out with
its unserved load stated (`island_deenergized`). A supplied island without a
reference bus gets one at its largest generator
(`island_reference_designated`). If one island states several references, the
AC power flow keeps the first as its slack (`island_extra_reference`).
