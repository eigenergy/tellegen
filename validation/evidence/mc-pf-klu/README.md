# Reproduced faer/KLU comparison

The original Austin P3U discrepancy in #153 is reproduced: **17.473052 V**
between faer and OpenDSS's default KLU on the same 431,897-node matrix and RHS.
KLU reproduces live OpenDSS exactly. Its condition estimate is 2.253e16.
The difference is a shift in the voltage to ground of two delta-bounded 69 kV
sections; KLU and faer are both volts away from the exact solution of the
matrix there (see "Where the 17 V comes from").
No production solver behavior or backend selection changes in this study.

## What was measured

The small Tellegen cases use `PreparedNetwork`'s actual passive matrix,
compensation reference, unknown ordering and fixed-terminal elimination RHS.
The exporter verifies its reconstructed matrix against the retained factor.
Three generated two-conductor trees have 200, 2,000 and 20,000 unknowns.
Two small OpenDSS cases export its full sparse Y and current RHS after PF.
The largest faer/KLU difference in those five cases is below 6e-10 V.

A further generated tree uses **316,000 unknowns and 1,895,984 nonzeros**
through the same actual preparation path (`large-reduced-results.json`).
Its faer/KLU difference is 6.031e-9 V (neutral maximum 3.490e-9 V), with
backward residuals below 3.4e-16. KLU COLAMD differs from AMD by 1.337e-10 V.
Scaling, reversal and both refinement methods remain below 6.5e-9 V from
the reference. This confirms agreement on a substantial Tellegen reduced
system; the generated two-conductor tree does not reproduce regional
transformers, controls or the original matrix conditioning.

