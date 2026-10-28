# The sensitivity interface

Every sensitivity comes from a converged solution by the implicit function theorem
on a residual $K(z, p) = 0$:

$$ \frac{dz}{dp} = -\left(\frac{\partial K}{\partial z}\right)^{-1} \frac{\partial K}{\partial p}. $$

## Operand / Parameter vocabulary

This vocabulary and the shared driver below serve the balanced DC/AC/conic
formulations. The experimental native MC OPF API is described
[separately below](#native-multiconductor-opf).

A sensitivity is named for the physical quantities it relates, not a formulation's
internal variables. The **operand** (what the derivative is *of*) and **parameter**
(what it is taken *with respect to*) are the orthogonal sub-axes (active/reactive,
from/to, voltage representation):

- `Operand`: `Price`, `Dispatch`, `Flow { power, end }`, `Voltage(kind)`.
- `Parameter`: `Demand`, `Cost`, `LineLimit`, `SeriesAdmittance`, `ShuntAdmittance`,
  `VoltageBound`, `GenBound`, `Transformer`, `Switching`, `ConstraintLimit`.

`ConstraintLimit` (DC OPF) shifts both limits of a request's linear constraint
by the same amount, or the fixed value of an equality, in MW. Its columns are
keyed `ElementId::Constraint(i)` by the constraint's position in the request.
The DC KKT carries a multiplier for each stated limit of each linear constraint,
so every other DC cell differentiates the constrained program as well.

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

## Native multiconductor OPF

The opt-in native `mc-opf` feature exposes `McOpfResult::sensitivities()` and
`McOpfSession::sensitivities()`. Solve with `McOpfOptions.collect_duals = true`
first. Construction checks the complete saved-report invariants, recompiles the
declared problem, and builds one sparse factorization from its verified primal
and original-scale multipliers. It does not rerun optimization. The returned
`McOpfSensitivities` borrows the accepted result and reuses its factorization for
subsequent queries. The feature enables the existing optional faer dependency;
no additional dependency versions or shipping adapters are enabled.

```rust,ignore
use tellegen::{McOpfParameterDirection, McOpfSensitivityOutput};

// `result` is an accepted MC OPF result with dual collection enabled.
let local = result.sensitivities()?;
// Select an explicit identity from local.parameters(), for example load coil P.
let direction = McOpfParameterDirection {
    parameter: selected_load_p,
    delta: 1.0, // one watt per unit change in the path parameter alpha
};
let derivatives = local.forward(
    &[direction],
    &[McOpfSensitivityOutput::Objective],
)?;
```

`forward` accepts a combined direction in physical parameter units and returns
`d(output)/d(alpha)` for each selected output. Repeated parameter entries add.
`adjoint` accepts `McOpfOutputWeight` entries defining one weighted scalar output
and returns its gradient with respect to selected `McOpfParameter` identities.
Each query uses one backsolve, apart from any numerical refinement; many
parameter or output contractions reuse that solve. Callers own the units of a
weighted scalar that combines different physical quantities. Returned entries
carry output or parameter units explicitly and retain request order.

Parameters use the same mappings as
[local marginal values](formulations.md#dual-diagnostics-and-local-marginal-values):
nominal load coil P/Q for constant-power/current/impedance, ZIP and exponential
laws, selected generator P/Q bounds, inverter availability and capabilities, effective branch
current/apparent-power ratings, transformer port ratings and physical continuous
tap bounds, prepared voltage limits, and inverter control-profile settings. Inactive supported
limits have zero derivatives on the fixed active set. Unknown, unsupported or
coincident-bound parameters produce an error even if their requested weight is
zero; they are never silently treated as inactive limits. Shared linecodes and
scalar bus bounds can affect several effective parameters.

For voltage-dependent loads, P/Q directions vary the canonical nominal coil
powers while holding the voltage-law coefficients, exponents and nominal voltage
fixed. The parameter column includes the exact mixed state–parameter derivative
in stationarity. Canonical metadata preserves terms removed at zero nominal
powers and follows scalar/per-coil defaults and coil incidence. Current-law
rows differentiate `conj(S)*U/abs(U)^2`; power-law rows differentiate the voltage
factor directly, using the log-voltage variable when the compiler introduced it.
This supports zero-P/Q transitions without changing the physical formulation.
At zero voltage, unsupported neighboring positive-voltage profiles are refused;
pure impedance current laws retain their finite derivative. Derivatives with
respect to ZIP fractions, exponents or nominal voltage are not exposed.

Inverter directions use canonical per-coil P/Q bounds and apparent-power ratings,
and terminal current ratings (coil current for three-leg delta). Availability
follows the retained-session edit: one allocated `p_max` entry changes and the
sum becomes `p_avail`. It requires a positive selected allocation and a coherent
observed total. The mappings include all affected volt-var/volt-watt references,
including cross-coil PAvailable targets and averaged voltage monitors. Exact
control-reference partials enter both feasibility and stationarity through the
compiler's existing magnitude lifts. This does not perturb curve coefficients or
introduce numerical finite differences. Two-wire current-rating ties, absent or
nonfinite ratings and zero nameplates do not provide independent partials. See
the linked marginal-value documentation for complete field and axis semantics.

`ControlProfile` directions vary a shared profile's volt-var/volt-watt voltage
breakpoints, output fractions, or signed power factor. They accumulate across
all assigned inverters and coils, including averaged voltage monitors. Exact
mixed derivatives include both the interval slopes and the breakpoint-dependent
softplus smoothing width. No runtime finite differences are used. Breakpoints
use volts; fractions and power factor use ratio units. Unity power factor has
no finite two-sided derivative, and unused profiles are unsupported. For the
bounded PF row `a*P + pf*Q`, the parameter partial is `(-pf/a)*P + Q`,
including both mixed state derivatives; no division by `pf` is needed. Control
families, references, assignments and aggregation stay fixed.

Transformer rating parameters address effective selected result ports in A/VA,
holding electrical coefficients fixed. A shared winding nameplate may require
several port directions and can also change impedance bases; arbitrary canonical
nameplate edits are not represented by a single port parameter. Tap-bound inputs
use physical ratio units, including the bound-side reversal and negative chain
rule of type-B regulators. The physical formulation and nominal tap stay fixed.

Outputs include operating cost, earth-referenced terminal voltage real/imaginary
parts, magnitude and local angle, device coil or terminal P/Q and rectangular
current, and branch-end P/Q and rectangular current. Units are currency/hour,
V, radians, W, var and A. Device selectors include kind and identity, preventing
load/generator name collisions. Coil indices and terminal labels are distinct;
neutral returns and delta terminal powers use the complete physical incidence.
Branch currents leave the bus toward the branch and include the selected end's
shunt. Device signs follow the physical result ledgers. Gradients differentiate
physical measurements rather than assuming auxiliary power columns are exact.
Voltage magnitude and angle at zero are unavailable; angle derivatives describe
a local continuous angle, not the discontinuity of a wrapped display value.

`Transformer` outputs use `McOpfTransformerAxis`: a coil index, a bus/terminal
identity, or a named rating port. Coil and terminal currents enter the transformer;
terminal outputs sum bare winding incidence, including delta and neutral returns.
Separate core/grounding shunt ledgers are excluded from these winding-terminal
outputs. Named ports reproduce the result's rating measurements, including
exciting-current and through-power conventions; they are not extra KCL injections.
P/Q and rectangular-current outputs use W/var/A and differentiate the physical
voltage-current products. `TransformerTap` returns the optimized physical ratio,
with the appropriate direct or inverse derivative. Fixed taps have no such output.
Unknown components, terminals, coils or port names are rejected.

Raw multiplier outputs, network-admittance parameters, fixed/discrete tap
setpoints and control-mode switching are not part of this interface.

For active constraints `c(x,theta)=0`, the stationary system is

$$F(x,\lambda,\theta)=
\begin{bmatrix}\nabla_x f + J_c^T\lambda\\c\end{bmatrix}=0,
\qquad
K=\begin{bmatrix}\nabla_{xx}^2 L & J_c^T\\J_c&0\end{bmatrix}.$$

Forward differentiation solves `K z' = -F_theta theta'`. Adjoint differentiation
solves `K^T a = h_z` and contracts `-a^T F_theta`. POUNCE supplies exact sparse
Jacobians and the Lagrangian Hessian, using only active-row multipliers and the
original objective scale. A normalized rating row `g=G(x)/theta^2` contributes
both `-2g/theta` in the constraint derivative and
`-2 lambda grad(g)/theta` in the stationarity derivative. Both branch ends are
included. No supported output has an additional explicit parameter term.

The symmetric KKT matrix is equilibrated and factored with sparse LU, without
diagonal regularization or a pseudoinverse. Before exposing a view, construction
solves every coordinate right-hand side, streaming the computed inverse `X`
through linear workspace. For the equilibrated matrix `B`, it requires
`||I-BX||_1 < 0.5` and the bound
`||B||_1 ||X||_1 / (1-||I-BX||_1) <= 1e12`, with a floating-point accumulation
allowance. This checks all directions, including null directions orthogonal to
dense probe vectors. Construction costs one sparse backsolve per KKT column;
no dense inverse is retained, and queries reuse the qualified factorization.
Each query also checks its componentwise residual against the original
unregularized matrix and can perform two refinement steps.

`view.diagnostics()` returns the active rank qualification, variable/active-row
counts, KKT dimension and assembled nonzeros. `forward_with_diagnostics` and
`adjoint_with_diagnostics` return the same derivatives as the ordinary methods,
plus the query's residual infinity norm, componentwise backward error,
refinement count and maximum residual/tolerance ratio. That ratio must be at
most one, with tolerance `1e-10 + 1e-8*(|rhs| + |K|*|x|)` per row. These statistics
describe the assembled matrix and backsolve, not factor memory or forward error.

The inverse check is
a numerical safeguard, not an interval-arithmetic or second-order sufficiency
certificate. Every returned derivative is conditional on the same
regular stationary branch and active set; it does not certify global optimality
or predict behavior across active-set transitions.

`McOpfSensitivityError` distinguishes missing duals, stale results, inconsistent
reports, unreliable duals, unsupported parameters/outputs, malformed requests,
derivative-evaluation failure, singular/unstable KKT systems and failed linear
solves. The existing conservative dual eligibility checks still apply, including
zero-limit reductions, weak activity and incomplete or deficient sparse rank
qualification. The rank audit budgets sparse fill and work instead of imposing
the legacy 256-row cutoff; its outcome and usage are retained in the dual report.
It normalizes rows and applies twice-reorthogonalized sparse QR, preferring the
sparsest residual row whose norm is at least half the largest remaining norm
and exceeds the `1e-9` pivot tolerance. The global largest norm controls
termination. This reduces fill on feeders without dropping small coefficients,
forming normal equations or relaxing the tolerance. The limits remain two
million live coefficients and 100 million work units; candidate visits also
count as work. A completed audit is a numerical rank test, not a singular-value
or conditioning certificate. Near the tolerance boundary, pivot order can affect
the reported numerical rank. Larger or denser feeders can still exhaust a budget
and return no rank; physical feasibility does not override that refusal.
Eligibility and factorization errors carry the available `active_rank`; a
failed backsolve carries its last finite `linear_solve` diagnostics when available.
Sensitivities additionally refuse
ambiguous narrow inequalities and loss of stationarity when inactive multipliers
are removed. Physical acceptance alone does not establish this eligibility.

The session entry point requires `current_result()`: edits, changed options and
failed or cancelled attempts invalidate access. Rust borrowing prevents edits
while a sensitivity view is in use. An explicitly selected or cloned historical
result remains an independent snapshot of its original instance. Portable
version 9 results can rebuild a view after import. Version 2–8 dual reports
are verified with their original mappings and rank audit (dense for versions
2–6, largest-residual sparse pivots for 7–8) before upgrading to current mappings
and sparsity-aware qualification. Versions 1–7 with assigned PF
controls are rejected because their constraint scaling differs; they require
a new solve. Factorizations and views are
native in-memory objects and are not serialized. Neither queries nor failures
change the session's accepted result, warm start or revision.
