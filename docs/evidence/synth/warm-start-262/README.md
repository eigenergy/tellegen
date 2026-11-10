# Generated-grid AC warm starts, 9 November 2026

Preserving the planning state increases AC convergence inside the voltage
band from **101/105 (96.19%) to 102/105 (97.14%)**. Total AC solve time in
the repeated country sweep falls from **4.280 s to 3.176 s (25.8%)**.
The United States at 1:110m, seed 7, now converges. All 102 successful
validations use the stored case state on their first attempt.

The separately measured restart experiment provides no repeatable saving.
The shipping solver keeps its existing restart policy.

## Change and provenance

The baseline is `112f0cac1225845fff2515c4220bd351d084fb83`. The warm-start
Rust implementation is `78e27688f0ce715488e738a46dc438d1273ee0f6`.
Subsequent changes normalize the JavaScript metadata field order for saved
results and add this evidence; they do not change the measured Rust code.

Generated buses retain the planned 1.02 pu magnitude and reactive-planning
angles, converted from radians to PowerIO's degrees. If reactive angle
planning fails, generation retains the final DC planning angles and the
existing warning. Mapped grids refresh the angles after successful reactive
planning at the constrained dispatch.

AC validation requests `start: "case"`. Modules with AC validation enabled
record that setting for a reopened Study and the viewer. An explicit caller
setting can still request `flat`.

`AcPfSolution` and the public AC response report `initialization`:
`requested`, `used`, `attempts`, and the one-based `restart` number when a
perturbed start succeeds. The field survives server responses, browser
results and saved Study views. Per-island generation results also carry it.
Failed solves retain their mismatch and total-start diagnostics. Responses
from older versions can omit the field.

## Measurements

The existing `run.py sweep` evaluates seeds **1, 7, 2026** on 18 regions at
Natural Earth 1:50m and 17 at the app's 1:110m outline. France is absent
from that app outline. All 35 regenerated specification hashes match the
committed release-audit sweeps. Every variant uses identical requests,
places, dispatch and physical equipment. The comparison checks physical
network equality after excluding only stored bus `vm` and `va`.

The binaries use the repository's native release profile (`opt-level = "s"`,
LTO), Rust 1.96.0 and the `conic` feature. Runs execute sequentially on one
Linux AMD EPYC 9V74 runner after compilation finishes. `environment.json`
records the toolchain and input hashes; each comparison records executable
hashes. These final timings were taken together after an environment reset.
The earlier measurements in commit `78e2768` belong to the earlier runner.

Times below sum `AcPfReport.solve_ms`, including every attempted start and
island. Generation, DC dispatch and file I/O are outside this timer. Each
variant has one complete sweep, so small timing differences are subject to
runner noise. The restart experiment also has alternating repeated trials.

| Outline | DC optimal, before / warm | AC in band, before / warm | AC total, before / warm (ms) | Median AC, before / warm (ms) | Failed AC total, before / warm (ms) |
| --- | ---: | ---: | ---: | ---: | ---: |
| 1:50m | 54 / 54 | 53 / 53 of 54 | 1770.829 / 1492.395 | 15.466 / 13.208 | 35.213 / 32.259 |
| 1:110m | 51 / 51 | 48 / 49 of 51 | 2509.071 / 1683.695 | 17.083 / 15.780 | 820.170 / 198.360 |
| Combined | 105 / 105 | 101 / 102 of 105 | 4279.900 / 3176.089 | | 855.383 / 230.619 |

The failed-run total includes the USA case that warm starts recover.
For the **same three remaining failures**, aggregate solve time rises
from **164.292 ms to 230.619 ms (40.4%)**. Trying the stored state adds an
attempt when it fails. That cost remains visible in this comparison.

| Remaining failure | Before (ms) | Warm (ms) | Warm best mismatch (pu) | Starts attempted |
| --- | ---: | ---: | ---: | ---: |
| Australia, 1:50m, seed 2026 | 35.213 | 32.259 | 5.053 | 6 |
| Australia, 1:110m, seed 1 | 64.818 | 90.073 | 0.2244 | 6 |
| Australia, 1:110m, seed 2026 | 64.261 | 108.288 | 1.458 | 6 |

Independent CLI re-solves at the emitted dispatch confirm every verdict.
All previously convergent cases remain inside 0.9–1.1 pu. The largest
differences over the 101 shared successes are:

| Quantity | Maximum absolute difference | Comparison gate |
| --- | ---: | ---: |
| Voltage magnitude | 1.756e-9 pu | 1e-6 pu |
| Voltage angle | 2.243e-9 rad | 1e-6 rad |
| Branch terminal active/reactive power | 1.471e-6 MW/MVAr | 1e-3 MW/MVAr |

The recovered USA case has residual **7.642e-9 pu**, below the solver's
1e-8 tolerance. It takes two Newton iterations and 94.421 ms, compared
with a 691.091 ms failure from the baseline. Its voltage range is
1.014759–1.025987 pu.

