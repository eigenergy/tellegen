# Single-machine references for the extension models

`generate.jl` runs PowerSimulationsDynamics.jl 0.16.2 (as installed, no
function overridden) on the single-machine cases of ESST1A and PSS2B
(`ext_model_variant_cases` in `crates/tellegen/tests/support/smib.rs`) and
writes `crates/tellegen/tests/data/rms_smib/psid_reference_ext_models.json`,
the reference of `smib_ext_models_against_psid` in
`crates/tellegen/tests/rms_psid_validation.rs`. The cases, events, channels
and the PowerSimulationsDynamics.jl details are described in
`crates/tellegen/tests/data/rms_smib/SOURCES.md`.

The script builds the cases itself from the WECC 240 files in
`crates/tellegen/tests/data/rms_examples/wecc240`, as the Rust support code
does, and stores the raw and DYR text in the reference; the test fails if its
own cases differ. Each run takes a few seconds after a compile of about two
minutes.

The tolerances are three times tellegen's difference at a 1 ms step, so the
reference is written in two passes:

```sh
# 1. PowerSimulationsDynamics.jl results (every tolerance at the floor 1e-6)
julia --project=validation/rms-models validation/rms-models/psid-smib/generate.jl
# 2. tellegen's differences to them, with held and with switching limits
RMS_PSID_MEASURED=/path/to/measured.json cargo test --release -p tellegen \
    --features rms-experimental --test rms_psid_validation smib_ext_models_against_psid
# 3. the same results with the tolerances from those differences
julia --project=validation/rms-models validation/rms-models/psid-smib/generate.jl /path/to/measured.json
```

The second step fails (the floor is below most differences) but writes the
file. PowerSimulationsDynamics.jl's results are the same in every pass.
