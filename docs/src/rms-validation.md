# RMS validation

The RMS engine is validated against
[PowerSimulationsDynamics.jl](https://github.com/NREL-Sienna/PowerSimulationsDynamics.jl)
0.16.2, an open-source tool, in the time domain. The validation runs as tests:

| Test | Cases | Data | Runs |
|---|---|---|---|
| `tests/rms_psid_validation.rs` | single-machine cases built from WECC 240 (the renewable plant included); WECC 240 | ships with the code | always |
| `tests/rms_validation.rs` | PowerSimulationsDynamics.jl's PSS/E benchmark cases (`test/benchmarks/psse`) | case data repository (`$TELLEGEN_CASE_DATA`) | where the data is |

No closed-source tool's output is used.

## Single-machine cases from WECC 240

One case per model combination of WECC 240 (`tests/support/smib.rs`): the first
in-service unit with the combination, with its DYR records, dispatch, MBASE,
`ZSORCE` and voltage setpoint, on the one-machine infinite-bus system of the
PowerSimulationsDynamics.jl PSS/E benchmarks (two parallel lines scaled with
MBASE to the unit). Two events each: one line opens at 1.0 s; a three-phase
fault at the unit through half its line reactance from 1.0 to 1.1 s.

The reference is PowerSimulationsDynamics.jl's mass-matrix model with Rodas4
at `reltol = abstol = 1e-8` (`1e-6` where it does not solve at `1e-8`).
tellegen runs at a 1 ms step. Every machine channel (`vm`, `va`, `omega`,
`delta`, `pe`, `qe`, `efd`, `tm`; angles relative to the infinite bus) must
stay within three times the difference of the validation study at 1 ms:

| Models | Event | Largest `|V|` difference (pu) | Largest relative difference |
|---|---|---|---|
| GENROU, SEXS, TGOV1 | trip | 1.5e-7 | 7e-6 (pe) |
| GENROU, SEXS, TGOV1 | fault | 1.3e-5 | 2e-3 (efd) |
| GENROU, SEXS, GAST | trip | 1.5e-7 | 8e-6 (pe) |
| GENROU, SEXS, GAST | fault | 1.3e-5 | 2e-3 (efd) |
| GENROU, SEXS, HYGOV | trip | 1.7e-8 | 5e-6 (va) |
| GENROU, SEXS, HYGOV | fault | 2.1e-7 | 3e-5 (pe) |
| GENROU, SEXS, GAST, IEEEST | trip | 1.3e-6 | 2e-4 (efd) |
| GENROU, SEXS, GAST, IEEEST | fault | 2.7e-6 | 6e-4 (efd) |
| GENROU, SEXS, HYGOV, IEEEST | trip | 4.5e-5 | 7e-4 (efd) |
| GENROU, SEXS, HYGOV, IEEEST | fault | 1.7e-6 | 3e-5 (efd) |
| GENROU, SEXS, TGOV1, IEEEST | trip | 5.0e-6 | 3e-4 (efd) |
| GENROU, SEXS, TGOV1, IEEEST | fault | 3.0e-6 | 6e-5 (efd) |

The differences at 1 ms are tellegen's step error: at 0.1 ms they shrink by
one to two orders of magnitude.

The renewable plant (`REGCA1` + `REECB1` + `REPCA1`) is compared against
IDA with `dtmax = 0.1 ms` on its voltage, angle and ten states, in three
events: the line trip and a shallow fault (through 8 times the line
reactance) keep its voltage above the low-voltage gain range, so they check
tellegen's own (PSS/E) plant; a deep fault (through 1/8 of it) passes that
range and runs with PowerSimulationsDynamics.jl's gain. Its network-side
powers are not compared, since PowerSimulationsDynamics.jl 0.16.2 derives
them in post-processing.

## WECC 240

The system of PowerSimulationsDynamics.jl's tutorial (PowerSystemCaseBuilder
"WECC 240 Bus"): the raw and dyr files of this repository with the loads as
that system stores them, constant power at the solved voltage, with its 37
renewable plants (`REGCA1` + `REECB1` + `REPCA1`) set, for this comparison, to
PowerSimulationsDynamics.jl's low-voltage active current gain. The reference
is its residual model with IDA (`dtmax = 1 ms`). Four scenarios over 20 s:

