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
  the flows violate together with those loaded at or above `near_binding`
  (default 0.98), and solves again until none is violated, for at most
  `max_rounds` (default 20) solves. A solution that violates none of the limits
  it was not held to is optimal for the program with all of them, so prices
  and dispatch match the full solve.

With `limits`, the response's `limit_rounds` reports each solve: the limits it
enforced, the violations it found, the limits it added, its program size, and
its interior point iterations. Keep the angle-difference rows with `lazy` when
every generator cost is linear. Without them, the first round bounds no angle
or flow, which can stall the interior point method on a degenerate linear
program. The `dcopf-scale-bench` binary in `crates/benchmarks` measures each
option on PGLib's `case78484` and on a synthetic 100k-bus mesh.

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
