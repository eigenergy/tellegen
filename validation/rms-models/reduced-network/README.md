# Reduced networks: references for network-level models

Independent references for RMS models that PowerSimulationsDynamics.jl does
not implement, each on a small network that
`crates/tellegen/tests/rms_reduced_network.rs` writes inline:

- `loads.jl`: load recovery, load saturation, the one-sided rise of load
  reactive power and the PSS/E `PQBRAK` breakpoint with and without its
  rounding, on a feeder with a classical machine (`loads.json`);
- `plant.jl`: the voltage exponent of a renewable plant's reactive power set
  point, PowerSimulationsDynamics.jl with the exponent added to its plant
  controller (`plant.json`);
- `hvdc.jl`: HVDC links in AC emulation between two classical machines, time
  domain and the inter-area mode (`hvdc.json`);
- `common.jl`: the shared model (network, classical machines, sources, the
  load laws, the links), its integration, the PowerSimulationsDynamics.jl
  runs of the same cases and the output;
- `tolerances.jl`: writes the tolerances into the reference files from
  tellegen's measured differences.

The equations, solver settings and cross-checks are in
`crates/tellegen/tests/data/rms_reduced_network/SOURCES.md`. The models are
written from the documented equations of each switch, not from tellegen's
code.

To regenerate (the pinned environment of `validation/rms-models`; about seven
minutes, most of it compilation):

```sh
for s in loads plant hvdc; do
    julia --project=validation/rms-models validation/rms-models/reduced-network/$s.jl
done
RMS_REDUCED_MEASURE=1 cargo test -p tellegen --features rms-experimental \
    --test rms_reduced_network -- --nocapture > measured.txt
julia --project=validation/rms-models validation/rms-models/reduced-network/tolerances.jl measured.txt
```

The measuring run prints, per reference run, tellegen's largest difference
of each channel at steps of 2, 1 and 0.5 ms; the tolerances come from the
1 ms column.
