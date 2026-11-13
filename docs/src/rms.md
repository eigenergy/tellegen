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
| Loads | constant impedance, current or power at the power flow voltage |

`RmsSimulation::with_options` takes `RmsOptions`, fixed for that simulation
only: whether renewable plants are simulated (the default) or held at their
power flow output, PowerSimulationsDynamics.jl 0.16.2's low-voltage gain in
place of the PSS/E one (for comparing with that tool), and whether step
matrices are factorized by blocks (the default) or as one sparse LU.

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
compensated voltage. A unit with other inverter records, or without the plant
controller, keeps its power flow output as a constant-current injection; the
notes say which.

A controller can also be a block graph (`BlockGraph`: lags with and without
non-windup limits, lead-lags, washouts, second-order filters, PI, integrators,
sums, gains, products, limits), given with `RmsSimulation::with_models` for
one exciter, governor or stabilizer slot of a unit. `template` turns a typed
PSS/E record into the graph of the same model, so a graph starts from a
validated model.

## What a run records

By default a run records every channel at every output step: two per bus, four
per branch and about eight per unit. A request's `record` (`RecordSpec`)
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
(device Jacobian blocks by forward differences, the network block exactly).
The step matrix is factorized by blocks where every unit is a machine: each
unit's states are eliminated onto its bus by a small dense LU, and a sparse LU
(faer) factorizes the network with a 2 × 2 contribution per unit and the frame
speed as a bordered column; otherwise the whole matrix goes to one sparse LU.
Both solve the same system. A factorization is reused while the step size, the
method and the topology are unchanged. An event is applied at its exact time
and the algebraic variables are re-solved with the differential states held.
The Newton iteration is damped (backtracking on the residual); a step the
trapezoidal rule cannot solve falls back to backward Euler, then halves, so
rectifiers and limiters that switch inside a step (an ESAC1A with KC ≠ 0
chattering at its bounds) do not stop a run.

## Known limitations

- **Voltage-dependent loads in the power flow.** The AC power flow takes
  constant-power loads; the RMS initialization reaches the ZIP/exponential
  operating point by fixed-point iteration over constant-power flows and
  reports it in the notes. A load's nominal voltage below 2 is in per unit,
  one of 2 or more in kV against its bus's base voltage.
- **Generator reactive limits.** The initialization holds every generator at
  its voltage setpoint and does not apply its reactive limits, so on a case
  whose dispatch needs more reactive power than a unit's limit it starts from
  another operating point than a power flow that applies them.
- **Inverter-based resources** other than the `REGCA1` + `REECB1` + `REPCA1`
  plant (for example `REECA1`, `WTGT*`) are not modeled: such a unit keeps its
  power flow output as a constant-current injection, named in the notes.
  Voltage-dip freezing (`Vdip`, `Vup`, `Vfrz`) and a plant controller's remote
  branch flow are not modelled, as in PowerSimulationsDynamics.jl.
- **Models outside the table above** are preserved by PowerIO and named in the
  notes, not simulated.
- **Block templates.** On two single-machine cases built from WECC 240 a run
  with the templates of its controllers departs from the compiled run
  (channels up to 5e-5, held below 1e-4 by `tests/rms_graph.rs`); the cause is
  open. Ruled out: the IEEEST filter with A3 = A4 = 0 and its output limit. The
  compiled models are the validated ones.
- **Trajectories** have no PowerIO value yet.

## Differences from the PSS/E Model Library

Where PowerSimulationsDynamics.jl 0.16.2 departs from the PSS/E definitions,
tellegen follows PSS/E for:

- the REGCA1 low-voltage active current gain between `Lvpnt0` and `Lvpnt1`
  (unless `RmsOptions::psid_lv_gain`) and `Xc` in the compensated voltage;
- an ESAC1A with `TR = 0`: no transducer lag (PowerSimulationsDynamics.jl
  substitutes 1 s);
- a washout (rate feedback) with `T = 0`: the block is off.

It follows PowerSimulationsDynamics.jl, and so differs from PSS/E, in:

- quadratic saturation `B (x − A)²/x`, with no zero region below `A`;
- the REGCA1 active current ramp limit, applied before the `Tg` lag, so the
  effective ramp is `Rrpwr/Tg` (500 pu/s for `Rrpwr = 10`, `Tg = 0.02 s`), and
  the placement of the low-voltage power limit;
- the non-windup PI: its integrator stops while the output is limited, so with
  `Kp = 0` it stays at the limit;
- the REECB1 reactive current PI with `QFLAG = 1`, which has no limit.

A fault needs a finite, nonzero impedance; a bolted fault is a small
reactance.
