# The sensitivity contract

Every sensitivity comes from a converged solution by the implicit function theorem
on a residual $K(z, p) = 0$:

$$ \frac{dz}{dp} = -\left(\frac{\partial K}{\partial z}\right)^{-1} \frac{\partial K}{\partial p}. $$

## Operand / Parameter vocabulary

A sensitivity is named for the physical quantities it relates, not a formulation's
internal variables. The **operand** (what the derivative is *of*) and **parameter**
(what it is taken *with respect to*) are the orthogonal sub-axes (active/reactive,
from/to, voltage representation):

- `Operand`: `Price`, `Dispatch`, `Flow { power, end }`, `Voltage(kind)`.
- `Parameter`: `Demand`, `Cost`, `LineLimit`, `SeriesAdmittance`, `ShuntAdmittance`,
  `VoltageBound`, `GenBound`, `Transformer`, `Switching`.

Each formulation maps a request to its own KKT rows or reports the combination as
unsupported.

## The object-safe `Differentiable` trait

One trait exposes the pieces the shared driver needs: the system Jacobian $K$, the
parameter column $\partial K/\partial p$, the operand selector $S$, and the
per-formulation regularization. It is object safe by construction
(`&dyn Differentiable`), so the DC KKT, the AC Newton system (`AcNewton`), and the
conic KKT (`ConicKkt`) all plug into one driver.

## Forward and adjoint

The single driver runs whichever direction is cheaper:

- **forward** solves $K X = \partial K/\partial p$ once per parameter, reads the
  operand rows;
- **adjoint** solves $K^\top Y = S^\top$ once per operand, contracts with
  $\partial K/\partial p$.

The two are algebraically identical; `Mode::Auto` picks the smaller dimension.

## Per-cell parity classes

Finite differences validate the analytic columns per cell:

- **clean**: cells routed through active power, relative error $< 10^{-3}$.
- **Jabr-coupled / soft**: squared-voltage or reactive cells, looser (the cone's
  degenerate directions).
- **norm-floor skip**: columns below the regularization floor carry no resolvable
  derivative and are not compared.

See [Validation](validation.md) for how these classes are checked, and
[Methodology](methodology.md) for how the figures are produced.
