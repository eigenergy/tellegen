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

DELTA equipment and THREE_LEG inverters require phase-only terminal maps.
PowerIO preparation rejects neutral and earth endpoints, honoring explicit
terminal conventions. Tellegen reports `InvalidInput` at `Preparation` with
`BUILD.MULTI.OPF_INVALID`, before optimization, for both one-shot solves and
retained-session construction. Two-terminal DELTA remains one phase-to-phase
coil; WYE and SINGLE_PHASE return connections remain supported. Session edits
change numerical values, not terminal roles or connection topology.

Fixed power-factor control uses bounded coefficients:
`sqrt((1 - abs(pf)) * (1 + abs(pf))) * P + pf * Q = 0`, with signed
`0 < abs(pf) <= 1`. Capability bounds still intersect this physical law.
Acceptance also recomputes the relation from original input profiles and
physical P/Q ledgers, independently of prepared coefficients. Passing an
absolute power-equation tolerance is not a relative PF accuracy certificate.
`MC_OPF.POWER_FACTOR_ACCURACY_UNQUALIFIED` reports requested and achieved PF
when the requested magnitude is at or below the acceptance tolerance, observed
relative accuracy exceeds that tolerance, the sign relation is wrong, or
zero dispatch makes operating PF undefined. This diagnostic does not declare
an otherwise accepted physical point infeasible.

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
terminal-referenced source voltages, floating source neutrals,
unknown semantic extras, unreferenced conductor islands, ideal cycles or ideal
paths joining multiple fixed references, bounded load laws, and unsupported
inverter policies. The latter include conflicting power-factor/droop settings,
three-leg droop, non-VA-fraction curves and extra minimum-active-power policies.
Legacy negative neutral resistance must be normalized upstream to an explicit
open connection. A profile rejection is not a proof of electrical infeasibility.

### Explicit initial points

Attach a canonical `OperatingPoint<MulticonductorNetwork>` using
`McAcOpfInstance::with_initial_point`. Build the point with PowerIO's
`MulticonductorOperatingPointBuilder::for_point(network.clone())`. Supported
quantity families are terminal voltage magnitude (V), terminal angle (rad),
load/generator terminal active and reactive power (W/var), and physical
transformer tap coefficients. Families may be omitted independently; a supplied
family must cover all of its canonical identities. Builder vectors use the
point network's table/terminal order. Projection matches stable identities, so
the target instance may use a different order or numerical bases.

For a converged canonical `McAcPfSolution`,
`mc_opf_initial_point_from_pf(&solution)` creates a voltage-only point. When
starting from Tellegen's `McPfResult`, first call
`result.to_powerio_solution(&pf_instance)`. This is an explicit caller choice;
the OPF entry point does not run PF automatically.

Only the numerical starting vector changes. Network data, objective, selected
constraints, bounds, and the physical fingerprint remain unchanged. Tellegen
prepares an equivalent physical instance without its seed, then projects the
original point into the compiled IVR coordinates. The returned solution and
canonical JSON retain the original physical instance and seed values. Returned
seed columns are normalized to the instance's order for portable JSON. When serializing an input instance
directly through PowerIO, construct its point in that instance's order too;
PowerIO's stored assignment decoder requires matching column order.

Fixed source/ground voltages and fixed transformer taps take precedence, with
`MC_OPF.INITIAL_POINT_PROJECTED` remarks. Active taps use the canonical physical
coefficient, including inverse Type B regulator scaling. Missing voltages use
the ordinary compiled defaults. Branch/winding currents are inferred from
seeded voltages where consistent; otherwise their unspecified guesses remain
zero, including for inconsistent ideal-branch voltage guesses. Device currents
default to the load law or prescribed power at the seeded voltage. Explicit
load/generator powers are terminal powers, including neutral returns, not delta
coil powers or changes to demand/dispatch limits. They determine current guesses
through the device's connection incidence. Power, sequence-voltage, droop
magnitude and logarithmic-voltage auxiliaries are then recomputed; compiler
zero reductions still apply.

Nonfinite values, negative magnitudes, nonpositive taps, mismatched identities,
terminal power guesses incompatible with the seeded voltages/connection, and
zero voltages in logarithmic load coordinates return
`MC_OPF.INITIAL_POINT_INVALID` with a target. Capacitor steps and switch states
return `MC_OPF.INITIAL_POINT_UNSUPPORTED`: equipment state belongs on the
network. A seed need not satisfy operating limits or KCL and does not guarantee
convergence; the usual physical acceptance gate still applies. Omitting the
initial point preserves the existing cold start.

### Retained sessions and edits

`McOpfSession::new(instance, options)` prepares and compiles a native MC OPF
instance without solving it. `solve()` uses the last accepted primal when
available. `solve_cancellable(McOpfStart::Declared, cancel)` instead selects the
instance's explicit initial point, or its ordinary cold start if absent.

