# benchmarks

PGLib-OPF validation and benchmark harness for tellegen. A non-shipping workspace member
(native only — it uses `std::fs`, `walkdir`, `rayon`, `csv`, `serde`). It drives tellegen's
public API over the PGLib-OPF v23.07 corpus and validates against the published
PGLib reference solves and finite difference derivatives.

See the project documentation for the methodology and validation writeup.

## Run

```sh
# Corpus at $PGLIB_OPF_PATH (default ~/Datasets/pglib-opf); skipped cleanly when absent.
cargo run -p benchmarks --release -- [flags]
```

| flag | default | effect |
| --- | --- | --- |
| `--variants typ\|api\|sad\|all` | `all` | which operating-condition set |
| `--max-bus N` | unlimited | skip cases above N buses (reproducible cap) |
| `--max-sens-bus N` | 1500 | skip finite difference sampling above N buses |
| `--timeout SECS` | 180 | per case wall time limit |
| `--limit N` | — | run only the first N (smallest) cases |
| `--no-sens` | — | disable finite difference parity sampling |
| `--pglib PATH` | env/default | corpus root override |
| `--out DIR` | `target/pglib-bench` | artifact directory |
| `--book` | — | also write the snapshot to `docs/src/benchmark-results.md` |

## Output

`results.json` contains full records and toolchain provenance, `results.csv`
contains one flat row per `(case, variant)`, and `results.md` is the snapshot
the book embeds. Each output records the toolchain and invocation used for the run.

## What it drives

| stage | tellegen entry point |
| --- | --- |
| DC OPF | `solve_instance` with a PowerIO `DcOpfInstance` |
| conic SOCWR | `solve_ac_instance` with a PowerIO `AcOpfInstance` |
| AC power flow | `solve_ac_pf_instance` with a PowerIO `AcPfInstance` |
| AC / conic sensitivities | the matching typed instance entry |
| DC sensitivities | `solve_instance` with `SensRequest` |

The harness never imports Tellegen's dense models, formulations, KKT systems,
or linear algebra. Finite difference edits target the stable identities returned
with each sensitivity matrix.

The corpus is never vendored; PGLib data is CC BY 4.0 (v23.07, arXiv:1908.02788).

## Multiconductor session benchmark (`mc-pf-session-bench`)

Latency and memory of the retained multiconductor PF session
(eigenergy/tellegen#132), measured through the public `McPfSession` API on
deterministic synthetic feeders from `benchmarks::mc_feeder`:

| preset | buses | terminals | lines | transformers | loads | load branches |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `tiny` | 60 | 198 | 53 | 6 | 39 | 55 |
| `feeder-10k` | 9,950 | 32,412 | 9,299 | 650 | 7,721 | 9,299 |
| `feeder-106k` | 27,239 | 106,038 | 27,803 | 0 | 2,103 | 3,391 |
| `x300k` | 81,406 | 316,904 | 83,095 | 0 | 6,285 | 10,135 |
| `x650k` | 173,787 | 676,530 | 177,391 | 0 | 13,418 | 21,636 |

`feeder-10k` and `feeder-106k` reproduce the shapes measured in #132;
`x300k` and `x650k` are exploratory scaling points at the SMART-DS Greensboro
and San Francisco P5U conductor-node counts. Each preset's `load_scale` is
frozen so the default `McPfOptions` converge with a minimum load-branch
voltage near 0.93 pu.

```sh
# Use the speed profile; `release` is opt-level "s" for wasm size.
cargo run -p benchmarks --profile release-py --bin mc-pf-session-bench -- --preset feeder-10k
```

| flag | default | effect |
| --- | --- | --- |
| `--preset NAME` / `--module PATH` / `--bmopf PATH` | — | generated preset, stored PowerIO module, or raw BMOPF input |
| `--edits N` | 20 | deterministic single-branch edits (each sends the accumulated absolute set) |
| `--seed N` | 132 | edit-sequence seed |
| `--write-module PATH` | — | also write the module JSON and its `.edits.json` sequence for the browser benchmark |
| `--out DIR` | `target/mc-pf-bench` | artifact directory |
| `--label NAME` | preset or file stem | artifact label |
| `--load-scale X` | preset value | override the preset load multiplier |
| `--calibrate X,Y,...` | — | cold-solve the preset at each multiplier and exit |

It writes `native-<label>.json` and `native-<label>.md`: generation and module
serialization, cold `from_module_json` (parse, prepare, factor, solve) with
retained and peak heap from a counting global allocator, per-edit
`replace_load_powers` and `serde_json::to_string(result)` (today's WASM per-edit
payload), a feeder-wide 1.05x edit checked against a fresh solve, the
factorization count, and `input_module_json`/`snapshot` materialization. The
artifacts record which profile built the binary. Written modules belong under
the ignored `target/` directory; never commit them.

The opt-in browser counterpart is `apps/web/tests/mc-pf-bench.spec.ts`. Build
the wasm first (`npm run wasm`), write the modules with `--write-module`, then:

```sh
TELLEGEN_MC_BENCH=1 \
TELLEGEN_MC_BENCH_MODULES=$PWD/target/mc-pf-bench/modules/feeder-10k.pio.json \
  npx --prefix apps/web playwright test --config apps/web/playwright.config.ts mc-pf-bench
```

The Playwright config also starts the SvelteKit preview server, so build the app
(`npm run build:web`) or have a server already listening on
`TELLEGEN_PREVIEW_PORT`; the benchmark itself only uses the multiconductor Vite
fixture server on `TELLEGEN_MC_PREVIEW_PORT`.

It times the engine path the Svelte controller uses (`createMcPfSession`,
`replaceLoadPowers`, `loadBranches`, one animation frame) and, through a
dedicated worker that calls the wasm `McPfSession` directly, the wasm call,
result size, worker-to-main transfer, and `JSON.parse` separately, plus wasm
linear memory and CDP heap metrics. Results go to
`target/mc-pf-bench/browser-<label>.json`.
