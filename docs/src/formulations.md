# Formulations

Tellegen solves DC power flow, DC OPF, AC power flow, and the Jabr SOCWR
relaxation through the PowerIO module boundary. The shared response has
optional fields for the quantities each formulation produces. Economic fields
are present only when the declared objective gives them that interpretation.
Supported implicit derivatives use the interface described in
[the sensitivity interface](sensitivity-interface.md).

## DC power flow and DC OPF (B–θ)

The linearized power flow couples bus angles $\theta$ to injections through the
susceptance-weighted graph Laplacian

$$ B = A^\top\operatorname{diag}(b)A, \qquad B\theta = p, $$

where $A$ is the branch by bus incidence and $b$ contains positive solver
weights derived from the public branch susceptances. The OPF
minimizes generation cost subject to the network balance and the thermal and
generation limits; it is a convex quadratic program solved with Clarabel.
`solve_module_json` is the portable entry. Rust callers that already own a
`DcOpfInstance` can use `solve_instance`.

MATPOWER model 2 quadratic generator costs are read directly. Convex model 1
piecewise linear costs use one epigraph variable and one inequality per segment,
so dispatch, objective values, prices, and supported implicit derivatives refer
to the declared curve. Malformed and nonconvex model 1 rows are rejected before
the program is assembled. At a breakpoint or an active set change, the marginal
value or its derivative need not be unique; the sensitivity API reports the
local KKT linearization and numerical checks identify stencils that cross a
different active set.

### Linear constraints

A DC OPF request may add linear constraints over branch flows, bus net
injections, and generator outputs. This is the usual way to state an interface
or transfer limit: a bound on a weighted sum of flows across a cut of the
network.

```json
{
  "formulation": "dcopf",
  "constraints": [
    {
      "id": "north-south",
      "terms": [
        { "kind": "branch_flow", "element": 3, "coefficient": 1.0 },
        { "kind": "branch_flow", "element": "branches:7", "coefficient": 1.0 }
      ],
      "upper": 400.0
    }
  ]
}
```

Each constraint states $l \le \sum_t c_t\,q_t \le u$ in MW. Either limit may be
omitted, but not both; equal limits state an equality. A term's `kind` selects
its quantity $q_t$:

| `kind`          | quantity                                              | `element`                       |
| --------------- | ----------------------------------------------------- | ------------------------------- |
| `branch_flow`   | from-end active flow, positive from `from` to `to`    | 1-based branch position or uid  |
| `bus_injection` | net injection: generation minus demand and shunt withdrawal | bus id or uid             |
| `generator`     | active output                                         | 1-based generator position or uid |

Terms naming the same quantity add. An element that is unknown, out of
service, on a de-energized island, or synthesized while lowering a
three-winding transformer is refused by name. A bus injection enters the program as the flow leaving the bus on its
branches, which equals the net injection by power balance. The constraint rows
therefore never depend on demand, and the reported LMP stays the marginal cost
of demand at each bus.

The response adds a `constraints` block in request order with each row's
`value`, `lower`, `upper`, `binding`, and `shadow_price`. The shadow price is
$\mu = -\partial(\text{objective})/\partial(\text{limit})$ in the LMP's units:
positive when the upper limit binds, negative when the lower limit binds, and
zero when neither does. Prices then decompose against any reference bus $r$
in the bus's island:

$$ \lambda_i = \lambda_r - \sum_\ell \mu_\ell \, s_{\ell i}, $$

where $s_{\ell i}$ is the change in constraint $\ell$'s value per MW injected at
bus $i$ and withdrawn at $r$, and the sum also runs over binding line limits.
The shadow price is omitted, like the LMP, when the declared objective is a
feasibility objective.

Linear constraints are a DC OPF feature; DC power flow, AC power flow, and
SOCWR refuse a request that carries them. The DC KKT system carries a
multiplier for each stated limit, so sensitivity cells differentiate the
constrained program. The `ConstraintLimit` parameter differentiates with
respect to a constraint's limits, shifting both by the same MW. Its columns are
keyed by the constraint's position in the request. A Rust `Study` carries
constraints through every commit with `Study::set_constraints`. Previews,
planning, and objective gradients see them. While it has any, it refuses to save
the problem instance or its solution, since the PowerIO problem instance has no
place for them.

