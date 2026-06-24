# Formulations

tellegen solves the DC power flow and DC OPF, the AC power flow, and the Jabr SOCWR
relaxation through one interface, each driven from the engine's public API. Every
formulation returns the same result shape — locational
marginal prices, voltages, branch flows, and generator dispatch — and exposes
analytical sensitivities through the contract described in
[the sensitivity contract](sensitivity-contract.md).

## DC power flow and DC OPF (B–θ)

The linearized power flow couples bus angles $\theta$ to injections through the
susceptance-weighted graph Laplacian

$$ B = A\,\operatorname{diag}(b)\,A^\top, \qquad B\,\theta = p, $$

where $A$ is the branch–bus incidence and $b$ the branch susceptances. The OPF
minimizes generation cost subject to the network balance and the thermal and
generation limits; it is a convex quadratic program solved with Clarabel. Entry
point: `solve_network` (or `solve_prebuilt` over a prebuilt `DcNetwork`).

## AC power flow (polar)

The nodal power balance in polar coordinates,

$$ S_i = V_i \sum_j \overline{Y_{ij}}\, \overline{V_j}, $$

is solved by Newton–Raphson on the reduced system
$\partial(P, Q)/\partial(\theta, V_m)$. Buses are typed slack / PV / PQ — PV and
slack buses hold the generator voltage setpoint, PQ buses solve for both angle and
magnitude — and the solve takes damped steps with a backtracking line search from
the setpoint start plus a few perturbations, keeping the lowest-residual result.
Entry point: `ac_pf`.

## Conic SOCWR (Jabr)

The Jabr second-order-cone relaxation lifts the voltage product to W-space
variables $w_i = |V_i|^2$, $w^r_{ij} = \Re(V_i \overline{V_j})$,
$w^i_{ij} = \Im(V_i \overline{V_j})$, with the rotated-cone coupling

$$ (w^r_{ij})^2 + (w^i_{ij})^2 \le w_i\, w_j. $$

The relaxation is a convex lower bound on AC OPF, solved with Clarabel's
second-order-cone support. Entry point: `socwr_opf`.

Every formulation is pure Rust and compiles to WebAssembly, so the same code runs on a
server and in the browser. The full nonlinear AC OPF (an interior-point program) is on the
roadmap; it runs natively, where it can use threads.
