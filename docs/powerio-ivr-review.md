# PowerIO IVR review packet

Updated on 2026-10-27 for [Tellegen #160](https://github.com/eigenergy/tellegen/pull/160).
Its former base #159 and the PowerIO prerequisites #564, #569 and #570 are
merged. The consumer now targets Tellegen main
`f6a9c538559e988f84e7cb979df29cd465fc083c` and uses main's published
PowerIO **0.11.6** and POUNCE **0.13.0** dependencies. PowerIO 0.11.6 supplies
the IVR preparation API and the ZIP 9 upgrade; the old unreleased-API and ZIP 8
blockers no longer apply. No shipping dependency is patched to a private tree.

The refresh adapts sequence-voltage row construction to PowerIO's public
non-exhaustive API. Balanced and MC formulations share the released POUNCE
restoration factory and cancellation adapter, including main's early-cancel
initialization guard. Balanced second-opinion recovery remains unchanged.
Hosted CI and `just ci` exercise native MC OPF with all features and MC OPF
alone; shipping adapters retain their existing feature boundaries and EPL guard.

The numerical counts below are historical October 5–17 evidence. Fresh test
results and CI identities belong on #160. The 64 frozen BMOPFTools references
and acceptance tolerances are unchanged. ENWL performance measurements retain
their original source identities and have not been rerun for this refresh.

Conversion must preserve the electrical problem before any optimizer sees it.
The preparation PR follows PowerIO's existing balanced AC/DC preparation boundary:
canonical semantics belong to `powerio-dist`/`powerio-prob`; indexed coefficients
belong to `powerio-matrix`; solver expression construction, differentiation,
execution and result acceptance belong to Tellegen. No new dependency, optimizer,
AD package, portable IR type, ABI entry point or package-version change is needed.

## Historical PowerIO #569 review

**Title:** Preserve BMOPF coil physics and edited regulator connections

BMOPF per-coil capacitor ratings cannot be represented by a scalar bank rating
when coils are unequal. Lower array-rated capacitors exactly into the existing
canonical terminal-shunt matrix, so Y-bus, LinDist3Flow and IVR consumers see the
same susceptance. Report the representation change as a remark, validate the
array and voltage domain, and avoid generated-name collisions. Scalar bank
capacitors keep their existing contract.

Preserve custom open-delta maps on both sides and n-winding apparent-power bounds.
Before recombining open-delta legs, verify retained source maps against the current
canonical windings; edited connections fall back to separately emitted legs with
a diagnostic. No private capacitor coefficient metadata can override edits.
Preserve scalar IBR capability bounds as one-entry vectors, including zero PV
availability; the ENWL snapshots exposed that these were previously dropped.

Validation: nine focused converter tests pass and assert hand-computed coefficients,
round trips after edits, malformed inputs, zero/two-wire/delta incidence, generated
names, custom regulator maps, winding bounds, and scalar IBR availability. Independent LinDist3Flow and
Y-bus witnesses check capacitor consumption downstream. Strict affected-crate
Clippy and formatting passed. The full feature/binding Clippy matrix also passed
independently on this branch's committed snapshot. The combined stack passed
2,268 Rust workspace tests, 3 existing ignored, after the scalar-bound fix.

## Historical PowerIO #570 review

**Title:** Add solver-independent multiconductor IVR preparation

Downstream multiconductor OPF solvers currently lack a shared preparation layer
for conductor and coil semantics. Add `build_mc_ac_opf_preparation` and documented
coefficient records with explicit SI/per-unit bases, source mappings, selected
bounds, line/shunt/device data and winding-current transformer descriptors.
Retain currents for zero/singular impedance and support the validated AC load,
generator, transformer, tap and IBR/control profile without introducing a solver
or expression dependency.

Document terminal versus coil/port axes, injection signs, tap multipliers and
unsupported formulation domains. Initial points, ideal cycles, certain source
references and angle windows are profile restrictions, not claims that the
canonical electrical network is infeasible. Nonpolynomial laws remain normalized
physical data; the consuming solver chooses its derivative implementation.

Validation: 2,268 Rust workspace tests passed, 3 existing ignored (Python extension
excluded from runtime tests), including C ABI and conversion compatibility tests.
The preparation suite has 24 tests. Full `scripts/ci-clippy.sh` passed all seven
feature/binding configurations, including the Python extension. Public rustdoc
passed with warnings denied. Tellegen passed 394 tests / 3 existing ignored with
defaults plus MC OPF, 175 / 2 with only MC OPF, and strict all-target Clippy against
this companion. This includes all 64 frozen BMOPFTools reference solves with
unchanged numerical baselines/tolerances, derivative checks and dispatch witnesses.
The [ENWL study](enwl-ivr-study.md) additionally passes 18 external snapshots up to
538 buses against fresh BMOPFTools solves, including original-input bound checks.
The identical comparator rejects all 18 pre-fix results as a negative control.

These small cases do not establish global optimality, arbitrary profile support,
large-feeder OPF performance, or solution sensitivities. The new preparation is a
Rust derived-data API; existing portable solution and binding schemas are unchanged.

## Historical source-reference integration

The IVR profile uses absolute earth-referenced source phasors. #564 adds a typed
reference terminal, whose voltage prescription is differential. Preparation now
calls `powerio_dist::readiness::require_earth_referenced_sources` before deriving
coefficients; terminal-referenced sources are rejected with the shared diagnostic.
Supporting differential source equations and reference-terminal return currents
remains outside this profile.

The new regression preserves an accepted earth-referenced control, then changes
one source to prescribe Va - Vb = 230 V while another fixes Vb = 230 V. Without
the guard, preparation incorrectly fixes both Va and Vb to 230 V. The regression
was observed to fail before the guard and passes with it; the preparation suite
now contains 25 tests. No derivative engine or solver architecture changes are
needed for this integration.


Validation of the integrated tree passed: 2,361 PowerIO workspace tests with
three existing ignores, all 25 IVR preparation tests, and all seven Clippy
configurations separately on each PowerIO branch. The consolidated CGMES suite
passed 120 tests. The advisory check, all-feature rustdoc with warnings denied,
mdBook build/tests, terminology and formatting checks passed. Tellegen's pinned
Rust 1.96 companion gate passed 395 tests (three existing ignores) with defaults
plus MC OPF, 176 tests (two existing ignores) with MC OPF alone, and strict
all-target Clippy. The 64 frozen BMOPFTools solves retain their numerical
baselines and tolerances. The earlier ENWL experiments were not rerun.

## Reproduction and review gates

From committed Tellegen, using the locked published PowerIO release:

```sh
bash scripts/check-mc-opf.sh
```

The script archives committed source and runs all-feature tests, MC-only tests
and strict all-target Clippy with `--locked`. An optional PowerIO checkout
argument patches all seven matching crates only in the temporary archive; it
never changes either checkout's lockfile. CI uses the published dependency path.
Run `just ci` for the complete repository gate set, including dependency audits,
shipping adapters, balanced AC OPF, JavaScript and browser checks.

The formulation remains native-only and experimental. Independent physical
residual and original-bound validation gate accepted solutions. Unsupported
canonical load laws, including bounded load laws, are rejected rather than
approximated. Frozen references, schema checks and self-roundtrips do not prove
arbitrary profile support or large-feeder performance. Maintainer review and
current-head CI remain required before any eventual merge; this refresh does
not authorize merging or publishing.
