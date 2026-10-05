# AC OPF solver decision

Status: accepted for feasibility work; distribution and product capability are
not yet approved. Recorded 24 September 2026.

## Main integration check — 5 October 2026

The feasibility branch includes main at `0d08c55`, including compact
multiconductor PF session responses, the generated feeder regression and the
additional center-tap reference cases. The `just ci` aggregate retains both
`acopf-probe` and `mc-pf-regression`, matching the Rust workflow. The existing
multiconductor PF sessions remain independent of this opt-in NLP boundary.

The POUNCE revision and release/restoration/distribution gates are unchanged.
The latest tagged release checked for this refresh is v0.12.0; the pinned
revision is 35 commits ahead of that tag. Native AC OPF compilation/solve and
the unpublished multiconductor IVR work remain separate layers of the stack.

Local validation for this refresh passed the HS071 probe (`17.014017274`,
maximum source-equation violation `2.484e-8`), PowerIO source check, EPL feature
isolation and formatting. Workspace tests excluding the Python extension passed
374 tests (2 existing ignores), the independent `mc-pf` suite passed 160
(1 ignore), the conic engine suite passed 312 (2 ignores), and the conic WASM
adapter passed 40. Strict Clippy passed for the shipping Rust adapters plus the
opt-in probe and separately for Python with `extension-module`. The generated
`feeder-10k --check` regression passed all deterministic checks; timings are not
acceptance thresholds. Cross-platform, JavaScript and distribution checks remain
in the GitHub workflows.

## Decision

Tellegen will use POUNCE as its first nonlinear solver for canonical balanced
transmission AC OPF. The choice favors an end-to-end Rust implementation and a
responsive upstream over building two solver adapters in the first cycle. The
public problem and solution boundary must remain backend-neutral.

