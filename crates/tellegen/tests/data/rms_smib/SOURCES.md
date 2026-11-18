# Single-machine model variants

`psid_reference_models.json`: results of PowerSimulationsDynamics.jl 0.16.2 (unmodified, run as
installed, no function overridden) on the single-machine cases of `model_variant_cases` in
`tests/support/smib.rs`, the reference of `smib_model_variants_against_psid` in
`tests/rms_psid_validation.rs`. It holds computed output only.

The cases cover the dynamic models tellegen accepts that no WECC 240 unit carries: GENROE,
GENSAL, GENSAE and GENCLS among the machines, ESAC1A and EXST1 among the exciters. Each is the
single-machine case of WECC 240's GENROU + SEXS + TGOV1 unit (`smib_genrou_sexs_tgov1`, built
from `tests/data/rms_examples/wecc240`, BSD-3-Clause, see `LICENSE-PowerSimulationsDynamics.jl`)
with one record replaced: the GENROU record as GENROE, its salient-pole subset as GENSAL and
GENSAE, its H and D as GENCLS (without exciter and governor), or SEXS replaced by the typical
ESAC1A or EXST1 set of tellegen's model library (`src/rms/library.rs`). Events: the trip of one
of the two lines at 1.0 s, and a three-phase fault at the unit through half its connection's
reactance from 1.0 to 1.1 s.

Where the exciter sets come from:

- ESAC1A: the exciters of PowerSimulationsDynamics.jl's IEEE 14-bus benchmark
  (`test/benchmarks/psse/GAST/dyn_data.dyr` of PowerSimulationsDynamics.jl 0.16.2, with
  `14bus.raw`; BSD-3-Clause, see `LICENSE-PowerSimulationsDynamics.jl`), unchanged.
- EXST1: the typical set of tellegen's model library; no published source is recorded for it.

`smib_genrou_esac1a_lim_tgov1` runs ESAC1A once more with its limits narrowed around the
operating point (at rest VA = VR = 2.92): VAMAX/VAMIN = 4/2.2, VRMAX/VRMIN = 3.5/2.5. In
PowerSimulationsDynamics.jl's run of the fault, VA rests on VAMAX for 0.77 s and on VAMIN for
0.23 s, and VR passes both of its limits; after the trip VA rests on VAMIN for 1.03 s. So the
limits themselves are compared, through the machine's response.

Per run the file holds PowerSimulationsDynamics.jl's MassMatrixModel solution with Rodas4 at
reltol = abstol = 1e-8 (saved every 10 ms, every second sample kept: 20 ms) over 3 s and
rounded to seven significant digits, and per compared channel a tolerance: three times tellegen's difference at a 1 ms step, at
least 1e-6, as in the validation study. Every machine's speed and rotor angle also meet the
test's physical tolerances (1e-4 pu, 0.5°).
