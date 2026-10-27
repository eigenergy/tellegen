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

Active three winding transformers are lowered to an equivalent star network for
solving and display. Tellegen currently rejects closed transmission switches,
in-service storage, and in-service HVDC links instead of silently omitting them;
open or out-of-service records remain valid metadata. Branch angle limits written
as `0/0` or with an unconstrained half-window of at least 90 degrees use Tellegen's
documented default of ±60 degrees.

## Use

`solve_module_json` accepts a generation-2 `pio-ir` document and a
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

## API compatibility

The 0.3 release uses PowerIO 0.11 typed problem instances and generation-2
`pio-ir` documents. Rust consumers must use the matching PowerIO types when
calling `solve_instance`, `solve_ac_pf_instance`, or `solve_ac_instance`.
The module JSON entry points require the same generation-2 input format.
These input changes require migration from the 0.2 API and are released as a
new minor version, rather than a compatible patch to 0.2.

0.4 makes a multiconductor session edit constant-size: `McPfSession::
replace_load_powers` returns the `McPfSummary` of the new operating point
instead of the full result. `McPfSession::result` and `McPfSession::instance`
are gone; call `build_result()` for the full terminal and equipment result,
`detail()` for one bus and a page of equipment ports, and `base_instance()` or
`edited_instance()` for the instance.

Persistent investigations use `document::StudyDocument` and the operations in
`study_ops`, with immutable states, goal revisions and explicit application.
The document's schema version identifies its storage format separately from
the crate version and PowerIO IR generation.

## Build

```sh
cargo test                          # native, with sensitivities (default features)
cargo build --no-default-features   # solve only, no faer (smaller wasm core)
```

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