The decision follows POUNCE's WASM-frontend
[restoration fix](https://github.com/jkitchin/pounce/pull/961) and the
[AC OPF follow-up experiments](https://github.com/jkitchin/pounce/issues/965).
Those results are feasibility and implementation evidence, not general solver
performance guarantees.

This decision does not relabel the existing SOCWR relaxation as AC OPF. The
native opt-in feature now makes `Problem::Acopf` available after adding the
exact PowerIO-prepared polar model, independent primal validation, truthful
status mapping, and portable `AcOpfSolution` emission. Product distribution
still requires the browser worker boundary and release approval below.

The dependency is pinned to POUNCE revision
`925e75fbd036de309929e398159f946d42d0d94b`, the head of POUNCE PR #961. That
change wires restoration and the second-opinion ladder into POUNCE's WASM
frontend; it does not retrofit callers that drive a bare `IpoptApplication`.
A released version containing the change should replace the Git pin before
publishing the Rust crate, and Tellegen's eventual solve driver must wire or
invoke the released restoration path explicitly.

## Feasibility boundary

The non-default `tellegen/acopf` feature enables `pounce-nl` and `pounce-rs`.
No adapter forwards it. The native `acopf_hs071_probe` example constructs
HS071 with `NlProblem::from_expressions`, builds `NlTnlp`, verifies the sparse
Jacobian and Lagrangian-Hessian structures, solves with exact derivatives, and
checks the result independently.

Run it with:

```sh
cargo run -p tellegen --example acopf_hs071_probe --features acopf --locked
```

The probe pins the FERAL backend, exact Hessians, identity NLP/linear-system
scaling, and `1e-9` solver/constraint tolerances. The acceptance check requires
the known HS071 objective within `1e-5`, source-equation violation below
`1e-7`, eight Jacobian nonzeros, and ten lower-triangle Hessian nonzeros.

PowerIO 0.11.3 provides the released preparation boundary used here. The model
compiler consumes its `AcOpfInstance`, `build_ac_opf_preparation`, and
`AcOpfSolution` APIs rather than reinterpreting MATPOWER data or reusing
Tellegen's modified CATS power-flow physics.

## Canonical model boundary

The opt-in feature now also contains a private, POUNCE-specific polar model
compiler. It consumes `AcOpfInstance` only through
`build_ac_opf_preparation`, preserves PowerIO's generator and branch columns
and source maps, and emits an in-memory POUNCE expression DAG with exact sparse
derivatives. Native module dispatch and capabilities are feature-gated; no
shipping adapter or browser package enables them.

The recorded assembly policy is per-unit values, no zero-impedance skipping,
no synthesized thermal ratings, and PowerIO's angle-interval correction
enabled. The preparation, including those choices, and the formulation version
are hashed into the model fingerprint. Changing any policy is therefore a
model change rather than a solver tuning option.

The variable map carries bus `Va`/`Vm`, generator-level `Pg`/`Qg`, and one
epigraph column for each active convex piecewise generator cost. Constraint
maps retain P/Q balance rows, zero reference angles, selected angle bounds,
both terminal thermal margins, and exact piecewise segments. Balance rows use
generation minus branch withdrawals, shunt consumption, and demand equal to
zero. Thermal and epigraph rows use nonnegative remaining margin. No dual is
published until these signs and unit conversions are independently validated.

The tests evaluate the full pi model at a non-flat point with taps, phase
shifts, charging and bus shunts, compare both-end flows and nodal balances to a
separate numerical calculation, and compare the sparse Jacobian and
Lagrangian Hessian to finite differences. Unsupported active storage,
voltage-dependent loads, and remote voltage regulation fail with the source
element's identity.

An ignored, path-driven `external_model_build_ladder` test makes the standard
MATPOWER 14/30/300-bus construction check reproducible without vendoring a
second copy of those fixtures. On 24 September 2026, a local arm64 debug build
after compilation produced:

| case | variables / rows | nnz Jacobian / Hessian | expression compile | derivative tape | max absolute directional J / H error |
| --- | ---: | ---: | ---: | ---: | ---: |
| 14 | 38 / 49 | 267 / 127 | 1.849 ms | 3.841 ms | `4.545e-9` / `1.557e-7` |
| 30 | 72 / 184 | 871 / 260 | 1.016 ms | 12.803 ms | `7.186e-9` / `3.412e-8` |
| 300 | 738 / 1,012 | 5,433 / 2,605 | 8.380 ms | 70.444 ms | `2.723e-7` / `3.601e-7` |

These are correctness/prototype measurements, not solver performance claims.
Run the ladder with `TELLEGEN_ACOPF_FIXTURES` pointing to a directory containing
`case14.m`, `case30.m`, and `case300.m`. Peak RSS was not available inside the
sandbox used for this run, and frozen-NL parity remains a separate acceptance
measurement. The regression test now reports both absolute error and error
scaled by the analytic and finite-difference magnitudes with a floor of one.
The latest scaled J/H errors were `3.478e-9` / `2.418e-9`, `1.791e-9` /
`2.239e-10`, and `9.095e-7` / `2.966e-9` for cases 14, 30, and 300
respectively.

## Typed native solve boundary

The opt-in native API now exposes `solve_ac_opf_instance` and
`solve_ac_opf_instance_to_solution` plus cancellable counterparts. The
cancellable paths poll an atomic flag before model construction and at every
iteration of the currently wired POUNCE solve. They run POUNCE with FERAL,
exact Hessians, identity NLP and linear-system scaling, a `1e-8`
solver/constraint tolerance, `1e-7` acceptable tolerance, and a 1,000-iteration
ceiling. The returned `AcOpfSolution` is scattered through PowerIO's source
row/winding maps in MW/MVAr and degrees, retains inactive source rows as
unavailable values, and round-trips through `PioModule`.

Only `SolveSucceeded` and `SolvedToAcceptableLevel` are candidates for
emission, and both must pass a separate calculation from the prepared arrays:
P/Q balance, voltage/generator boxes, references, angle and both-end thermal
limits, exact piecewise epigraphs, finite outputs, and source-cost objective.
The portable residuals report the independently calculated MW/MVAr balance
mismatch. A local NLP infeasibility report remains a diagnostic error rather
than PowerIO's proof-strength `Infeasible`. No LMP or limit multiplier is
emitted yet; the POUNCE dual arrays stay private pending sign and source-unit
perturbation tests.

Native `solve_module_json` promotes a balanced network or consumes a stored
`AcOpfInstance` when the request selects `acopf`, and the capability advertises
only `vm`, `va`, `injections`, `flows`, and `dispatch`. Request edits currently
fail with an instruction to amend the canonical instance, rather than being
silently dropped, and every sensitivity request fails until the NLP KKT
contract exists. A successful nonlinear solve is reported as `feasible`, not
globally `optimal`, and its response includes the POUNCE status, iteration
count, solver constraint/KKT residuals, independently checked primal residual,
and model fingerprint. Failure and cancellation errors include the available
iteration and residual diagnostics.

### Native API example

With `tellegen` built with `features = ["acopf"]`, a stored canonical instance
can be solved for either the compact API response or the portable solution:

```rust
use std::sync::Arc;
use tellegen::{Problem, SolveRequest};

# fn example(module_json: &str) -> Result<(), String> {
let module = tellegen::ir::deserialize_module(module_json)?;
let powerio::PioValue::AcOpfInstance(instance) = module.into_value() else {
    return Err("expected an ac_opf_instance".into());
};
let request = SolveRequest { formulation: Problem::Acopf, ..Default::default() };
let response = tellegen::solve_ac_opf_instance(&instance, &request)?;
// Compact response: powers in MW/MVAr, voltage angles in radians.
assert!(response.lmp.is_none());
let solution = tellegen::solve_ac_opf_instance_to_solution(Arc::new(instance), "my-app")?;
// Portable solution: source table order, powers in MW/MVAr, angles in degrees.
let stored = powerio::PioModule::new(powerio::PioValue::AcOpfSolution(solution));
let json = tellegen::ir::serialize_module(&stored)?;
# Ok(())
# }
```

These are alternative entry points; calling both performs two solves. Inactive
source rows, including three-winding transformer terminals, remain unavailable
(`NaN` in Rust, the PowerIO unavailable representation in serialized modules).

### Reproducible external comparisons

The committed `acopf_benchmark` example accepts MATPOWER files and emits one
JSONL record per case, including input and executable SHA-256, elapsed time,
objective, model fingerprint, residuals, solver status, and failures. It exits
nonzero if any case fails, while continuing through the remaining cases.

```sh
cargo run -p tellegen --release --example acopf_benchmark --features acopf --locked -- \
  - /path/to/case14.m /path/to/case30.m /path/to/case300.m > results.jsonl
```

For an independent objective comparison, replace `-` with a JSON file containing
`provenance` (corpus revision, reference solver version/options, and result source)
and `objectives` (a map from exact input SHA-256 to its reference objective).
Missing checksums fail rather than comparing different case revisions. The
comparison tolerance is `1e-4 * max(1, abs(reference objective))`. Keep that
reference file and the corpus provenance alongside the resulting JSONL. This
runner does not supply or regenerate an independent oracle: the frozen
14/30/300 references and the large PGLib rerun remain release gates.

## Browser ABI

POUNCE's proven browser target is a separate `wasm32-wasip1` module with its
WASI clock/random/output shim. A direct `wasm32-unknown-unknown`/wasm-bindgen
probe compiles and loads but traps when the solver first calls
`std::time::Instant::now()`. Therefore `tellegen-wasm` does not forward
`acopf`, and a browser AC OPF solve must use a dedicated, terminable worker with
a small typed message boundary and one POUNCE memory. There must be no
synchronous main-thread fallback.

A single wasm-bindgen module can be reconsidered after POUNCE has a portable
clock. That reconsideration must measure artifact size, memory growth, and hard
worker cancellation in real browsers.

## License and release gate

POUNCE is EPL-2.0 while Tellegen is MIT. `deny.toml` admits EPL-2.0 only for the
named POUNCE crates, and `scripts/epl-guard.sh` proves POUNCE is absent from the
default engine and all shipping adapters while present in the opt-in probe.

That mechanical boundary is not legal approval. Before distributing a native
binary or WASI asset containing POUNCE, the release owner must approve the
EPL-2.0 obligations, include its license/notices, and provide a reasonable
corresponding-source location. Until then, the feature remains a
development-only native capability and `Problem::Acopf` remains unavailable in
default and shipping builds.

## Remaining release gates

Before distribution, replace the Git pin with a tagged POUNCE release, wire or
invoke its restoration and second-opinion path explicitly, and add a small
regression that proves restoration is exercised (`restoration_calls > 0`). Then
rerun the large PGLib comparison. Also record native peak memory and compare the
14/30/300 expressions against frozen benchmark values in addition to the
finite-difference checks above. A failure of expression-DAG scaling changes the
private solver adapter, not the PowerIO problem or solution contract.

### Input preparation safeguards

PowerIO 0.11.3 aggregates load and shunt injections without checking service
status. Tellegen therefore masks inactive injections in a private instance copy
before preparation, retaining all source rows, units, objective, selected
constraints, and initial-point identities. The caller's instance is unchanged.
As with the existing models, a network claiming to be normalized while still
carrying inactive loads/shunts or isolated buses is rejected. Active voltage
and generator bounds must be finite and ordered; invalid bounds return a
source-identifying error before starting-point clamping.

