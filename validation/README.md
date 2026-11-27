# Validation

Comparisons of tellegen against other tools, and the records of studies run
with it. Nothing here ships in a package. `scripts/` holds the build, release
and data-staging tools; the mdBook is under `docs/`.

| Folder | What it holds |
| --- | --- |
| `dc-variants/` | The five DC branch conventions compared with PowerModels (Julia) and replayed through AC. Run with `dc-variants/run.sh`. |
| `mc-opf/` | Multiconductor OPF compared with BMOPFTools (Julia). `mc_opf_oracle.jl` writes the reference files under `crates/tellegen/tests/data/mc_opf`. |
| `rms-models/` | Independent references for the RMS models: PowerSimulationsDynamics.jl single-machine runs, and closed-loop reduced systems for the models it does not implement. The scripts write the reference files under `crates/tellegen/tests/data/rms_*`. |
| `mc-pf/` | Multiconductor power flow: the test cases, the comparison with OpenDSS and the notes from the reader audit. |
| `evidence/` | Recorded study runs with the scripts that reproduce them: capacity planning, diagrams, load models, primitive admittances, Studies, synthetic grids and usage reports. |

The Rust tests read reference outputs from `crates/tellegen/tests/data`, not
from here, so the test suite needs neither Julia nor OpenDSS.
