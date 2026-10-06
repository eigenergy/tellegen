# PowerIO IVR review packet

Prepared on 2026-10-05; updated after the ENWL study on 2026-10-06. No branches in this packet have been pushed and no PRs
have been created. Existing Tellegen PR #143 is independent of this publication
approval. The original PowerIO development commits remain available on
`codex/multiconductor-ivr-preparation-original`.

## Branches and ownership

| Change | Branch | Head | Intended PR base |
| --- | --- | --- | --- |
| Converter fidelity | `codex/bmopf-component-fidelity` | `e8dcace9acd24d8db5ba24a1a5c5552d222c9c70` | PowerIO `main`, prepared at `c8184eba8ab67fa3a5c60cf4c64ccbe878963bfa` |
| IVR preparation | `codex/multiconductor-ivr-api` | `4d00b7696e0863c4a240a1d2f0a3e97ed04c739d` | `codex/bmopf-component-fidelity` |

The active PowerIO checkout is
`/Users/uqfgeth/.codex/worktrees/multiconductor-ivr-preparation/powerio`.
The unpublished Tellegen consumer is `codex/multiconductor-ivr-opf`, in
`/Users/uqfgeth/.codex/worktrees/multiconductor-ivr-opf/tellegen`.

Conversion must preserve the electrical problem before any optimizer sees it.
The second PR follows PowerIO's existing balanced AC/DC preparation boundary:
canonical semantics belong to `powerio-dist`/`powerio-prob`; indexed coefficients
belong to `powerio-matrix`; solver expression construction, differentiation,
execution and result acceptance belong to Tellegen. No new dependency, optimizer,
AD package, portable IR type, ABI entry point or package-version change is needed.

## Draft PR 1

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

## Draft PR 2

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

After approval, push converter fidelity and open its PR against main; push IVR
preparation and open it against the converter branch. Attach each created PR to
this task. After converter merge, retarget/replay the API branch and rerun its
checks. Use PowerIO's established reviewed release procedure. Only after an API
release should Tellegen consume that released version in its requirements and
lockfile, wire optional MC CI, and publish its separately approved PR. The
Tellegen integration branch now includes #159 at `8e087c6`; use that branch as
the intended PR base, or main once the refreshed stack has merged.