On the command line, the default command takes the request as its argument,
`tellegen capabilities` lists the accepted term kinds under each formulation's
`constraints`, and `tellegen describe` includes the `solve_request` and
`solve_result` schemas:

```sh
tellegen '{"constraints":[{"id":"north-south","upper":400,"terms":[{"kind":"branch_flow","element":3,"coefficient":1}]}]}' < case.pio.json
```

### Limits at scale

The program has no row for a limit it does not enforce, and no shedding
variable for a bus that cannot shed. A portable solve never sheds, so that
removes a column and a row pair per bus. These pinned pairs have no interior,
and on PGLib's 78,484-bus case they kept the interior point solve from
converging at all.

A DC OPF request may also choose which limits the program carries:

```json
{ "formulation": "dcopf",
  "limits": { "angle_difference": false, "thermal": "rated", "lazy": {} } }
```

- `angle_difference: false` drops the angle-difference rows, two per branch.
- `thermal` keeps `all` declared thermal limits (the default), only the `rated`
  ones (an unrated branch's synthesized limit is dropped), or the limits of a
  list of `branches`.
- `lazy` solves with no thermal limit enforced, adds every enforceable limit
  the flows violate or meet together with those loaded at or above
  `near_binding` (default 0.98), and solves again until every limit left out
  is strictly slack, for at most `max_rounds` (default 20) solves. A solution
  that violates none of the limits it was not held to is optimal for the
  program with all of them, so prices and dispatch match the full solve.

With `limits`, the response's `limit_rounds` reports each solve: the limits it
enforced, the violations it found, the limits it added, its program size, and
its interior point iterations. Keep the angle-difference rows with `lazy` when
every generator cost is linear. Without them, the first round bounds no angle
or flow, which can stall the interior point method on a degenerate linear
program. The `dcopf-scale-bench` binary in `crates/benchmarks` measures each
option on PGLib's `case78484` and on a synthetic 100k-bus mesh.

Limit options combine with linear constraints and with several islands: every
round carries the request's constraint rows and one reference row per island.
Sensitivity cells differentiate the program the last round solved, in which a
limit left out holds a zero multiplier. With `lazy` that is the full program's
derivative: the last round leaves out only limits with slack, which the full
program would also hold at a zero multiplier, and a limit the flows meet exactly
joins another round rather than being left out at its bound. With `thermal`
set to `rated` or a list of `branches`, or with `angle_difference: false`, it
is the derivative of the reduced program the request asked for: a dropped limit
stays dropped when the parameters move, even if the flows would then cross it.

Branch angle-difference bounds are enforced in radians after normalization.
MATPOWER's unconstrained `-360`/`360` spelling and an unset `0`/`0` pair become
exactly -60/+60 degrees. When a branch has no thermal rating, Tellegen
synthesizes its fallback rating from that same 60 degree window and the terminal
voltage bands. Explicit tighter source bounds are preserved.

## Islands

A network may hold several islands: sets of buses joined by in-service
branches and three-winding transformers but not by each other. Every
formulation solves each supplied island on its own terms. The DC programs fix
the angle at every reference bus. The AC power flow keeps one slack per island,
and each island's slack absorbs that island's balance. With nothing to trade
across islands, each island clears at its own price.

Two kinds of island need a decision first, and the response's `diagnostics`
block reports each one:

- `island_deenergized`: an island with no in-service generator cannot serve its
  load, so it is left out of the solve. Its buses carry no results, and the
  message states the unserved load in MW.
- `island_reference_designated`: a supplied island that states no reference bus
  gets one at the bus of its largest generator by `pmax`, the rule PowerIO
  applies to a network with no reference at all.

The AC power flow also reports `island_extra_reference` when one island states
several reference buses. The first one is the slack, and the others hold their
voltage magnitude as PV buses, or as PQ buses when they host no generator.

## AC power flow (polar)

The nodal power balance in polar coordinates,

$$ S_i = V_i \sum_j \overline{Y_{ij}} \overline{V_j}, $$

is solved by Newton–Raphson on the reduced system
$\partial(P, Q)/\partial(\theta, V_m)$. Buses are typed slack / PV / PQ (PV and
slack buses hold the generator voltage setpoint; PQ buses solve for both angle and
magnitude), and the solve takes damped steps with a backtracking line search from
the setpoint start plus a few perturbations, keeping the lowest-residual result.
Each island's slack holds its stated reference angle (zero at a designated
reference), and every other angle in the island is measured on it. Select `acpf`
in `solve_module_json`.

