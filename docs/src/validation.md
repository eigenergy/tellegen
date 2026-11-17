# Validation

tellegen is checked against the published PGLib reference solves. For each case and
variant, `$PGLIB_OPF_PATH/BASELINE.md` tabulates the PowerModels.jl with IPOPT
reference: `DC ($/h)`, `AC ($/h)`, `QC Gap (%)`, and `SOC Gap (%)`. The
[Methodology](validation.md#methodology) chapter describes how the harness produces the
comparison; this chapter records what each comparison asserts. The measured figures are in
[benchmark results](benchmark-results.md), a committed snapshot of a full
harness run with its source information.

## DC objective

tellegen's declared DC objective, including its constant cost term, is compared
against the published DC baseline. Both values come from the same PGLib cost
data. Per-unit power scaling cancels; tellegen does not assign a currency to
objectives from other sources.

## Relaxation lower bound

The SOCWR objective (`socwr_opf` objective) must be a lower bound on the published
AC optimum (`AC ($/h)`):

$$ \text{socwr} \le \text{AC} + \text{tol}. $$

A bound violation is a correctness failure.

## SOC gap

The gap is

$$ \text{gap} = \frac{\text{AC} - \text{socwr}}{\text{AC}} \cdot 100, $$

compared against the baseline `SOC Gap (%)`. tellegen's SOCWR is the Jabr SOC
relaxation, the same family as the baseline `SOC` column, so a near-zero
difference in gap is the expected result. The published SOC bound is recovered as
$\text{AC} \cdot (1 - \text{SOC gap}/100)$.

All three variants are exercised: typical, congested (API), and small-angle (SAD).
tellegen's `AcNetwork` carries the angle difference limits and the SOCWR enforces
them in W-space, so the SAD relaxation tracks the published SAD SOC. Where a case
shows a large SOC gap, that measures relaxation quality, not a failure.

## Multiconductor IVR OPF

The experimental native [IVR formulation](formulations.md#experimental-native-multiconductor-ivr-opf)
requires both a successful solver status and an independent physical acceptance
check. Tellegen recomputes KCL, KVL, device power links, load/control laws,
selected limits and the objective from the solution and prepared electrical data.
The default maximum normalized residual tolerance is `1e-6` in the model's
working units. Nonfinite values fail acceptance. Local numerical success does
not establish global optimality.

This check is independent of the solver expression graph but shares PowerIO's
preparation. It cannot detect a physical constraint discarded before preparation.
The module-JSON entry point and external-case probe therefore use the same
`validate_mc_opf_input_diagnostics` guard: error diagnostics and codes containing
`UNSUPPORTED`, `MALFORMED`, `FIELD_DROPPED` or `RECORD_DROPPED` reject the input.
Lossless representation-change remarks, such as capacitor-to-shunt lowering,
are permitted. External comparisons also inspect original input bounds, including
scalar inverter availability and zero available active power.

The experimental MC solve APIs return `McOpfError`, which implements
`std::error::Error` and serializes to JSON. Callers match `kind` and `stage`
rather than parsing its display message. Kinds distinguish invalid input,
unsupported input, cancellation, numerical failure, physical-validation failure
and serialization failure. Upstream findings retain their codes, severities,
messages and targets. Numerical failures include the solver status when one is
available; they are not infeasibility certificates.

Physical-validation errors include the finite residual summary and the worst
failing witness in each residual family, with component and axis identities.
A witness whose residual cannot be represented as a finite number uses `null`.
No failure contains an accepted solution. The external-case probe uses the same
structured error object in its `error` field.

Regression coverage combines:

- Hand-computed off-solution KCL/KVL/device-law witnesses, mutual coupling,
  unequal end shunts, and zero/singular branch impedances.
- Quadratic-row classification and forced sparse-AD comparisons, directional
  Jacobian/Hessian checks at multiple points with nonzero multipliers and
  objective factors, and control derivatives at knots and softplus extremes.
- Base/order invariance, cost-driven dispatch, constraint selection, cancellation,
  and deliberately corrupted results including NaN and infinity.
- Feasible and infeasible zero current/apparent-power ratings, partial conductor
  limits, rotated source phasors and both-end shunts. A grounded terminal may
  carry nonzero current under a zero apparent-power limit; fixed nonzero voltage
  makes that same rating require zero current.
- Frozen BMOPFTools component and unbalanced-network solves, comparing objectives
  and complex physical voltages, currents, powers and taps with explicit
  per-fixture tolerances.
- Complete result export/import across delta devices, neutral returns,
  transformer rating ports, mixed windings, shunts and optimized taps. Tests
  reconstruct KCL and complex power from public result identities, preserve
  diagnostic/source information records, and reject incomplete or mismatched reports.
- Explicit initial-point identity/order and base conversion, partial and poor
  voltage starts, PF-to-OPF seeding, delta terminal power mapping, physical tap
  conventions, auxiliary consistency, and zero/singular equipment. Regressions
  check that seeds preserve objective/constraint evaluations, bounds and the
  physical fingerprint, survive canonical JSON round trips, and reject invalid
  or unsupported fields with structured diagnostics.
- Retained sessions versus fresh solves after load, availability, cost and limit
  edits, including zero-rating transitions, reordered tables and changed bases.
  Tests verify atomic batch rejection, current/historical result revisions,
  explicit declared-start selection, compiled-cache reuse, and preservation of
  the accepted primal across cancellation, numerical/physical-validation failure
  and infeasible edits.
- Opt-in dual reports, analytic load and generator-bound marginal signs/units,
  numerical-base and objective-scale invariance, and centered cost perturbations
  for lossy P/Q loads, delta coils, both-end branch ratings and voltage floors.
  Analytic KKT examples and electrical cases cover weak activity, dependent
  active rows, zero reductions and bounded rank audits without invalidating
  accepted physical solutions. Saved dual reports reject altered identities,
  multipliers, scaling, derived values and physical ledgers; version 1 reports
  without dual data remain readable. Session edits refresh duals with the
  accepted revision.
- Local forward/adjoint consistency and centered perturb-and-resolve checks for
  objective, voltage, dispatch and current/power ledgers under load and limit
  directions, including delta coils, neutral returns and unequal end shunts.
  Tests check physical-unit/base/order/objective-scale invariance, objective
  gradients against reported marginals, normalized-rating multiplier tangents,
  saved-result reconstruction, inactive/unsupported parameters, and refusal of
  unresolved active sets or nonunique dispatch. Analytic sparse-KKT tests reject
  singular/unstable systems, including a full-row-rank constraint Jacobian with
  a KKT null direction missed by fixed dense probes. Small matrix-only tests
  check shifted regular systems, reordered/scaled coordinates and inverse
  residual failures without optimization. Session tests and a compile-fail borrowing example
  guard current versus historical sensitivity access across edits and failures.
- Voltage-dependent nominal P/Q marginals and sensitivities for constant-current,
  constant-impedance, ZIP and exponential loads. Analytic rotated-voltage cases,
  centered perturbations of wye/delta networks and interior generator dispatch,
  and direct compiled residual/Jacobian parameter differences check units, signs
  and mixed derivatives. Coverage includes zero nominal P/Q across equation-form
  changes, per-coil coefficient/default handling, zero-voltage domain refusals,
  base/order/objective-scale invariance, session edits and verified legacy
  report upgrades. Metadata construction preserves the physical graph and bounds.
- Inverter availability and capability marginals and sensitivities: centered
  perturb-and-resolve comparisons cover active/inactive P/Q bounds, apparent and
  current ratings, single-phase return minima, four-leg neutral limits and delta
  coils. Control cases cover PMax/PAvailable/SMax references, per-phase/averaged
  monitors, cross-coil availability and deselected capability rows. Interior
  dispatch and direct residual/Jacobian parameter differences check mixed terms.
  Other regressions check physical units across bases/objective scales and bus
  order, inconsistent availability totals, ties and zero-rating refusals, session
  edits, version 4 round trips and verified version 2/3 upgrades with tamper rejection.
- Transformer port rating and physical tap-bound marginals and sensitivities.
  Centered perturbations check winding, winding-terminal and rating-port P/Q and
  currents, including exciting current, neutral returns, mixed connections and
  n-winding equipment. Direct/inverse regulator cases exercise both physical
  bound sides, interior taps and distinct open-delta leg identities. Coverage
  includes forward/adjoint agreement, cost envelopes, unit/base/scale invariance,
  session freshness, unsupported axes/fixed taps, zero-rating diagnostic refusal,
  unchanged physical graphs and version 5 reports with verified legacy upgrades.
- Shared inverter control-profile marginals and sensitivities. Centered solves
  check every volt-var/volt-watt breakpoint and endpoint fraction across all
  power references, leading/lagging power factors, multiple assigned inverters,
  delta/four-leg coils and averaged phase-ground/neutral/phase monitors. Direct
  residual/Jacobian differences at knots and saturated voltages check exact
  mixed derivatives, including smoothing-width changes and flat curves. Further
  cases check interior dispatch, forward/adjoint agreement, cost envelopes,
  units and numerical scaling, atomic profile edits, unity-PF/unused-profile
  refusals, and version 6 round trips with verified legacy upgrades.
- Sparse rank qualification agrees with the dense pivoted reference on small
  matrices and covers large diagonal and connected sparse cases, numerical
  dependence, rescaling/permutations, and explicit fill/work/nonfinite refusals.
  A known-rank triangular matrix completes within a budget that the former
  pivot order exhausts; a near-threshold case prevents a sparse sub-tolerance
  candidate from terminating the audit early. A generated 30-bus, two-wire
  radial feeder checks qualification, forward/adjoint agreement, cost marginals
  and linear residuals with bounded CI work.
  An opt-in connected 48-section feeder exceeds both old dense-audit limits and checks
  centered perturbations, forward/adjoint agreement, physical cost marginals,
  base/objective-scale invariance and backsolve diagnostics. Report tests verify
  legacy dense and sparse rank reports before upgrading and reject tampered audit
  claims; session tests preserve accepted results and enforce stale-view rules.

- PF correctness tests use bounded-coefficient algebra at both signs, unity,
  near-unity and tiny nonzero PF; original-profile validation checks wrong Q and
  explicit relative-accuracy diagnostics. A one-bus case requires P = 400 W and
  Q = -300 var at PF 0.8, with an analytic marginal checked by forward/adjoint
  queries and two perturbed solves (three solves total, 50-iteration ceilings).
  Retained edits invalidate sensitivities, and legacy PF reports cannot reuse
  multipliers from the old scaling.

### Feeder validation and runtime budgets

`mc_opf_feeder` uses one authored four-bus feeder with a delta-wye transformer,
two coupled four-wire lines, ZIP wye/delta loads and a controlled four-leg
inverter. Its recorded BMOPFTools reference checks rectangular voltages, line
currents and P/Q, losses, dispatch and original inverter bounds. The test then
checks a nominal-load sensitivity direction against centered solves, reuses one
factorization for forward/adjoint queries, and compares a warm demand/availability/
control edit with a fresh solve. It checks stale access and rejected-edit rollback.
The reference records the exact input hash, oracle revision, Julia/JuMP/Ipopt
versions, solver options, units and explicit tolerances. Objective comparison is
specific to this fixture and matching formulation, not a global-optimality claim.

The CI budget is **one fixture, five solves and one sensitivity factorization**,
with at most 100 iterations per solve. Structural guards keep it at four buses
and a KKT dimension below 600. Perturbed and fresh comparison solves skip dual
collection. A local unoptimized run took about 3.1 seconds;
elapsed time is printed for observation, not asserted against a machine-dependent
deadline. A separate generated **30-bus rank regression adds one solve and one
sensitivity factorization**, capped at 100 iterations, fewer than 1,000 variables,
five million rank work units and 50,000 peak coefficients. It took about 1.7
seconds locally in an unoptimized build (352 active rows, 3,120,827 work units,
13,986 peak coefficients). These structural/work guards are asserted; elapsed
time is only reported. Compact rank and version 6–9 verification/tamper tests
remain in CI. The two larger 48-section studies are ignored by default
and require an explicit command:

```sh
cargo test -p tellegen --no-default-features --features mc-opf --locked \
  --test mc_opf_feeder -- --nocapture
cargo test -p tellegen --no-default-features --features mc-opf --locked \
  --lib mc_opf::rank -- --nocapture
cargo test -p tellegen --no-default-features --features mc-opf --locked \
  --lib mc_opf::rank_tests -- --ignored --nocapture
python3 scripts/test_enwl_ivr_study.py
```

The public `mc_opf_workflow` example is also registered as a test target, so its
external-consumer API imports compile and its three contract tests run in the
ordinary `mc-opf` suite. It reuses the same four-bus fixture for **two solves and
one sensitivity factorization**, capped at 100 iterations per solve. A local
debug run took about 3.3 seconds. It checks physical results, edit invalidation,
warm starts, complete report persistence, forward/adjoint agreement with a local
marginal, and typed unavailable/stale diagnostics. Rejected edit rollback,
pre-cancelled attempts, unsupported report versions, declared input preservation,
lossy input diagnostics and selector/edit JSON contracts add no optimization.
There are no new fixtures, oracle runs or finite-difference sweeps. Run it alone:

```sh
cargo test -p tellegen --no-default-features --features mc-opf --locked \
  --example mc_opf_workflow -- --nocapture
```

Large external studies stay opt-in. Build the existing probe in release mode;
`--qualify` adds dual/rank details, KKT structure, view/query timings and one
objective forward/adjoint query using a single factorization, without extra
optimization. An unavailable derivative remains distinct from physical rejection.
In an opt-in check of ENWL LN snapshot `t12_1330`, the 30-bus case completed all
756 rank pivots using 62,133,130 work units and 87,612 peak coefficients; forward
and adjoint queries were available. The 99- and 538-bus cases still exhausted the
unchanged 100-million-work budget and explicitly refused qualification. This
is evidence for the selected snapshots, not a guarantee for every feeder size
or operating point. The runner records those refusals even when the physical
solve is accepted; its successful exit alone does not establish rank eligibility.
The runner records the executable hash and supports explicit case selection,
case limits and per-process timeouts. Relative paths in a case list resolve
against the list's directory. Duplicate output filenames are rejected; failed
reruns replace previous accepted records so comparisons cannot use stale data.

```sh
cargo build -p tellegen --release --example mc_opf_case_probe \
  --no-default-features --features mc-opf --locked
python3 scripts/enwl_ivr_study.py --cases /path/to/selected-cases.json \
  --binary target/release/examples/mc_opf_case_probe --output /tmp/feeder-study \
  --limit 2 --timeout 120 --qualify
```

The case list is a JSON array of BMOPF file paths. Existing `--case-root` ENWL
selection and independent oracle comparison remain available. No dataset or
Julia process is invoked by ordinary CI tests. Regenerate only the new feeder
reference with `validation/mc-opf/mc_opf_oracle.jl feeder` in the recorded oracle environment.

The inputs, reference JSON, attribution and license files in
`crates/tellegen/tests/data/mc_opf/` are regression assets. Their source information
records input SHA-256 hashes, oracle revision, units and numerical tolerances;
the component bundles also record Julia/JuMP/Ipopt versions and solver options.
Ordinary Rust tests consume the frozen results without Julia, network access
or an external feeder dataset. Intentional regeneration uses:

```sh
julia --project=/path/to/BMOPFTools.jl/test validation/mc-opf/mc_opf_oracle.jl
```

The script requires a clean BMOPFTools checkout at
`a8b52e069bfd4a7a57434c91bc0470ac03cfdd55`. Review changed inputs, hashes and
reference values together; do not relax tolerances to hide a changed electrical
problem. The [external study protocol](validation.md#methodology-external-multiconductor-ivr-studies)
adds larger-feeder comparisons without vendoring the dataset.


<a id="methodology"></a>

<a id="methodology-methodology"></a>

## Methodology

The `benchmarks` crate is a private workspace member that drives tellegen's
public API over the PGLib-OPF corpus for validation and timing. It never vendors
the corpus and reads it from `$PGLIB_OPF_PATH`, skipping when the path is absent.

<a id="methodology-corpus"></a>

### Corpus

PGLib-OPF v23.07 at `$PGLIB_OPF_PATH`: 66 base, 66 congested (`api/`), and 66
small angle (`sad/`) MATPOWER files, spanning a range of bus counts at
baseMVA 100. PGLib data is CC BY 4.0 (see [References](validation.md#references)).

<a id="methodology-what-is-driven"></a>

### What is driven

Per `(case, variant)`, the harness parses a PowerIO module and declares the
calculation as `DcOpfInstance`, `AcOpfInstance`, or `AcPfInstance`. It then
drives the same typed facade available to downstream consumers:

- DC OPF: `solve_instance` (objective, dispatch, LMPs);
- conic SOCWR: `solve_ac_instance` (objective, gap, W-space primals);
- AC power flow: `solve_ac_pf_instance` (convergence, residual);
- sensitivities: the same typed solve entries in forward and adjoint modes.

<a id="methodology-timing"></a>

### Timing

The engine exposes iteration traces and residuals rather than wall time, so the
harness times the public calls itself, per stage (parse / build / solve /
sensitivity). Solves run on one thread for clean timing.

<a id="methodology-metrics"></a>

### Metrics

1. **OPF correctness** vs the published reference: the DC objective, the SOCWR
   relaxation lower bound, and the SOC gap, rolled up into a per-case reproduction
   verdict (see [Validation](validation.md)).
2. **Sensitivity parity**: adjoint equals forward, and central finite differences
   against the analytic columns, classified per parity class (see
   [the sensitivity interface](sensitivity-interface.md)).
3. **Feasibility / convergence**: per-case status, interior-point iterations,
   residuals.
4. **Performance**: wall time per stage, and a scaling curve against bus count.
5. **Coverage**: a per-case status table, including the size caps applied by the
   harness flags.

<a id="methodology-baselines"></a>

### Baselines

Correctness is checked against two independent baselines:

- the published PGLib reference solves (PowerModels.jl with IPOPT), tabulated per
  case and variant in `$PGLIB_OPF_PATH/BASELINE.md`; and
- finite differences, which validate the analytic sensitivity columns by
  perturbing the public network fields.

<a id="methodology-reproducibility"></a>

### Reproducibility

The harness records the source revision, toolchain, dependency versions, host,
and invocation used for each run. It writes `results.json` (one record per
`(case, variant)`) and `results.csv`. With its book flag it also writes the
markdown snapshot to `docs/src/benchmark-results.md`. The repository keeps that
generated snapshot, and a rerun can be compared with its recorded source information:

```sh
cargo run -p benchmarks --release -- [flags]
```

<a id="methodology-external-multiconductor-ivr-studies"></a>

### External multiconductor IVR studies

The opt-in ENWL tools complement the small frozen
[MC regression fixtures](validation.md#multiconductor-ivr-opf). They read external
BMOPFDraftData ENWL LVNS snapshots: 30, 99 and 538 buses, LG/LN control variants,
and slots 01, 12 and 25 (18 cases). Reproduce the input selection with dataset
revision `52b8da905bffa41bcae8c1c5c2ce25fc54b2a424`; the inputs identify reduction
via PMD and generation through VVWO and declare CC BY 4.0. Keep the dataset and
generated study outputs outside the source tree.

Build with the published dependencies in the lockfile, then run fresh native and
external reference solves from the repository root:

```sh
cargo build -p tellegen --example mc_opf_case_probe --features mc-opf \
  --profile release-py --locked
python3 scripts/enwl_ivr_study.py \
  --case-root /path/to/BMOPFDraftData/benchmarks/ENWLsnapshots \
  --binary target/release-py/examples/mc_opf_case_probe \
  --output /path/to/study-output
julia --project=/path/to/BMOPFTools.jl/test validation/mc-opf/mc_opf_external_oracle.jl \
  /path/to/study-output/cases.json /path/to/study-output
python3 scripts/enwl_ivr_study.py --output /path/to/study-output --compare
```

If `CARGO_TARGET_DIR` is set, substitute its probe path for `target/...`.
The oracle requires clean BMOPFTools revision
`a8b52e069bfd4a7a57434c91bc0470ac03cfdd55`, a 1,000 VA base, `1e-9` Ipopt
tolerances, no bound relaxation or acceptable-iteration shortcut, and a
500-iteration cap. Native runs use 230 V / 1,000 VA bases, objective scale 1,
`1e-6` independent acceptance tolerance and the same iteration cap.

The comparator checks input hashes, objective, complex terminal voltages,
both-end line currents, inverter coil powers and original active-power bounds.
Checking the original bounds matters: a dropped scalar availability bound can
leave a physically different optimization problem with excellent solver
residuals. Historical dataset objective summaries use different conventions and
are not current golden files; compare explicit SI quantities and currency/hour
against fresh solves instead.

<a id="methodology-controlled-timing-and-profiling"></a>

#### Controlled timing and profiling

For repeated sequential comparisons on the six 538-bus cases:

```sh
OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 VECLIB_MAXIMUM_THREADS=1 \
RAYON_NUM_THREADS=1 JULIA_NUM_THREADS=1 \
julia --startup-file=no --project=/path/to/BMOPFTools.jl/test \
  validation/mc-opf/enwl_ivr_profile.jl /path/to/study-output/cases.json \
  /path/to/target/release-py/examples/mc_opf_case_probe /path/to/profile-output 5
python3 scripts/enwl_ivr_study.py --output /path/to/profile-output --compare
```

The harness warms each pair, randomizes case order with a fixed seed, alternates
native/Julia order, and defaults to five repetitions. Julia stays in one process;
each native invocation starts a new process, with process launch excluded from
its internal timer. File reading and final JSON output are excluded. Avoid
concurrent builds or benchmarks, and retain source revisions, input hashes,
dependency versions, host, thread settings and timing distributions with results.
These implementation-level measurements do not isolate linear-solver backends
on identical matrix sequences or establish general solver speed rankings.

`McOpfOptions.collect_profile` enables stage/KKT diagnostics and evaluation
counters; it is off by default. The probe's fifth argument enables profiling and
iteration capture. The harness collects detailed profiles separately from
unprofiled timing repetitions. Inspect factorization counts, fill, regularization
and iteration trajectories alongside derivative costs before choosing a
performance target. Timer scopes overlap: POUNCE's
`linear_system_factorization` includes the first back solve, while the backend
numeric-factorization counter measures factorization alone. Do not sum nested
timers or compare differently scoped native and Ipopt counters as backend speed
ratios. Current performance claims require a fresh run on the stated revisions.

<a id="references"></a>

<a id="references-references"></a>

## References

<a id="references-citing-tellegen"></a>

### Citing tellegen

- S. Talkington, F. Geth, Q. Zhang, L. Xie, and S. Liu, *Interactive Power Flow
  in the Browser*, The Inaugural ACM Conference on Digital Transformation (ACM
  DXConf 2026), Ann Arbor, MI, USA, October 2026. ACM, New York, NY, USA, 9 pages.
  [doi:10.1145/3847238.3849038](https://doi.org/10.1145/3847238.3849038),
  [arXiv:2610.01922](https://arxiv.org/abs/2610.01922). The repository's
  `CITATION.cff` and README carry the BibTeX entry.

<a id="references-pglib-opf"></a>

### PGLib-OPF

The benchmark corpus and the reference solves.

- **IEEE PES Power Grid Library — Optimal Power Flow**, v23.07.
  <https://github.com/power-grid-lib/pglib-opf>
- Archive report: S. Babaeinejadsarookolaee et al., *The Power Grid Library for
  Benchmarking AC Optimal Power Flow Algorithms*, arXiv:1908.02788.
- **License:** the PGLib data is **CC BY 4.0**; the software is MIT. The corpus is
  read from `$PGLIB_OPF_PATH` and never vendored. Per-file header attribution is
  preserved for any case quoted in these docs.
- Reference solves: `$PGLIB_OPF_PATH/BASELINE.md` (PowerModels.jl with IPOPT).

<a id="references-formulation-references"></a>

### Formulation references

- **Jabr SOCWR**: R. A. Jabr, *Radial distribution load flow using conic
  programming*, IEEE Transactions on Power Systems, 21(3), 2006. The paper gives
  the W-space second-order cone relaxation, also implemented in
  [PowerModels.jl](https://github.com/lanl-ansi/PowerModels.jl) (BSD-3-Clause); the
  conic implementation in tellegen is independent.

<a id="references-solvers-and-linear-algebra"></a>

### Solvers and linear algebra

- [Clarabel.rs](https://github.com/oxfordcontrol/Clarabel.rs): the convex conic
  and quadratic program solver.
- [faer](https://docs.rs/faer/latest/faer/): the dense linear algebra used by the
  sensitivity driver.
