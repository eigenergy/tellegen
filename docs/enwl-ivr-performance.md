# ENWL IVR performance investigation — 2026-10-06

Controlled repeated runs identify KKT linear algebra and convergence as the main
native costs. They do **not** support the apparent 1.6× aggregate native speedup
from the initial single-run study: across the six 538-bus cases, native elapsed
time totals 21.128 s versus BMOPFTools' 17.143 s over five repetitions, making
native about **23% slower in aggregate** under these settings. It wins four of
the six cases; the two midday cases reverse the aggregate result.

[Machine-readable evidence](evidence/enwl-ivr-performance-2026-10-06.json) retains
all timing samples, versions, input hashes, solver counters, physical comparisons
and the difficult case's complete iteration traces. These are six snapshots of
one feeder size, not a general ranking of the solvers.

## Protocol

- Same unchanged six 538-bus ENWL inputs as the correctness study, 1,000 VA base,
  tight tolerances and 500-iteration limit. No numerical defaults were changed in
  Tellegen. Native build: `release-py`, optimization level 3, thin LTO, macOS arm64.
- POUNCE/Feral versus BMOPFTools/JuMP/Ipopt 3.14.19 with MUMPS 5.9.0. The pinned
  BMOPFTools source is clean; Ipopt explicitly selects `linear_solver=mumps`.
- One thread requested through Julia, BLAS, OpenMP, Accelerate and Rayon settings.
  Six warmup pairs precede five measured pairs per case. Case order is shuffled
  with seed 20261006, and native/Julia order alternates between repetitions.
  Runs are sequential; no build or other benchmark is run concurrently by the
  harness. This is a normal desktop, without CPU affinity or frequency control.
- Julia stays in one process. Each native invocation starts a fresh process;
  its internal timer excludes process launch. Both timers exclude input file
  reading and final JSON encoding/writing. Native also excludes input hashing;
  the Julia hash is likewise computed after its elapsed time is captured.
- `GC.gc()` runs before each pair, outside the timer, so previous Julia solves
  and harness JSON allocations do not leave collection debt for the next pair.
  These timings describe warmed, controlled solves, not cold startup throughput.
- Detailed native timers and both iteration traces are captured in a separate
  profiling pass, after the unprofiled repeats. Profile timings must not be
  substituted for the unprofiled medians or used to estimate instrumentation
  overhead. The first Julia profiling invocation additionally compiles its
  callback; the hard-case callback has already been warmed.

## Repeated elapsed times

Seconds, median of five; parentheses give the observed minimum–maximum.

| Case | Tellegen | BMOPFTools + Ipopt/MUMPS | Iterations, native / Ipopt |
| --- | ---: | ---: | ---: |
| LG 08:00 | 0.278 (0.273–0.283) | 0.509 (0.489–0.522) | 5 / 18 |
| LG 13:30 | 0.766 (0.753–0.799) | 0.592 (0.571–0.603) | 31 / 26 |
| LG 20:00 | 0.400 (0.389–0.402) | 0.522 (0.515–0.525) | 9 / 18 |
| LN 08:00 | 0.278 (0.268–0.280) | 0.502 (0.485–0.512) | 5 / 18 |
| LN 13:30 | 2.080 (2.005–2.096) | 0.795 (0.772–0.798) | 68 / 37 |
| LN 20:00 | 0.436 (0.429–0.448) | 0.529 (0.507–0.539) | 11 / 19 |

Iteration counts are identical in all five repetitions for each implementation.
These are implementation-level comparisons: the variable/constraint layouts,
initialization and internal scaling are not forced identical. They do not isolate
Feral versus MUMPS on an identical matrix sequence.

## What the native profile establishes

The separate LN 13:30 profile takes 1.948 s inside `optimize_tnlp`:

| Measurement | Seconds | Fraction of native solve |
| --- | ---: | ---: |
| Backend numeric factorization | 1.251 | 64.2% |
| Factorization plus its first back solve | 1.478 | 75.9% |
| Additional back solves | 0.198 | 10.2% |
| Jacobian evaluation | 0.045 | 2.3% |
| Hessian evaluation | 0.051 | 2.6% |
| All function/derivative evaluations | 0.138 | 7.1% |

Rows overlap. In particular, numeric factorization is contained in the next
row, and Jacobian/Hessian evaluation is contained in all evaluations. POUNCE's
`linear_system_factorization` timer wraps factorization **and the first back
solve**, as documented in `std_aug_system_solver.rs`; it is not pure factor time.
The backend's separate numeric-factor counter provides the first row.

The backend reports **99 factorizations**, **98 symbolic-pattern reuses**, one
initial pattern, AMF ordering, final matrix/factor nonzeros of 101,762 / 726,043,
and maximum fill ratio 7.26. Thus repeated symbolic analysis is not the problem.
The numeric work and the number of factorization attempts are useful targets.
The fill ratio and delayed-column counts are evidence to compare during ordering
experiments, not proof that the ordering is defective.