The original regional source was recovered from the public
[OEDI dataset](https://data.openei.org/submissions/2981), using the exact
Austin P3U prefix recorded in #132's October 1 discussion. The 2,273 files
contain 151,665,314 bytes. `smartds-source-files.jsonl` records their paths,
sizes and SHA-256 hashes; the large source/matrix files are not vendored.
`run_smartds.py` compiles the unmodified region Master, lets its initial
controls finish, freezes controls, solves once more and exports Y/current RHS.

`smartds-aus-p3u-results.json` records the full and source-eliminated systems.
The full system has 2,427,669 nonzeros. Findings:

- The worst absolute voltage differences occur at energized 69 kV buses.
  They are mostly a common shift of all three phase voltages; the largest
  within-bus phase-to-phase discrepancy is 0.015041 V. The 294 nodes whose
  KLU voltage is below 1e-6 V have zero discrepancy in this run.
- KLU COLAMD differs by 23.922 V from default KLU AMD. Row/column scaling and
  reversing input order also alter the answer. This is not evidence that
  changing the backend or ordering alone fixes the original issue.
- Three ordinary double-precision refinement steps reduce the backward
  residual but increase the difference from KLU to 27.546 V.
- Eight refinement steps with double-double residual accumulation (`faer`'s
  `fx128`, backed by `qd`) converge from both faer's and KLU's initial answers
  to the **same final float64 vector**. Corrections fall to about 1.3e-11 V.
  This vector remains 17.462 V from the agreed KLU reference. It is evidence
  about the rounded input matrix, not a certificate of physical model accuracy.
- Eliminating the three `ST_MAT` source terminals from the exported OpenDSS
  matrix leaves a 17.470 V faer/KLU difference and the same condition estimate.
  This experiment isolates source elimination; it is **not** Tellegen's own
  regional reduced matrix.

Actual Tellegen regional import was attempted without dropping diagnostics.
PowerIO 0.12.1 refuses it at its 64 MiB include budget and line-history work
budget (`tellegen-regional-import.json`). Consequently #153's regional
Tellegen-preparation question remains open. No import limits were bypassed.
The public `ParseOptions` API exposes format and acquisition-root selection,
not OpenDSS resource-limit overrides; these limits are private constants.
Two official nested inputs were also attempted unchanged: substation
`p3uhs18_1247/Master.dss` fits within the size budget but is refused for duplicate
Linecode declarations and regulator diagnostics; its `p3udt32182` feeder fits
but retains unsupported fuse, capcontrol and line objects. The new fixed-control
conversion handles regulators and supported capacitors, not protection devices
or duplicate definitions. `tellegen-subset-imports.json` records both attempts.
No source declarations or protection states were silently discarded.

### PowerIO 0.12.2 follow-up

The same regional, substation, and feeder imports were rechecked against PowerIO
0.12.2 release source (merged commit `6794629d0e8617a7c73b9a55ca3cbafddd19df6f`, tree
`1657774bad40ce6f39eea0025756f5b9a972222e`) using local Cargo patches, with all 2,273
downloaded files verified against the original source manifest and the default import
limits unchanged. None reached Tellegen matrix preparation. The regional parser again
reported the 4,096-file/64 MiB include-budget refusal at Master.dss line 483 and the
line-calculation-history work-budget refusal at line 572; its diagnostics also
include duplicate definitions. The substation still fails validation on duplicate
Linecode definitions; the earlier regulator diagnostic is no longer part of that
refusal. The feeder still retains an unsupported padswitch line, fuses, and capacitor
controls. No records were discarded or diagnostics bypassed. See
`tellegen-imports-powerio-0.12.2.json`; the separate 0.12.1 evidence remains
unchanged. These checks confirm that the original regional reduced-system question
remains unresolved; they are a source-candidate test, not a registry-package test.

## Where the 17 V comes from

`export_gap_case.py` and `explain_gap.py` locate the disagreement and test an
explanation; `smartds-aus-p3u-gap-results.json` holds every number below.
"Exact" means the float64 vector reached by refinement with residuals summed
without rounding error (error-free products and `math.fsum`). It matches faer's
`fx128`-refined vector to 7.3e-12 V. It is the solution of the exported
float64 matrix with OpenDSS's current vector held fixed.

**Where.** The disagreement is confined to the 657 nodes (219 buses) of the
69 kV level. They form two sections of 315 and 342 nodes that are not connected
to each other at 69 kV. Every transformer winding on them is a delta: 5
delta 230 kV / delta 69 kV 300 MVA transformers above and 33 delta 69 kV /
wye 12.47 kV substation transformers below. Raising all nodes of one section
by the same voltage therefore produces no current at any node outside it (the
matrix gives exactly 0 S), and the right-hand side is zero on both sections.

**What ties each section to ground.** The 221 lines (78.6 km and 68.8 km)
carry capacitance to ground: j8.677e-5 S and j8.705e-5 S in total per section.
`ppm_antifloat` (1 ppm on every winding) contributes −j2.9e-7 S and −j3.7e-7 S,
under 0.5% of that. The same sections contain 12 lines (linecode
`fuse_1_69kv_3`, length 0.001) whose matrix entries are 7.07e7 S. One unit in
the last place of such an entry is 7.45e-9 S; at 39.5 kV that is 2.9e-4 A,
which the section's ground path turns into a 3.4 V shift of the whole section.

**What the matrix says.** In the exact solution the mean voltage to ground of
each section is 2.8e-7 V and 2.2e-6 V: the three phase voltages sum to zero,
as the balanced capacitances require. The float64 LU answers do not reproduce
this. Mean voltage to ground of each section (magnitude, volts):

| solution | section of 315 nodes | section of 342 nodes |
| --- | --- | --- |
| exact | 2.8e-7 | 2.2e-6 |
| OpenDSS KLU | 2.719 | 17.458 |
| faer | 7.475 | 0.235 |
| SuperLU, COLAMD | 11.663 | 1.872 |
| SuperLU, MMD(Aᵀ+A) | 14.526 | 11.446 |

For every solver this value equals the net current its solution sends to
ground through the section divided by the section's admittance to ground
(KLU: 1.52e-3 A, about five units in the last place of a 7.07e7 S entry).

**Shape of the difference.** faer minus KLU is a common shift of 6.003 V on
the first section and 17.467 V on the second; the shifts hold more than
0.9999998 of the squared norm of the difference there. After removing them the
largest remainder is 0.0109 V (RMS 0.0043 V) and the largest difference
between two directly coupled 69 kV nodes is 0.0150 V. Outside the two sections
the largest difference is 1.86 mV; 3,451 nodes exceed 1e-3 V. Across the 259
lines and transformers at 69 kV, conductor currents differ by at most 2.7e-3 A
(largest current 2,421.5 A) and terminal powers by at most 283 VA (largest
235.5 MVA). The 17.47 V maximum is therefore KLU's 17.46 V error on the second
section; on the first section faer is the farther of the two.

**Sensitivity.** Moving the real and imaginary parts of the 144 entries above
1e7 S by −1, 0 or +1 unit in the last place, independently, and solving
exactly, shifts the sections by 13.4 V to 57.4 V over five draws. Doing the
same to the other 2,427,525 entries shifts them by at most 1.7e-3 V. The
exported matrix itself is not free of such rounding: its rows leave up to
4.6e-9 S to ground that the element data do not contain. But that leftover
differs by at most 1e-15 S between the three phases of any bus, so it
multiplies voltages that sum to zero, and its estimated effect on the exact
solution is below 3e-6 V.

**Singular values.** The 2.253e16 condition estimate does not describe these
sections. The smallest singular value of the full matrix, 6.28e-9 S, belongs
to 48 pieces (294 nodes) with no connection to the source; their right-hand
side is zero and every solver returns exactly 0 V there. Largest singular
value 1.414e8 S over 6.28e-9 S gives 2.253e16. In the connected piece the two
smallest singular values are 2.546e-7 S and 2.754e-7 S and the next is
4.04e-4 S. The vectors equal to one on a section and zero elsewhere lie in the
span of the two smallest singular vectors to within 1e-10, and their Rayleigh
quotients (2.749e-7 S and 2.542e-7 S) are the sections' admittance to ground
divided by their node counts. That ratio is 5.6e14.

**Changing `ppm_antifloat` in OpenDSS.** Scaling it by 0.1, 10 and 100 on the
38 transformers and re-solving moves KLU's mean section voltage by 3.6 V and
7.0 V, 15.3 V and 28.6 V, 23.7 V and 41.9 V. The exact solution's mean section
voltage stays below 4e-6 V in all three. The remaining voltage changes agree
between KLU and exact (0.0099 V, 0.092 V and 1.01 V at 69 kV) and grow in
proportion to the added shunt, which sits on the wye windings too.

**What this supports.** The matrix does fix the voltage to ground of both
69 kV sections, at a balanced value, but only through an admittance 1.2e-12
times the largest entries on the same nodes. No float64 LU answer measured
here resolves it: KLU is off by 2.7 V and 17.5 V, faer by 7.5 V and 0.2 V.
Neither is the more accurate overall, so KLU is not a reference for voltages
to ground on these sections. Voltages between conductors, currents and powers
agree to the figures above, and refinement with exactly summed residuals
recovers the balanced solution from either start in four or five steps.

**Limits.** The shift of each solver was not traced to individual elimination
steps; only its size is accounted for. The remainders of up to 0.011 V at
69 kV and 1.9 mV elsewhere were measured, not explained. The exact solution
holds OpenDSS's compensation currents fixed, so it is not a re-converged power
flow. Only this region was examined, and Tellegen's own regional matrix is
still unavailable (below).

## Timing and memory

Machine: Apple M5 Pro, 64 GiB RAM. Rust 1.96.0, faer 0.24.4,
`release-py` (optimization 3, thin LTO). OpenDSSDirect.py 0.9.4 supplies
DSS C-API 0.14.5 and the KLU library; its version/hash are recorded.
The library's build flags were not rebuilt to match Rust's. These samples
ran alongside development work and are not controlled speed comparisons.

The regional faer factor takes about 0.23 s and a retained solve about 5.4 ms.
Default KLU takes about 0.10 s and 5.3 ms. Timings exclude JSON decoding,
input construction and the KLU wrapper's matrix verification. Warm timings
average 20 solves after one warm-up. The C ordering harness includes the
RHS workspace copy, as the Rust retained solver does.

The regional retained allocation is about 204 MB for faer (macOS allocator
delta around the actual factor/workspace) and 112 MB for KLU AMD (KLU's own
allocation accounting, including symbolic/numeric workspaces). KLU reports
154 MB peak allocations for AMD and 1,046 MB for COLAMD; COLAMD retains
127 MB. The allocator methods differ and neither is full process RSS.
The study does not claim a comparable peak-heap measurement for faer.

For the generated 316,000-unknown prepared system, faer factorization takes
0.113 s and a retained solve 3.73 ms; KLU AMD takes 0.042 s and 3.01 ms.
Retained allocations are 146 MB for faer and 78 MB for KLU AMD, which reports
91 MB peak; COLAMD reports 599 MB peak. The same timing and allocator
limitations above apply. The entire scripted comparison, including matrix
construction, serialization, scaling and alternative solves, took 62 s.

## Reproduction

Install numpy and OpenDSSDirect.py 0.9.4 in an isolated Python environment.
Use the library distributed with `dss_python_backend`, then build the small
C measurement helper against the published SuiteSparse 5.6.0 headers:

```sh
bash build_klu_metrics.sh /path/to/libklusolvex.dylib /tmp/klu-headers /tmp/klu-metrics.dylib
cargo test -p tellegen --features mc-pf --lib --profile release-py --no-run
python compare.py /path/to/the/printed/test-executable /path/to/libklusolvex.dylib /tmp/klu-metrics.dylib > results.json
python run_reduced.py /path/to/test-executable /path/to/libklusolvex.dylib /tmp/klu-metrics.dylib /tmp/large-reduced 158000
python recover_smartds.py /tmp/smartds-aus-p3u
python run_smartds.py /path/to/test-executable /path/to/libklusolvex.dylib /tmp/klu-metrics.dylib /tmp/smartds-aus-p3u /tmp/smartds-results
python export_gap_case.py /tmp/smartds-aus-p3u /tmp/smartds-gap
python explain_gap.py /tmp/smartds-gap /path/to/test-executable > smartds-aus-p3u-gap-results.json
```

`run_smartds.py` retains a roughly 144 MiB matrix JSON plus its result summary
in the chosen output directory. The two gap scripts also need scipy; the export
keeps about 135 MB of matrices, and the test executable is optional (without
it the faer rows are omitted). The Rust exporter compiles only for tests;
its ignored test requires an explicit output path. `TELLEGEN_LINEAR_DSS`
selects an actual DSS import attempt; `TELLEGEN_LINEAR_INPUT` selects an
already exported matrix. Neither changes normal engine behavior.

The KLU wrapper reads back and exactly verifies its assembled matrix before
solving. The direct C harness passes the identical CSC arrays to KLU and
checks AMD/COLAMD independently. Existing public KLU ABI functions are used;
no production dependency on KLU or new solver API is introduced.

## Remaining limits

KLU remains the agreed reference except for voltages to ground on the two
69 kV sections described above, where it is 17.46 V from the exact solution
of its own matrix. There is no supported regional Tellegen
import/preparation result, interval-validated solution, or demonstrated need
for a WASM backend change. The other original regions were located in the
same public data tree but were not remeasured here. This is a reproducible
investigation of #153, not grounds to close its unresolved regional question.
