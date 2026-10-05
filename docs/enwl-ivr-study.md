# ENWL IVR snapshot study — 2026-10-06

All 18 selected snapshots passed against fresh, tightly solved BMOPFTools IVR
references after fixing a scalar-bound conversion defect. Cases cover 30, 99,
and 538 buses; both LG and LN voltage-control variants; and snapshots at 08:00,
13:30 and 20:00. Each 538-bus input has 2,152 terminals, 537 lines, 302 loads and
302 single-phase inverters. Inputs were used unchanged from the external dataset;
no feeder fixtures are vendored. [Machine-readable evidence](evidence/enwl-ivr-2026-10-06.json)
records hashes, versions, options, tolerances and each comparison.

## Defect exposed and fixed

The BMOPF reader accepted array-valued IBR bounds but silently dropped scalar
`p_min`, `p_max`, `q_min` and `q_max`. ENWL expresses single-phase bounds as
scalars. A morning `p_max: 0` became an absent bound, permitting PV exports with
no available solar power. For example, 538bus_LG at 08:00 initially reported
-999.054650 currency/hour; the corrected and reference objective is
+166.155568 currency/hour. The initial answer satisfied the incorrectly prepared
model, demonstrating why solver residuals alone are insufficient.

The converter now retains a scalar as a one-entry vector, matching BMOPFTools'
single-coil convention; it does not broadcast a scalar across phases. A converter
regression checks all capability fields, zero availability, malformed values and
round trips. A small Tellegen regression solves scalar availability of 0 W and
50 W and locks the resulting dispatch and source cost. The external comparison
also checks original input active bounds independently of preparation. As a
negative control, the same comparator rejects all 18 pre-fix results.

## Numerical evidence

Across all 18 cases, maximum differences from the fresh BMOPFTools reference:

| Quantity | Observed maximum | Acceptance tolerance |
| --- | ---: | ---: |
| Objective, currency/hour | 7.350e-08 | 1e-4 |
| Complex terminal voltage, V | 4.478e-07 | 1e-3 |
| Complex line current at both ends, A | 4.411e-07 | 1e-2 |
| Complex inverter coil power, VA | 3.507e-05 | 0.1 |
| Original active-bound violation, W | 3.851e-08 | 1e-4 |

The maximum independently recomputed normalized native residual was
9.551e-11, below the unchanged
1e-6 acceptance threshold. All reference solves returned `LOCALLY_SOLVED` and
reported feasible. No numerical reference values or existing unit-test tolerances
were relaxed.

| 538-bus case | Native iterations | Native elapsed, s | Objective, currency/hour |
| --- | ---: | ---: | ---: |
| 538bus_LG_t01_0800 | 5 | 0.247 | 166.155567851 |
| 538bus_LG_t12_1330 | 31 | 0.614 | -1024.239345662 |
| 538bus_LG_t25_2000 | 9 | 0.340 | 237.282866408 |
| 538bus_LN_t01_0800 | 5 | 0.250 | 165.882507917 |
| 538bus_LN_t12_1330 | 68 | 1.762 | -959.812497104 |
| 538bus_LN_t25_2000 | 11 | 0.400 | 237.388516744 |

The later [controlled performance study](enwl-ivr-performance.md) supersedes
these single-run timings for speed comparisons. Its repeated runs show that the
earlier apparent aggregate native speedup does not hold under controlled settings.

Native timing uses `release-py` (optimization level 3, thin LTO) on macOS arm64.
It includes parse/preparation/model compilation/solve/validation/result projection,
not the Rust build. These are single observations, not a comparative performance
benchmark. Julia's first oracle case includes JIT compilation. Maximum resident
memory and repeated-run timing distributions were not measured.

The old dataset reports use historical objective units/conventions and looser
solver tolerances. They were not treated as current numerical golden files.
Fresh BMOPFTools uses input cost in currency/kWh and reports currency/hour;
SI/per-unit conversions and full complex physical results were compared explicitly.

## Reproduce

Use the companion PowerIO API branch listed in the evidence. Build the optional
`mc_opf_case_probe` example in a temporary Tellegen archive with all six PowerIO
crates patched, following `scripts/check-mc-opf.sh`'s dependency-isolation method:

```sh
# Run only in the temporary archive; update all six patches consistently.
cargo update -p powerio -p powerio-core -p powerio-dist -p powerio-prob \
  -p powerio-matrix -p powerio-tx --config /path/to/temporary-powerio-patches.toml
cargo build -p tellegen --example mc_opf_case_probe --features mc-opf \
  --profile release-py --config /path/to/temporary-powerio-patches.toml
python3 scripts/enwl_ivr_study.py \
  --case-root /path/to/BMOPFDraftData/benchmarks/ENWLsnapshots \
  --binary /path/to/target/release-py/examples/mc_opf_case_probe \
  --output /path/to/study-output
julia --project=/path/to/BMOPFTools.jl/test scripts/mc_opf_external_oracle.jl \
  /path/to/study-output/cases.json /path/to/study-output
python3 scripts/enwl_ivr_study.py --output /path/to/study-output --compare
```

The oracle runner requires clean BMOPFTools commit
`a8b52e069bfd4a7a57434c91bc0470ac03cfdd55`. It uses a 1,000 VA base, 1e-9
Ipopt tolerances, no bound relaxation, no acceptable-iteration shortcut, and a
500-iteration cap. The native runs use their default 230 V / 1,000 VA bases,
objective scale 1, 1e-6 independent acceptance tolerance and a 500-iteration cap.

Dataset provenance: BMOPFDraftData commit
`52b8da905bffa41bcae8c1c5c2ce25fc54b2a424`; inputs identify ENWL LVNS data reduced
via PMD and generated through VVWO, and declare CC BY 4.0. The benchmark scripts
read that local dataset; the committed evidence contains derived measurements
and input hashes only. Full raw outputs for this run are in
`/private/tmp/enwl-ivr-study` and may be removed by temporary-directory cleanup.

## Regression checks after the fix

PowerIO's complete Rust workspace passed 2,268 tests (3 existing ignored),
including C ABI and conversion compatibility. The full feature/binding Clippy
matrix passed on both updated PowerIO branches independently. Tellegen passed
394 tests with default features plus MC OPF and 175 with only MC OPF, including
all 64 existing frozen solves and the new scalar availability regression; strict
all-target Clippy passed. Shipping lockfiles and original checkouts were unchanged.

## Remaining work

This strengthens correctness evidence through 538 buses. It does not establish
global optimality, cover all 150 snapshots, or prove memory/scaling behavior on
larger networks. The observed import/export and voltage-control cases complement
the 64 small frozen component solves; they do not replace them.

Publication still requires approval for the two PowerIO PRs, review and release
of the preparation API, then Tellegen's released dependency update and optional
MC CI gate. POUNCE release/restoration/distribution gates, browser integration,
retained solve sessions, solution sensitivities, unsupported physics/profile
extensions, and larger performance/memory studies remain separate work.
