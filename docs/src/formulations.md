# Formulations

Tellegen solves DC power flow, DC OPF, AC power flow, and the Jabr SOCWR
relaxation through the PowerIO module boundary. The shared response has
optional fields for the quantities each formulation produces. Economic fields
are present only when the declared objective gives them that interpretation.
Supported implicit derivatives use the contract described in
[the sensitivity contract](sensitivity-contract.md).

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
service, or synthesized while lowering a three-winding transformer is refused
by name. A bus injection enters the program as the flow leaving the bus on its
branches, which equals the net injection by power balance. The constraint rows
therefore never depend on demand, and the reported LMP stays the marginal cost
of demand at each bus.

The response adds a `constraints` block in request order with each row's
`value`, `lower`, `upper`, `binding`, and `shadow_price`. The shadow price is
$\mu = -\partial(\text{objective})/\partial(\text{limit})$ in the LMP's units:
positive when the upper limit binds, negative when the lower limit binds, and
zero when neither does. Prices then decompose against any reference bus $r$:

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

Branch angle-difference bounds are enforced in radians after normalization.
MATPOWER's unconstrained `-360`/`360` spelling and an unset `0`/`0` pair become
exactly -60/+60 degrees. When a branch has no thermal rating, Tellegen
synthesizes its fallback rating from that same 60 degree window and the terminal
voltage bands. Explicit tighter source bounds are preserved.

## AC power flow (polar)

The nodal power balance in polar coordinates,

$$ S_i = V_i \sum_j \overline{Y_{ij}} \overline{V_j}, $$

is solved by Newton–Raphson on the reduced system
$\partial(P, Q)/\partial(\theta, V_m)$. Buses are typed slack / PV / PQ (PV and
slack buses hold the generator voltage setpoint; PQ buses solve for both angle and
magnitude), and the solve takes damped steps with a backtracking line search from
the setpoint start plus a few perturbations, keeping the lowest-residual result.
Select `acpf` in `solve_module_json`.

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
