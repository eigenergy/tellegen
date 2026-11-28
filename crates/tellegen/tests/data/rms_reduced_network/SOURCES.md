# Reduced networks: network-level models

References of `tests/rms_reduced_network.rs` for RMS models that
PowerSimulationsDynamics.jl does not implement. Every file holds computed
output only, written by the scripts in `validation/rms-models/reduced-network`
(Julia 1.11.9 and the pinned environment of `validation/rms-models`:
PowerSimulationsDynamics.jl 0.16.2, PowerSystems.jl 5.12.5, OrdinaryDiffEq.jl
6.111.0). The networks are the ones the test writes inline; the reference
models were written from the documented equations of each model, not from
tellegen's code.

Per run a file holds the switches, the sample times (every 20 ms; every event
lies between two samples), the compared channels rounded to seven significant
digits, and per channel a tolerance: three times tellegen's largest
difference at the 1 ms step of the test, at least 1e-6, rounded up to two
digits (`tolerances.jl`). Besides it the test holds every channel to a fixed
physical tolerance: speed 1e-4 pu, angles 0.5°, voltage 1e-3 pu, power
1e-3 pu on 100 MVA.

## The reference model (`common.jl`)

A differential-algebraic model in rectangular bus voltages, on 100 MVA and
60 Hz:

- network: per bus `I_inj(V) = Y V`, `Y` from the series reactances of the
  lines in service and the fault admittances `1 / (j x_f)`;
- classical machine (GENCLS; PowerSimulationsDynamics.jl's `BaseMachine` with
  `SingleMass`): the EMF `e'q ∠δ` behind `jX'd` on the machine base,
  `dδ/dt = 2π 60 (ω − ω_sys)`, `2H dω/dt = Pm − Pe − D (ω − 1) / ω`, `Pe` the
  power at the EMF, `Pm` constant; `ω_sys` is 1 with a voltage source in the
  case, else the speed of the machine at the slack bus;
- voltage source (GENCLS without inertia): a constant EMF behind its ZSORCE
  reactance;
- loads: `I = (P − jQ) / conj(V)` with `P0`, `Q0` the power flow values (times
  the scale of a load change) and `V0` the power flow voltage:
  - constant power `P = P0`, `Q = Q0` (the exact law; every voltage here stays
    above 0.2 `V0`), or constant impedance `P0 (V/V0)²`, `Q0 (V/V0)²`;
  - load recovery (`set_load_recovery(α, T)`): `P = P0 (V / Vf)^α` with
    `dVf/dt = (V − Vf) / T`, `Vf(0) = V0`;
  - load saturation (`set_load_saturation(c, Vk)`): `Q += c P0 ((V − Vk') / 0.1)²`
    for `V > Vk' = max(Vk, V0)`;
  - one-sided rise (`set_load_rise(c)`): `Q += c P0 ((V − V0) / 0.1)²` for
    `V > V0`;
  - PSS/E `PQBRAK` at `Vb` (0.7 pu, capped at `V0`): `P = P0 (V / u)²`,
    `Q = Q0 (V / u)²` with `u = max(V, Vb)`; rounded off over `Vb ± d`
    (`TELLEGEN_PQBRAK_SMOOTH=d`, for a bus starting at least `d` above its
    break) by the parabola `u = Vb + (V − Vb + d)² / (4d)`, which meets the
    plain break with the same value and slope at both ends of the band;
- HVDC link in AC emulation (`set_hvdc_emulation`): the active power
  `clamp(K (Δθ − Δθ0), −Pmax, Pmax)`, `K` in pu per radian, drawn at the from
  bus as `P V / |V|²` and injected at the to bus likewise, `Δθ` the angle of
  the from bus against the to bus and `Δθ0` its start value.

It starts from its own Newton power flow (generators at their setpoints, no
reactive limits; tellegen runs with `reactive_limits: false`). It is
integrated with Rodas5P at reltol = abstol = 1e-10 on the mass-matrix form
with a step forced on every sample; each event ends a segment, after which
the algebraic variables are solved again by Newton with the differential
states held. Eigenvalues: the Jacobian of the model by forward-mode automatic
differentiation at the start, the algebraic variables eliminated.

Cross-check of the model itself: without switches it agrees with
PowerSimulationsDynamics.jl 0.16.2 (unmodified; Rodas4 at reltol = abstol =
1e-8, the algebraic states solved again after each event) on the same cases
to 6.9e-7 pu in power, 1.9e-7 rad in rotor angle, 7.6e-9 pu in speed and
1.9e-8 pu in voltage (feeder, three events, `psid_check` in `loads.json`) and
to 3.0e-7, 8.6e-8, 9.0e-10 and 6.7e-8 rad in bus angle (two areas,
`psid_check` in `hvdc.json`); the inter-area mode without a link agrees to
1e-8 rad/s.

## `loads.json`: load models on a feeder

