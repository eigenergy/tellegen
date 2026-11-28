# Controller models without a PowerSimulationsDynamics.jl counterpart

`closed_loop.json`: the reference of `tests/rms_reduced_controllers.rs` for the controller models
that PowerSimulationsDynamics.jl does not implement. It holds computed output only, written by
`validation/rms-models/reduced-controllers/run.jl` (see the README there).

## Method

Closed loop, for every model. Each controller was written once more in Julia
(`validation/rms-models/reduced-controllers/controllers.jl`) from the sources below, never from
tellegen's code, as a PowerSimulationsDynamics.jl 0.16.2 extension: a new PowerSystems.jl 5.12.5
component type (a subtype of `AVR` or `TurbineGov`) with the `mdl_avr_ode!` / `mdl_tg_ode!` and
`initialize_avr!` / `initialize_tg!` methods PowerSimulationsDynamics.jl dispatches on. The
machine (GENROU), shaft, the other controller, the network, the events and the solver are
PowerSimulationsDynamics.jl's, unmodified; no function of it is overridden. Where a controller
needs the stator current (compensator, P–Q limiters) it evaluates PowerSimulationsDynamics.jl's
GENROU stator equations at the step's states.

As a check of the extension itself, the unit's own SEXS and GAST written the same way (without
compensator, limiter or rate limit) reproduce PowerSimulationsDynamics.jl's `SEXS` and `GasTG`
through a line trip and a reference step within 4.8e-10 (every channel); `run.jl` fails if they
do not agree within 1e-6.

## Cases

The single machines of WECC 240 that `tests/support/smib.rs` builds (`smib_genrou_sexs_tgov1.*`,
`smib_genrou_gast_sexs.*`, unchanged; from `tests/data/rms_examples/wecc240`, BSD-3-Clause, see
`../rms_smib/LICENSE-PowerSimulationsDynamics.jl`): the unit at bus 102 on two parallel lines to
an infinite bus 101. Per run `records` replaces the unit's SEXS by ESAC6A (and adds OELIT and
UELPQ), `switches` sets tellegen's process-wide model switches (`set_vcomp`, `set_uel`, `set_gast_rate`) and
the reference's controller with the same values.

The ESAC6A set is the one of tellegen's model library (`src/rms/library.rs`; there is no published
ESAC6A set): the regulator of the ESST1A of PowerSystems.jl 5.12.5's
`test/test_dynamic_generator.jl` (KA 200, TA 0.1, TC/TB 2.5/13.25, VAMAX/VAMIN 9.5/−9.5), no
regulator lead (TK 0), and the transducer, AC exciter and field voltage limits of the ESAC1A of
PowerSimulationsDynamics.jl 0.16.2's IEEE 14-bus benchmark (`test/benchmarks/psse/GAST/dyn_data.dyr`:
TR 0.001, TE 0.8, KC 0.2, KD 0.38, KE 1, E1/SE(E1) 3.14/0.03, E2/SE(E2) 4.18/0.1, VRMAX/VRMIN
7.32/−7.32), the exciter field current limiter off (KH 0). Both packages are BSD-3-Clause.

| run | controller | events |
|---|---|---|
| `esac6a_trip`, `esac6a_fault` | ESAC6A, the set of tellegen's model library | trip of circuit 2 at 1.01 s; fault through half a line's reactance 1.01–1.11 s |
| `esac6a_limits_trip`, `esac6a_limits_fault` | ESAC6A, VAMAX/VAMIN 4/2, VRMAX/VRMIN 3.6/2.85, VFELIM 2.93, KH 2, VHMAX 0.3, TH 0.2, TJ 0.1 | as above: the trip holds VA on VAMIN 1.9 s and VR on `Vt VRMIN` 2.0 s; the fault holds VA on VAMAX 0.6 s, VR on `Vt VRMAX` 0.1 s and VH on VHMAX 0.1 s |
| `esac6a_vfelim_trip` | the same with VFELIM 2.95 | trip (the test is ignored: tellegen does not initialize it, see the test) |
| `esac6a_oelit_uelpq` | ESAC6A of the library + OELIT (IFDRAT 2, IPICK 1.02, ICONT 1.05, ILIM 1.15, C 1, TCOOL 10, KP 0.2, KI 0.2, VOELMIN −0.3) + UELPQ (Q0 −0.2, KQP 0.1, KV 2, KP 0.2, KI 0.2, VUELMAX 0.3) | V_ref +0.03 at 1.01 s, −0.03 at 8.01 s, back at 13.01 s, 18 s |
| `sexs_compensator`, `sexs_compensator_lag` | SEXS + compensator Rc 0.01, Xc 0.05, T 0 / 0.5 s | trip at 1.01 s, V_ref +0.02 at 2.01 s |
| `sexs_uel` | SEXS + summing-point UEL q_lim 0.05, gain 2 | V_ref −0.08 at 1.01 s, back at 3.01 s |
| `gast_rate` | GAST + valve rate 0.05 pu/s | P_ref +0.1 at 1.01 s, −0.15 at 4.01 s, 11 s |

