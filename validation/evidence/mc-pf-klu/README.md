# Reproduced faer/KLU comparison

The original Austin P3U discrepancy in #153 is reproduced: **17.473052 V**
between faer and OpenDSS's default KLU on the same 431,897-node matrix and RHS.
KLU reproduces live OpenDSS exactly. Its condition estimate is 2.253e16.
No production solver behavior or backend selection changes in this study.

## What was measured

The small Tellegen cases use `PreparedNetwork`'s actual passive matrix,
compensation reference, unknown ordering and fixed-terminal elimination RHS.
The exporter verifies its reconstructed matrix against the retained factor.
Three generated two-conductor trees have 200, 2,000 and 20,000 unknowns.
Two small OpenDSS cases export its full sparse Y and current RHS after PF.
The largest faer/KLU difference in those five cases is below 6e-10 V.

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

## Reproduction

Install numpy and OpenDSSDirect.py 0.9.4 in an isolated Python environment.
Use the library distributed with `dss_python_backend`, then build the small
C measurement helper against the published SuiteSparse 5.6.0 headers:

```sh
bash build_klu_metrics.sh /path/to/libklusolvex.dylib /tmp/klu-headers /tmp/klu-metrics.dylib
cargo test -p tellegen --features mc-pf --lib --profile release-py --no-run
python compare.py /path/to/the/printed/test-executable /path/to/libklusolvex.dylib /tmp/klu-metrics.dylib > results.json
python recover_smartds.py /tmp/smartds-aus-p3u
python run_smartds.py /path/to/test-executable /path/to/libklusolvex.dylib /tmp/klu-metrics.dylib /tmp/smartds-aus-p3u /tmp/smartds-results
```

The last command retains a roughly 144 MiB matrix JSON plus its result summary
in the chosen output directory. The Rust exporter compiles only for tests;
its ignored test requires an explicit output path. `TELLEGEN_LINEAR_DSS`
selects an actual DSS import attempt; `TELLEGEN_LINEAR_INPUT` selects an
already exported matrix. Neither changes normal engine behavior.

The KLU wrapper reads back and exactly verifies its assembled matrix before
solving. The direct C harness passes the identical CSC arrays to KLU and
checks AMD/COLAMD independently. Existing public KLU ABI functions are used;
no production dependency on KLU or new solver API is introduced.

## Remaining limits

KLU remains the agreed reference. There is no supported regional Tellegen
import/preparation result, interval-validated solution, or demonstrated need
for a WASM backend change. The other original regions were located in the
same public data tree but were not remeasured here. This is a reproducible
investigation of #153, not grounds to close its unresolved regional question.
