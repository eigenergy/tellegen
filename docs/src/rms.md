# RMS dynamic simulation

`tellegen::rms` simulates electromechanical (RMS, phasor-domain) dynamics. A
PSS/E `.raw` case is paired with its `.dyr` dynamic data; PowerIO reads the
`.dyr` records through typed PSS/E schemas and links them to the case's
generators; tellegen initializes every machine from the AC power flow,
integrates the network and device equations, applies events, and records the
trajectories.

## The boundary

| Layer | Owns |
|---|---|
| **PowerIO** | `.dyr` parsing (loss-preserving), typed model views (`typed_model`), PSS/E machine and circuit ids, `ZSORCE`, linking records to generators (`DynamicStudy::machines()`) |
| **tellegen** (`tellegen::rms`) | model equations, initialization, the DAE integrator, events, small-signal analysis, live sessions, the JSON front door (`simulate_module_json`, `inspect_module_json`) |

A `.dyr` file is attached to a case explicitly; nothing reads one implicitly.

## Cargo features

Neither is a default feature of the `tellegen` crate.

- `rms` (implies `sensitivity`): the engine, its initialization from the AC
  power flow, the integrator, the models validated against
  PowerSimulationsDynamics.jl, events and recording, and the JSON front door
  (`simulate_module_json`, `inspect_module_json`).
- `rms-experimental` (implies `rms`): the less validated product layer:
  controller block graphs (`RmsSimulation::with_models`), small-signal analysis
  (`RmsRequest::small_signal`, `linear_analysis`), live sessions, checkpoints
  and replay, packed result buffers, DYR parameter editing, the model library
  and dynamize, and `tellegen::case_edit`. Without it a request for
  small-signal analysis or checkpoints is noted in the result and skipped.
  Small-signal analysis (a dense `O(n³)` eigensolve) runs only when a request
  asks for it (`small_signal` is false by default).

Julia is not a dependency. PowerSimulationsDynamics.jl is the reference the
equations are translated from (BSD-3, see
[third-party notices](third-party-notices.md)) and the validation reference
([RMS validation](rms-validation.md)).

## Models

| Role | PSS/E models |
|---|---|
| Machine | `GENCLS`, `GENROU`, `GENROE`, `GENSAL`, `GENSAE` |
| Exciter | `SEXS`, `ESAC1A`, `EXST1` |
| Governor | `TGOV1`, `GAST`, `HYGOV` |
| Stabilizer | `IEEEST` |
| Renewable plant | `REGCA1` + `REECB1` + `REPCA1` (WECC generic converter, electrical and plant control) |
| Loads | each load's static law (constant power, ZIP, exponential), or one law for every bus |

`RmsSimulation::with_options` (with graphs, `with_models_and_options`) takes `RmsOptions`, fixed for that simulation
only: whether renewable plants are simulated (the default) or held at their
power flow output, whether the initialization enforces reactive limits (the
default), the loads' low-voltage behaviour, PowerSimulationsDynamics.jl
0.16.2's low-voltage gain and saturation in place of the PSS/E ones (for
comparing with that tool), and whether step matrices are factorized by blocks
(the default) or as one sparse LU.

### Initialization

The initialization starts from the static study's AC power flow, with the
generators' reactive limits enforced (PV→PQ switching), so the dynamic run
starts where the static study is. A generator bus released at a limit starts
there, its voltage off the setpoint; the notes name each such bus with its
reactive power and the voltage difference. `RmsOptions::reactive_limits =
false` holds every generator at its setpoint whatever reactive power that
takes, as PowerSimulationsDynamics.jl does; the notes then name each bus whose
reactive generation exceeds its limits, and by how much. A generator that
regulates a remote bus holds its terminal at the case's voltage instead (the
static AC power flow refuses remote regulation). Each machine is then solved
for its states and references so every derivative vanishes and it injects its
power flow current: `‖f(x0)‖∞` is below 1e-8 on every shipped case
(`RmsSimulation::initial_residual`).

