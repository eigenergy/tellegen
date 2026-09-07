# Independent MC correctness cases

## Distribution Study integration

The distribution simulation branch uses parsed MC capability metadata, with
raw BMOPF preflight before normalization. Ingest regressions cover supported
BMOPF, view-only raw DSS, and unsupported BMOPF load semantics. Saved snapshots
validate the full typed instance, portable solution, terminal and element-port
identities, finite values, power consistency, and convergence evidence without
a new global factorization. Malformed saved load connections return errors.

The final three Chromium MC regressions passed: raw/typed worker solving;
Study earth/neutral voltage values plus distinct saved results across reloads
and validated reopening; and the built-in demo plus a three-wire case with no
neutral-relative columns. Twelve multiconductor UI helper tests passed,
including declared neutral identity and absent-neutral behavior. Custom-neutral
WASM ingest tests passed within the 26-test WASM suite.
Four existing balanced Study regressions also passed: proposal and
application, artifact tampering, storage exhaustion/recovery, and demand edits
with base reset. The previously reproduced cancellation failure remains excluded
from this passing subset.

Manual browser inspection of the four-wire example confirmed 23 iterations,
49.8253352802 kW source power, 16.507917275 kvar source reactive power, and
4.8253352808 kW passive loss. Load-bus voltages to earth are 221.761933914,
228.615871255, 213.351123776, and 11.897554292 V for a/b/c/n. Independent complex
subtraction gives phase-to-neutral magnitudes 222.332976215, 238.828149852,
202.975849439 V and angles -2.757520307, -118.601871145, 121.639709693 degrees.

## Current acceptance summary

This summary supersedes historical checkpoint statuses below. Independent scientific
review accepts the documented supported profile. Final local checks passed 36 MC
unit tests and nine frozen oracle tests. This acceptance covers the stated strict
constant-power/ideal-source profile, not default OpenDSS voltage fallback or
unsupported equipment. Raw and typed module endpoints both reject retained
unsupported/lossy parse diagnostics.

Final integration checks passed: `cargo test --workspace --locked`, the
independent `mc-pf` feature suite, shipping-crate Clippy with warnings denied,
Rust formatting, WASM release compilation, JavaScript checks/build, web
formatting, and offline crate package verification. The corrected Chromium
MC test passes with both raw BMOPF and typed-module solves in a real worker.
The full parallel browser run was not clean: two existing layout/help tests
passed isolated reruns, while the Study cancellation test timed out when its
Cancel button disappeared before the click. That cancellation failure was
also reproduced with one worker on an unchanged `git archive HEAD` baseline
build in `/tmp/tellegen-baseline-qc`; it is not counted as a passing gate.

| Scientific gate | Current evidence/status |
| --- | --- |
| Constant-power signs, fixed compensation RHS and retained sparse factorization | Corrected and exercised by analytical and feeder tests |
| Representative ENWL/MV terminal voltage/current agreement | Passed matched strict-CP, near-ideal-source profile; detailed measured errors below |
| Immutable ENWL 128-case sweep | 127 converged comparisons, all nodes matched, all one factorization; remaining case also fails strict-CP OpenDSS |
| Raw/common two-winding YY/DD/YD/DY leakage and per-winding DELTA orientation | Independently passed 14 full DSS Yprim comparisons through canonical and exact PowerIO 0.11 raw-reader paths |
| Loaded raw YY and DD | Independently converged with one factorization; max complex DSS voltage error 2.43356e-6 V, raw KCL below 1.53e-12 A |
| Excitation side, sign and tap base | Independently passed seven additional full DSS Yprim comparisons with explicit winding-2 per-coil g=0.001,b=-0.002 S |
| Exact solid and floating transformer neutral | Code handles exact zero constraint and negative floating sentinel; no hidden 1e6 S approximation |
| Physical KCL scaling | Uses actual incident element and load-branch currents; documented absolute floor 1e-6 A, relative 1e-8 |
| Missing nominal load voltage | Global-source fallback removed; explicit positive nominal branch voltage required |
| Malformed tables, unknown connection, duplicated ideal sources | Explicit rejection guards added; regression inputs retained |
| Browser raw and typed transport | Parent reports corrected raw+typed Chromium regression passed (224 ms). Full browser suite has failures outside this regression, so is not claimed green |
| Frozen small-oracle Rust regression harness | Passed 9/9 in independent final rerun: actual DSS NodeOrder, numeric line/load currents and complex powers, global complex source/element balance; transformer/reactor ports presence+finite only |
| Declared neutral ordering/profile coverage | Closed for declared raw WYE load/transformer conventions: known non-final neutral rejected; conventional no-conventions inputs retained. Typed load explicit neutral extras selects all remaining phases |

