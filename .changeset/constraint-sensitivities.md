---
"@tellegen/engine": minor
---

DC OPF sensitivities now cover requests with linear `constraints`: every cell
differentiates the constrained program. The new `ConstraintLimit` parameter
differentiates with respect to a constraint's limits, with columns keyed
`{ Constraint: index }` by request position.
