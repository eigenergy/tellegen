# tellegen

`tellegen` solves PowerIO problem instances and computes their implicit
sensitivities. It supports DC OPF, AC power flow, and the SOCWR relaxation of AC
OPF. Clarabel solves the convex problems. The same crate builds for native
targets and WebAssembly.

PowerIO modules are the portable input and persistence boundary. Tellegen
converts their networks and problem instances into private dense solver
workspaces.

## Status

Version 0.2.0 is under development. DC OPF and AC power flow are included by
default. The `conic` feature adds SOCWR. The `sensitivity` feature supplies the
implicit derivative API and the retained `Study` runtime.

Active three winding transformers are lowered to an equivalent star network for
solving and display. Tellegen currently rejects closed transmission switches,
in-service storage, and in-service HVDC links instead of silently omitting them;
open or out-of-service records remain valid metadata. Branch angle limits written
as `0/0` or with an unconstrained half-window of at least 90 degrees use Tellegen's
documented default of ±60 degrees.

## Use

`solve_module_json` accepts a stored `powerio.module/1` document and a
`SolveRequest`. A module holding a balanced network is promoted to the default
problem instance for the requested formulation. A module holding a declared
problem instance keeps its objective and active constraint selections.

```rust,ignore
let source = powerio::Source::from_bytes(
    "case.m",
    case_text.as_bytes().to_vec(),
)?;
let parsed = powerio::parse(source)?;
let module: powerio::PioModule<powerio::BalancedNetwork> =
    powerio::try_into_typed(parsed)?;
let module = module.map_value(powerio::PioValue::from);
let module_json = powerio::stored::write_module(&module)?;
let request = r#"{
    "formulation": "dcopf",
    "edits": { "deltas": { "2": 50.0 } },
    "sensitivities": [
        { "operand": {"Price":"Active"}, "parameter": {"Demand":"Active"} }
    ]
}"#;
let response_json = tellegen::solve_module_json(&module_json, request)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

`Study` retains one module for repeated preview and commit calls. Each commit
solves from the retained base module plus the complete current edit set.

```rust,ignore
use tellegen::{ElementKey, NetworkEdit, Problem, Study};

let mut study = Study::new(&module_json, Problem::DcOpf)?;
let response = study.commit(&[NetworkEdit::AddLoad {
    bus: ElementKey::Id(2),
    p_mw: 50.0,
}])?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

`capabilities_json` reports the formulations and sensitivity cells compiled
into the current build.

## Build

```sh
cargo test                          # native, with sensitivities (default features)
cargo build --no-default-features   # solve only, no faer (smaller wasm core)
```

## License

Dual-licensed under either Apache-2.0 or MIT, at your option. See LICENSE-APACHE,
LICENSE-MIT, and NOTICE.