Independent primitive evidence is in `docs/evidence/mc-pf-yprim`: three reports,
21 full reference matrices with DSS commands and terminal identities, and the
corresponding canonical/raw inputs. It uses a frozen transformer source snapshot
SHA-256 `03fd5850cb47af69090e2fefb88ab855ed1e167ed3e274e9a8b27aa3864eb786`,
OpenDSSDirect.py 0.9.4 and DSS C-API 0.14.5 revision
`87d85c2622c8281b92255335bc7c09b11191b21d` (SVN 3723 basis). Canonical cases
include fixed taps 1.03/0.97; raw two-winding cases use nominal taps. Opposite
DELTA coil directions are compared with an equivalent bank of three explicit
single-phase DSS transformers, preserving the specified physical coil pairs.
All entries satisfy `abs(error)<=1e-10 S+1e-8*abs(reference)`; maximum observed
error is 4.47545209131181e-16 S. This does not assert the C-API executable is the
supplied r4176 Pascal executable: the latter is a separate source baseline.

Loaded YY/DD evidence is `docs/evidence/mc-pf-yprim/loaded-report.json` with
inputs `inputs/loaded-yy.json` and `inputs/loaded-dd.json`. The YY case fixes
both explicit neutrals to ground; the DD case fixes one secondary corner
(DSS `lv.1.2.0`) to remove the otherwise floating common-mode reference.
Both apply three 1000+j200 VA constant-power branches. The recorded native
binary hash identifies this separate end-to-end check.

Final corpus binary SHA-256:
`0b01eb71f2f4e24034481406864570f9e999f5553e50b30c94d0f458c81ab70c`.
Inspection of `/tmp/tellegen-enwl-final-results.json` confirms 127 of 128 cases
converged with complete node coverage; maximum voltage error
1.1681925520891858e-7 V, maximum size 28,092 terminals, maximum raw KCL
4.446980287140455e-7 A. Only network 13 feeder 4 fails; its matched strict-CP
OpenDSS oracle also fails after 1,000 iterations. The earlier network 6 feeder 1
residual-floor rejection is resolved by the documented 1e-6 A absolute floor.

## Historical investigation and regression specifications

Initial review specification, 2026-09-07. These are acceptance tests requested
from the implementers, not claims of passing tests. They target errors observed
in the initial evolving implementation.

1. **Rotated analytic feeder.** E=10 exp(j pi/3) V, R=1 ohm, P=1 W,
   Q=0, nominal branch voltage=10 V. Exact high root is
   `(10+sqrt(96))/2 * exp(j pi/3)`. Repeat at angle 0 and pi/2. This detects
   `conj(S)/V` instead of `conj(S/V)`, invisible on a purely real test.
2. **Reactive power identity.** At prescribed branch U=3+j4 V and S=10+j5 VA,
   load withdrawal is 2+j1 A; assert U*conj(I)=10+j5, independent of PF.
   Repeat with WYE neutral displaced and DELTA incidence, testing total terminal
   power and zero sum of branch terminal currents.
3. **Nonzero fixed-return branch.** A load connects an unknown terminal to a
   fixed terminal with nonzero voltage. Verify initialization and compensation
   both subtract full `(Ypassive+Yref)_uf vf`. Physical KCL uses passive Y only.
4. **End shunts.** One 2 m line with G_from=0.01 S/m, G_to=0.03 S/m and
   distinct B ends. Verify end primitive additions 0.02 and 0.06 S, without
   another division by two. Include full coupled off-diagonal shunts.
5. **Source reaction.** In resistive analytic feeder source supply current is
   +(10-Vload) A. Add a 2 W load directly on the source bus and assert supply
   rises by 0.2 A without changing the downstream voltage. Make source.name
   equal an unrelated bus ID and ensure source mapping still uses its own bus.
6. **Instance semantics.** Deserialize a stored PF instance with prescribed load
   powers/voltage model differing from retained network nameplate. Solve those
   prescribed values or explicitly reject inconsistency. Never ignore them.