The feeder: an infinite source at 101 (GENCLS without inertia behind 1e-5 pu)
feeds the load bus 103 over two lines of 0.1 pu; a classical machine at 102
(80 MW at 1.02 pu, 100 MVA, X'd 0.2 pu, H 3 s, D 2 pu) feeds it over
0.05 pu; 103 feeds the load bus 104 over 0.05 pu. Loads 150 MW + 30 Mvar at
103 and 60 MW + 20 Mvar at 104, constant power. 4 s per run.

Events: `trip`, line 101-103 circuit 1 opens at 0.505 s (103 dips to 0.988
pu); `shed`, the load at 104 scaled by 0.4 at 0.505 s (103 rises to 1.0025
pu); `fault`, a fault at 103 through 0.05 pu from 0.505 to 0.605 s (103 at
0.52 pu, then swings to both sides of its start voltage); `fault_band`, a
fault at 103 through 0.1 pu from 0.505 to 0.705 s (103 and 104 held near
0.69 pu, within 0.1 pu of the break).

Runs: `base` (no switch) through `trip`, `shed`, `fault`; `recovery` (α = 2,
T = 1 s) through `trip`, `fault`; `saturation` (c = 2, Vk = 0.99 pu, above
the start voltage of 104, 0.985 pu, and below that of 103, 0.9955 pu) and
`rise` (c = 2) through `shed`, `fault`; `pqbrak` (Vb = 0.7) and
`pqbrak_smooth` (Vb = 0.7, d = 0.1) through `fault_band`.

Largest differences of tellegen at 1 ms (and their tolerances): rotor angle
1.2e-4 rad (3.8e-4), speed 5.3e-6 pu, machine power 4.5e-4 pu (1.4e-3),
load power 6.5e-5 pu, voltage 3.3e-6 pu, all in `recovery fault`; without
recovery 2.3e-4 pu in machine power (`pqbrak_smooth`), 6.4e-7 pu in load
reactive power and 1.3e-6 pu in voltage. Halving the step quarters every
difference (tellegen's trapezoidal steps), down to the rounding of the
reference (seven significant digits: up to 5e-7 for a value from 1 to 10,
such as a voltage or speed above 1 pu), with one exception: the active power of a recovering load, and
what follows from it, halves, because tellegen moves `Vf` once after each
step, `Vf += (dt / T) (V − Vf)`, a first-order update of the documented
lag.

## `plant.json`: voltage exponent of a plant's reactive set point

The plant feeder: the infinite source at 101 feeds the load bus 103 (150 MW +
30 Mvar, constant impedance) over two lines of 0.1 pu; a renewable plant at
104 (REGCA1, REECB1, REPCA1 as in the test; 50 MW on 100 MVA at 1.01 pu, in
reactive power control: `Ref_Flag = 0`, `PF_Flag = 0`, `V_Flag = 1`) feeds it
over 0.05 pu. 5 s per run. Events: `step`, the load at 103 scaled by 1.5 at
0.505 s (104 settles 1 % lower); `trip`, line 101-103 circuit 1 opens at
0.505 s.

Reference: PowerSimulationsDynamics.jl 0.16.2 (Rodas4 at reltol = abstol =
1e-8, the algebraic states solved again after each event) with one method
added (`plant.jl`): for this plant and these flags the plant controller
regulates its filtered reactive power to `Q_ref (V / V0)^n`, `V` the plant's
measured terminal voltage (the REECB1 `Trv` filter state `Vt_filt`) and `V0`
its start value; the rest of the method is PowerSimulationsDynamics.jl's.
tellegen runs the plant as PowerSimulationsDynamics.jl has it
(`psid_options`, its low-voltage active current gain). Runs: `base` (n = 0,
PowerSimulationsDynamics.jl as installed) through `step`, `q_exponent`
(n = 2) through `step` and `trip`; with n = 2 the plant settles at
`Q0 (V / V0)²` (0.2696 pu against 0.2751 at the start).

Largest differences at 1 ms: 4.3e-7 pu in active power, 4.1e-8 rad in angle;
reactive power 1.02e-5 pu and voltage 5.0e-7 pu, both constant over the
step and present without the exponent: the reactive power the plant's source
reactance (`X_source` = 1e5 pu, PowerSystems.jl's default for a DYR plant)
draws, `V² / X_source`, which PowerSimulationsDynamics.jl counts in the
plant's reactive power and tellegen does not.

## `hvdc.json`: HVDC links in AC emulation

Two areas: classical machines at 1 and 2 (500 MVA, X'd 0.3 pu, H 4 s, no
damping) feed the loads at 3 and 4 (250 MW each, constant impedance) over
lines of 0.02 pu; the tie 3-4 has 0.3 pu. Event `step`: the load at 4 scaled
by 1.2 at 0.105 s. 5 s per run. Runs: `base` (no link), `link` (3-4,
K = 50 MW/degree, unlimited: up to 27 MW), `link_limited` (the same, limited
to 20 MW, which it reaches). Angles are compared against machine 1 and bus 1,
and `link:3-4` is the power the link moves, which tellegen's test takes from
the balance at bus 3.

Largest differences at 1 ms: rotor angle 7.3e-6 rad, speed 1.1e-7 pu, power
1.6e-5 pu, link power 1.5e-5 pu; halving the step quarters them, down to
the rounding of the reference.

`inter_area_mode`: the oscillatory eigenvalue of the model linearized at the
start with a link of K = 0, 10, 50 MW/degree (0.985, 1.299, 1.553 Hz;
tellegen within 1.1e-7 rad/s), PowerSimulationsDynamics.jl's without a link
(`psid`), and PowerSimulationsDynamics.jl's with an AC line 3-4 in place of the
link, of reactance `1/K` (`psid_ac_line_inverse_k`: 1.308 and 1.570 Hz) and
of the reactance whose synchronizing power `V3 V4 cos(θ3 − θ4) / x` at the
start is `K` (`psid_ac_line_matched`: 1.311 and 1.572 Hz, 0.9 % and 1.2 %
above the link's). The third value of each AC-line entry is its reactance.
