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
