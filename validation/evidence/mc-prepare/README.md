# Multiconductor preparation measurements

Issue #363 already has CLI `profile: true` and `output: summary|tables|full`
in main. This measurement completes its preparation investigation and removes
one unnecessary reconstruction of the ordered matrix entries.

The synthetic `feeder-10k` has 9,950 buses and 32,412 terminals. It is not the
10,885-terminal Ann Arbor feeder from the issue, so its absolute times should
not be compared with that table. Both executables use Rust 1.96.0 and
`release-py`, with debug symbols for sampling. Runs alternate before/after;
each makes 30 fresh sessions. Other build jobs were active on the machine.

| Round | Preparation before / after (ms) | Total before / after (ms) |
| --- | --- | --- |
| 1 | 154.0 / 143.9 | 240.8 / 231.4 |
| 2 | 150.8 / 139.6 | 235.2 / 222.8 |
| 3 | 153.1 / 151.3 | 236.6 / 242.3 |

Preparation improved in all three rounds; whole-call timing was noisier and
regressed in the last round. This is a small preparation improvement, not a
claim about overall application speed. All six runs passed the benchmark's
numerical, memory and result-size checks with the same voltage range.

An eight-second macOS `sample` capture during 200 repeated cold starts is in
`sample.txt`. Its call tree shows substantial work in PowerIO matrix assembly,
conversion into Tellegen's ordered map, input validation/indexing, and sparse
factorization. In particular, assembly at `network.rs:396` accounts for 861
samples and conversion at line 406 for 492 within the shown session-creation
stack. Counts are samples, not milliseconds or mutually exclusive totals.

The changed loop at the later matrix-combination step inserted every passive
entry into a new ordered map before adding load compensation. Cloning that
already ordered map avoids those repeated searches and preserves its entries
and subsequent addition order. No electrical model or solver setting changes.

## Reproduce

```sh
CARGO_PROFILE_RELEASE_PY_DEBUG=1 cargo build -p benchmarks --profile release-py --bin mc-pf-session-bench
target/release-py/mc-pf-session-bench --preset feeder-10k --cold-repeats 200 --edits 5 --check --out /tmp/mc-prepare
# In another terminal on macOS while that command runs:
sample mc-pf-session-bench 8 -file /tmp/mc-prepare-sample.txt
```

For a comparison, retain the executable from before the map change, build the
new executable with the same compiler/settings, and alternate the two with
`--cold-repeats 30`. `timings.csv` contains all 180 measured phase profiles;
`summary.json` records per-run medians and regression checks. The benchmark
also accepts the original feeder as `--bmopf PATH` if it is available locally.

The 67 multiconductor power-flow unit tests pass. The `tiny` preset with two
edits fails the existing warm/fresh absolute comparison limits in both
binaries, with identical differences (3.247e-6 V and 5.013e-5 A). The
`feeder-10k` runs above pass those same limits; they were not relaxed for this
change. The repeat count is also checked with two sessions and with zero
rejected as invalid input.