Across all six profiles, pure numeric factorization accounts for roughly 63–69%
of solve time. On the hard case, parsing plus preparation, fingerprinting, model
construction and evaluator setup take about 0.062 s; physical validation and
solution projection together take about 0.0006 s. These are secondary costs.

Jacobian plus Hessian evaluation is about 5% on the hard case. Even eliminating
that entire cost would yield only about a 1.05× solve speedup. This evidence
supports retaining the current exact polynomial derivatives plus sparse AD for
nonpolynomial rows while targeting the KKT work and convergence path first.

## What the trajectories establish

For LN 13:30, POUNCE takes 68 iterations and Ipopt takes 37. Both start with
primal infeasibility approximately 2.644 and zero objective, but their initial
dual infeasibilities differ; this is not evidence of identical complete starts.

| Trajectory observation | POUNCE | Ipopt |
| --- | ---: | ---: |
| First reduction from initial barrier parameter 0.1 | iteration 44 | iteration 13 |
| Iterations with nonzero Hessian regularization | 16 | 5 |
| Total line-search trials | 68 | 37 |
| Iterations with more than one line-search trial | 0 | 0 |

POUNCE has no restoration calls or linear-solver quality escalations in this
case. The extra iterations accompany small accepted steps and regularization;
they do not come from repeated line-search backtracking. These observations
localize the convergence problem but do not establish its root cause.

A separate three-run ablation disables Ipopt's default gradient scaling. It
still takes **37 iterations** on every run and agrees on the objective within
1e-4. Consequently, that default setting difference alone does not explain the
68-versus-37 gap. It does not rule out benefits from improving native scaling.

Ipopt's printed MUMPS timing block reports zero in `LinearSystemFactorization`
while substantial time appears in the aggregate augmented-system timer. Do not
compare that zero with POUNCE's factor timer or infer that MUMPS factorization
is free. The current evidence is insufficient for a factorization-only speed
ratio between the two backends.

## BMOPFTools stages

Averaging the per-case median stages gives approximately 0.008 s parsing,
0.125 s preparation/model construction/KCL stamping, 0.381 s in
`JuMP.optimize!`, and 0.060 s postprocessing. The optimize timer includes MOI
transfer and setup as well as Ipopt; the separate reported Ipopt solve timer is
also retained in the evidence. Under these warmed conditions, roughly one-third
of elapsed time lies outside `JuMP.optimize!`, rather than the earlier
single-run estimate of 62% outside the reported optimizer time.

## Correctness and implementation checks

Every timed native solve passed its independent physical acceptance gate; every
reference returned `LOCALLY_SOLVED` and feasible. Every timed pair agreed on
objective within 1e-4. The full physical comparator was also rerun on all six
profiled pairs, checking original input inverter bounds and complex physical
quantities. Maximum differences: objective 7.84e-11 currency/hour, voltage
2.32e-10 V, line current 2.06e-9 A, and inverter power 8.82e-8 VA. Maximum native
normalized residual was 1.38e-11. No tolerances were relaxed.

The opt-in `McOpfOptions.collect_profile` returns `McOpfResult.profile` with
stage timers, solver timers, evaluation counts, backend counters and final
optimality residuals. It is disabled by default. The probe's optional fifth
argument enables profiling and iteration capture. A regression verifies that
profiling preserves the solution and iteration count. The default-feature plus
MC OPF suite passed 395 tests (3 existing ignored); strict all-target Clippy
passed. No solver settings, derivatives or electrical equations were changed.

## Reproduce and next experiments

Build the probe against the companion PowerIO branch using the isolated archive
procedure in [the correctness study](enwl-ivr-study.md). Then run:

```sh
OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 VECLIB_MAXIMUM_THREADS=1 \
RAYON_NUM_THREADS=1 JULIA_NUM_THREADS=1 \
julia --startup-file=no --project=/path/to/BMOPFTools.jl/test \
  scripts/enwl_ivr_profile.jl /path/to/original-study/cases.json \
  /path/to/release-py/examples/mc_opf_case_probe /path/to/profile-output 5
python3 scripts/enwl_ivr_study.py --output /path/to/profile-output --compare
```

Implementation commit: `88e4f3c`. Raw outputs for this run are in
`/private/tmp/enwl-ivr-profile-final`; committed evidence survives temporary-file
cleanup. Input files remain external and unchanged.

The next controlled experiments should (1) drive both algorithms with the same
compiled NLP and explicit primal/dual start, (2) compare factorization counts,
regularization and ordering on identical KKT matrices, and (3) evaluate improved
initialization/scaling across all six cases and the component regression suite.
Do not change numerical defaults from one successful hard-case experiment, and
do not redesign AD to address a cost that these measurements show is secondary.
