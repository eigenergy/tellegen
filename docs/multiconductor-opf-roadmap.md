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

For the polynomial subset, equations are affine or quadratic in rectangular
voltage, winding/device current, tap and auxiliary power variables:

- KCL, `V_from - V_to = Z I_series`, fixed shunts and ideal switches are affine.
- `P = Vr Ir + Vi Ii` and `Q = Vi Ir - Vr Ii` are bilinear.
- Squared voltage/current bounds are quadratic. Apparent-power bounds are
  quadratic in explicit P/Q auxiliaries, linked to V/I by bilinear rows.
  Substituting V/I into the apparent-power circle would instead make it quartic.
- Fixed transformer leakage and ampere-turn equations are affine; continuous
  ordinary/regulator tap coefficients multiply voltage or current, so remain
  bilinear. The descriptor retains currents at zero leakage and never inverts Z.
- Constant-impedance loads and fixed-power-factor controls are affine current
  or power laws. Fortescue sequence voltages use affine auxiliaries before
  squaring, avoiding cancellation in the backend's quadratic recognizer.
- Dispatch cost is linear in the P auxiliaries.

Writing a row as `c_i + a_i' x + 1/2 x' Q_i x` gives the exact Jacobian row
`a_i' + x' Q_i`. Its Hessian `Q_i` is constant, but the Lagrangian Hessian
`sigma Q_f + sum_i lambda_i Q_i` changes with objective weight and multipliers.
The implementation emits local expanded monomials that POUNCE recognizes,
then uses POUNCE's existing sparse quadratic evaluator. It does not add a
symbolic algebra package or maintain a second derivative implementation.

The expression boundary also handles genuinely nonpolynomial laws. General
constant-current, ZIP and exponential loads use a log-voltage auxiliary:
`ell = log(|U| / U_nom)`, `|U/U_nom|² = exp(2 ell)`, and each power term is
`coefficient * exp(exponent * ell)`. This is an exact positive-voltage domain,
with no arbitrary epsilon voltage floor. Pure impedance laws instead use
`I = conj(S_nom) U / U_nom²`, including at zero voltage. Constant-equivalent
laws reduce to fixed P/Q. A later optimization could recognize more polynomial
ZIP special cases without changing the public preparation contract.

Volt-var and volt-watt controls use nonnegative magnitude auxiliaries and
stable softplus curves with BMOPFTools' default smoothing width (0.002 times
mean physical knot voltage). Matching conditional branches keep value, first
and second derivatives correct at knots and finite at extreme arguments.
These rows use POUNCE's existing sparse AD. Forced AD and directional finite
differences are independent tests of the polynomial evaluator; finite
differences are never production derivatives. No new AD or symbolic-algebra
package is introduced.

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

As of 2026-10-05, stages 1–4 are implemented for the AC profile below.
Repeated solves/sensitivities and browser integration remain stages 5–6.

- Tellegen branch: `codex/multiconductor-ivr-opf`, based on #145 above.
- Companion PowerIO branch: `codex/multiconductor-ivr-preparation`, commit
  `0dc40abadc1c8a6a356167eb64077575af64d4e3`,
  based on `v0.11.3` to match Tellegen's released lockfile.
- BMOPFTools oracle: clean commit
  `a8b52e069bfd4a7a57434c91bc0470ac03cfdd55` using its local test environment,
  JuMP/Ipopt. Authored fixtures use 1000 VA; imported binding-limit references
  use BMOPFTools defaults (1 MVA), matching its documented unit-test profile.
- POUNCE remains pinned to `925e75fbd036de309929e398159f946d42d0d94b` from #145.

Implemented AC components and controls:

- Coupled lines, both end shunts, standalone shunts, ideal open/closed switches,
  WYE/delta/two-wire loads (constant P, I, Z, ZIP and exponential), generators,
  grounded ideal sources, and connection-aware capacitor banks with unequal
  per-coil ratings.
- Single-phase, center-tapped, WYE–delta, delta–WYE, arbitrary fixed n-winding
  transformers, Type A/B autotransformer regulators and ABBC/BCAC/CABA open-delta
  banks. Coverage includes winding polarity, delta roll, leakage mutual terms,
  core shunts on explicit windings, neutral impedances/return bonds, coil versus
  terminal ratings, and continuous taps for the native two-sided subtypes.
  N-winding mixed connections and two-phase delta incidence have separate
  external witnesses. Nameplate impedance bases are distinct from coil count.
- Single-phase, three-leg and four-leg IBRs; explicit P/Q, current, neutral-return
  and apparent-power limits; signed power factor; PG/PN/PP volt-var and volt-watt
  controls, per-phase or averaged; and isolated DC-link active-power coupling.
- PG, PN, PP and neutral voltage limits, positive/negative/zero sequence bounds,
  and bus/line angle windows strictly inside +/- pi/2. Exact-zero ratings become
  component equalities, with redundant zero-converter rows removed.

