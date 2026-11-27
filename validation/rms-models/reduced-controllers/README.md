# Controllers without a PowerSimulationsDynamics.jl model

References for the controller models of tellegen that PowerSimulationsDynamics.jl does not
implement: ESAC6A, the OELIT and UELPQ limiters at an exciter's summing point, the AVR load
compensator (with and without its lag), the summing-point under-excitation limiter and the GAST
fuel valve rate limit.

- `controllers.jl` writes each controller from IEEE Std 421.5-2005, the PSS/E model descriptions
  or tellegen's doc comments (never from tellegen's code) as a PowerSimulationsDynamics.jl
  extension: a new PowerSystems.jl `AVR` or `TurbineGov` type with the model and initialization
  methods PowerSimulationsDynamics.jl dispatches on.
- `run.jl` runs them closed loop on two single-machine cases of WECC 240 (in
  `crates/tellegen/tests/data/rms_reduced_controllers`) through line trips, faults and
  reference steps, checks the extension against PowerSimulationsDynamics.jl's own SEXS and GasTG,
  and writes `closed_loop.json` there.

```sh
JULIA_PKG_OFFLINE=true julia --project=validation/rms-models \
    validation/rms-models/reduced-controllers/run.jl
```

About three minutes, most of it compilation. The Rust test
`crates/tellegen/tests/rms_reduced_controllers.rs` reads the file; `SOURCES.md` next to it states
the equations' sources, the solver settings and how the tolerances in `tolerances.json` were
derived (`RMS_REDUCED_REPORT=1` prints tellegen's differences).
