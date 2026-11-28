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

# Extension models (ESST1A, PSS2B)

`psid_reference_ext_models.json`: results of PowerSimulationsDynamics.jl 0.16.2 (unmodified, run
as installed, no function overridden) on the single-machine cases of `ext_model_variant_cases` in
`tests/support/smib.rs`, the reference of `smib_ext_models_against_psid` in
`tests/rms_psid_validation.rs` (feature `rms-experimental`). It is written by
`validation/rms-models/psid-smib/generate.jl` and holds computed output, and per case the raw and
DYR text it was computed from (the test checks them against its own cases).

The cases are the single-machine case of WECC 240's GENROU + SEXS + TGOV1 unit
(`smib_genrou_sexs_tgov1`, as above) with SEXS replaced by ESST1A or with PSS2B added, through the
same events (the trip of one of the two lines at 1.0 s, a three-phase fault at the unit through
half its connection's reactance from 1.0 to 1.1 s):

- `smib_genrou_esst1a_tgov1`: the ESST1A set of tellegen's model library (`src/rms/library.rs`),
  the ESST1A of PowerSystems.jl 5.12.5's `test/test_dynamic_generator.jl` (BSD-3-Clause):
  UEL 1, VOS 1, TR 0, VIMAX/VIMIN 99/−99, TC/TB 2.5/13.25, TC1/TB1 0/0, KA 200, TA 0.1,
  VAMAX/VAMIN 9.5/−9.5, VRMAX/VRMIN 9.5/−9.5, KC 0, KF 0, TF 1, KLR 0, ILR 999.
  PowerSimulationsDynamics.jl divides by TB1 when it initializes the second
  lead-lag, so it is given TC1 = TB1 = 1 for the bypassed block: the same unit transfer function,
  its state stays at zero.
- `smib_genrou_esst1a_lim_tgov1`: that set with its limits narrowed around the operating point (at
  rest VI = 0.0098, VA = Efd = 1.956, field voltage limits Vt VRMIN = 1.88 and
  Vt VRMAX − KC Ifd = 2.64): VIMAX/VIMIN = 0.1/0.006, VAMAX/VAMIN = 3/1.8,
  VRMAX/VRMIN = 2.6/1.85. In PowerSimulationsDynamics.jl's run of the fault, VI rests on VIMAX for
  0.10 s, VA on VAMAX for 0.07 s and the field voltage on its upper limit for 0.20 s; after the
  trip VI rests on VIMIN for 2.00 s (to the end of the run), VA on VAMIN for 1.58 s and the field
  voltage on its lower limit for 1.95 s (10 ms samples). So every limit is compared, through VA,
  the field voltage and the machine's response.
- `smib_genrou_pss2b_sexs_tgov1`: the unit with the PSS2B set of tellegen's model library, the
  PSS2B of PowerSimulationsDynamics.jl 0.16.2's `test/test_case53_pss2b.jl` (BSD-3-Clause): speed
  (input code 1) and electrical power (code 3) inputs, TW1 = TW2 = TW3 = 1.5, T6 = TW4 = 0,
  T7 = 1.5, KS2 = T7 / (2H) = 0.3112 for the unit's H = 2.41 s (the 0.1875 of that test and of
  the library is for H = 4 s), KS3 = 1, ramp-tracking filter T8 = 0.5, T9 = 0.1, M = 5, N = 1,
  KS1 = 2, T1/T2 = T3/T4 = 0.59451/0.0447, T10/T11 = 1/1, VST ±0.1. That test limits the inputs to
  a narrow window around its own operating point (VS1 −0.00055 to 0.00035, VS2 0.895 to 0.915);
  the library leaves them open (VS1, VS2 ±99). PowerSystems.jl 5.12.5 does not read PSS2B
  records; the script attaches the
  stabilizer to the parsed unit, as PowerSimulationsDynamics.jl's own PSS2B test does. The fault
  drives its output to both limits.
- `smib_genrou_esst1a_pss2b_tgov1`: the library ESST1A and that PSS2B together.

The fault is a `NetworkSwitch` to the admittance matrix with the fault's shunt at bus 102 and back,
the trip a `BranchTrip` of circuit 2. Per run the file holds PowerSimulationsDynamics.jl's
MassMatrixModel solution with Rodas4 at reltol = abstol = 1e-8 (saved every 10 ms, every second
sample kept: 20 ms) over 3 s, rounded to seven significant digits. Besides the channels above it
holds ESST1A's VA (the state of its non-windup lag, tellegen's `sig:exciter.Va`) and PSS2B's output
(`vs`), the latter computed from the saved states with PowerSimulationsDynamics.jl's own block
functions as its model does (its post-processing has no PSS2B output). ESST1A's field voltage is
computed from the saved states as the model computes it: PowerSimulationsDynamics.jl 0.16.2's
field current post-processing for GENROU (`_field_current` in `post_proc_generator.jl`) reads the
state ψ_kd where the model has ψ_kq, which would show on ESST1A's upper field voltage limit
`Vt VRMAX − KC Ifd` wherever KC is not zero.

Per compared channel the file holds two tolerances, each three times tellegen's difference at a
1 ms step, at least 1e-6 (rounded up to four significant digits): `tolerance` with tellegen's
default held limits (`RmsOptions::freeze_limits`), `tolerance_free_limits` with limits switching
within the step. Every machine's speed and rotor angle also meet the test's physical tolerances
(1e-4 pu, 0.5°) in both.