- the tutorial's event, a trip of line 1101–1401 at 1.0 s;
- a trip of Palo Verde unit 1431 N (2355 MW) at 1.0 s;
- three-phase faults at bus 1032 (through j0.05 pu) and at bus 1101 (through
  j0.01 pu), 1.0 to 1.1 s.

tellegen runs at its 5 ms step on its own double-precision network. Compared
are, every 100 ms, the channel with the largest difference of every quantity
in the validation study (bus angles relative to bus 3933, rotor angles to
unit 1032 C, a tripped unit up to its trip):

| Quantity | Largest difference | Where | Bound |
|---|---|---|---|
| bus \|V\| (pu) | 1.2e-5 | bus 1131, fault at 1101 | 3e-5 |
| bus angle (rad) | 5.1e-5 | bus 1034, unit trip | 1e-4 |
| machine speed (pu) | 3.2e-6 | 1032 G, fault at 1101 | 7e-6 |
| rotor angle (rad) | 2.2e-4 | 3133 NG, fault at 1101 | 5e-4 |
| machine P (pu, 100 MVA) | 3.8e-3 | 1431 G, unit trip | 8e-3 |
| machine Q (pu, 100 MVA) | 1.2e-3 | 1131 G, fault at 1101 | 3e-3 |
| mechanical torque (pu) | 3.4e-6 | 1032 C, fault at 1101 | 7e-6 |
| field voltage (pu) | 5.3e-4 | 6533 C, fault at 1101 | 1e-3 |
| plant states (pu) | 4.3e-5 | 1032 S, fault at 1032 | 1e-4 |

The response of the whole system, from every machine speed and rotor angle
and every bus voltage on the 10 ms grid (a tripped unit up to its trip):

| Quantity | Largest difference | Bound |
|---|---|---|
| lowest machine speed (pu) | 5.5e-7 | 1.2e-6 |
| highest machine speed (pu) | 5.9e-7 | 1.2e-6 |
| mean speed at 20 s (pu) | 3.1e-10 | 1e-9 |
| largest rotor angle swing (rad) | 2.4e-5 | 5e-5 |
| lowest bus voltage (pu) | 6.9e-6 | 1.5e-5 |
| lowest bus voltage at 20 s (pu) | 1.3e-6 | 3e-6 |

## PowerSimulationsDynamics.jl's PSS/E benchmark cases

`tests/rms_validation.rs` runs every case of PowerSimulationsDynamics.jl's
`test/benchmarks/psse` in tellegen and compares every channel the two tools
share. The cases and PowerSimulationsDynamics.jl's results on them are data,
read from `$TELLEGEN_CASE_DATA/rms_validation` and
`$TELLEGEN_CASE_DATA/psd-reference/refs`.

- Stage A (single-machine ladder and the IEEE 14-bus system): every shared
  channel within 1e-3.
- Stage B (the remaining cases, one model each): the same comparison, reported.

