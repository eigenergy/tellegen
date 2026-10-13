---
"@tellegen/engine": minor
---

`SolveRequest` accepts `constraints`: linear constraints on the DC OPF over
branch flows, bus net injections, and generator outputs, such as interface
or transfer limits. Each states `lower <= sum(coefficient * quantity) <= upper`
in MW. `SolveResponse.constraints` reports each row's value, limits, shadow
price, and whether it binds, and `ProblemCaps.constraints` lists the term kinds
a formulation accepts. The new types are `LinearConstraint`, `ConstraintTerm`,
`ConstraintTermKind`, and `ConstraintResult`. Other formulations, and DC OPF
requests that also ask for sensitivities, refuse constraints.
