# Dynamics in the app

The app's **Dynamics** mode runs the RMS (electromechanical) engine of
[RMS dynamic simulation](rms.md) in the browser: what the machines, exciters,
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
[RMS validation](rms-validation.md). Choosing one also sets that study's
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
  state each figure with its criterion; see [Screening](rms.md#screening).

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
