# Optional Moreau execution

Tellegen can link Moreau's Rust CPU core into the existing native engine and
browser WASM module. Clarabel and Tellegen's specialized sensitivity system
remain the defaults. PowerIO continues to own portable cases, problem
instances, identities, and solution records.

Enable `moreau` on the Rust engine or WASM adapter. The dependency disables
Moreau's default features, selecting its pure Rust QDLDL configuration. Browser
builds do not need native threads, BLAS, Python, or a separate Moreau npm package.

```sh
cargo test -p tellegen --features moreau,conic
npm --workspace @tellegen/engine run wasm:moreau
npm run contracts
npm run build:engine
```

## Execution choices

`ExecutionOptions` selects `dc_solver` (`clarabel` or `moreau`) independently
from `dc_derivatives` (`tellegen` or `moreau_selected`). Options belong to a
live operation or Study and do not change a PowerIO declaration. A build
without Moreau rejects either Moreau selection explicitly. Other formulations
keep their existing solvers and derivatives.

Rust callers use `solve_instance_with_execution`,
`solve_module_json_with_execution`, `Study::new_with_execution`, or
`plan_capacity_with_execution`. Existing entry points retain their defaults.

```ts
import { createStudy, executionCapabilities, solveModule } from "@tellegen/engine";

const execution = {
  dc_solver: "moreau",
  dc_derivatives: "moreau_selected",
} as const;
const available = await executionCapabilities();
const study = await createStudy(moduleJson, "dcopf", { execution });
const result = await solveModule(moduleJson, {}, execution);
```

`study.execution` reports the retained policy. Commits, forks, and isolated
capacity-planning workers retain these choices. Saved cases omit execution
configuration; opening a saved case uses the caller's selected options.

`moreau_selected` uses Moreau for demand and rating columns of active price,
dispatch, flow, and angle, and for weighted-LMP rating gradients. Other
operations use Tellegen's specialized derivatives. Sensitivity matrices include
`implementation` when this policy is selected. Errors do not change backends.

## Numerical ownership

Tellegen assembles OPF data, maps physical perturbations to constraint rows,
selects the local active set, and converts result units. Moreau solves the
optimization problem and computes the selected implicit derivatives.

For DC derivatives, Tellegen removes fixed-zero shedding variables and their
redundant bounds, retains all equality rows, and selects scalar inequality
rows whose multiplier exceeds the existing `1e-6` complementarity threshold.
The local derivative problem is an equality-constrained QP. Demand includes
the shedding-cap derivative when shedding is available at positive demand;
ratings perturb both thermal-limit rows, with only active rows contributing
locally. This keeps structural handling in Tellegen and leaves Moreau's
numerical algorithms unchanged.

These are local active-set derivatives. At an active-set transition or a
nonunique optimum, the derivative can be discontinuous or nonunique. Tests
compare differentiable operating points; the optional backend does not promise
identical nonunique duals. The default specialized path remains available.

## Reproducible comparisons

Run native measurements in release mode. Each operation records one cold
sample, three warmups, and ten timed samples. Derivative timings include their
setup and matrix conversion; they do not imply a reused numerical factorization.

```sh
TELLEGEN_COMPARE_OUT=target/moreau-case3.json cargo test -p tellegen \
  --release --features moreau,conic --lib compare_dc_backends -- --ignored
TELLEGEN_COMPARE_CASE=/path/to/pglib_opf_case200_activ.m \
  TELLEGEN_COMPARE_OUT=target/moreau-case200.json cargo test -p tellegen \
  --release --features moreau,conic --lib compare_dc_backends -- --ignored
```

The built-in comparison uses a congested three-bus fixture. External MATPOWER
cases are read from the provided path and never vendored. Failed stages are
recorded as errors, rather than counted as fast solves.

For browser comparisons, build the engine with Moreau, start the
`examples/browser-minimal` Vite server, and open `/moreau.html`. It uses isolated
workers for all four combinations, measuring initialization, demand previews,
commits with a sensitivity column, and bounded capacity planning. An optional
file input selects a local MATPOWER case.

```sh
npm --workspace @tellegen/example-browser-minimal run dev -- --host 127.0.0.1 --port 5175
TELLEGEN_COMPARE_OUT=target/moreau-browser.json node scripts/moreau-browser-smoke.mjs
```

The [recorded comparison results](moreau-results/README.md) include native
three- and 200-bus measurements, browser 14- and 200-bus measurements, and
WASM bundle sizes.

Retain browser/toolchain versions, dependency revisions, case identities,
WASM sizes, and raw samples with benchmark results. No automatic backend
promotion or removal of specialized methods follows from this integration.

## Dependency and release status

The integration is a candidate until Moreau's portability changes are released
as a crates.io dependency. The candidate pins `samtalki/moreau` at
`6f2949933cdab7d8444b64d1e0917e64d5e3e73b`, the commit in
[Moreau PR #4](https://github.com/moreau-project/moreau/pull/4). Tellegen's Rust release must use a registry version containing
those changes; the Moreau Python version alone does not establish that release.
The git-only dependency has no registry version requirement, so `cargo package`
and the packaging CI gate remain blocked until that dependency is replaced.
The Moreau license and attribution notices ship in the engine package's
`third-party/moreau` directory.