7. **Connection semantics.** SINGLE_PHASE two-terminal load with a displaced
   return conductor; reordered WYE neutral using explicit terminal_conventions;
   DELTA three-coil unbalance; reject unsupported terminal cardinalities.
8. **One-phase transformer dimensional oracle.** V1=100 V,V2=10 V,S=1000 VA,
   r1=r2=1%,x=0,taps=1, grounded returns. HV effective R=0.2 ohm, so reduced
   Y11=5 S,Y12=Y21=-50 S,Y22=500 S. Repeat unequal S2 and fixed tap, comparing
   physical referred resistance independently. Tests merely checking finite
   entries/nonzero coupling do not detect a faulty resistance base.
9. **Convergence and rank.** Both voltage change and physical KCL pass on the
   returned iterate. Damping near zero must not yield false convergence. A
   floating source-free passive island with nominal load admittance is rejected
   as lacking physical reference; Yref is not physical grounding. Test zero
   power at zero voltage, nonfinite powers on fixed-only terminals and singular
   line/transformer local matrices with scale-aware error handling.
10. **Sparse preparation scaling.** A growing chain retains O(N) stored global
    entries; no N-by-N passive/Yref allocations or row scans. Preparation
    factors once and initialization/iterations only perform retained solves.
    Report true factor instrumentation and line/transformer local operations
    separately. Include actual browser WASM execution.
11. **Output completeness.** Element terminal currents/powers, source identities,
    and explicit neutral/grounding contributions permit independent total power
    balance. Shared ideal sources either report aggregate reaction as such or
    reject ambiguous individual current allocation. No duplicated supply.
12. **Preflight completeness.** Controls, untyped active devices, nontrivial
    statuses, finite source metadata and unsupported edit/sensitivity requests
    cannot disappear. A fixed capacitor must implement its actual configuration
    or be rejected; an all-pairs graph is not a general WYE capacitor.

## Intermediate correction review

The second code inspection confirmed corrections for load-current conjugation,
SINGLE_PHASE two-terminal incidence, end-shunt scaling, stored instance p/q and
voltage model values, global sparse storage, transformer ZB dimensional scaling,
and conventional BMOPF subtype delta orientation. These are code-review closures,
not end-to-end numerical acceptance. Global sparse storage still had quadratic
extraction/scans at that inspection.

The transformer direct numerical 5/-50/500 S matrix and secondary tap=2 matrix
5/-25/125 S assertions are independent useful primitive gates. The unequal-rating
formula is correct for canonical r_pct on each winding's own base. Original DSS
r4176 Rpu, however, is on winding-1 S base (Transformer.pas line69 and line2160),
while PowerIO0.11 DSS read.rs1438 copies r_pct without base conversion. To feed
such original DSS values into an own-rating primitive requires r_pct_j*=Sj/S1,
with explicit source provenance. A BMOPF document already exported using the
older inconsistent conversion must not be silently altered to match original DSS.

Remaining observed review items: per-winding n_winding delta rolls; declared
neutral ordering and exact-vs-1e6 S solid grounding policy; full fixed Yref RHS;
source reactions/output identities; load prescribed terminal order; nominal
voltage propagation across transformer ratios; rank/reference validation;
physical/scaled convergence evidence; raw preflight and browser execution.

## MV oracle case preparation boundary

The parent prepared a 984-node, 327-line MV-only case with the original 11 kV,
-30 degree source boundary and 43 balanced DELTA leaf loads (20 kW + j6 kvar
per load). OpenDSS `switch=y` lines have finite specified series impedance
(0.001+j0.001 ohm in this case); they are not ideal closed switches. The parent
materialized 84 such lines as ordinary R/X/C lines in
`/tmp/tellegen-mv/Switches-materialized.dss` and reports an entry-by-entry DSS
Yprim difference of exactly zero for every changed line. This transformation
preserves the passive electrical model; retain its source, command, hashes and
comparison output in the final oracle manifest. The corresponding export is
`/tmp/tellegen-mv-materialized.json`. Earlier `/tmp/tellegen-mv.json` variants
are not acceptance evidence. These facts record parent preparation context;
this review has not independently executed the complete MV oracle comparison.

## Representative feeder measurements received during integration