PowerIO preparation preserves source identity and SI/per-unit meaning. It also
retains BMOPF capacitor coil ratings, n-winding ratings and open-delta maps across
conversion. Stale capacitor source metadata cannot overwrite an edited canonical
nameplate. Transformer limit selections use `transformer:<name>` under
`conductor_limits`; IBR capability selections use `ibr:<name>` under
`generator_capability`. Droop/PF and DC-link laws remain physical equations when
capability bounds are deselected.

The supported profile still rejects explicit DC networks, floating source
neutrals, initial points, unsupported semantic extras, unreferenced conductor
islands, ideal line/switch cycles and ideal paths between multiple fixed
references. Negative neutral resistance must be normalized to an explicit open
branch upstream. Arbitrary IBR control policies (including conflicting PF/droop,
three-leg droop, non-VA-fraction curves and additional minimum-P policies) are
rejected rather than skipped. N-winding tap optimization is unsupported by the
reference engine and this implementation. These restrictions are explicit
profile boundaries, not proofs of electrical infeasibility.

The native API is `solve_mc_ac_opf_instance[_cancellable]` and
`solve_mc_ac_opf_module_json`, behind `mc-opf`. The module API accepts an explicit
`McAcOpfInstance`. `McOpfResult` includes independent residuals and physical
current/power ledgers; the portable solution retains its original instance and
uses the canonical terminal axes. Native results add transformer winding/tap
and IBR coil ledgers; the current portable solution schema retains terminal
voltages and its existing source/generator dispatch fields. Only accepted POUNCE statuses followed by
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

Ordinary Rust tests need neither Julia nor network access. Each frozen bundle
records the input SHA-256, clean oracle commit, Julia/JuMP/Ipopt versions, units,
solver options and comparison tolerances. Every fixture file is below 100 KiB.
The 12 imported `pmd_bounds` fixtures retain their electrical coefficients and
carry CC BY 4.0 attribution and license text. Other inputs are independently
authored under Tellegen's MIT license; see `ATTRIBUTION.txt`.

### Numerical acceptance and validation

The suite follows BMOPFTools `docs/src/validation.md`: feasibility, derivative
correctness, optimality witnesses and external comparisons are separate checks.
An objective match alone is insufficient.

- **63 component reference cases:** 31 fixed-component/load/control cases,
  12 imported active-bound cases, 9 optimized-tap cases and 11 focused winding,
  neutral/core and binding-IBR cases. The earlier unbalanced feeder oracle is
  retained, giving 64 frozen solves in total.
- Imported bounds recompute the active voltage/sequence/angle/current/VA quantity
  from physical results, compare each generator's total dispatch within 10 W,
  and compare objective within 0.001 currency/hour. Voltage bounds use 0.01 V,
  line currents 0.01 A, transformer ratings 0.01 VA and angle 1e-5 rad. These
  mirror the reference documentation's unit profile. Near-nonunique phase
  allocations (notably S1) are not locked to one optimizer's individual phasors.
- Authored cases compare objective within 1e-7 currency/hour and complex bus
  voltages within 0.0002 V; optimized taps use 0.001 V because some loss objectives
  are weak in the tap direction. Binding IBR tests additionally check physical
  phase/return current and apparent power. Native acceptance recomputes physical
  residuals independently of the expression tree and requires at most 1e-6.
- Reversing two generators' costs moves dispatch while the transformer rating
  remains binding. Different working bases and bus/terminal storage orders
  preserve mapped component physics. PowerIO round trips cover unequal capacitor
  coils, custom open-delta maps and n-winding ratings.
- **18 native model unit tests** include exact/zero/singular impedance,
  independent off-solution equation checks, every tested polynomial row's
  quadratic classification, forced AD parity and central Jv/Hv checks at multiple
  points/steps with changing nonzero multipliers. New derivative cases include
  21 component/control/bound models. Softplus tests lock value/gradient/Hessian
  at the breakpoint and at +/-1000. Zero-voltage impedance and zero-rated IBR
  regressions, cancellation, and corrupted current/tap/power/nonfinite results
  exercise acceptance failures.
- **PowerIO: 670 tests passed** across `powerio-dist` and `powerio-matrix`, including
  22 preparation tests and 4 new converter regressions; strict all-target Clippy
  passed for both crates.

The native feature suites, committed-snapshot reproduction and stock feature
boundary checks are recorded below after the final local build. No global
optimality, OpenDSS end-to-end parity, large-feeder performance or solution
sensitivity claim follows from these small witnesses.

### Publication gates

Before publishing, replay/rebase the companion onto the agreed PowerIO target,
run its required `scripts/ci-clippy.sh` matrix, obtain approval for the companion
PR, and consume the released preparation API in Tellegen's dependency lockfile.
Then add the optional MC suite to the backend CI gate and obtain approval for
the Tellegen PR targeting #145's successor. Browser forwarding, POUNCE release,
restoration wiring and distribution approval are separate remaining gates.
Neither branch has been pushed and no PR has been opened.