In `esac6a_oelit_uelpq` the over-excitation limiter acts from 2.35 s and keeps the field current
near ILIM (2.3, at most 2.42), its heating passes 95 % of C at 6.05 s and the limit falls to ICONT
(2.1); the field current then falls below the pickup, the limit returns to ILIM from 7.35 s and
the limiter winds back and cools through the step down; the under-excitation limiter acts from
9.8 s and winds back after the step back, on which the field current passes ILIM once more
(14.6 to 15.5 s, at most 2.36) and the over-excitation limiter acts again. Events sit between the
20 ms samples, so no sample falls on a discontinuity.

## Where the equations come from

- ESAC6A: IEEE Std 421.5-2005, Figure 6-6 (type AC6A): `VA = clamp(KA (1 + s TK)/(1 + s TA) (V_ref
  + Vs − Vm), VAMIN, VAMAX)` (an output limit, as tellegen's doc comment of `esac6a` states), the
  lead-lag `(1 + s TC)/(1 + s TB)`, the exciter field current limiter `VH = clamp(KH (1 + s TJ)/(1
  + s TH) (VFE − VFELIM), 0, VHMAX)` subtracted, `VR = clamp(·, Vt VRMIN, Vt VRMAX)`, and the AC
  exciter of type AC1A (Figure 6-1): `TE dVE/dt = VR − VFE`, `VFE = VE (KE + SE(VE)) + KD Ifd`,
  `Efd = VE FEX(KC Ifd / VE)`, `FEX` of Annex D; `TR` the voltage transducer lag (Figure 3-1).
  `SE` is PSS/E's quadratic saturation `B (VE − A)² / VE` above `A`, zero below, through
  `(E1, SE(E1))` and `(E2, SE(E2))`.
- OELIT, UELPQ and their sum at the summing point: tellegen's own models, from their doc comments
  in `src/rms/ext_models.rs` (`oel_nodes`, `uel_nodes`, `dead_zone_pi` with its reset time
  `LIMITER_RESET` = 1 s, `with_limiters`): the signals enter the voltage error with the
  stabilizer's.
- AVR load compensator: IEEE Std 421.5-2005, Figure 3-1, `Vc = |Vt + (Rc + j Xc) It|` (machine
  base); the variant with a time constant from the doc comments of `set_vcomp` / `avr_vt` and the
  compensator's lag in `src/rms/system.rs`: `|Vt + Rc It| + Xc z`, `T dz/dt = Q / Vt − z`.
- Summing-point under-excitation limiter (`set_uel`): its doc comment in `src/rms/system.rs`:
  the voltage error is raised by `gain · max(0, −q_lim Vt² − Q)`, Q on the machine base.
- SEXS: the PSS/E model (`(1 + s TA)/(1 + s TB)`, `K/(1 + s TE)` with non-windup limits EMIN,
  EMAX).
- GAST: the PSS/E model after W. I. Rowen, ASME J. Eng. Power 105(4), 1983, as
  PowerSimulationsDynamics.jl's `GasTG` states it (fuel demand `min(P_ref − Δω/R, AT + KT (AT −
  x3))`, valve lag T1 with non-windup limits VMIN, VMAX, turbine lag T2, exhaust lag T3, power
  `x2 − DTURB Δω`); the rate limit from the doc comment of `set_gast_rate` (the fuel valve moves
  at most the rate, pu/s): `dx1/dt` clamped to `±rate`.

## Solver and data

PowerSimulationsDynamics.jl 0.16.2 `MassMatrixModel`, Rodas5P, reltol = abstol = 1e-9, at most
5 ms per step, faults as a constant-impedance load switched at the bus (`LoadChange`), reference
steps as `ControlReferenceChange`. The solution is sampled every 20 ms (50 ms in the 18 s and 11 s
runs) from the solver's dense output, each channel evaluated from the states with the device's
own model functions, and rounded to seven significant digits. Channels: `vm`, `omega`, `delta`
(rad), `pe`, `qe` (system base), `efd`, `tm` (machine base), and for ESAC6A its signals under
tellegen's node names (`sig:exciter.<node>:102`). `v_ref0` and `p_ref0` are the initial
references; the test checks tellegen's against them (1e-6).

## Tolerances

`tolerances.json`, per run and channel: three times tellegen's difference to the reference at its
1 ms step, at least 1e-6, rounded up to two digits, as for the PowerSimulationsDynamics.jl
comparisons in `../rms_smib`. The differences are the integration error of tellegen's step: at a
0.25 ms step every one above 2e-6 falls by 3.2 to 18 times, toward the reference's seven-digit
rounding (5e-7 on `vm` and `omega`); for example in `esac6a_fault` `efd` 6.3e-6 → 1.1e-6, `qe` 2.7e-4 → 1.7e-5,
`delta` 2.7e-6 → 2.1e-7, and in `esac6a_oelit_uelpq` `Va` 5.7e-5 → 3.1e-6. The largest at 1 ms:
`delta` 2.7e-6 rad, `vm` 2.1e-6, `efd` 6.3e-6 (all `esac6a_fault`), `qe` 2.8e-4
(`esac6a_limits_fault`), `omega` 5.5e-7 (`esac6a_trip`, the rounding
floor). `esac6a_vfelim_trip` has the bounds of
`esac6a_limits_trip`, the same run but for a limiter that stays at zero. Every run also meets
the test's physical tolerances: speed 1e-4 pu, rotor angle 0.5°, terminal voltage 1e-3 pu,
reactive power 1e-2 pu, field voltage 1e-2 pu.
