# Methodology

The `benchmarks` crate is a private workspace member that drives tellegen's
public API over the PGLib-OPF corpus for validation and timing. It never vendors
the corpus and reads it from `$PGLIB_OPF_PATH`, skipping when the path is absent.

## Corpus

PGLib-OPF v23.07 at `$PGLIB_OPF_PATH`: 66 base, 66 congested (`api/`), and 66
small angle (`sad/`) MATPOWER files, spanning a range of bus counts at
baseMVA 100. PGLib data is CC BY 4.0 (see [References](references.md)).

## What is driven

Per `(case, variant)`, the harness parses a PowerIO module and declares the
calculation as `DcOpfInstance`, `AcOpfInstance`, or `AcPfInstance`. It then
drives the same typed facade available to downstream consumers:

- DC OPF: `solve_instance` (objective, dispatch, LMPs);
- conic SOCWR: `solve_ac_instance` (objective, gap, W-space primals);
- AC power flow: `solve_ac_pf_instance` (convergence, residual);
- sensitivities: the same typed solve entries in forward and adjoint modes.

## Timing

The engine exposes iteration traces and residuals rather than wall time, so the
harness times the public calls itself, per stage (parse / build / solve /
sensitivity). Solves run on one thread for clean timing.

## Metrics

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

## Baselines

Correctness is checked against two independent baselines:

- the published PGLib reference solves (PowerModels.jl with IPOPT), tabulated per
  case and variant in `$PGLIB_OPF_PATH/BASELINE.md`; and
- finite differences, which validate the analytic sensitivity columns by
  perturbing the public network fields.

## Reproducibility

The harness records the source revision, toolchain, dependency versions, host,
and invocation used for each run. It writes `results.json` (one record per
`(case, variant)`) and `results.csv`. With its book flag it also writes the
markdown snapshot to `docs/src/benchmark-results.md`. The repository keeps that
generated snapshot, and a rerun can be compared with its recorded provenance:

```sh
cargo run -p benchmarks --release -- [flags]
```

## External multiconductor IVR studies

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
julia --project=/path/to/BMOPFTools.jl/test scripts/mc_opf_external_oracle.jl \
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

### Controlled timing and profiling

For repeated sequential comparisons on the six 538-bus cases:

```sh
OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 VECLIB_MAXIMUM_THREADS=1 \
RAYON_NUM_THREADS=1 JULIA_NUM_THREADS=1 \
julia --startup-file=no --project=/path/to/BMOPFTools.jl/test \
  scripts/enwl_ivr_profile.jl /path/to/study-output/cases.json \
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
