# tellegen

Differentiable optimal power flow and sensitivities, in Rust.

`tellegen` solves the DC optimal power flow, the AC power flow, and the SOCWR (Jabr) conic
relaxation of AC OPF, and computes analytical KKT sensitivities for each through one
unified `sensitivity(operand, parameter)` function.
Case files are parsed through [`powerio`](https://github.com/eigenergy/powerio).
Convex problems are solved with the Clarabel solver.

`tellegen` compiles to both native targets and WebAssembly, so it runs on a server and in
the browser. The full nonlinear AC OPF is on the roadmap; it runs natively, where it can
use threads.

## Status

Early (v0.1.0). DC OPF (locational marginal prices, branch flows, generator dispatch),
AC power flow voltage sensitivities, and the SOCWR conic relaxation with sensitivities —
all under the one object-safe `Differentiable` sensitivity contract.

## Use

`solve_json` is the one front door: a `SolveRequest` in, a `SolveResponse` out.
`capabilities_json` reports which `(formulation, operand, parameter)` combinations a
build supports.

```rust
let network_json = powerio::parse_str(case_text, "matpower")?.network.to_json()?;
let request = r#"{
    "formulation": "dcopf",
    "edits": { "deltas": { "2": 50.0 } },
    "sensitivities": [
        { "operand": {"Price":"Active"}, "parameter": {"Demand":"Active"} }
    ]
}"#;
let out = tellegen::solve_json(&network_json, request)?; // { lmp, flows, dispatch, sensitivities, ... }
```

For a server that solves the same case repeatedly, build the model once and reuse it:

```rust
let net = powerio::parse_str(case_text, "matpower")?.network;
let dc = tellegen::DcNetwork::from_network(&net)?;       // build once
let out = tellegen::solve_prebuilt(&dc, &tellegen::SolveRequest::default())?;
```

## Build

```sh
cargo test                          # native, with sensitivities (default features)
cargo build --no-default-features   # solve only, no faer (smaller wasm core)
```

## License

Dual-licensed under either Apache-2.0 or MIT, at your option. See LICENSE-APACHE,
LICENSE-MIT, and NOTICE.
