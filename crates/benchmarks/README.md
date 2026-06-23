# benchmarks

PGLib-OPF validation and benchmark harness for phasor. A non-shipping workspace member
(native only — it uses `std::fs`, `walkdir`, `rayon`, `csv`, `serde`). It drives phasor's
public API over the PGLib-OPF v23.07 corpus and validates against two independent baselines:
the published PGLib reference solves (`BASELINE.md`) and finite-difference derivatives.

See the `book/` (Methodology, Validation, Limitations) for the writeup.

## Run

```sh
# Corpus at $PGLIB_OPF_PATH (default ~/Datasets/pglib-opf); skipped cleanly when absent.
cargo run -p benchmarks --release -- [flags]
```

| flag | default | effect |
| --- | --- | --- |
| `--variants typ\|api\|sad\|all` | `all` | which operating-condition set |
| `--max-bus N` | unlimited | skip cases above N buses (reproducible cap) |
| `--max-acopf-bus N` | 3000 | skip the nonlinear AC OPF above N buses |
| `--max-sens-bus N` | 1500 | skip finite-difference sampling above N buses |
| `--timeout SECS` | 180 | per-case wall-clock guard |
| `--limit N` | — | run only the first N (smallest) cases |
| `--no-sens` | — | disable finite-difference parity sampling |
| `--pglib PATH` | env/default | corpus root override |
| `--out DIR` | `target/pglib-bench` | artifact directory |
| `--book` | — | also write the snapshot to `book/src/benchmark-results.md` |

## Output

`results.json` (full records + toolchain provenance), `results.csv` (one flat row per
`(case, variant)`), and `results.md` (the markdown snapshot the book embeds). The solves are
deterministic, so the numbers reproduce on the recorded toolchain.

## What it drives

| stage | phasor entry point |
| --- | --- |
| DC OPF | `solve_prebuilt` (`DcNetwork::from_network` + the prebuilt solve = `solve_network`) |
| conic SOCWR | `socwr_opf` |
| AC OPF | `acopf` (the exact nonconvex optimum vs the published `AC ($/h)`) |
| AC power flow | `ac_pf(&AcPolar::new(), &AcNetwork::from_network(..))` |
| AC / conic / AC OPF sensitivities | `AcNewton::new` / `ConicKkt::new` / `AcOpfKkt::new` + `sensitivity` |
| DC sensitivities | `sensitivity_json` (the only external route to the DC KKT) |

The corpus is never vendored; PGLib data is CC BY 4.0 (v23.07, arXiv:1908.02788).