Inspected `/tmp/tellegen-feeder-comparison.json` from the parent's pinned oracle
harness. ENWL network 1 feeder 1 matches all 3,628 oracle terminals, maximum
complex-voltage error 8.11834243857703e-8 V and Tellegen raw unknown-terminal
KCL 1.184384274892499e-8 A. Parent reports one factorization and eight iterations.
The materialized MV case matches all 984 oracle nodes, plus one additional
PowerIO source-neutral terminal `b1726.4` at exactly zero (not an unmatched
energized conductor). Maximum complex-voltage error is 4.683695749071584e-6 V,
raw KCL 2.2794393510786228e-7 A; parent reports one factorization, five iterations.

The oracle uses a matched constant-power profile and a 1e-12 ohm source
approximation to ideal prescribed voltage. These comparisons do not validate
original finite-impedance sources or OpenDSS voltage-dependent load fallback.
The then-reported MV scaled KCL of approximately 2.25e-8 is NOT an accepted
physical current-relative residual: its denominator used sum |Yij|*|Vj|.
Final evidence must rerun with actual incident element-current scaling.

Parent independently compared aggregate `(element,bus,node)` line and load
terminal currents, summing DELTA sub-branch ports before comparison, in
`/tmp/tellegen-feeder-currents.json`: ENWL 7,358 matched ports, no missing/extra,
maximum 3.80e-8 A; MV 2,091 matched ports, no missing/extra, maximum 1.256e-6 A.
Parent reports source supply minus all element absorbed complex power of about
(1.88e-6+j2.73e-6) VA for ENWL and (-0.00439+j0.00709) VA for the 860 kW MV case.
The 1e-12 ohm source oracle suffers subtraction/cancellation in source current,
so use independent global KCL and power balance for source reactions rather
than claiming exact source-current agreement with that approximation.

The two reproduced adversarial failures at this checkpoint are retained in
`/tmp/tellegen-qc-shared-source.json` (two coincident sources each report full
reaction, doubling total supply) and `/tmp/tellegen-qc-unknown-connection.json`
(unknown load configuration silently defaults WYE and converges). These require
regressions and correction before final acceptance; successful feeder tests do
not close them.

## Latest rolling review status

This section supersedes earlier open/closed labels only for the items listed;
older sections retain reproduction and investigation history.

| Item | Latest reviewed state |
| --- | --- |
| CP current conjugation and fixed Yref elimination | Corrected in code |
| Global sparse storage, extraction, row products and load scatter | Corrected in code |
| Line/load/shunt/transformer terminal ports and aggregate injection signs | Corrected in code |
| Physical incident-current KCL denominator | Corrected in code: actual local element currents plus each load-branch magnitude |
| Coincident ideal-source duplicated reaction | Corrected in code by explicit indeterminate-allocation rejection; reproduction retained for regression |
| Unknown load configuration | Raw enum guard and regression added; reproduction retained |
| Floating DSS negative neutral sentinel | Explicit handling added |
| Solid-neutral arbitrary 1e6 S stamp | Now requires explicit `neutral_profile=opendss_1e6_siemens`; exact constraint support remains limited |
| Per-winding BMOPF delta roll | Still requires final primitive review; input acceptance alone is insufficient |
| Missing nominal load voltage across transformer ratios | Global maximum-source fallback still requires correction or explicit restricted-profile documentation |
| Fixed capacitor configurations/ports | Ports added; topology/cardinality/voltage-unit scope needs verification or narrowing |
| Reader/projection diagnostics | Still omitted by raw entry point at this inspection |

A further native reproduction `/tmp/tellegen-qc-array-load-table.json` demonstrates
that malformed top-level `load` as an array is silently skipped, yielding a
successful zero-load result. Validate electrical table/row shape before the
permissive reader. Broad unsupported-shape rejection is appropriate; benign
frequency inference and documented inline-length normalization should not be
mistaken for dropped physics.

## ENWL corpus sweep checkpoint

Inspected `/tmp/tellegen-enwl-corpus-results.json`: 128 cases, 126 completed
comparisons, all 126 oracle-converged, all oracle nodes matched with no extras,
maximum complex-voltage error 1.1681925520891858e-7 V. Every completed case
reports exactly one global factorization.