| Case | Models | Largest difference |
|---|---|---|
| `smib_gencls` | GENCLS (infinite bus), GENCLS | 3.1e-4 |
| `smib_genrou` | GENCLS (infinite bus), GENROU | 4.8e-4 |
| `smib_gensal` | GENCLS (infinite bus), GENSAL | 1.8e-4 |
| `smib_genrou_sexs` | GENCLS (infinite bus), GENROU, SEXS | 4.8e-4 |
| `smib_genrou_esac1a` | GENCLS (infinite bus), GENROU, ESAC1A | 1.3e-4 |
| `smib_gensal_exst1` | GENCLS (infinite bus), GENSAL, EXST1 | 1.3e-4 |
| `smib_genrou_sexs_tgov1` | GENCLS (infinite bus), GENROU, SEXS, TGOV1 | 4.8e-4 |
| `smib_genrou_esac1a_gast` | GENCLS (infinite bus), GENROU, ESAC1A, GAST | 1.3e-4 |
| `smib_genrou_sexs_hygov` | GENCLS (infinite bus), GENROU, SEXS, HYGOV | 4.8e-4 |
| `smib_genrou_sexs_ieeest` | GENCLS (infinite bus), GENROU, SEXS, IEEEST | 1.1e-4 |
| `ieee14` | GENROU, ESAC1A, GAST | 1.3e-4 |
| `omib_gencls` (B) | GENCLS | 3.1e-4 |
| `genrou`, `genrou_nosat`, `genrou_highsat` (B) | GENCLS, GENROU | 4.8e-4 to 5.8e-4 |
| `genroe`, `genroe_highsat` (B) | GENCLS, GENROE | 5.1e-4 |
| `gensal` (B) | GENCLS, GENSAL | 5.1e-4 |
| `gensae` (B) | GENCLS, GENSAE | 1.2e-4 |
| `sexs`, `sexs_note` (B) | GENCLS, GENROU, SEXS | 6.2e-4, 1.2e-3 |
| `esac1a`, `esac1a_sat` (B) | GENCLS, GENROU, ESAC1A | 2.9e-3, 1.1e-2 |
| `ieeest`, `ieeest_filter` (B) | GENCLS, GENROU, SEXS, IEEEST | 2.5e-4, 1.2e-4 |
| `tgov1` (B) | GENCLS, GENROU, ESAC1A, TGOV1 | 1.2e-3 |
| `gast` (B) | GENCLS, GENROU, ESAC1A, GAST | 1.2e-3 |
| `hygov` (B) | GENCLS, GENROU, SEXS, HYGOV | 6.2e-4 |
| `load_constant_power`, `load_constant_current` (B) | GENCLS, GENROU | 3.1e-4, 2.8e-4 |
| `tvc_exst1` (B) | GENCLS (infinite bus), GENSAL, EXST1, HYGOV | 1.1e-2 |

Every single branch trip of the IEEE 14-bus system runs to completion
(`ieee14_every_branch_trip_completes`). After the trip of line 1-2 the slack
machine's ESAC1A (KC = 0.2) chatters between its rectifier and limiter bounds;
a damped Newton iteration handles it, and the run agrees with
PowerSimulationsDynamics.jl to 7.8e-4 in voltage and 5e-5 in speed
(`ieee14_line_1_2_against_psd`).

## Findings

**Tripped branch flows.** After a trip, PowerSimulationsDynamics.jl's
branch-flow post-processing still evaluates the opened line's pi model (about
0.53 pu); tellegen reports zero. Those channels are not compared.

**GENSAE electrical power.** PowerSimulationsDynamics.jl 0.16.2 computes the
GENSAE stator current in post-processing with the GENSAL formula
(`post_proc_generator.jl`): its `pe` at t = 0 is 3.49 pu against a power flow
dispatch of 1.0 pu. tellegen's `pe` is 1.0, and its `delta` and branch flows
agree with PowerSimulationsDynamics.jl to 1e-4.

**ESAC1A field voltage with KC ≠ 0 (IEEE 14-bus).** PowerSimulationsDynamics.jl
reports Efd(0) = 0.540 for machine 1, but Xad·Ifd on its own initial states is
1.2596, which steady state requires Efd to equal; tellegen's Efd(0) is 1.2596.
The machine dynamics are unaffected: `delta`, `omega` and `|V|` agree to 1e-6.

**ESAC1A field forcing (`esac1a`, `esac1a_sat`).** During the forcing at
t ≈ 1.2–1.5 s the field voltage differs from PowerSimulationsDynamics.jl by up
to 1e-2. Refining tellegen's step tenfold changes it by 1e-3 and moves it away
from the reference, so the difference comes from the reference's IDA
tolerances at the non-windup switching.
