# Multiconductor IVR OPF implementation roadmap

The native implementation now includes [Tellegen PR #159](https://github.com/eigenergy/tellegen/pull/159)
at `8e087c6d3a7f14ac1c31cf07f24443f80387ff02`, including upstream main
`fc041430d66abe6ce9b0c64298818204ad930946`, through merge commit `629b947`.
The original #145 ancestry and all IVR implementation/evidence commits are
preserved. The maintainer approved publishing the Tellegen contribution as a
draft PR on 2026-10-07. The companion PowerIO branches remain separate and
unpublished; the existing POUNCE release/restoration/distribution gates remain
in force.

The reviewed stack is [#143](https://github.com/eigenergy/tellegen/pull/143)
(optional backend), [#144](https://github.com/eigenergy/tellegen/pull/144)
(canonical balanced model), [#145](https://github.com/eigenergy/tellegen/pull/145)
(solve and emission), and [#159](https://github.com/eigenergy/tellegen/pull/159)
(the current-main browser refresh of [#146](https://github.com/eigenergy/tellegen/pull/146)).
The IVR branch originally branched from #145 as a sibling of #146. It now stacks
above #159 for integration with the refreshed native stack and browser baseline;
MC OPF itself remains native-only and is not forwarded by the browser worker.

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
6. **Browser integration.** Extend PR #159's refreshed optional worker/reactor path after
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

- Tellegen branch: `codex/multiconductor-ivr-opf`, retaining #145 ancestry and
  incorporating the refreshed #143/main base in `f02b3a4` and the #159/current-main
  stack in `629b947`. The pre-#159 head is preserved on
  `codex/multiconductor-ivr-before-pr159` at `2e385ee`.
- Companion PowerIO branch: `codex/multiconductor-ivr-api`, commit
  `4d00b7696e0863c4a240a1d2f0a3e97ed04c739d`, stacked on
  `codex/bmopf-component-fidelity` at `e8dcace9acd24d8db5ba24a1a5c5552d222c9c70`.
  Both include PowerIO main `c8184eba` (0.11.4). The original 0.11.3-based
  development branch remains preserved. Tellegen's shipping lockfile remains
  on released 0.11.3 until the preparation API is released; temporary validation
  resolves all six companion crates consistently to the local 0.11.4 tree.
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
lowers BMOPF per-coil capacitor arrays to exact canonical terminal shunts, shared
with Y-bus and LinDist3Flow consumers. Scalar bank capacitors retain their canonical
nameplate contract. No private capacitor metadata can overwrite an edit. N-winding
ratings retain their axes; open-delta emission uses retained maps only while they
agree with current canonical winding maps. Transformer limit selections use `transformer:<name>` under
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
six PowerIO crates consistently, updates their resolution only in that archive,
and runs native tests, the independent optional feature suite, and Clippy. It leaves both checkout lockfiles unchanged. Set
`CARGO_NET_OFFLINE=true` when the dependency cache is populated and optionally
`CARGO_TARGET_DIR` to reuse a build cache.

PowerIO checks, from its companion checkout:

```sh
cargo test --workspace --exclude powerio-py --locked
bash scripts/ci-clippy.sh
RUSTDOCFLAGS="-D warnings" cargo doc -p powerio-matrix --no-deps --locked
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
- **20 native model unit tests** include exact/zero/singular impedance,
  independent off-solution equation checks, every tested polynomial row's
  quadratic classification, forced AD parity and central Jv/Hv checks at multiple
  points/steps with changing nonzero multipliers. New derivative cases include
  21 component/control/bound models. Softplus tests lock value/gradient/Hessian
  at the breakpoint and at +/-1000. Zero-voltage impedance and zero-rated IBR
  regressions, cancellation, and corrupted current/tap/power/nonfinite results
  exercise acceptance failures.
- **PowerIO current-main stack: 2,268 tests passed / 3 existing ignored** across
  the Rust workspace excluding the Python extension crate, including C ABI and
  conversion compatibility tests. The suite includes 24 IVR preparation tests,
  9 converter contract tests, and a separate capacitor Y-bus witness. The full
  Clippy feature/binding matrix passed, including the Python extension; rustdoc
  passed with warnings denied. The initial converter revision separately passed
  661 tests / 2 existing ignored across `powerio-dist` and `powerio-matrix`.

- **Committed Tellegen snapshot:** `scripts/check-mc-opf.sh` passed against the
  committed companion after both main refreshes: **394 passed / 3 existing ignored** with defaults plus
  `mc-opf`, **175 passed / 2 existing ignored** with only `mc-opf`, and strict
  all-target Clippy passed. This checks committed files through a temporary
  archive with all six PowerIO crates patched consistently.
- **Before the main refresh, unpatched released dependencies:** locked default
  build **276 passed / 2
  existing ignored**; locked no-default build **66 passed / 1 existing ignored**.
  Both normal dependency graphs exclude POUNCE. Local path patches do not alter
  the committed lockfiles; the main refresh includes only main's lockfile changes.
  The original checkouts and their unrelated edits are preserved.

No global optimality, OpenDSS end-to-end parity, large-feeder performance or
solution sensitivity claim follows from these small witnesses.

### Main and backend PR refresh (2026-10-05)

At the maintainer's request, existing draft PR #143 now includes upstream main
`0d08c55`, preserving its history and the ancestry of #144–146. Its pushed head
is `ca4d2ac853ca08bc855e132d28f8add06d292a69`. The refresh includes main's compact
multiconductor PF sessions and center-tap/neutral regression fixtures. Both the
POUNCE feasibility gate and main's multiconductor PF regression gate remain in
`just ci` and the CI workflow. The POUNCE pin and release/restoration/distribution
requirements are unchanged. #144–146 heads were not rewritten or pushed.

PR #143's released-dependency checks passed locally: locked workspace tests
excluding the Python crate (374 passed, 2 existing ignored), standalone `mc-pf`
(160 passed, 1 existing ignored), conic (312 passed, 2 existing ignored), WASM
conic (40 passed), strict native and Python Clippy, formatting, PowerIO pin and
EPL guards. HS071 returned `SolveSucceeded`, objective 17.014017274 and maximum
violation 2.484e-8. The new `feeder-10k` PF session benchmark passed its deterministic
checks. Hosted cross-platform/distribution checks remain separate CI evidence.

The unpublished IVR branch merges this refreshed base without changing the
companion preparation API or reference values. Its committed-snapshot check
passed again, including all 64 frozen BMOPFTools solves, with the updated suite
counts above. This refresh does not publish either new implementation branch.

### PowerIO review preparation (2026-10-05)

The converter fidelity and IVR API changes are separate local branches based on
current PowerIO main. The API now documents global terminal, local conductor,
coil and physical-port axes; explicit per-unit bases; tap multiplier recovery;
and formulation-specific rejection conditions. Assembly options and output
records follow the extensible API convention. No optimizer, AD, dependency,
portable IR type, C ABI symbol or package-version change is introduced.

The downstream integration uses `McAcOpfAssemblyOptions::new` and recognizes only
the exact capacitor-lowering remark in the capacitor oracle fixtures. All 64
frozen reference solves passed again with their original numerical tolerances
and unchanged oracle data. See [the PowerIO review packet](powerio-ivr-review.md)
for exact branch bases, draft descriptions and the publication sequence.

### ENWL external validation (2026-10-06)

Eighteen original ENWL snapshots passed fresh BMOPFTools comparisons, spanning
30/99/538 buses, both LG/LN control variants, and morning/noon/evening conditions.
The 538-bus cases include 2,152 terminals and 302 IBRs. This exposed and fixed
scalar IBR bounds being dropped by the converter. A small analytic dispatch
regression now locks zero and nonzero availability. The opt-in study independently
checks original input bounds as well as voltages, currents, powers and objective;
all 18 pre-fix cases fail that comparator. Numerical evidence and reproduction
commands are in [the ENWL report](enwl-ivr-study.md). No large input is vendored.

### PR #159 integration refresh (2026-10-07)

The remote refresh was checked directly: #159 is an open draft at
`8e087c6d3a7f14ac1c31cf07f24443f80387ff02`, based on the refreshed #145 branch
`f1d5b23`. It includes current upstream main `fc041430d66abe6ce9b0c64298818204ad930946`,
the SvelteKit 3 migration, and the original #146 browser commits. Merge
`629b947` incorporates that head without conflicts or rewriting either history.
The IVR compiler, solver, shared TNLP adapter and benchmark scripts are unchanged
from the pre-refresh branch; its head `2e385ee` is preserved as
`codex/multiconductor-ivr-before-pr159`.

Integration commit `1b87ace` adjusts the incoming backend CI command from
`--all-features` to `--features conic,schema,acopf` with defaults enabled. This
covers every feature supported by the shipping released PowerIO dependency.
The additional experimental `mc-opf` feature requires the unpublished companion
preparation API, so its existing separate `scripts/check-mc-opf.sh` path is used
until that API is released and pinned. This distinction is explicit in the
workflow rather than leaving the new all-features gate unable to compile.
POUNCE pins, PowerIO release pins, solver tolerances and electrical equations
were not changed.

Validation of the merged source, using isolated archives rather than modifying
shipping lockfiles:

| Configuration | Result |
| --- | --- |
| Companion PowerIO, default features + MC OPF | 395 passed; 3 existing ignored |
| Companion PowerIO, MC OPF only | 176 passed; 2 existing ignored |
| Released PowerIO, defaults + conic/schema/AC OPF | 336 passed; 3 existing ignored |
| Opt-in balanced AC OPF WASI adapter | 1 passed |
| Strict all-target Clippy, companion and released paths | passed |
| PowerIO source pin, Rust formatting, default-workspace/adapter EPL isolation | passed |
| Shipping WASM configuration, development build with conic | compiled; fresh bindings generated |
| Engine JavaScript unit tests and engine TypeScript/contracts build | 28 tests passed; build passed |
| Svelte/controller unit tests after SvelteKit sync and prerequisite builds | 127 passed |

The browser tests use freshly generated bindings from this merged source, not
old cached declarations. Production web builds and browser end-to-end tests
were not rerun in this refresh. Previous ENWL performance measurements retain
their original source identities; they are not relabelled as new measurements.
Raw refresh logs are in `/private/tmp/ivr-pr159-validation`. The primary checkout
and its existing uncommitted work were left untouched. No branch was pushed,
no new PR was opened, and #159 itself was not modified.

### Publication gates

Obtain maintainer approval to publish the two prepared PowerIO branches, targeting
main for converter fidelity and the converter branch for IVR preparation. Retarget
or replay the second after the first merges. Follow PowerIO's existing reviewed
release procedure; then consume the released preparation API in Tellegen's
requirements and lockfile, add the optional MC suite to backend CI, and obtain
approval for the Tellegen PR targeting #159's branch (or main after it merges).
Browser forwarding,
POUNCE release, restoration wiring and distribution approval remain separate gates.
The maintainer has now authorized the Tellegen draft PR above #159. This does
not publish the companion PowerIO branches or authorize release/distribution.
