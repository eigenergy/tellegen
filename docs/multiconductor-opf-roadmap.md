# Multiconductor IVR OPF implementation roadmap

The native implementation is stacked on Tellegen PR #145, commit
`231d20e373315a1a431304488c893e8f45d0d86a`. No branch is to be published and no
pull request opened until the maintainer approves. The existing POUNCE
release/restoration/distribution gates remain in force.

The reviewed stack is [#143](https://github.com/eigenergy/tellegen/pull/143)
(optional backend), [#144](https://github.com/eigenergy/tellegen/pull/144)
(canonical balanced model), [#145](https://github.com/eigenergy/tellegen/pull/145)
(solve and emission), and [#146](https://github.com/eigenergy/tellegen/pull/146)
(browser worker). The native IVR work is a sibling of #146: it needs #145's
backend and portable solve boundary, not the browser transport changes.

## Ownership

PowerIO owns canonical network semantics, instances, source/terminal mappings,
units and numerical preparation. Tellegen owns the IVR compiler, POUNCE
execution, independent physical residual checks and solution emission.
BMOPFTools is the equation and external validation reference, not a runtime
dependency. POUNCE supplies its existing quadratic recognition and sparse AD;
no additional differentiation package is required.

## Differentiation decision

For the implemented fixed-equipment profile, every equation is affine or
quadratic in rectangular voltage, current and auxiliary power variables:

- KCL, `V_from - V_to = Z I_series`, fixed shunts and ideal switches are affine.
- `P = Vr Ir + Vi Ii` and `Q = Vi Ir - Vr Ii` are bilinear.
- Squared voltage/current bounds are quadratic. Apparent-power bounds are
  quadratic in explicit P/Q auxiliaries, linked to V/I by bilinear rows.
  Substituting V/I into the apparent-power circle would instead make it quartic.
- Dispatch cost is linear in the P auxiliaries.

Writing a row as `c_i + a_i' x + 1/2 x' Q_i x` gives the exact Jacobian row
`a_i' + x' Q_i`. Its Hessian `Q_i` is constant, but the Lagrangian Hessian
`sigma Q_f + sum_i lambda_i Q_i` changes with objective weight and multipliers.
The implementation emits local expanded monomials that POUNCE recognizes,
then uses POUNCE's existing sparse quadratic evaluator. It does not add a
symbolic algebra package or maintain a second derivative implementation.

Keep the expression boundary: nonpolynomial load/control laws can use POUNCE's
existing sparse AD when implemented and domain-tested. AD is also an independent
comparison path for the polynomial evaluator. Numerical differences are test
oracles only. There is no need to select one global AD backend before writing
the fixed-equipment model; do not generalize this polynomial result to all of
BMOPFTools' devices and controls.

Derivatives of the *solution* require a separate contract. Later work must map
physical parameters to rows/columns, transform normalized multipliers back to
physical units, check local KKT regularity and active-set stability, and include
direct output dependence on parameters. Do not differentiate optimizer iterations
or reuse balanced bus/generator IDs for terminal and coil parameters. This first
implementation exposes neither prices nor solution sensitivities.

## Delivery sequence

1. **Preparation and profile.** Add solver-independent IVR preparation in
   powerio-matrix, with explicit terminal/coil maps, units, selected limits,
   source/generator costs and unsupported-physics diagnostics.
2. **Native vertical slice.** Compile constant-power WYE/delta devices,
   explicit neutral conductors, coupled impedance lines with both shunts,
   fixed shunts, ideal sources and switches. Retain current variables at zero
   impedance. Solve and independently validate before emitting a typed result.
3. **Equipment coverage.** Extend preparation and reference witnesses for
   fixed transformer subtypes, IBRs and additional voltage/angle families.
   A family is unsupported until its complete semantics and results are tested.
4. **General nonlinear laws.** Add domain-qualified ZIP/exponential loads and
   smooth controls using the same expression boundary, with AD fallback.
5. **Repeated solves and sensitivities.** Define parameter identity, cache
   invalidation and structural rebuilds; validate dual signs/scales and local
   KKT regularity before exposing any price or derivative.
6. **Browser integration.** Extend PR #146's optional worker/reactor path after
   native correctness, memory, cancellation and artifact-size measurements.

Each implementation commit records supported scope and evidence below. A
roadmap item is not a claim that the corresponding feature already works.

## Test architecture

Tests are layered so an optimizer cannot hide incorrect equations. Default
unit tests use small synthetic fixtures with no network downloads or Julia.

| Layer | Oracle and assertions |
| --- | --- |
| Preparation | Explicit expected SI/per-unit values, coil incidence, source identities, limit selection and vector cardinality; malformed and unsupported cases fail before solving |
| Device equations | Independently calculated complex voltage/current/power residuals at non-solution points, including mutual coupling and unequal end shunts |
| Derivatives | Structural sparsity and quadratic classification; analytical blocks; quadratic fast path versus forced AD; central directional differences over several step sizes, points and nonzero multipliers |
| Metamorphic | Reordered elements/terminals with matching matrix permutations, changed physical bases, renamed identities, and redundant unselected limits preserve mapped physics |
| Solver acceptance | Small analytic dispatch cases; independently recomputed residuals and objectives; inject corrupted primal/objective values and reject NaN/Inf |
| Portable results | Correct terminal versus coil axes, units, inactive/open rows, source cost sign, instance retention and module serialization round trips |
| External oracle | Pinned BMOPFTools staged models: mapped residuals/objectives at prescribed points, then solutions; explicit fixture provenance and tolerances |
| Feature boundary | Default and no-default builds exclude POUNCE; optional native feature builds, runs and passes Clippy without changing shipping adapters |

Every new device needs positive, negative, zero/absent-limit, mapping, and
derivative witnesses. Important boundaries include floating neutrals, two-wire
delta coils, zero impedance, ideal cycles, conflicting references, zero-radius
limits, both-end thermal limits, disabled constraint selections and malformed
extra physics. Never infer a missing current bound from an unenforced voltage
or power bound. Reject unsupported semantic extras instead of ignoring them.

Finite differences are checks, never production derivatives. Check the
weighted Lagrangian Hessian with changing multipliers and objective weights;
constant quadratic row Hessians do not make the Lagrangian Hessian constant.
Quadratic recognition is asserted on generated expressions because algebraic
degree alone does not guarantee POUNCE admits their expression shape.

First-order model derivatives do not establish solution differentiability.
Sensitivity tests must separately cover parameter units, active-set changes,
rank deficiency, weak complementarity and direct parameter dependence of outputs.

## Development evidence

As of 2026-10-05, stages 1 and 2 are implemented locally. Stages 3–6 are future
work, not supported capabilities.

- Tellegen branch: `codex/multiconductor-ivr-opf`, based on #145 above.
- Companion PowerIO branch: `codex/multiconductor-ivr-preparation`, commit
  `e681e031` (preparation commit `ede42994` plus canonical metadata/role handling),
  based on `v0.11.3` to match Tellegen's released lockfile.
- BMOPFTools oracle: clean commit
  `a8b52e069bfd4a7a57434c91bc0470ac03cfdd55` using its local test environment,
  JuMP/Ipopt and a 1000 VA working power base.
- POUNCE remains pinned to `925e75fbd036de309929e398159f946d42d0d94b` from #145.

Implemented: coupled lines with both end shunts, independent fixed shunts,
open/closed ideal switches, constant-power WYE/single-phase/delta loads,
generator P/Q and current/apparent-power limits, grounded ideal voltage
sources and linear generation/import cost, explicit floating load neutrals,
terminal/coil result projection, cancellation and canonical module emission.
Grounded voltage variables and unrestricted ground slack currents are
eliminated; dropping grounded-terminal KCL is the corresponding exact reduction.
No line impedance inverse or near-zero impedance regularization is used.

Preparation rejects unsupported equipment/semantic extras, nonlinear loads,
sequence/angle limits, floating source neutrals, initial points, conductor
components without fixed references, ideal cycles and ideal paths between
multiple fixed references. This is a conservative supported profile: rejection
does not prove the underlying electrical problem invalid or infeasible. Source
reference arrays must cover the whole declared terminal map. Generator current
ratings use the BMOPF phase-coil order with a trailing neutral rating for WYE;
single-phase outgoing/return ratings collapse to the tighter cap. Missing or
positive-infinite upper caps are absent; zero caps become component equalities.

The native API is `solve_mc_ac_opf_instance[_cancellable]` and
`solve_mc_ac_opf_module_json`, behind `mc-opf`. The module API accepts an explicit
`McAcOpfInstance`. `McOpfResult` includes independent residuals and physical
current/power ledgers; the portable solution retains its original instance and
uses the canonical terminal axes. Only accepted POUNCE statuses followed by
successful physical validation produce a result. Success is local, not a global
optimality certificate. Model preparation/compilation is repeated for each solve;
this is not yet a retained session or large-feeder performance claim.

### Reproduce locally

The new preparation API has not been released by PowerIO. Do not commit absolute
path patches or an altered lockfile. After committing local Tellegen changes:

```sh
bash scripts/check-mc-opf.sh /path/to/powerio-with-mc-ivr-preparation
```

The script archives committed Tellegen into a temporary directory, patches all
six PowerIO crates consistently, and runs native tests, the independent optional
feature suite, and Clippy. It leaves both checkout lockfiles unchanged. Set
`CARGO_NET_OFFLINE=true` when the dependency cache is populated and optionally
`CARGO_TARGET_DIR` to reuse a build cache.

PowerIO checks, from its companion checkout:

```sh
cargo test -p powerio-matrix --offline
cargo clippy -p powerio-matrix --all-targets --offline -- -D warnings
```

Regenerate the pinned external reference only when intentionally reviewing an
oracle change:

```sh
julia --project=/path/to/BMOPFTools.jl/test scripts/mc_opf_oracle.jl
```

The synthetic fixture and derived results in `crates/tellegen/tests/data/mc_opf`
are authored for this repository and use its MIT license. They total under
10 KiB. The reference records the input SHA-256 and oracle commit; ordinary Rust
tests check the hash and require neither Julia nor a network connection.

### Validation recorded

- PowerIO matrix suite: **242 passed**, including 14 new preparation tests;
  strict all-target Clippy passed.
- Tellegen with `mc-opf` and default features: **315 passed**, 3 existing ignored;
  strict all-target Clippy passed.
- Tellegen with only `mc-opf`: **104 passed**, 2 existing ignored. The complete
  committed-snapshot reproduction script passed against the durable companion
  checkout, including both feature suites and strict Clippy.
- Unpatched, locked released dependencies: **276 passed** with default features
  (2 existing ignored), **66 passed** with no default features (1 existing
  ignored). The default normal dependency graph excludes POUNCE. Both development
  checkout lockfiles remain unchanged.
- The IVR suite has 13 unit tests and 2 external/metamorphic tests. It covers
  exact zero and singular nonzero impedance, complex equation witnesses away
  from a solution, all-row/objective quadratic recognition, forced AD parity,
  directional Jacobian and weighted-Hessian differences at four points and
  three step sizes, corrupted/nonfinite result rejection, optional limits,
  units, cancellation, terminal projection and portable round trips.
- The external three-phase unbalanced WYE/delta dispatch witness agrees with
  BMOPFTools at `1e-7` currency/hour objective, `1e-5` V complex voltage and
  `1e-5` A both-end current tolerances. The reference objective is
  `0.053888485005030434` currency/hour. Conductor/matrix permutations and scalar
  cost broadcasting preserve the physical solution.
- Changing voltage/power bases exposed an initialization defect, fixed by
  propagating physical source phasors along conductor components. The regression
  now checks a different voltage and power base, not just reordered variables.

### Publication gates

Before publishing, replay/rebase the companion onto the agreed PowerIO target,
run its required `scripts/ci-clippy.sh` matrix, obtain approval for the companion
PR, and consume the released preparation API in Tellegen's dependency lockfile.
Then add the optional MC suite to the backend CI gate and obtain approval for
the Tellegen PR targeting #145's successor. Browser forwarding, POUNCE release,
restoration wiring and distribution approval are separate remaining gates.
Neither branch has been pushed and no PR has been opened.
