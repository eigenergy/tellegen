# AC OPF speed and sensitivities

The solver keeps its `1e-8` accuracy target and now scales derivatives before solving.
It limits the base solve and all recovery attempts to 2,000 iterations in total.
The separate WASI build uses optimization level 3 and fixed-width SIMD. The browser
keeps the worker and compiled module between calls, but creates a fresh solver
instance for each call. Cancellation terminates the worker.

The saved measurements include input and executable checksums, objective values,
iteration counts and independently calculated primal residuals. Timings are
measurements on one Apple Silicon development machine, with other build work
running, rather than a controlled hardware comparison. Native final builds use
Rust 1.96.0. The baseline source is commit `199c1e73`; the final implementation is
`85a787c8`. The WASI baseline is the preserved executable from the interrupted
session; its checksum identifies exactly what was measured.

| Case | Native before | Native after | WASI before | WASI after, median of 3 |
| --- | ---: | ---: | ---: | ---: |
| case793_goc | 1.47 s | 0.28 s | 2.30 s | 0.43 s |
| case1888_rte | 16.49 s | 5.24 s | 30.25 s | 5.16 s |
| case4661_sdet | failed after 307.75 s | 11.22 s | failed after 298.31 s | 12.25 s |

The case4661 improvement is a successful solve where the baseline did not return
one; it is not a speed ratio between equivalent successful solves. Objectives on
cases solved by both versions agree to better than `1e-9` relative. The solver
still checks voltage, generation, balance, thermal and angle constraints before
returning a solution. Stored residuals remain available in each record.

Automatic AC power flow initialization was tried in the recovered experiments.
It failed on case1888 and case4661 and increased their solve time, so the solver
does not run an extra power flow by default. A supplied initial point is still
used. The optional `wasm-opt` pass was omitted because the recorded comparison
made these solves slower; release output should not depend on whether that
program happens to be installed.

## Reproduce

Build `acopf_benchmark` at the baseline and final commits with Rust 1.96.0:

```sh
cargo build -p tellegen --example acopf_benchmark --features acopf --release
# Repeat using the baseline and final executable, saving their JSON lines.
./target/release/examples/acopf_benchmark - /path/to/pglib_opf_case793_goc.m /path/to/pglib_opf_case1888_rte.m /path/to/pglib_opf_case4661_sdet.m
bash scripts/build-acopf-wasi.sh
node validation/evidence/acopf-cleanup/wasi-bench.mjs target/experimental-acopf/tellegen_acopf_wasi.wasm /path/to/case793_goc.json /path/to/case1888_rte.json /path/to/case4661_sdet.json
```

The `.json` inputs are those MATPOWER cases parsed to PowerIO module JSON with
`ingest_case`; their checksums are in the records. `RUNS=1` selects one WASI run.
The script uses the same WASI host as the browser and records compilation,
instantiation, solve time and final linear-memory size separately. The module
is compiled once and reused, matching the worker.

For the real browser check, start Vite at the repository root, then run:

```sh
node validation/evidence/acopf-cleanup/browser-check.mjs http://127.0.0.1:5173 /path/to/solver.wasm /path/to/case3.json
```

This sends a dispatch/demand sensitivity request through the real dedicated
worker twice, checks equal 3-by-3 matrices, releases the idle worker and checks
for page errors. The saved Chromium 158 result is `browser-check.json`.

## Numerical checks

Nine AC OPF sensitivity tests compare complete central finite-difference solves
against dispatch, prices, voltages and flows for active/reactive demand, line
limits and generator bounds. They include binding voltage constraints, binding
line/generator limits, two islands, forward/adjoint agreement, served units and
element IDs. A lossless case checks price signs against DC OPF. Weakly active
constraints and malformed/unsupported requests return errors.

These derivatives describe one local solution while its active constraints stay
fixed. They do not claim a globally smooth or globally optimal solution map.
Shared KKT code is also exercised by the multiconductor OPF tests: the combined
Rust suite passed 668 tests, with 5 existing ignored tests. The ACOPF-only suite
passed 204 tests, the worker adapter passed 2, and the engine package passed 81.
Clippy with warnings denied and the patch-compatibility check against v0.8.0 pass.