`"start": "case"` starts Newton from the case's stored bus voltages instead:
each bus's stored magnitude, and its stored angle relative to its island's
reference bus. Star buses of three-winding transformers start from the transformer's
stored star voltage, and a typed AC power flow instance's initial point
overrides the stored values. A bus whose stored magnitude is not positive
starts at its setpoint, and a stored state that does not converge falls back to
the flat start. A case saved from a converged solve needs no Newton steps. The
default is `"start": "flat"`; other formulations refuse `"case"`, since none of
them iterates from a start.

## Conic SOCWR (Jabr)

The Jabr second-order cone relaxation lifts the voltage product to W-space
variables $w_i = |V_i|^2$, $w^r_{ij} = \Re(V_i \overline{V_j})$,
$w^i_{ij} = \Im(V_i \overline{V_j})$, with the rotated cone coupling

$$ (w^r_{ij})^2 + (w^i_{ij})^2 \le w_i w_j. $$

The relaxation is a convex lower bound on AC OPF, solved with Clarabel's
second-order cone support. It uses the same exact quadratic and convex piecewise
linear generator cost representation as DC OPF. Select `socwr` in
`solve_module_json`.

These formulations compile to native Rust and WebAssembly.

## Experimental native multiconductor IVR OPF

The opt-in `mc-opf` feature solves unbalanced AC OPF in current–voltage
rectangular (IVR) coordinates, with explicit terminals and neutrals. It has
separate native entry points over `McAcOpfInstance`; see the
[solver decision](acopf-pounce-decision.md#experimental-native-multiconductor-ivr-opf)
for build commands and API availability.

PowerIO owns canonical electrical semantics, source mappings, units, selected
bounds and solver-independent IVR preparation. Tellegen compiles that prepared
data into expressions, invokes POUNCE, and independently validates the physical
result. BMOPFTools is an external reference for tests, not a runtime dependency.

DELTA equipment and THREE_LEG inverters require phase-role endpoints; neutral,
earth and other return roles are rejected during preparation. Two-terminal DELTA
remains one phase-to-phase coil.

Fixed power-factor control uses the bounded row
`sqrt((1 - abs(pf)) * (1 + abs(pf))) * P + pf * Q = 0`.
The signed domain is `0 < abs(pf) <= 1`; P/Q/S/I capability bounds continue to
intersect this control law. Absolute equation tolerance alone does not establish
relative accuracy of a tiny requested PF.

### Equations and derivatives

Variables include rectangular terminal voltages, branch and winding currents,
continuous taps, and explicit active/reactive power auxiliaries. KCL, fixed
shunts, ideal switches and the coupled branch law

$$ V_{\mathrm{from}} - V_{\mathrm{to}} = Z I $$

are affine. Retaining currents avoids inverting zero or singular impedance
matrices. Complex power uses

$$ P = V_r I_r + V_i I_i, \qquad Q = V_i I_r - V_r I_i. $$

These links, squared voltage/current limits, and continuous tap products are
quadratic. Apparent-power limits use explicit $P,Q$ auxiliaries so that
$P^2 + Q^2 \le S_{\max}^2$ remains quadratic; substituting the voltage–current
products into this circle would make it quartic. Sequence voltages similarly
use affine auxiliaries before imposing norm bounds. Dispatch cost is linear in
the active-power auxiliaries.

POUNCE's exact sparse quadratic evaluator handles these polynomial rows. Each
row Hessian is constant; the weighted Lagrangian Hessian still changes with the
constraint multipliers. General load laws and smooth inverter controls use
POUNCE's existing sparse automatic differentiation. Finite differences are
validation tools, not production derivatives.

ZIP, constant-current and exponential laws use a positive voltage domain via
$\ell = \log(|U|/U_{\mathrm{nom}})$ and
$|U/U_{\mathrm{nom}}|^2 = \exp(2\ell)$, with power terms expressed as exponentials
of $\ell$. This avoids an artificial epsilon floor. Pure impedance laws instead
use the affine relation
$I = \overline{S_{\mathrm{nom}}}U/U_{\mathrm{nom}}^2$, including at zero voltage;
constant-power equivalents use fixed $P,Q$. Smooth volt-var/volt-watt controls
use a nonnegative magnitude auxiliary and stable softplus transitions with width
0.002 times the mean physical knot voltage.

### Supported electrical profile

- Coupled lines with shunts at both ends, standalone shunts, ideal open/closed
  switches, and earth-referenced ideal sources with grounded neutrals.
- WYE, delta and two-wire constant-power, constant-current, constant-impedance,
  ZIP and exponential loads; generators; and connection-aware capacitors.
  Unequal capacitor coil ratings are lowered to the exact terminal-shunt matrix.
- Single-phase, center-tapped, WYE/delta and fixed n-winding transformers;
  Type A/B autotransformer regulators; and ABBC/BCAC/CABA open-delta regulators.
  Preparation preserves polarity, delta roll, leakage coupling, core shunts,
  neutral impedance/return bonds, and distinct coil and terminal ratings.
  Supported two-sided subtypes have continuous taps; n-winding transformers
  have fixed taps and may use mixed connections or two-phase delta windings.
- Single-, three- and four-leg inverters, with active/reactive power, current,
  neutral-return and apparent-power limits; signed power factor; supported
  phase-ground, phase-neutral and phase-phase volt-var/volt-watt controls with
  per-phase or averaged sensing; and isolated DC-link active-power coupling.
- Phase-ground, phase-neutral, phase-phase and neutral-voltage limits;
  positive-, negative- and zero-sequence voltage limits; and bus/line angle
  windows strictly inside $(-\pi/2,\pi/2)$.

Instance constraint selection uses `transformer:<name>` in `conductor_limits`
for transformer limits and `ibr:<name>` in `generator_capability` for inverter
capabilities. Droop, power-factor and DC-link equations remain physical laws
when capability limits are deselected.

Preparation rejects unsupported profiles explicitly: DC networks,
terminal-referenced source voltages, floating source neutrals, initial points,
unknown semantic extras, unreferenced conductor islands, ideal cycles or ideal
paths joining multiple fixed references, bounded load laws, and unsupported
inverter policies. The latter include conflicting power-factor/droop settings,
three-leg droop, non-VA-fraction curves and extra minimum-active-power policies.
Legacy negative neutral resistance must be normalized upstream to an explicit
open connection. A profile rejection is not a proof of electrical infeasibility.

### Results

`McOpfResult` reports SI terminal voltages and physical branch, device and winding
ledgers, keeping terminal and coil axes distinct, including neutral returns.
Each terminal is identified by bus and terminal label. Each coil has a local
zero-based index and explicit positive/negative terminals; an absent negative
terminal means physical ground. Complex arrays are `[real, imaginary]`, with
field suffixes identifying volts, amperes and VA. Device currents and powers
are positive for load withdrawal and source/generator/inverter injection.
Branch currents leave the bus toward the branch. Transformer coil and shunt
currents enter the equipment.

Transformer rating ports retain their prepared winding/port names and selected
limits. These ports may include magnetizing current or represent through-power;
they are measurements, not additional additive terminal injections. Bare winding
currents and separate core/grounding shunt ledgers reconstruct physical KCL.
The optional transformer `tap` is the optimized physical coefficient in the
canonical winding convention; fixed taps remain in the retained instance.

`McOpfResult::to_module_json` and `solve_mc_ac_opf_module_json` emit a canonical
PowerIO `McAcOpfSolution` plus version 1 of the namespaced module extension
`org.eigenergy.tellegen.mc_opf`. The canonical value retains voltages,
source/generator dispatch and the original instance. The extension carries the
complete ledgers, axes, optimized taps, numerical options/bases, residuals,
input diagnostic summaries, fingerprint, iterations and optional profile.
This uses PowerIO's existing extension mechanism without changing its schema.

`McOpfResult::from_module_json` reads this complete report and preserves module
records and unrelated extensions on re-export. It rejects missing/unknown report
versions, inconsistent identities or vector lengths, changed instance/base
fingerprints, and disagreement with canonical voltage columns. Reading a report
does not rerun the solver or certify an external result. A canonical solution
without the extension remains readable through PowerIO, but cannot be imported
as a complete `McOpfResult`.

Accepted solves pass the [physical validation gate](validation.md#multiconductor-ivr-opf).
They establish local numerical success, without a global-optimality certificate,
prices, solution sensitivities or retained solve sessions.