Two results require distinct treatment. `network_6_Feeder_1` hit the earlier
absolute KCL floor after 100 iterations despite voltage change 2.562e-10 V
and raw KCL 3.766e-7 A; rerun with corrected physical-current scaling.
`network_13_Feeder_4` failed strict CP convergence (voltage change 55.85 V,
raw KCL 400.7 A). Parent independently found strict-CP OpenDSS also fails after
1,000 iterations, whereas original OpenDSS voltage-dependent fallback converges
in four. This is an expected profile/nonconvergence distinction, not evidence
that the strict-CP solver should silently adopt a different load law. Preserve
both model settings and the controlled Tellegen failure as a regression.

This checkpoint predates the immutable final binary. Acceptance must rerun the
complete corpus against the final code and retain executable/source hashes.
Exact solid transformer grounds can already be normalized to bus.grounded
constraints: remove the redundant transformer zero-impedance field, retain that
normalization explicitly, and do not substitute the 1e6 S approximation.

## Frozen small-oracle audit requirements

The first inspected `tests/generate_mc_pf_oracle.py` and `tests/mc_pf_oracle.rs`
are not accepted evidence until the following corrections are verified:

- Missing DSS node lookup must fail; only physically verified common-ground
  aliases may be exactly zero. Enumerated map positions are not DSS node IDs.
- Ground aliases apply to specific bus/node identities across every element
  terminal, including to-side line ends, loads and transformer windings. Do not
  short every reactor located on the source bus: a phase shunt is real physics.
- Freeze physical port addresses (element class/name, terminal side, BusNames,
  NodeOrder and conductor count), actual currents in A and powers converted
  from OpenDSS kW/kvar to W/var. Compare values, not element-name presence.
- Aggregate DELTA load sub-branch currents by physical terminal. For DSS
  transformers include their BMOPF separately materialized core shunts using
  recorded ownership; keep external neutral-grounding reactors separate.
- Source `puZideal=[1e-12,0]` is per-unit impedance, not 1e-12 ohm. Record the
  actual executed profile, including vlowpu and neutral alias modifications.
- Capture installed wrapper and actual DSS backend versions, hash the DSS
  inputs/include closure and generated BMOPF files, and retain actual licenses.
  Hardcoded version/commit assertions alone do not establish provenance.
- Tests require appropriate feature gates, finite values and unique identities,
  precise coverage, voltage/current/power gates informed by observed errors,
  and one-factor evidence. A 2e-3 V gate around observed 2e-6 V errors requires
  justification; names-only current checks establish no numerical accuracy.

These are generator/test review findings. Parent feeder harness measurements
above are a separate evidence source and are not invalidated by weaknesses in
this initial frozen-fixture implementation.

## Independently checked source and producer provenance

SHA-256 of supplied `electricdss-code-r4176-trunk.zip`:
`e51c834785d96888ea039a8e30b8bb9bd1d55ec2cabf8a4681a0ae281761bbcf`.
The inspected extracted files match these exact archive members (not CMD_Lazz):

- `Version8/Source/PDElements/Transformer.pas`:
  `374b2cea251f371522c4a46913daee6f4e54fb8dc4a09989bd55269376d0c647`.
- `Version8/Source/Common/Solution.pas`:
  `c4108ec26b59b923920e2eda292e7d8d530185bb930baacda843a9e889bea00b`.

The isolated Julia producer manifest `/tmp/tellegen-bmopf-export/Manifest.toml`
pins PowerIO.jl 0.11.0 to git-tree-sha1
`f283e791c88df505ec28f33409d3bf390fc3e641` from
`https://github.com/eigenergy/PowerIO.jl` (requested rev `main`). Record the tree,
not merely the moving branch name. BMOPFTools is the previously recorded
8ca84ab commit. Parent exports used `from_dss` followed by `write_bmopf`, which
preserves provenance and performs its documented schema normalization; raw
JSON3 serialization of the mutable dictionary is not interchangeable.

Equivalent per-file reproduction command, after restoring that manifest:

```sh
julia --project=/tmp/tellegen-bmopf-export -e \
  'using BMOPFTools; write_bmopf(from_dss(ARGS[1]), ARGS[2])' \
  /absolute/input.dss /absolute/output.json
```

Latest source review closes malformed raw electrical tables, unknown connection
fallback, sparse physical-current scaling, duplicated ideal-source allocation,
and exact solid-neutral constraint stamping in code. Per-winding raw two-winding
transformer support and the rewritten frozen-oracle port comparisons are still
under active review; do not treat earlier temporary n_winding rejection as the
final supported scope.
