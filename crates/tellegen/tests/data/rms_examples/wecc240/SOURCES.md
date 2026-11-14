# WECC 240-bus dynamic example

- `case.raw`, `case.dyr`: `test/data_tests/240busWECC_2018_PSS32_fixed_shunts.raw` and
  `240busWECC_2018_PSS.dyr` from PowerSimulationsDynamics.jl (commit dfb56d80, BSD-3-Clause,
  see LICENSE-PowerSimulationsDynamics.jl). The model is the reduced 240-bus WECC system of
  H. Yuan, R. S. Biswas, J. Tan, Y. Zhang, "Developing a Reduced 240-Bus WECC Dynamic Model for
  Frequency Response Study of High Renewable Integration", IEEE PES T&D 2020.
- `case.geo.csv`: bus latitude and longitude from `wecc240/data/bus_gis.csv` of
  https://github.com/eudoxys/wecc240 (commit b60cf13bb2fb4d5a141d706613f42d2581b42ca6, MIT License, Copyright (c) 2025
  David P. Chassin; see LICENSE-wecc240-coordinates),
  reduced to `bus_i,lat,lon,name`.

- `psid_reference.json`: results of PowerSimulationsDynamics.jl 0.16.2 (unmodified) on this case
  and on the single-machine cases built from it (`tests/support/smib.rs`), the reference of
  `tests/rms_psid_validation.rs`, run as installed (no function overridden). The WECC 240
  runs use the system of PowerSimulationsDynamics.jl's tutorial (PowerSystemCaseBuilder.jl
  "WECC 240 Bus", BSD-3-Clause, see LICENSE-PowerSystemCaseBuilder.jl), whose loads are stored
  as constant power at the solved voltage; the file also carries those load values, the only
  data in it that is not computed output. Per single-machine run it holds the compared channels
  and their tolerances (three times tellegen's difference at 1 ms in the validation study); per
  WECC 240 scenario the channels with the largest difference of each quantity in that study and
  the response of the whole system (lowest and highest speed, mean speed at the end, largest
  rotor angle swing, lowest voltage), computed from every channel of the reference. Channels are sampled every 20 ms (single machine) or 100 ms
  (WECC 240) and rounded to seven significant digits.
