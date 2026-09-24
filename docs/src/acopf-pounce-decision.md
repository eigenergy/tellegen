# AC OPF solver decision

Status: accepted for feasibility work; distribution and product capability are
not yet approved. Recorded 24 September 2026.

## Decision

Tellegen will use POUNCE as its first nonlinear solver for canonical balanced
transmission AC OPF. The choice favors an end-to-end Rust implementation and a
responsive upstream over building two solver adapters in the first cycle. The
internal model/solver boundary must remain backend-neutral.

The decision follows POUNCE's
[restoration fix](https://github.com/jkitchin/pounce/pull/961) and the
[AC OPF follow-up experiments](https://github.com/jkitchin/pounce/issues/965).
Those results are feasibility and implementation evidence, not general solver
performance guarantees.

This decision does not relabel the existing SOCWR relaxation as AC OPF and does
not make `Problem::Acopf` available. Product capability requires the exact
PowerIO-prepared polar model, independent primal validation, truthful status
mapping, portable `AcOpfSolution` emission, and the browser worker boundary.

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

PowerIO 0.11.3 is the released preparation boundary for the next increment. It
contains `AcOpfInstance`, `build_ac_opf_preparation`, and `AcOpfSolution`; the
model compiler must consume those APIs rather than reinterpret MATPOWER data or
reuse Tellegen's modified CATS power-flow physics.

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
corresponding-source location. Until then, the feature remains a development
probe and `Problem::Acopf` remains unavailable.

## Next gate

The next branch may build 3-, 14-, and 300-bus expression prototypes only
after it records model-construction time and memory and compares objective,
constraints, sparse Jacobian, and Lagrangian Hessian against the frozen
benchmark and finite differences. A failure of expression-DAG scaling changes
the private solver adapter, not the PowerIO problem or solution contract.
