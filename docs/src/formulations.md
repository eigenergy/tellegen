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
