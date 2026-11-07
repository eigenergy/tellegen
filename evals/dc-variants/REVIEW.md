# Review plan for Tellegen #206

Status: implementation branches prepared; PR creation awaits the user's
approval. No remote merge is authorized. The legacy default remains unchanged.

## PowerIO PR

Proposed title: **Add explicit tap-adjusted series and PowerModels DCP conventions**

The existing series-susceptance approximation ignores tap magnitudes while
including phase shifts. Add `tap_adjusted_series_susceptance` for the
conductance-neglect transformer approximation and
`series_susceptance_no_phase_shift` for PowerModels DCP. Keep all existing names,
formulas and defaults unchanged. Use the chosen coefficient consistently in
operators, OPF preparation and affine phase injections without scaling ratings.

Expose both names through stored instances, CLI, Python and existing C ABI
string arguments. The IR approximation field remains a string; there is no
schema layout change, ABI symbol change, or required Julia binding symbol.
Old readers reject the new names. Cover raw/normalized networks, zero-tap
sentinels, three-winding lowering, stable names, saved instances, PTDF/LODF,
and the independent complex AC angle derivative.

Validation: default Rust workspace 3,617 tests passed (41 ignored), C ABI
56 passed (1 ignored), Python 260 passed (3 optional-dependency skips),
changed matrix/operator tests and Clippy passed, shipping libraries/binaries
Clippy passed, documentation and format checks passed. The full CI Clippy
matrix was attempted: unchanged tests in `powerio-tx/src/format/psse.rs`,
`powerio-dist/tests/pmd_math.rs`, and
`powerio-matrix/tests/synth_facade_parity.rs` fail strict floating-point
comparison lints. These files have no diff on this branch. The C ABI and
Python extension feature combinations pass. Resolve or rebase past the
upstream lint failures before claiming a green full CI matrix.

## Tellegen PR

Proposed title: **Select explicit DC OPF variants without changing the default**

Related issue: https://github.com/eigenergy/tellegen/issues/206

Add an optional `branch_susceptance_formula` request/study setting and resolved
result metadata. The five choices cover legacy Tellegen, PowerModels DCP /
PGLib DC, MATPOWER / PowerModels DCMP, tap-adjusted series susceptance, and
reactance only. Typed instances own their convention: omission inherits it,
a matching explicit choice is accepted, and a conflict is rejected. AC
formulations reject DC options.

Carry the setting through Rust, JSON/CLI, Python, WebAssembly and browser
studies. Save it in typed instances and solutions, including edited/reloaded
and durable studies. Recompute raw-admittance sensitivities with the selected
coefficient's chain rule. The browser selector clears stale solves/previews,
keeps the variant through WebMCP previews/re-solves, and refuses a silent
legacy-only server fallback. Imported typed instances lock the selector.

Link the selector to the [optimality compatibility matrix](../../docs/src/dc-compatibility.md).
Document equations separately from objective, dispatch, price and AC claims.
Use built-in PowerModels DCP and DCMP, independent extensions for the remaining
three conventions, and full complex AC equations for derivative validation.
The [harness README](README.md) records reproduction commands, exact versions,
input hashes, orientation handling, tolerances, numerical results and AC
failures. The small synthetic oracle ships inside the Rust crate.

Validation: the pinned Rust workspace passed 600 tests (4 ignored); the
all-feature engine passed 435 (3 ignored), standalone multiconductor feature
209 (1 ignored); shipping Clippy and native WASM-adapter tests passed. Python
passed 51 tests (1 optional-dependency skip), engine checks 59, Svelte tests
341 (1 skipped); web and Svelte type checks and production build passed.
Browser coverage exercises all selectable modes and WebMCP preview/update/reset.

The three-bus fixture and PGLib case300 match PowerModels across all variants.
Case13659 remains a numerical qualification failure for several variants,
including DCP: its DC PF equations agree, but the existing solver accepts
reduced-accuracy termination. Do not describe this as universal PGLib
optimality parity. No solver tuning or status remapping is bundled here.
AC feasibility and global optimality are not promised; failed AC runs remain
explicitly reported and are excluded from accuracy metrics.

## Approval and dependency order

1. User reviews both branch diffs and approves opening the two PRs. Until then,
   only development branches are pushed; no PRs are created.
2. Open the PowerIO PR first. Open the dependent Tellegen PR with the immutable
   PowerIO candidate pin and the numerical/CI limitations above visible.
3. After upstream review and a separately authorized PowerIO release, update
   Tellegen's minimum PowerIO version to that release, remove all eight git
   patches together, regenerate the lockfile, and run the pin guard.
4. Re-run package verification and required CI on the published dependency.
   `cargo package` currently fails as expected because Cargo drops workspace
   patches and crates.io PowerIO 0.11.9 lacks the new variants. Do not bypass
   this publication-order gate.
5. Keep the default unchanged. A future default change, solver status fix or
   large-case numerical tuning requires its own explicit decision and
   qualification. No remote merge is performed as part of this work.

The final PowerIO candidate is
`c25eeb54557ab9017ea5a05a3ff38df6108cda4f`; Tellegen pins all eight components
to that commit. The broad pinned Rust runs used its parent implementation
commit `b1d49113de1d705be4a0d149497a8a9596c15f0e`. The only intervening change
is a PowerIO operator regression test; the final-pin Tellegen integration
suite was also run. Browser/WebAssembly and Python checks exercise the same
implementation sources.
