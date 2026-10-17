# PowerIO IVR review packet

Updated on 2026-10-17. The companion changes are published and ready for
review as [PowerIO #569](https://github.com/eigenergy/powerio/pull/569) and
[#570](https://github.com/eigenergy/powerio/pull/570); the consumer is
[Tellegen #160](https://github.com/eigenergy/tellegen/pull/160). Review readiness
and release readiness are separate: the preparation API is still unreleased.
The original development commits remain on
`codex/multiconductor-ivr-preparation-original`.

## Branches and ownership

| Change | Branch | Head | PR base |
| --- | --- | --- | --- |
| Converter fidelity (#569) | `codex/bmopf-component-fidelity` | `47ecbfb744e25747280c1f27f9b29dc41ef99fff` | PowerIO `main` at `39f94ebf65f35044b7fc5edc5369559831ab2510` |
| IVR preparation (#570) | `codex/multiconductor-ivr-api` | `84d09b10032a94b3a3f923a2a464e885f7a34ad9` | `codex/bmopf-component-fidelity` |

Both PowerIO branches include current main, whose workspace version is 0.11.5;
the 0.11.6 schema snapshot comes from main's upcoming CIM distribution support.
Tellegen #160 includes main `192605a66d34be05ce27da651f799092f40f8a48`
and retains #159 (`dot/acopf-browser-refresh`) as its review base.
Tellegen's shipping dependency remains the released 0.11.3 set. The review script
below temporarily selects all six companion crates at 0.11.5.

The numerical counts below describe the original October 5–6 test packet.
Current refresh results and exact CI heads are recorded in the linked PRs;
ENWL measurements retain their original source identities.

Conversion must preserve the electrical problem before any optimizer sees it.
The second PR follows PowerIO's existing balanced AC/DC preparation boundary:
canonical semantics belong to `powerio-dist`/`powerio-prob`; indexed coefficients
belong to `powerio-matrix`; solver expression construction, differentiation,
execution and result acceptance belong to Tellegen. No new dependency, optimizer,
AD package, portable IR type, ABI entry point or package-version change is needed.

## PowerIO #569

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

## PowerIO #570

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

## Reproduction and publication

From PowerIO:

```sh
cargo test --workspace --exclude powerio-py --locked
bash scripts/ci-clippy.sh
cargo fmt --all --check
RUSTDOCFLAGS="-D warnings" cargo doc -p powerio-matrix --no-deps --locked
```

From committed Tellegen:

```sh
bash scripts/check-mc-opf.sh /path/to/companion/powerio
```

The Tellegen script archives committed source, patches all six companion crates,
and updates resolution only in the temporary archive. Neither checkout's lockfile
acquires path dependencies. Ordinary tests require no Julia or network when the
locked dependency cache is populated.

Review and merge #569 first, then retarget #570 to main and recheck the combined
tree. Follow PowerIO's current tag-driven release procedure after maintainer
release approval. Once the preparation API has a published release, update
Tellegen's requirements and lockfile and enable hosted MC OPF tests. Tellegen
#160 stays stacked on #159 until its base stack merges. Its existing POUNCE
release/restoration/distribution and CI requirements remain merge gates.
