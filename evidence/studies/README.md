# Reproducible Studies

These declarations exercise capacity changes, conserved demand redistribution
and an AC power-flow voltage target through the installed native interface.
The browser and PowerMCP use the same Rust contracts and experiment model.

Build the optimized CLI for large grids:

```sh
cargo build --release -p tellegen-cli --features conic
python evidence/studies/run.py evidence/studies/case3-voltage.json \
  --source evidence/studies/case3.m --powerio /path/to/powerio \
  --tellegen target/release/tellegen --output /path/to/new-evidence-directory
```

The output directory must not exist. It receives the input PowerIO IR, complete
portable Study, conversion diagnostics and `result.json`. The result records
source, request and executable hashes, exact trial outcomes, comparisons,
termination and numerical tolerances. Planning leaves the starting state applied;
the best verified candidate remains a recommendation. Import the Study in the
browser to inspect its branches and continue with another goal or agent.

The CATS declarations require the unmodified `CaliforniaTestSystem.m` identified
by their source hashes. The Texas7k declarations similarly identify
`Texas7k_20210804.m`. Large third-party datasets are not vendored here.

## Explicit Texas7k economic scenario

The original Texas7k input has 124 generator cost curves with decreasing
piecewise slopes. The convex DC OPF builder rejects these curves. A separate,
labelled scenario replaces each such curve by its lower convex envelope:

```sh
cargo run --release -p tellegen --example convex_cost_scenario -- \
  /path/to/Texas7k_20210804.m /path/to/scenario.pio.json /path/to/cost-comparison.json
python evidence/studies/run.py evidence/studies/texas7k-convex-cost-capacity.json \
  --source /path/to/scenario.pio.json --scenario-evidence /path/to/cost-comparison.json \
  --powerio /path/to/powerio --tellegen target/release/tellegen \
  --output /path/to/new-evidence-directory
```

The comparison records each changed curve, its maximum reduction at source
breakpoints and both source hashes. The runner attaches it as Study evidence
before planning. This example evaluates a different inner economic model; it
does not claim an optimum for the original nonconvex OPF. The Study's outer
objective still measures the selected regional prices.

## Browser and cancellation checks

Run `npm run test:browser` with `TELLEGEN_STUDY_CLI` set to the built CLI to compare
native and browser trials for the same declared Study. The browser suite checks
persistence, branching, goal revisions, approvals, failed saves and continuation.

```sh
TELLEGEN_STUDY_CLI=target/release/tellegen POWERIO_CLI=/path/to/powerio \
  python evidence/studies/test_native_cancellation.py
```

The POSIX cancellation test sends SIGTERM after an exact trial checkpoint and
checks the saved completed evidence, cancelled termination and unapplied best
candidate. Native WebMCP call records provide separate interactive evidence.
