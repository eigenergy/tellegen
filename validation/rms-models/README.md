# RMS models: independent references

Reference outputs for the RMS models that the PowerSimulationsDynamics.jl
comparison of `crates/tellegen/tests/rms_psid_validation.rs` did not cover yet.
The scripts here write small JSON files under `crates/tellegen/tests/data`;
the Rust tests read those files, so the test suite needs no Julia.

`Manifest.toml` pins the environment: Julia 1.11.9, PowerSimulationsDynamics.jl
0.16.2, PowerSystems.jl 5.12.5, OrdinaryDiffEq.jl 6.111.0, Sundials.jl, JSON3.jl.
Instantiate it once:

```sh
julia --project=validation/rms-models -e 'using Pkg; Pkg.instantiate()'
```