```rust
use tellegen::{McOpfEdit, McOpfSession};

// `instance` is the caller's declared McAcOpfInstance.
let mut session = McOpfSession::new(instance, Default::default())?;
session.solve()?;
session.apply_edits(&[McOpfEdit::LoadPower {
    load: "load_1".into(),
    p_w: vec![120.0],
    q_var: vec![25.0],
}])?;
let accepted = session.solve()?;
let json = accepted.result.to_module_json()?;
```

Edits use canonical identities and equipment ordering. Supported fields are
load nominal coil P/Q (preserving its voltage law), inverter availability,
generator/source dispatch costs, generator P/Q bounds, line current/apparent
power overrides, scalar bus phase-ground voltage bounds, and constraint-family
selection. Load edits specify coil powers; initial-point power columns specify
terminal powers. Cost units are the canonical currency/kWh, with either a scalar
or one value per coil. Removing a line override restores linecode inheritance;
removing a scalar bus voltage bound restores any canonical vector fallback.

`InverterAvailability` takes an explicit per-coil `p_max_w` vector. It replaces
canonical `p_max` and sets `p_avail` to the vector's sum for controls; `None`
clears both fields. Other capability fields stay unchanged, and selection still
determines which bounds are enforced. Released PowerIO preparation uses
`p_avail` as a control reference, so changing that scalar alone would not impose
new dispatch bounds. The session API does not infer a phase allocation or a
separate shared DC-link constraint.

`InverterControlProfile` patches a named shared profile's power factor,
volt-var breakpoints/q limits, or volt-watt breakpoints/p limits. `None` leaves
that field unchanged. Breakpoints are positive, strictly ordered volts; output
limits are finite dimensionless fractions in canonical array order. The edit
affects every inverter using the profile. Existing control families, voltage and
power references, aggregation and assignments stay fixed. Power factor must
satisfy `0 < |pf| <= 1`. Invalid edits reject the entire batch.

A nonempty edit batch is applied to a candidate instance, prepared and compiled
before committing one new revision. Rejected batches leave the session intact.
The compiled model is reused for repeated solves and solver/acceptance option
changes. Physical edits always recompile because coefficients are embedded in
expressions and zero ratings can change the reduced equations. Numerical base
changes also recompile. Topology, connection maps and other unsupported edits
require a new session. `compilation_count()` makes this cache policy observable.

Compatible warm starts carry primal coordinates by compiler identity, reset
proven zero columns and recompute dependent auxiliaries. After a base change
or ambiguous compiler names, the previous physical voltage/tap report is
projected in SI units instead. An unusable warm seed falls back to the declared
start with `MC_OPF.SESSION_WARM_START_DISCARDED`. `McOpfSessionResult::start`
records the path used. Each attempt creates a fresh optimizer and evaluator;
the session does not reuse multipliers or matrix factorizations as optimizer state.
Optional dual reports remain attached to their accepted result revision. Warm starts
need not be feasible and do not guarantee convergence or fewer iterations.

Results carry the input `revision`, solve `attempt`, and `compilation` counters.
`current_result()` returns `None` after an edit, changed options or a failed or
cancelled attempt. `last_accepted()` explicitly exposes the historical result
with its original revision and instance. Failed attempts never replace that
result or its warm-start primal. Empty edit batches, identical options and
rejected changes leave the current result intact. Accepted results use the same
independent physical validation and portable result format as one-shot solves;
session counters and cache state are native, in-memory metadata.

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
PowerIO `McAcOpfSolution` plus version 9 of the namespaced module extension
`org.eigenergy.tellegen.mc_opf`. The canonical value retains voltages,
source/generator dispatch and the original instance. The extension carries the
complete ledgers, axes, optimized taps, numerical options/bases, residuals,
input diagnostic summaries, fingerprint, iterations, optional profile and optional
dual report. Reports from versions 1–7 with assigned fixed-PF controls are
rejected and must be solved again: their PF rows and multipliers use the old
scaling. Version 1 reports remain readable when they contain neither PF controls
nor dual data. Version 2–6 dual reports without PF controls are checked using
their original parameter mappings and dense rank
audit before current sparse rank qualification and newly
supported load/inverter/transformer/control-profile derivatives are recomputed from the retained canonical
instance. Versions 7–8 use their original largest-residual sparse pivot order
and work accounting for verification before sparsity-aware rank qualification
is recomputed. Re-export writes version 9; older unsupported entries are not silently
accepted as current claims. Upgrading does not rerun optimization.
This uses PowerIO's existing extension mechanism without changing its schema.

