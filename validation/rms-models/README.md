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

| Folder | What it writes |
| --- | --- |
| `psid-smib/` | `crates/tellegen/tests/data/rms_smib/psid_reference_ext_models.json`: single-machine runs of ESST1A and PSS2B in PowerSimulationsDynamics.jl |
| `reduced-controllers/` | `crates/tellegen/tests/data/rms_reduced_controllers/`: closed-loop single-machine runs of ESAC6A, the OELIT/UELPQ limiters, the AVR load compensator, the summing-point UEL and the GAST valve rate limit, each written as a PowerSimulationsDynamics.jl extension |
| `reduced-network/` | `crates/tellegen/tests/data/rms_reduced_network/`: small networks for load recovery, saturation and rise, the `PQBRAK` rounding, the voltage exponent of a plant's reactive set point and HVDC links in AC emulation (time domain and inter-area mode) |
