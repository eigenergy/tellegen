# AC optimal power flow

AC OPF minimizes generation cost subject to nonlinear AC power balance,
voltage bounds, generator limits and branch limits. Tellegen uses POUNCE and
checks the electrical equations and limits after the solver returns.
A successful solver status alone does not establish a usable result.

## Browser

The demo runs AC OPF in a separate worker. Its WebAssembly file is built and
released separately from the ordinary engine because POUNCE uses the EPL-2.0
license. See [licenses](third-party-notices.md) and
[release instructions](release-architecture.md#separate-ac-opf-download).
The app must finish loading that file before AC OPF is available.

Large or difficult cases may take substantially longer than a power flow or
DC OPF. A cancellation stops the worker. Compare timings for the same case,
settings and initial state; browser startup time and numerical solve time
are different measurements.

## Native Rust

Enable the `acopf` feature. This example accepts a stored PowerIO AC OPF
instance and returns the solve result:

```rust,no_run
# extern crate tellegen;
# extern crate powerio;
use tellegen::{Problem, SolveRequest};

fn solve(module_json: &str) -> Result<(), String> {
    let module = tellegen::ir::deserialize_module(module_json)?;
    let powerio::PioValue::AcOpfInstance(instance) = module.into_value() else {
        return Err("expected an ac_opf_instance".into());
    };
    let request = SolveRequest {
        formulation: Problem::Acopf,
        ..Default::default()
    };
    let result = tellegen::solve_ac_opf_instance(&instance, &request)?;
    println!("{:?}", result.status);
    Ok(())
}
```

For the equations, supported inputs and multiconductor formulation, see
[calculations](formulations.md). See [validation](validation.md) for numerical
comparisons and their limits. Historical development measurements are kept in
[`validation/history`](https://github.com/eigenergy/tellegen/tree/main/validation/history).