### Loads

With `LoadModel::PowerFlow` (the default) each load keeps the law the AC power
flow solved it with: the constant-power part, the ZIP constant-current and
constant-impedance parts, and exponential loads, the decomposition of
`load_voltage`. A part with exponent `γ` and power `P0` at the power flow
voltage `V0` draws `P(V) = P0 (V / V0)^γ` (a ZIP part `c (V / v_nom)^γ` is
`c (V0 / v_nom)^γ (V / V0)^γ`), as the current `I = (P − jQ) V / |V|²`; the
constant-impedance parts enter the admittance matrix. At `t = 0` the loads
draw exactly the power flow demand. `ConstantImpedance`, `ConstantCurrent` and
`ConstantPower` are a global override: the whole bus demand at `V0` follows
that one law (PSS/E `CONL` with all of P and Q in one class).

Below a breakpoint the parts with `γ < 2` (constant power, constant current,
exponential loads with `γ < 2`) draw the constant admittance of their power
there, `P(V) = P(V_b) (V / V_b)²`, as PSS/E converts constant-power loads
below `PQBRAK`: `RmsOptions::low_voltage_load` is `Break { v_break: 0.7 }` by
default (PSS/E's default `PQBRAK`; a bus starting below it breaks at its own
`V0`). The power is continuous at `V_b` and the current falls linearly to zero
with the voltage, where an unconverted constant-power load would draw an
unbounded current. `Exact` keeps every law down to 20 % of `V0` (below that, a
smooth floor keeps the current's direction defined at 0 V), as
PowerSimulationsDynamics.jl does. The power drawn by the loads of each bus is
recorded as `pl:<bus>` and `ql:<bus>`.

Following PowerSystems.jl, a generator without a machine record, or a
`GENCLS` with zero inertia, is a Thevenin source behind its `ZSORCE` (an
infinite bus); a missing exciter holds the field voltage and a missing
governor the mechanical torque. Records of other models are kept and named in
the run's notes, not simulated.

A renewable plant whose unit carries all three of `REGCA1`, `REECB1` and
`REPCA1` is simulated (`rms/renewable.rs`, a translation of
PowerSimulationsDynamics.jl's `RenewableEnergyConverterTypeA`,
`RECurrentControlB` and `Active/ReactiveRenewableControllerAB`, with every
flag combination it implements). Its states, the converter currents, the
REECB1 voltage filter and reactive current state and the REPCA1 filters and
integrators, are recorded as `inv.<state>:<unit>`. Where PowerSimulationsDynamics.jl
0.16.2 departs from the PSS/E definition, tellegen follows PSS/E: the
low-voltage active current gain between `Lvpnt0` and `Lvpnt1`, and `Xc` in the
compensated voltage, the converter ramp limits as rates on the currents, and
the high-voltage reactive current clamp (`Khv`, `Iolim`). A unit with other inverter records, or without the plant
controller, keeps its power flow output as a constant-current injection; the
notes say which.

A controller can also be a block graph (`BlockGraph`: lags with and without
non-windup limits, lead-lags, washouts, second-order filters, PI, integrators,
sums, gains, products, limits), given with `RmsSimulation::with_models` for
one exciter, governor or stabilizer slot of a unit. `template` turns a typed
PSS/E record into the graph of the same model, so a graph starts from a
validated model.

## What a run records

By default a run records every channel at every output step: two per bus (four
at a bus with load), four per branch and about eight per unit. A request's `record` (`RecordSpec`)
narrows that without coarsening anything:

- `channels`: the watch set, recorded at the full output step (keys such as
  `omega:2178`, or prefixes ending in `:` such as `omega:`). The values are
  bit-identical to a full recording's.
- `frames` with `frame_step`: further channels at their own step, exact
  samples.
- `checkpoint_step`: the full state every so many seconds, with what the
  integrator carries from step to step. `RmsSimulation::replay` runs the same
  request from a checkpoint and records any other channel after the fact: the
  samples are the run's, bit for bit.

A live session (`RmsSimulation::live`) advances on demand and takes events at
its current time; it reproduces a batch run of its event log. It takes the
same watch set (`set_record`) and gives any channel at its current time
(`snapshot`).

## Linear analysis

`RmsSimulation::linear_analysis` linearizes `M ẋ = f(x, u)` at the operating
point. Modes are the eigenvalues of `M_d⁻¹ (J_dd − J_da J_aa⁻¹ J_ad)` with
participation factors `|v_ki w_ik|`; algebraic states with an all-zero row and
column in `J_aa` are dropped first, as PowerSimulationsDynamics.jl does.
Frequency responses solve the descriptor system at each frequency, so no
algebraic elimination is needed.

## Formulation

The state is `[Vr; Vi; device states]` with `M ẋ = f(x)`: the bus rows are the
algebraic current balance in rectangular coordinates, each machine works in its
own dq frame, and the control blocks keep their time constants in `M`, so a
zero time constant makes a state algebraic. The reference frame is the speed of
a dynamic machine at the slack bus, or the synchronous frame when a source
holds it. Each step solves the implicit trapezoidal rule with Newton's method
(device Jacobian blocks by forward differences with the step
`1e-7 · max(|x|, 1)` rounded to the step actually taken, the network and load
blocks exactly). The rule is of second order: halving a 4 ms step quarters
the difference to the next finer run (`tests/rms_physics.rs`; on a
single-machine line trip, 3.3e-5 rad of rotor angle from 4 to 2 ms, 8.2e-6
rad from 2 to 1 ms), so the default 2.5 ms step is within about 2e-5 rad of
the converged trajectory there. That holds for a trajectory on which no
limit switches. A limiter, non-windup bound or rectifier that switches inside
a step is not located in time, so through such a switch the error no longer
falls with the step: on a 0.1 s fault at the same unit through half its line
reactance, which drives its exciter to the ceiling, the rotor angle of runs
at 5, 2.5, 1 and 0.5 ms lies 1.9e-4, 1.3e-4, 1.2e-4 and 1.2e-4 rad from a
0.1 ms run, and through a twentieth of it 7.6e-4, 3.9e-4, 3.5e-4 and 3.5e-4
rad. The step is fixed: there is no local error estimate or step-size
control. A run takes at most 1e7 steps (`t_end / dt`), with `dt ≥ 1e-6` s.
The step matrix is factorized by blocks where every unit is a machine: each
unit's states are eliminated onto its bus by a small dense LU, and a sparse LU
(faer) factorizes the network with a 2 × 2 contribution per unit and the frame
speed as a bordered column; otherwise the whole matrix goes to one sparse LU.
Both solve the same system. A factorization is reused while the step size, the
method and the topology are unchanged. An event is applied at its exact time
and the algebraic variables are re-solved with the differential states held.
The Newton iteration is damped (backtracking on the residual): a step that
does not lower the residual is never taken; the iteration refactorizes, and if
a fresh factorization finds no descent either, the step falls back to backward
Euler, then halves, so rectifiers and limiters that switch inside a step (an
ESAC1A with KC ≠ 0 chattering at its bounds) do not stop a run.

Events are checked before a run starts: each needs a finite time in
`[0, t_end)` (an event at `t_end` would act on nothing the run records), an
element the system has (judged in time order, as the run applies them), a
fault impedance with `r ≥ 0` and `x ≥ 0`, and a load scale `≥ 0`, or the run
is refused with the event named. A run ends exactly at `t_end`: when `t_end`
is not a whole number of output steps, the last output interval is shorter.

After a topology event, an island apart from the reference bus that keeps a
synchronous machine or source is named in the run's events: it runs at its
own frequency (renewable plants there measure the reference's frequency, as
PowerSimulationsDynamics.jl's `FixedFrequency` does, so an island fed by one
can run away). An island left with only inverters or held units has nothing
to hold its voltage: those units trip and its buses go dead. When the
integration fails with the network split, the error names the islands.

## Known limitations

- **Loads** are static: no induction motor or composite load models, and no
  frequency dependence.
- **Remote voltage regulation** is held at the case's terminal voltage, not
  solved (the static AC power flow refuses it).
- **Step size** is fixed (see Formulation), and switching limiters are not
  located inside a step.
- **HVDC lines** are not modelled: a case with an in-service HVDC line is
  refused (the static AC power flow holds them as fixed injections).
- **Islands** run on (see Formulation); there is no under/over-frequency or
  overspeed protection, so an island without load accelerates until the run
  fails.
- **Inverter-based resources** other than the `REGCA1` + `REECB1` + `REPCA1`
  plant (for example `REECA1`, `WTGT*`) are not modeled: such a unit keeps its
  power flow output as a constant-current injection, named in the notes.
  Voltage-dip freezing (`Vdip`, `Vup`, `Vfrz`) and a plant controller's remote
  branch flow are not modelled, as in PowerSimulationsDynamics.jl.
- **Models outside the table above** are preserved by PowerIO and named in the
  notes, not simulated.
- **Trajectories** have no PowerIO value yet.

## Differences from the PSS/E Model Library

Where PowerSimulationsDynamics.jl 0.16.2 departs from the PSS/E definitions,
tellegen follows PSS/E for:

- the REGCA1 low-voltage active current gain between `Lvpnt0` and `Lvpnt1`
  (unless `RmsOptions::psid_lv_gain`) and `Xc` in the compensated voltage;
- the REGCA1 ramp limits `Rrpwr`, `Iqrmax`, `Iqrmin` as rates in pu/s on the
  converter currents (unless `RmsOptions::psid_converter_ramp`, which limits
  the `Tg` lag's error instead: an effective ramp `Rrpwr/Tg`, 500 pu/s for
  `Rrpwr = 10`, `Tg = 0.02 s`);
- the REGCA1 high-voltage reactive current clamp: above `Volim` the injected
  reactive current falls by `Khv (V − Volim)`, down to `Iolim`
  (PowerSimulationsDynamics.jl adds the term and ignores `Iolim`);
- the IEEEST output cut-off: `VCU` and `VCL` each ignored only when it is
  zero (PowerSimulationsDynamics.jl ignores both when either is);
- a non-windup lag with a zero time constant (SEXS `TE`, ESAC1A `TA`, REGCA1
  `Tg`, REECB1 `Tpord`): algebraic, its output the clamped input;
- exciter saturation points in either order (IEEE 421.5 lists `E1 > E2`);
- quadratic saturation `B (x − A)²/x` with `Se = 0` for `x ≤ A` (unless
  `RmsOptions::psid_saturation`; the formula alone rises again below `A` and
  diverges as `x → 0`);
- an ESAC1A's rectifier current `IN = KC Ifd / VE` with `VE` floored at 1e-3,
  so it stays finite as `VE` collapses, and `TB` as the lead-lag's mass, so
  `TB = TC = 0` bypasses it;
- constant-power and constant-current loads below `PQBRAK` (see Loads; unless
  `LowVoltageLoad::Exact`);
- the initialization with reactive limits enforced (unless
  `RmsOptions::reactive_limits = false`);
- an ESAC1A with `TR = 0`: no transducer lag (PowerSimulationsDynamics.jl
  substitutes 1 s);
- a washout (rate feedback) with `T = 0`: the block is off.

It follows PowerSimulationsDynamics.jl, and so differs from PSS/E, in:

- the placement of the REGCA1 low-voltage power limit;
- the non-windup PI: its integrator stops while the output is limited, so with
  `Kp = 0` it stays at the limit;
- the REECB1 reactive current PI with `QFLAG = 1`, which has no limit.

A fault needs a finite, nonzero impedance; a bolted fault is a small
reactance.
