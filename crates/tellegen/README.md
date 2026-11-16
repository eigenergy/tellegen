# tellegen

`tellegen` solves PowerIO problem instances and computes their implicit
sensitivities. It supports DC OPF, AC power flow, and the SOCWR relaxation of AC
OPF. Clarabel solves the convex problems. The same crate builds for native
targets and WebAssembly.

PowerIO modules are the portable input and persistence boundary. Tellegen
converts their networks and problem instances into private dense solver
workspaces.

## Status

DC OPF and AC power flow are included by default. The `conic` feature adds
SOCWR. The `sensitivity` feature supplies the
implicit derivative API and the retained `Study` runtime.

The opt-in native `mc-opf` feature adds experimental unbalanced IVR AC OPF,
retained `McOpfSession` edits and warm starts, complete physical reports, and
qualified local forward/adjoint sensitivities. These use separate Rust APIs;
the generic `SolveRequest` and `Study` interfaces do not expose MC OPF.

Active three winding transformers are lowered to an equivalent star network for
solving and display. Tellegen currently rejects closed transmission switches,
in-service storage, and in-service HVDC links instead of silently omitting them;
open or out-of-service records remain valid metadata. Branch angle limits written
as `0/0` or with an unconstrained half-window of at least 90 degrees use Tellegen's
documented default of ±60 degrees.

## Use

`solve_module_json` accepts a PowerIO IR (`pio-ir`) document and a
`SolveRequest`. A module holding a balanced network is promoted to the default
problem instance for the requested formulation. A module holding a declared
problem instance keeps its objective and active constraint selections.

```rust,ignore
let source = powerio::Source::from_memory("case.m", case_text.as_bytes().to_vec())?;
let parsed = powerio::parse(source)?;
let module = tellegen::ir::balanced_module(parsed)?;
let module_json = tellegen::ir::serialize_module(&module)?;
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

For a complete native MC OPF workflow, run from the repository root:

```sh
cargo run -p tellegen --no-default-features --features mc-opf --locked \
  --example mc_opf_workflow
```

The example solves the bundled four-bus feeder, edits load power, warm-starts,
round-trips the result and reuses one sensitivity factorization. It writes a
summary to stdout without creating files. To use a declared PowerIO
`McAcOpfInstance` module and a JSON array of `McOpfEdit` values, append
`-- instance.module.json edits.json [result.module.json]`. The optional result
path saves the complete report. See the repository book's
[public native workflow](../../docs/src/formulations.md#public-native-workflow)
for units, input semantics and qualification/error handling.

## API compatibility

`solve_instance`, `solve_ac_pf_instance` and `solve_ac_instance` take PowerIO
typed problem instances, and the module JSON entry points take generation-2
`pio-ir` documents. Use the PowerIO release this crate depends on; a different
minor version of PowerIO has different types. Changes between releases are in
the [changelog](CHANGELOG.md).

## Build

```sh
cargo test                          # native, with sensitivities (default features)
cargo build --no-default-features   # solve only, no faer (smaller wasm core)
```

For all shipping numerical paths:

```sh
cargo test -p tellegen --features sensitivity,mc-pf,conic,acopf,mc-opf,schema
```

The `powerio-012` feature is a no-op kept for existing build commands; PowerIO
0.12 is the only supported dependency line.

## Citing

If you use this crate in published work, please cite the tellegen paper:
Samuel Talkington, Frederik Geth, Qian Zhang, Le Xie, and Skyler Liu. 2026.
*Interactive Power Flow in the Browser*. The Inaugural ACM Conference on Digital
Transformation (ACM DXConf 2026), Ann Arbor, MI, USA.
[doi:10.1145/3847238.3849038](https://doi.org/10.1145/3847238.3849038). The
[repository README](https://github.com/eigenergy/tellegen#citing) has the BibTeX
entry.

## License

MIT. See LICENSE.
