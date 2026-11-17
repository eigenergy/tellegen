# Dynamics in the app

The app's **Dynamics** mode runs the RMS (electromechanical) engine of
[RMS dynamic simulation](dynamics.md#rms) in the browser: what the machines, exciters,
governors and renewable plants of a case do in the seconds after a fault, a
trip or a load step. Switch to it with **Steady state | Dynamics** in the
header, the **Dynamics** panel button, or the command palette ("Open
Dynamics").

## Opening a dynamic case

Dynamics needs PSS/E dynamic data (`.dyr`) for a balanced case opened from a
file:

- **WECC 240 demo.** The Dynamics panel's first button (or the palette
  command "Open WECC 240 dynamics demo") opens the reduced 240-bus WECC system
  with its dynamic data and bus coordinates. Sources and licences are on the
  Credits page.
- **Your own case.** Drop a `.raw` with its `.dyr` (and a geographic file if
  the case has no coordinates). A `.dyr` dropped alone attaches to the active
  case; a second one replaces the first after a confirm. A `.txt` that reads
  as DYR is taken as one.

Attaching initializes the simulation from the AC power flow, once. The panel
then reports what was linked (machines, renewable plants, units held at their
power-flow output, voltage sources, DYR records), the initial residual
`‖f(x₀)‖∞`, which unit's speed turns the angle reference frame, and a
**flat-start check**: a 2 s run with no events. A case at rest drifts by less
than 1e-6 pu in speed; more is shown as "the case is not at rest; results
include that drift". The engine's notes are listed verbatim, grouped.

Dynamics uses the case **as loaded**: steady-state demand and rating edits are
not included, and the panel says how many there are.

## Events

Select a bus or branch on the map, or type a bus number in the Network
panel's bus lookup, and pick an action in the Network panel:

| Selection | Action | What runs |
|---|---|---|
| Bus | Apply a fault here | a bolted three-phase fault (j1e-4 pu) at 1.0 s, cleared after 6 cycles of the base frequency |
| Bus with load | Step load ±% | the bus's load scaled at 1.0 s |
| Bus with units | Trip (per unit) | the unit disconnected at 1.0 s |
| Branch | Trip circuit | the circuit opened at 1.0 s (parallel circuits are listed) |
| Branch | Fault on this line | a fault 1 % from the chosen end, cleared by opening both ends of the line |

Each event is a sentence in the panel with its time and clearing time
(cycles or milliseconds) editable in place. Up to 16 events; each is checked
as the engine checks it before a run, and an engine refusal is shown
verbatim.

**Validated scenarios** (WECC 240 only) are the four runs of
[RMS validation](benchmark-results.md#rms-validation). Choosing one also sets that study's
options: constant-impedance loads, PowerSimulationsDynamics.jl's renewable
low-voltage gain, saturation and converter ramp, exact load law, no reactive
limits at initialization, a 5 ms step and 20 s, on the network with the
study's load values. The run then carries the badge "Validated scenario:
matches PowerSimulationsDynamics.jl 0.16.2 within 3.2e-6 pu speed, 1.2e-5 pu
|V|". Change any of that and the badge says which settings differ instead.

## Running

Duration (5–120 s), step (2.5 ms standard, 5 ms fast, 1 ms fine) and the
load model are in the Run section; **Advanced** holds the low-voltage load
law, reactive limits at initialization, held renewable plants and the
PowerSimulationsDynamics.jl comparison switches. **Run** (Ctrl/Cmd+Enter)
starts the run in its own worker with a progress bar; **Stop** ends it at
once without touching steady-state results. The last three runs stay as
chips.

## Reading the result

- **The map** plays the run back: bus voltage magnitude on a fixed scale
  (neutral 0.97–1.03 pu, amber to maroon as voltage sags, lavender to purple
  above 1.05 pu), rings at generating buses whose size grows with the
  machine's speed deviation (teal below nominal, magenta above, grey for held
  units), the faulted bus while the fault is on and opened branches. The
  legend's "What am I looking at?" explains it for a first visit.
- **The timeline** plays, pauses (Space), steps (←/→, Shift for 100 ms),
  jumps between events (`[`/`]`) and slows to 0.25× around each event. After
  a run it plays from half a second before the first event; with reduced
  motion it waits one cycle after it. Its accessible value reads the time,
  the time since the last event in cycles, the system frequency and the
  lowest voltage.
- **Traces**: Frequency (centre of inertia in bold, the five buses with the
  lowest frequency, the range of all bus frequencies), Rotor angles relative
  to the centre of inertia (the three largest swings highlighted), Voltages
  (range of all buses, faulted buses, 0.8 and 0.9 pu lines) and Loads.
  Hovering a trace highlights its bus on the map; clicking pins it and
  selects the bus. Drag to zoom. "Show as table" lists the shown traces every
  0.5 s.
- **The verdict** leads with an icon and a word: Stable, Unstable: loss of
  synchronism, Unstable: island frequency runs away, Inconclusive: not
  settled, or Run failed at t (with the engine's message). The rows below
  state each figure with its criterion; see [Screening](dynamics.md#rms-screening).

Screening criteria are tellegen's own, versioned (`tellegen-rms-screening-1`)
and stated in the card: they are not a TPL-001 or WECC planning assessment,
and no protection, under-frequency load shedding or motor loads are modelled.

## Experimental tools

**Settings → Experimental → Show experimental dynamics tools** adds, with an
Experimental label on everything they affect:

- **Modes**: eigenvalues of the system linearized at the operating point,
  plotted against frequency with 5 % and 10 % damping rays, and the states
  that take part most in each mode. The analysis is dense: WECC 240 (1,658
  states) takes about 8 s natively and many minutes in the browser; Stop
  ends it.
- **Add default dynamics** for a case without `.dyr` data: typical models
  from published unit data, stamped "Synthetic dynamic data: typical
  parameters, not the owner's".

## Agents

The `simulate_dynamics` WebMCP tool runs the same simulation for an agent and
answers with the verdict and its figures; see [WebMCP](webmcp.md).


<a id="rms"></a>

<a id="rms-rms-dynamic-simulation"></a>

## RMS dynamic simulation

`tellegen::rms` simulates electromechanical (RMS, phasor-domain) dynamics. A
PSS/E `.raw` case is paired with its `.dyr` dynamic data; PowerIO reads the
`.dyr` records through typed PSS/E schemas and links them to the case's
generators; tellegen initializes every machine from the AC power flow,
integrates the network and device equations, applies events, and records the
trajectories.

<a id="rms-the-boundary"></a>

### The boundary

| Layer | Owns |
|---|---|
| **PowerIO** | `.dyr` parsing (loss-preserving), typed model views (`typed_model`), PSS/E machine and circuit ids, `ZSORCE`, linking records to generators (`DynamicStudy::machines()`) |
| **tellegen** (`tellegen::rms`) | model equations, initialization, the DAE integrator, events, small-signal analysis, live sessions, the JSON front door (`simulate_module_json`, `inspect_module_json`) |

A `.dyr` file is attached to a case explicitly; nothing reads one implicitly.

<a id="rms-cargo-features"></a>

### Cargo features

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
  The request and result types have the same fields under both features
  (`RecordSpec::checkpoint_step`, `RmsResult::checkpoints`), so turning on
  `rms-experimental` breaks no code written against `rms`.
  Small-signal analysis (a dense `O(n³)` eigensolve) runs only when a request
  asks for it (`small_signal` is false by default).

Julia is not a dependency. PowerSimulationsDynamics.jl is the reference the
equations are translated from (BSD-3, see
[third-party notices](third-party-notices.md)) and the validation reference
([RMS validation](benchmark-results.md#rms-validation)).

<a id="rms-models"></a>

### Models

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

<a id="rms-initialization"></a>

#### Initialization

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
(`RmsSimulation::initial_residual`). The exciter and governor steady states
are checked against their limits, as PowerSimulationsDynamics.jl does at
initialization. SEXS's field voltage and ESAC1A's regulator voltage `VA` are
their non-windup lags' states clamped to `EMIN`/`EMAX` (`VAMIN`/`VAMAX`), as in
PSS/E, so a step that carries the state past a limit does not pass the
overshoot on (PowerSimulationsDynamics.jl applies the state itself). A
dispatch that needs one beyond its limits still rests there, as in
PowerSimulationsDynamics.jl (the non-windup lag holds a state it is pushing
further out): that unit's clamp is widened to its starting value, and a note
names the unit, the limit and the value. With `TE = 0` (`TA = 0`) the lag is
algebraic and cannot rest past its limit, so that breach fails as below. No steady state exists when the
dispatch needs ESAC1A's `VR` beyond `VRMIN`/`VRMAX`, EXST1's voltage error
beyond `VIMIN`/`VIMAX` or its field voltage beyond `VT·VRMIN − KC·Ifd`/`VT·VRMAX − KC·Ifd`, a TGOV1 or GAST
valve beyond `VMIN`/`VMAX`, GAST power above `AT`, or a HYGOV gate beyond
`GMIN`/`GMAX`. Then the initialization fails with an error that names the unit,
the limit and the value.

<a id="rms-loads"></a>

#### Loads

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
the high-voltage reactive current clamp (`Khv`, `Iolim`). A unit
with other inverter records, or without the plant controller, keeps its power
flow output as a constant-current injection; so does a plant whose `REECB1`
or `REPCA1` regulates another bus (`IREG`), which the controllers do not
measure. The notes say which.

Limits in the dynamic data must be ordered (each minimum at most its maximum,
HYGOV's `VELM` not negative, REGCA1's `Zerox` below `Brkpt` and `Lvpnt0`
below `Lvpnt1`): a generator's swapped limits refuse the case with the unit
and parameters named, and a renewable plant's hold the plant with a note.
A renewable plant whose power flow output lies past a controller limit
(REPCA1 `Qmin`/`Qmax` or `Pmin`/`Pmax`, REECB1 `Pmin`/`Pmax`, `Qmin`/`Qmax` or
`Vmin`/`Vmax`, the current limits or the LVPL ceiling), or whose terminal lies
below `Lvpnt1` or above `Volim` with `Khv > 0` so the converter injects another
current than the power flow's, has no steady state there: it is held at its
power flow output, with a note naming the limit and the value.

A controller can also be a block graph (`BlockGraph`: lags with and without
non-windup limits, lead-lags, washouts, second-order filters, PI, integrators,
sums, gains, products, limits), given with `RmsSimulation::with_models` for
one exciter, governor or stabilizer slot of a unit. `template` turns a typed
PSS/E record into the graph of the same model, so a graph starts from a
validated model.

<a id="rms-what-a-run-records"></a>

### What a run records

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

<a id="rms-screening"></a>

### Screening

`RmsSimulation::screen(request, result, criteria)` gives a finished run a
verdict against versioned criteria (`ScreeningCriteria`, version 1 by
default; every field can be changed), from the run's own samples. It needs
`omega:`, `delta:`, `vm:` and `va:` recorded (`RecordSpec::display()` records
them) and names any that are missing.

- **Centre of inertia (COI).** Per island, the machines in service weighted
  by `H · MBASE`; a tripped machine leaves the COI from its trip, and each
  island formed by the events is screened on its own COI. An island holding
  a voltage source (an infinite bus) is referred to the source.
- **Pole slip.** A machine whose rotor angle relative to its island's COI
  passes ±180°: *Unstable, loss of synchronism*, naming the unit and time.
- **Island runaway.** An island whose COI speed ends more than 0.05 pu from
  1: *Unstable, island frequency runs away*.
- **Settling.** COI frequency within ±0.01 Hz of its final value and the
  largest angle separation within ±1° over the last 2 s, which must follow
  the last event; otherwise *Inconclusive*.
- **Voltage recovery.** Every live bus at or above 0.8 pu within 1 s of the
  last event; a miss is reported as a voltage concern.
- **Reported figures.** COI frequency nadir and peak, RoCoF over a 0.5 s
  window, the largest `|δ − δ_COI|` (the figure the pole-slip criterion
  reads), the largest angle separation from the first event on (pair, time)
  next to the steady-state separation before it, the first swing (per
  machine, the largest excursion of `δ − δ_COI` from its value before the
  first event up to its first peak after the last event), the lowest unit
  speed and bus frequency, the lowest voltage and the final voltage range.
- **Swing trend.** From the peaks of the separation's distance to its final
  value after the last event, counting only peaks above 0.5° and above 10 %
  of the largest distance (smaller wiggles are numerical): *decaying* or
  *growing* when the last three counted peaks each shrink or grow by more
  than 2 %, *damped out* when the last peak is below that floor and within
  the settling criterion's angle band, *sustained* when three or more peaks
  count, the last among them, without a monotone trend, otherwise
  *undetermined* (too few counted peaks to tell).

`derived_series` gives `coi:omega`, `coi:delta`, `coi:f` and `f:<bus>`, each
bus's frequency estimated as `f0 (ω_ref + (1/ω0) d(va)/dt)` with the
derivative through a 20 ms first-order filter (the PSS/E `FRQ` convention),
held over the sample at each event and below 0.2 pu, and NaN while the bus is
dead.

These are screening criteria chosen by tellegen, not a planning standard.

<a id="rms-in-the-browser"></a>

### In the browser

`tellegen-wasm` exports `RmsCase` (features `rms`, and `rms-experimental` for
modes, default dynamics and DYR parameters): prepared once from a stored
module and DYR text, inspected without a run, checked with a flat run, and run
with the display channels, packed channel-major into one `Float64Array` with
the screening in a JSON head. The bus angles are replaced by the bus
frequencies; a run past 4e6 values records at a coarser output step and says
so. `@tellegen/engine` runs it in a dedicated worker (`openRmsCase`,
`BrowserRmsCase`, see [Engine API Reference](engine-api.md)); the app's
[Dynamics mode](dynamics.md) is built on it.

<a id="rms-linear-analysis"></a>

### Linear analysis

`RmsSimulation::linear_analysis` linearizes `M ẋ = f(x, u)` at the operating
point. Modes are the eigenvalues of `M_d⁻¹ (J_dd − J_da J_aa⁻¹ J_ad)` with
participation factors `|v_ki w_ik|`; algebraic states with an all-zero row and
column in `J_aa` are dropped first, as PowerSimulationsDynamics.jl does.
Frequency responses solve the descriptor system at each frequency, so no
algebraic elimination is needed.

<a id="rms-formulation"></a>

### Formulation

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
at 5, 2.5, 1 and 0.5 ms lies 1.4e-4, 8.3e-5, 7.0e-5 and 6.8e-5 rad from a
0.05 ms run, and through a twentieth of it 6.2e-4, 2.4e-4, 1.5e-4 and 1.4e-4
rad (`sexs_field_voltage_never_passes_emax` in `tests/rms_physics.rs`). The step is fixed: there is no local error estimate or step-size
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
ESAC1A with KC ≠ 0 chattering at its bounds) do not stop a run. A limit a
state slides along (a PI output held at a limit while its proportional path
pulls inside, the REGCA1 active current on its rising low-voltage power limit)
is written so the state's rate is continuous there, and every implicit step
has a solution. When a step still fails, the error names the variable with
the largest residual.

Events are checked before a run starts: each needs a finite time in
`[0, t_end)`, and more than a billionth of a step before `t_end` (an event at
`t_end` would act on nothing the run records), an
element the system has (judged in time order, as the run applies them), a
fault impedance with `r ≥ 0` and `x ≥ 0`, and a load scale `≥ 0`, or the run
is refused with the event named. A run ends exactly at `t_end`: when `t_end`
is not a whole number of output steps, the last output interval is shorter.
An output step of more than 10⁷ integration steps is refused.

After a topology event, an island apart from the reference bus that keeps a
synchronous machine or source is named in the run's events: it runs at its
own frequency (renewable plants there measure the reference's frequency, as
PowerSimulationsDynamics.jl's `FixedFrequency` does, so an island fed by one
can run away). An island left with only inverters or held units, the
reference bus's included, has nothing to hold its voltage: those units trip
and its buses go dead. When the
integration fails with the network split, the error names the islands.

<a id="rms-known-limitations"></a>

### Known limitations

- **Loads** are static: no induction motor or composite load models, and no
  frequency dependence.
- **Remote voltage regulation** is held at the case's terminal voltage, not
  solved (the static AC power flow refuses it).
- **Step size** is fixed (see Formulation), and switching limiters are not
  located inside a step.
- **HVDC lines** are not modelled: a case with an in-service HVDC line is
  refused (the static AC power flow holds them as fixed injections).
- **Islands** run on (see Formulation); there is no under/over-frequency or
  overspeed protection, so an island without load accelerates without bound
  (WECC 240 with transformer 1004-1034 open: bus 1034's units reach 1.9 pu
  speed in 9 s).
- **Inverter-based resources** other than the `REGCA1` + `REECB1` + `REPCA1`
  plant (for example `REECA1`, `WTGT*`) are not modeled: such a unit keeps its
  power flow output as a constant-current injection, named in the notes.
  Voltage-dip freezing (`Vdip`, `Vup`, `Vfrz`) and a plant controller's remote
  branch flow are not modelled, as in PowerSimulationsDynamics.jl, nor is
  remote voltage regulation by a plant (`IREG`; such a plant is held).
- **Models outside the table above** are preserved by PowerIO and named in the
  notes, not simulated.
- **Trajectories** have no PowerIO value yet.

<a id="rms-differences-from-the-psse-model-library"></a>

### Differences from the PSS/E Model Library

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
- the SEXS field voltage and the ESAC1A regulator voltage `VA`: the
  non-windup lag's state clamped to its limits, so a step that carries the
  state past `EMAX` (`VAMAX`) does not pass the overshoot to the machine
  (PowerSimulationsDynamics.jl applies the state, which then rests past the
  limit while the input pushes further; a unit that starts past it keeps
  that rest, see Initialization);
- the IEEEST output cut-off: `VCU` and `VCL` each ignored only when it is
  zero (PowerSimulationsDynamics.jl ignores both when either is);
- a non-windup lag with a zero time constant (SEXS `TE`, ESAC1A `TA`, REGCA1
  `Tg`, REECB1 `Tpord`): algebraic, its output the clamped input;
- exciter saturation points in either order (IEEE 421.5 lists `E1 > E2`);
- the non-windup PI (HYGOV, REPCA1, REECB1): its integrator stops only while
  its input drives the output further past a limit, and integrates as soon as
  the input points back inside (PowerSimulationsDynamics.jl stops it whenever
  the output is limited, so with `Kp = 0` it never leaves the limit). Near a
  limit the integrator's outward rate is capped at `(limit − y)/(Ki τ)`,
  `τ` = 1 ms, so an output sliding along the limit has a solution (the same
  for the REGCA1 low-voltage power limit below);
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

- the REECB1 reactive current PI with `QFLAG = 1`, which has no limit;
- the placement of the REGCA1 low-voltage power limit (`LVPLSW = 1`): the
  active current is `min(Ip, LVPL)` and `Ip` stops rising at the ceiling
  (PSS/E limits the `Tg` lag itself, so after a fault `Ip` recovers from the
  ceiling at `Rrpwr`). `Ip` holds above the ceiling; below it, where
  PowerSimulationsDynamics.jl switches the rate of `Ip` to zero at the
  ceiling, tellegen caps the rising rate at `(LVPL − Ip)/τ`, `τ` = 1 ms, so
  `Ip` can ride a recovering ceiling (with a switched rate no implicit step
  has a solution there).

A fault needs a finite, nonzero impedance; a bolted fault is a small
reactance.
