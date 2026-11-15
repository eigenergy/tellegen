# Dynamic example cases

Code and data are kept apart. One case ships with the code:

| Case | Folder | Source and license |
|---|---|---|
| WECC 240-bus | `wecc240` | PowerSimulationsDynamics.jl, BSD-3-Clause; coordinates MIT |

The tests build further single-machine cases from it in code (`../../support/smib.rs`).

Every other case (PowerSimulationsDynamics.jl's PSS/E benchmark cases, Kundur, Nordic 44, ...)
is data under its own license and is not part of this repository. The tests look for a case
`<name>` (a folder with `case.raw`, `case.dyr` and its `SOURCES.md` and license file) in

1. `$TELLEGEN_CASE_DATA/rms_examples/<name>`: a checkout of a data repository, or
2. `data/rms_examples/<name>` at the workspace root (`data/` is git-ignored).

PowerSimulationsDynamics.jl's PSS/E benchmark cases and its results on them are read from
`$TELLEGEN_CASE_DATA/rms_validation` and `$TELLEGEN_CASE_DATA/psd-reference/refs`.

A test on a case that is not there is skipped and says so on stderr. Set
`TELLEGEN_REQUIRE_CASE_DATA=1` to make a missing case a failure, as on a machine that is meant
to run everything. The lookup is `../../support/case_data.rs`.