`McOpfResult::from_module_json` reads this complete report and preserves module
records and unrelated extensions on re-export. It rejects missing/unknown report
versions, inconsistent identities or vector lengths, changed instance/base
fingerprints, and disagreement with canonical voltage columns. Reading a report
with dual data recompiles its declared problem, recomputes the derivative/KKT
diagnostics, and checks the stored primal against the physical ledgers and
canonical voltages. It rejects inconsistent multipliers, scaling metadata and
derived marginal claims.
Reading does not rerun the solver or certify an external result. A canonical solution
without the extension remains readable through PowerIO, but cannot be imported
as a complete `McOpfResult`.

Accepted solves pass the [physical validation gate](validation.md#multiconductor-ivr-opf).
They establish local numerical success, without a global-optimality certificate.
[Local forward and adjoint sensitivities](sensitivity-interface.md#native-multiconductor-opf)
are available for a verified regular dual point and supported physical parameters.

### Dual diagnostics and local marginal values

Set `McOpfOptions.collect_duals = true` to populate `McOpfResult.duals`.
Collection is off by default. The report includes original-objective-scale
constraint and variable-bound multipliers, stationarity, complementarity,
dual-feasibility and primal residuals, and an active-constraint rank audit.
Physical acceptance remains independent: a failed diagnostic or unavailable
marginal value does not invalidate an otherwise accepted physical solution.
Missing multipliers or failed derivative evaluation produce
`MC_OPF.DUALS_UNAVAILABLE`; finite but unreliable duals remain inspectable with
an explicit report status.

Each row has a unique compiler ordinal, a descriptive label and any squared-norm
normalization radius in working units. Ordinals identify rows within that compiled
problem, not across revisions. Labels need not be unique and are never parsed
to recover parameter identities. Omitted fixed equations and zero-reduced rows
retain provenance; synthetic zero definitions are identified explicitly. Raw
row multipliers use `L = f + lambda*g`: lower-side multipliers are negative,
upper-side multipliers positive, and equality multipliers unrestricted.
They use compiled row units. **IVR KCL multipliers are not nodal energy prices.**

Typed marginal entries instead report the envelope derivative of operating cost
(currency/hour) with respect to an identified physical parameter in W, var, A,
VA, V or dimensionless ratio. A positive value means increasing that parameter raises local cost.
For example, a lossless load served at 0.2 currency/kWh has an active-power
marginal of 0.0002 (currency/hour)/W. Supported parameters are nominal
load coil P/Q, selected generator coil P/Q bounds, inverter availability and
capabilities, effective branch conductor and transformer port current/apparent-power
ratings, physical continuous tap bounds, prepared voltage-magnitude limits,
and inverter control-profile settings.
Branch ratings sum contributions from both ends. Squared-norm rows include the
derivative of their normalization radius, so their raw multiplier is not itself
the physical limit marginal. Values are invariant to numerical bases and solver
objective scaling within numerical tolerances.

Parameter identities describe effective prepared limits. A shared linecode edit
can affect several branch parameters; a scalar bus bound may affect several
voltage measurements. These entries are not derivatives of arbitrary canonical
input fields. Load mappings cover constant power, constant current, constant
impedance, ZIP and exponential laws. They hold nominal voltage, exponents and
ZIP coefficients fixed. Canonical scalar/per-coil coefficients and their defaults
are retained even when nominal P/Q is zero and preparation removes its terms.
The derivative is with respect to nominal W/var, not the load's realized power at
the solved voltage. Fixed-tap setpoints and transformer
impedances do not have parameter mappings. Coincident generator/inverter lower and upper bounds do not
yield separate one-sided bound marginals.

Inverter entries use owner `Inverter` and the canonical inverter name. The
`Availability` direction (W) follows `McOpfEdit::InverterAvailability`: increment
one explicitly allocated `p_max` coil and recompute `p_avail` as the sum. It
includes the selected upper P bound, a PMax-referenced volt-watt target on that
coil, and every PAvailable-referenced target through the summed availability.
The vector must be finite and nonnegative, the selected allocation positive,
and an observed `p_avail` must already agree with its sum. Otherwise availability
is `UnsupportedParameter`; the solver does not invent an allocation or reset a
control reference as part of differentiation.

Inverter `ActivePowerLower/Upper` and `ReactivePowerLower/Upper` vary canonical
per-coil bounds in W/var while holding other fields, including `p_avail`, fixed.
`ApparentPowerRating` varies per-coil `s_max` in VA. These mappings include
PMax/SMax-referenced volt-watt and SMax-referenced volt-var effects, even when
capability constraints are deselected. Curve knots, fractions, voltage references,
aggregation and power-factor coefficients stay fixed. A reference change can
therefore have nonzero value even if its own capability bound is inactive.

`CurrentRating` varies `i_max` in A: its axis follows canonical `terminal_map`
for single-phase/four-leg equipment, including the neutral, and coil order for
three-leg delta equipment. For a two-wire inverter preparation takes the minimum
of phase and return ratings; the looser finite rating has zero local derivative,
and an exact tie refuses independent current-rating partials. Ratings must exist
and be finite. Zero-rating reductions retain the existing conservative refusal;
a zero nameplate used only as a control reference is also unsupported. All these
entries remain conditional on the same regular local branch and active set.

Shared profile entries use owner `ControlProfile` and the canonical profile
name. `VoltVarBreakpoint` and `VoltWattBreakpoint` use volts and an axis indexing
the canonical breakpoint array. `VoltVarFraction` and `VoltWattFraction` use
dimensionless ratio units and index `q_limits` and `p_limits`, respectively:
index 0 is the high-voltage endpoint, index 1 the low-voltage endpoint.
`PowerFactor` has no axis and varies the signed canonical `pf` in ratio units.
Each derivative sums all affected coils of every inverter assigned that profile,
holding nameplates, availability, references and aggregation fixed. Breakpoint
derivatives include the preparation rule for smoothing width as well as interval
slopes. Averaged voltage monitors retain their full chain rule. Unity power
factor is a valid setting but has no finite two-sided derivative; its marginal
and sensitivity are unsupported, as are unused profiles. Switching control
families, references or profile assignments is outside this local interface.

Transformer entries use owner `Transformer` and the prepared transformer identity.
For `CurrentRating` (A) and `ApparentPowerRating` (VA), `axis` indexes that
transformer's result `ports` array, whose name and endpoints identify the physical
measurement. These are effective selected port limits, with all electrical
coefficients held fixed. A canonical nameplate or winding rating can feed several
ports and can also set impedance bases; its full derivative is not generally one
port derivative. Combine port directions only when the canonical edit leaves
electrical coefficients fixed. Current ports can include exciting current, while
through-power ports may use bare winding currents. Zero-rating reductions retain
the existing conservative refusal; absent or deselected ratings are unsupported.

`TapLower` and `TapUpper` have no axis and vary the physical continuous tap bounds
in ratio units, holding the nominal tap and transformer coefficients fixed. The
mapper applies the chain rule for both `tap_nominal * alpha` and the type-B
regulator convention `tap_nominal / alpha`. The inverse convention swaps the
compiled bound sides and has a negative bound derivative. An inactive tap bound
has zero derivative on the same active set. Fixed/discrete tap choices and edits
that change the transformer equations are outside this interface.

The load mapper follows the chosen compiled current or power law without
changing the physical equations. It includes both real and imaginary current
rows for an impedance-form load and uses the logarithmic-voltage coordinate
when present. At positive voltage this also handles a zero coefficient that
hides a nonconstant law. At zero voltage, pure impedance current-law parameter
derivatives remain defined; hidden non-impedance laws are reported unsupported
because their neighboring compiler profile requires positive voltage. Nonfinite
derivative evaluations are likewise withheld without invalidating an accepted
physical solution. Voltage-law coefficient and nominal-voltage derivatives are
not exposed.

`Conditional` means a first-order envelope value assuming the same regular local
stationary branch and active set. It is not a second-order sufficiency test,
global price, or proof that the optimal-value function is differentiable.
Values are withheld after failed KKT checks, any zero-limit reduction,
numerically dependent active rows, weakly active inequalities, or an unaudited
rank. The rank audit uses normalized, pivoted, twice-reorthogonalized sparse row
QR with a `1e-9` numerical rank threshold. Column incidence restricts updates to
overlapping rows; no dense active-Jacobian copy, normal equations or numerical
fill dropping is used. The audit permits up to 2,000,000 live coefficients and
100,000,000 sparse coefficient visits. These bounds limit fill and work rather
than the number of active rows. `active_rank.qualification` reports completion,
fill/work exhaustion or nonfinite arithmetic, along with input/peak nonzeros,
work, limits, accepted pivots and the smallest accepted pivot. Peak nonzeros are
coefficient counts, not allocated bytes. An incomplete audit reports
`RankNotChecked` and no rank; a completed deficient audit reports `RankDeficient`.
Both preserve physical results, raw multipliers and KKT residuals. This is a
numerical independence test, not a condition-number estimate or uniqueness proof.
Active-side tolerance is `1e-7*(1+abs(bound))`
in compiled coordinates. KKT acceptance uses `1e-6` for relative stationarity,
relative complementarity, normalized primal violation and absolute dual-sign
violation. Relative stationarity divides each coordinate residual by one plus
the sum of absolute gradient, Jacobian-multiplier and bound-dual contributions;
relative complementarity divides by `1+abs(objective)`. Active inequality rows
are weak when `abs(lambda)*norm(row_gradient) <= 1e-8` (variable bounds use
`z <= 1e-8`). This conservative policy exposes diagnostic limits explicitly.
