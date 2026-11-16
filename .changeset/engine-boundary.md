---
'@tellegen/engine': patch
---

OPF results say what their objective is, and engine errors carry a code.

- `SolveResponse` and `Solution` have an optional `objective_kind`: `'cost'` when the OPF minimized the case's generator cost, `'feasibility'` when the case declares no cost (the objective is then zero and there are no prices). It is absent for power flows. `OBJECTIVE_KINDS` and the `ObjectiveKind` type are exported.
- An error thrown by the engine has a `code` property when the engine knows the class of the failure: one of `ENGINE_ERROR_CODES` (`EngineErrorCode`). `errorCode(error)` reads it, and the code survives the worker boundary. Messages are unchanged.
- A sensitivity request on the DC power flow is refused with code `unsupported`. It used to return an empty list.