`50m.json` and `110m.json` retain each request hash, verdict, timer,
iteration count, residual, initialization and numerical comparison.

## Separate restart experiment

`restart-experiment.patch` is applied only to a separate experimental
worktree. On deterministic perturbed starts, Newton stops when mismatch
has improved by at most 1% over an eight-iteration window. Case and flat
starts keep their existing iteration policy. The patch is retained here
for reproduction and is absent from the shipping solver.

The experimental 105-run sweep preserves all verdicts, all successful
initializations and every compared voltage and flow exactly. Failed
best-mismatch messages also remain identical. Its aggregate AC timer is
3221.735 ms versus 3176.089 ms with the existing policy. Full records are
in `restart-50m.json` and `restart-110m.json`.

For each remaining failure, `time_restarts.py` discards one warm-up per
binary, then alternates execution order for ten paired trials. These
regenerate the identical saved request and record the generator's AC timer.

| Failed request | Existing policy median (ms) | Experiment median (ms) | Time reduction |
| --- | ---: | ---: | ---: |
| Australia, 1:50m, seed 2026 | 38.780 | 41.933 | -8.13% |
| Australia, 1:110m, seed 1 | 104.880 | 106.351 | -1.40% |
| Australia, 1:110m, seed 2026 | 101.484 | 100.839 | 0.64% |
| Sum of case medians | 245.143 | 249.124 | -1.62% |

The raw samples are in `restart-timing.json`. The trial shows no benefit
that justifies changing the shared solver. **Keep the existing policy.**
This evaluation covers the current synthetic sweep; it does not establish
the safety of removing perturbations across other AC cases.

## Validation

Local Rust checks pass **573 tests**, with four existing ignored tests,
across `tellegen`, `tellegen-synth` and `tellegen-server` with `conic`.
They cover serialization units, reopened Study settings, explicit flat
overrides, and successful case, flat-fallback and perturbed starts. The
generated schemas and TypeScript declarations pass their consistency check.

CI passed the Rust workspace, independent multiconductor and conic checks,
Python tests and wheel builds, engine checks, Svelte tests and web type
checks. Its browser suite exposed a saved-result mismatch caused by the
new metadata arriving in a different JSON key order through the commit
envelope. A regression test reproduces that mismatch, and passes after
the compact projection writes metadata fields in a fixed order. The final
CI result is available on PR #357.

## Reproduction

Follow the outline preparation commands in [the parent README](../README.md).
Use separate target directories when building different worktrees so Cargo
cannot reuse workspace artifacts based on relative paths and timestamps:

```sh
git worktree add /tmp/tellegen-before 112f0cac1225845fff2515c4220bd351d084fb83
git worktree add /tmp/tellegen-warm 78e27688f0ce715488e738a46dc438d1273ee0f6
git worktree add /tmp/tellegen-experiment 78e27688f0ce715488e738a46dc438d1273ee0f6
git -C /tmp/tellegen-experiment apply /path/to/restart-experiment.patch
for variant in before warm experiment; do
  TELLEGEN_ALLOW_FALLBACK=1 CARGO_BUILD_JOBS=2 \
    CARGO_TARGET_DIR=/tmp/target-$variant cargo build --release --locked \
    --manifest-path /tmp/tellegen-$variant/Cargo.toml -p tellegen-cli --features conic
done

# Run these commands from the final PR checkout, which supplies the scripts.
for variant in before warm experiment; do
  python3 docs/evidence/synth/run.py sweep /path/to/specs50/*.json \
    --seeds 1,7,2026 --tellegen /tmp/target-$variant/release/tellegen \
    --output /tmp/sweep-$variant-50 --keep
  python3 docs/evidence/synth/run.py sweep /path/to/specs110/*.json \
    --seeds 1,7,2026 --missing fra --outline 'MapToGrid 1:110m' \
    --tellegen /tmp/target-$variant/release/tellegen \
    --output /tmp/sweep-$variant-110 --keep
done
for outline in 50 110; do
  python3 docs/evidence/synth/compare_warm_starts.py \
    /tmp/sweep-before-$outline /tmp/sweep-warm-$outline \
    --before-binary /tmp/target-before/release/tellegen \
    --after-binary /tmp/target-warm/release/tellegen \
    --output /tmp/warm-$outline.json
  python3 docs/evidence/synth/compare_warm_starts.py \
    /tmp/sweep-warm-$outline /tmp/sweep-experiment-$outline --before-start case \
    --before-binary /tmp/target-warm/release/tellegen \
    --after-binary /tmp/target-experiment/release/tellegen \
    --output /tmp/restart-$outline.json
done
python3 docs/evidence/synth/time_restarts.py \
  --warm /tmp/target-warm/release/tellegen \
  --experiment /tmp/target-experiment/release/tellegen \
  --sweep /tmp/sweep-warm-50 --sweep /tmp/sweep-warm-110 \
  --repeats 10 --output /tmp/restart-timing.json
```
