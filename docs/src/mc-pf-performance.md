# Multiconductor PF interactive performance

A retained multiconductor power-flow session (`McPfSession` in Rust,
`createMcPfSession` in the browser engine) keeps the prepared network, the
compensation operator, its sparse LU factorization, and the last converged
voltages. This page states what an ordinary load edit is allowed to cost, how
that is measured, and the measurements behind it
([#132](https://github.com/eigenergy/tellegen/issues/132)).

## Target and budgets

| Budget | Value | Measured |
| --- | --- | --- |
| UI-visible latency, single-branch load edit, generated ~10,000-bus feeder (9,950 buses, 32,412 terminals), Chromium on Apple M-series | ≤ 100 ms p95 | 15.6 ms p95 |
| Per-edit response | constant size, ≤ 2 KiB (`McPfSummary`) | 563–571 bytes at every size up to 676,530 terminals |
| Results-panel detail page (one bus, 20 equipment ports) | ≤ 32 KiB | ≈ 4.9 KB |
| Terminal arrays, fetched on demand | 16 bytes per terminal per quantity, transferred rather than copied | 0.5 MiB (32,412 terminals) to 10.3 MiB (676,530) |
| Network materializations during an ordinary edit | 0 | 0 |
| Live engine heap after an edit | unchanged (± 64 KiB) | unchanged (0 bytes) |
| WASM linear memory across edits | no growth | no growth over 20 edits |
| Transient heap during an edit | proportional to terminals, no network or result copy | about 85 bytes per terminal |

The 180 ms debounce that coalesces rapid edits in the Studies panel is a
deliberate delay and is excluded from every latency figure. Saving, exporting,
attaching geography, and opening the full result build portable documents on
demand; their cost is reported below but is not part of an ordinary edit.

## What an edit costs

An ordinary edit replaces the absolute set of load-branch powers. The session
updates only the prepared current laws of the loads that changed (physical
nominal admittance is recomputed; the fixed-point compensation reference and
the LU factorization stay frozen), warm-starts from the last converged
voltages, and reduces the converged phasors to a summary: convergence,
residuals, the load voltage range and violation count, source power, passive
loss, and counts. The summary runs the same output finiteness checks as the
full result without building per-terminal records. The base network and
instance are never copied or rebuilt by an edit; `materialization_count`
counts the edited networks built for portable output, and stays zero across
ordinary edits.

The browser makes one engine round trip per edit. Terminal and equipment
detail is fetched only when a view asks for it: the Studies panel requests one
bus's terminals and one 20-row equipment page, `terminalVoltages()` and
`terminalCurrents()` return transferable `Float64Array`s, and `result()` still
builds the complete `McPfResult` for export. Detail pages carry the solve count
they describe, so a page from an older operating point is discarded.

The four latency components are measured separately:

| Component | Meaning | How it is measured |
| --- | --- | --- |
| Numerical solve | Engine time for the edit request: the warm fixed point and the summary | `engine_ms` from the worker, split by the session profile into load evaluation, KCL residuals and compensated right-hand sides, retained LU solves, and the summary |
| Network materialization | Building an edited network for portable output | Zero per edit (`materializationCount()`); `inputModule()` and `snapshot()` are timed separately |
| Serialization and parsing | Moving the response to the page | Summary JSON in the engine, transfer (round trip minus engine time), and `JSON.parse` on the page |
| UI-visible | From the edit request to the updated state and the next animation frame | Round trip, parse, and one `requestAnimationFrame`; with the results panel open, plus its detail request |

## Results

Measurements below use deterministic generated feeders
(`benchmarks::mc_feeder`), on an Apple M5 Pro (18 CPUs, 64 GiB). `feeder-10k`
and `feeder-106k` match the shapes reported in #132; `x300k` and `x650k` are
exploratory points at the SMART-DS Greensboro and San Francisco P5U
conductor-node counts. Each preset converges in 9–11 iterations with a minimum
load-branch voltage of about 0.93 pu; single-branch edits take 6–8. "Before"
is the session as of 0.3.0 (commit `ee23ab8`, which cloned the network,
rebuilt the instance, and returned the complete result on every edit), run
with the same harness, inputs, and edit sequence.

| Preset | Buses | Terminals | Lines | Transformers | Loads (branches) | Module |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `feeder-10k` | 9,950 | 32,412 | 9,299 | 650 | 7,721 (9,299) | 11.4 MiB |
| `feeder-106k` | 27,239 | 106,038 | 27,803 | 0 | 2,103 (3,391) | 21.7 MiB |
| `x300k` | 81,406 | 316,904 | 83,095 | 0 | 6,285 (10,135) | 65.1 MiB |
| `x650k` | 173,787 | 676,530 | 177,391 | 0 | 13,418 (21,636) | 139.2 MiB |

### Browser

Chromium 153 (headless), the shipped WASM build (`release` profile,
opt-level "s"), 20 single-branch edits.

| Preset | UI-visible median / p95, before | after | Per-edit response, before | after |
| --- | ---: | ---: | ---: | ---: |
| `feeder-10k` | 147.5 / 153.3 ms | **15.2 / 15.6 ms** | 25.8 MB result + 1.4 MB load branches | 566 B |
| `feeder-106k` | 427.4 / 446.7 ms | **50.0 / 65.3 ms** | 75.9 MB + 0.5 MB | 568 B |
| `x300k` | 1,365 / 1,480 ms | **181 / 203 ms** | 227.1 MB + 1.6 MB | 569 B |

Opening the results panel adds one detail request of 0.1–0.2 ms and about
4.9 KB. Median components after the change:

| Preset | Engine solve | Load evaluation | KCL and matvec | LU solves | Summary | Transfer + parse | Materialization |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `feeder-10k` | 11.8 ms | 1.7 | 3.2 | 4.7 | 1.4 | < 0.1 ms | 0 |
| `feeder-106k` | 49.8 ms | 0.7 | 9.5 | 32.3 | 4.4 | < 0.1 ms | 0 |
| `x300k` | 180.6 ms | 2.7 | 30.8 | 121.7 | 13.1 | < 0.2 ms | 0 |

Before the change the corresponding engine call took 102.6, 299.2, and 979 ms
(solve, network copy, instance rebuild, result construction, and JSON
serialization together), and parsing the result on the page took a further
37.3, 111.5, and 356 ms. Building the complete result on demand now costs
51 / 153 / 450 ms in the engine plus 41 / 117 / 361 ms of parsing, only when
it is requested.

Creating a session (parse, prepare, factor, first solve, summary, and the load
branch list) takes 256 ms, 832 ms, and 2.79 s, compared with 428 ms, 1.22 s,
and 3.98 s when the full result was fetched with it.

### Native

`release-py` profile (opt-level 3), 20 single-branch edits; the counting
allocator reports heap bytes.

| Preset | Edit median / p95, before | after | Result or summary JSON per edit, before | after | Edit heap peak, before | after |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `feeder-10k` | 42.9 / 46.8 ms + 20.8 ms JSON | **5.7 / 6.5 ms** | 24.6 MiB | 565 B | 39.1 MiB | 2.6 MiB |
| `feeder-106k` | 105.5 / 119.3 ms + 61.3 ms | **18.8 / 25.3 ms** | 72.4 MiB | 569 B | 80.2 MiB | 8.7 MiB |
| `x300k` | 356.3 / 381.5 ms + 185.1 ms | **72.4 / 79.5 ms** | 216.6 MiB | 570 B | 274.7 MiB | 25.9 MiB |
| `x650k` | 733.3 / 768.8 ms + 399.2 ms | **139.0 / 154.6 ms** | 464.4 MiB | 571 B | 570.0 MiB | 55.3 MiB |

| Preset | Load evaluation | KCL and matvec | LU solves | Summary | Cold start, before → after | Retained heap, before → after |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `feeder-10k` | 0.8 ms | 1.4 | 2.4 | 0.7 | 182 → 131 ms | 106 → 67 MiB |
| `feeder-106k` | 0.4 ms | 3.7 | 11.6 | 2.1 | 442 → 391 ms | 319 → 230 MiB |
| `x300k` | 1.5 ms | 12.6 | 46.0 | 6.5 | 1.43 → 1.19 s | 968 → 667 MiB |
| `x650k` | 3.4 ms | 25.5 | 87.5 | 14.1 | 3.12 → 2.61 s | 1.86 → 1.25 GiB |

The retained heap fell because the session no longer keeps a second copy of
the stored module text and freezes the stamped admittances into compressed
rows. A feeder-wide 1.05× edit takes 9.5 ms, 31 ms, 97 ms, and 216 ms (before:
47, 129, 392, and 890 ms) and agrees with a freshly prepared solve to
8.7×10⁻¹⁰ V / 3.5×10⁻⁹ A, 6.8×10⁻⁹ V / 2.0×10⁻⁸ A, 5.1×10⁻⁹ V / 2.9×10⁻⁸ A,
and 5.1×10⁻⁹ V / 4.2×10⁻⁸ A, unchanged from before, with one factorization
throughout. Results are bit-identical to the previous solver on every oracle
and load-model fixture, for one-shot solves and for warm session edits.

### Memory in the browser engine

| Preset | Live heap after create | Linear memory after create | Transient per edit | Linear memory after 20 edits, before → after | Page JS heap after edits, before → after |
| --- | ---: | ---: | ---: | ---: | ---: |
| `feeder-10k` | 47.8 MiB | 115.6 MiB | 2.6 MiB | 171 → 115.6 MiB | 195 → 4.6 MiB |
| `feeder-106k` | 177.8 MiB | 394.1 MiB | 8.7 MiB | 587 → 394.1 MiB | 543 → 3.4 MiB |
| `x300k` | 508.5 MiB | 1,008 MiB | 25.9 MiB | 1,394 → 1,008 MiB | 541 → 5.1 MiB |

WASM linear memory only grows, so it records the largest transient since the
worker started; the live heap is what a session holds. Saving or exporting is
the memory high-water mark: at 106,038 terminals the snapshot takes 668 ms,
peaks at 563 MiB of engine heap, and grows linear memory to 938 MiB.

## Reproduce

```sh
# Native: any preset, a stored PowerIO module, or a raw BMOPF file.
just mc-pf-bench feeder-106k
cargo run -p benchmarks --profile release-py --bin mc-pf-session-bench -- --bmopf case.json
# The CI regression (deterministic checks only, never timing).
just mc-pf-regression

# Browser: build the WASM, write the modules, then run the opt-in spec.
npm run wasm
cargo run -p benchmarks --profile release-py --bin mc-pf-session-bench -- \
  --preset feeder-10k --write-module target/mc-pf-bench/modules/feeder-10k.pio.json
TELLEGEN_MC_BENCH=1 \
TELLEGEN_MC_BENCH_MODULES=$PWD/target/mc-pf-bench/modules/feeder-10k.pio.json \
  npx --prefix apps/web playwright test --config apps/web/playwright.config.ts mc-pf-bench
```

The native harness writes `native-<label>.json` and `.md`, and the browser
spec writes `browser-<label>.json`, under `target/mc-pf-bench/`.

## Scope and limits

- The generated feeders reproduce the dimensions of the cases reported in
  #132, not their files: the original ~10,000-bus case needed 21 cold
  iterations and carried a 70.3 MB BMOPF document, while the generated preset
  needs 9 and 11.4 MiB. `--bmopf` and `--module` run the same measurements on
  a real file.
- `x650k`'s 139 MiB module exceeds the browser's 128 MiB input boundary, so
  the 676,530-terminal point is native only. A saved Study embeds its input,
  its PowerIO solution, and the full result; at 106,038 terminals that is
  about 138 MiB, beyond the same boundary. Reading back a stored solution with
  more than 65,536 terminal values needs a PowerIO release with
  [eigenergy/powerio#550](https://github.com/eigenergy/powerio/pull/550); the
  document-size contract is tracked in
  [eigenergy/powerio#544](https://github.com/eigenergy/powerio/issues/544).
- With copying and result construction removed, the retained LU solves are
  the largest share of a large edit. Backend speed, conditioning, and the
  faer/KLU comparison are tracked in
  [#153](https://github.com/eigenergy/tellegen/issues/153).
- At the largest sizes the cold start is dominated by preparation (admittance
  stamping), 1.48 of 2.61 s at 676,530 terminals; the per-load lookup that
  made preparation quadratic in the load count has been removed.
- Raw SMART-DS import and nonlinear Tellegen solves of those regions are not
  established; the exploratory presets are generated models of the same size.
