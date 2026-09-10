# Multiconductor fixed-point power flow

In the web app, open **Studies** and choose **Load 4-conductor example**, or
upload a supported BMOPF JSON distribution case. The Studies panel changes to
**Distribution power flow** for multiconductor inputs. Choose **Run power
flow**, then select a bus or branch on the map to inspect its results.

Bus tables report each terminal's voltage magnitude to earth in volts and
angle in degrees, including an explicit neutral. When a bus has a neutral,
additional columns show the magnitude and angle of the complex difference
`V_terminal - V_neutral`. The neutral-to-neutral voltage is zero and its angle
is undefined. Branch tables report conductor
currents in amperes and terminal active/reactive powers. The summary reports
source power in kW/kvar and passive active losses in kW. These are simulation
results; the distribution Study does not expose balanced-network optimization
goals or sensitivities.

Solver availability follows the parsed input type and supported electrical
semantics. Balanced transmission cases retain their existing Study workflow.
Unsupported distribution cases can still be inspected on the map, with a
reason explaining why the power-flow action is unavailable. Raw OpenDSS
imports remain view-only until their source and load semantics can be retained
without approximation; use a supported normalized BMOPF input for this solver.

Saved distribution results use a separate, versioned simulation snapshot in
the browser's IndexedDB. A snapshot contains the typed input, solver options,
portable PowerIO solution, and detailed terminal results. Reopening validates
the saved input/result association without running a new factorization.
Use **Save result** to retain it locally, **Export** to download the snapshot,
and **Import** or the **Saved study** selector to reopen it. Browser storage is
local to the app's origin; export a snapshot to move it to another browser.

The corresponding engine APIs are `solveMcStudy` and `replayMcStudy`;
Rust exposes `solve_mc_study_json` and `replay_mc_study_json`. These simulation
snapshots are distinct from the balanced-network `StudyDocument`.

The multiconductor PF entry point is `tellegen::solve_bmopf_json`. It accepts
raw BMOPF JSON, validates fields that PowerIO 0.11.0 would otherwise collapse,
then parses the document through PowerIO and solves the resulting typed
`McAcPfInstance`. The native runner has the same path:

```text
cargo run -p tellegen --example mc_pf --features mc-pf -- case.json
```

The browser build exposes the same operation as `solve_mc_bmopf(text,
options_json)`. The TypeScript transport is `solveMcBmopf(text, options)`.
For a serialized PowerIO MC module, use `solve_mc_module_json` in Rust,
`solve_mc_module` in WASM, or `solveMcModule` in TypeScript. Typed Rust callers
can pass an `McAcPfInstance` directly to `solve_mc_ac_pf_instance`.

Options are JSON fields `tolerance`, `max_iterations`, `damping`,
`zero_voltage_tolerance`, `absolute_kcl_tolerance`,
`relative_kcl_tolerance`, `voltage_envelope`, `v_low_pu`, `v_min_pu`, and
`v_max_pu`; defaults are `1e-8`, `100`, `1.0`, `1e-9`, `1e-6 A`, `1e-8`,
`true`, `0.5`, `0.85`, and `1.15`. Convergence requires both the
voltage-change tolerance and the physical KCL test: the residual must be below
`absolute_kcl_tolerance + relative_kcl_tolerance * incident_current_scale`.
Voltages are volts, currents are amperes, and terminal powers are VA. Complex
values cross JSON as `{ "re": ..., "im": ... }`.

The solver supports voltage-dependent loads, ideal voltage sources,
lines, passive shunts, and finite-leakage two-winding transformers, including
two-winding `n_winding` records with explicit BMOPF delta rolls and fixed-tap
`single_phase_autotransformer` regulator snapshots. Transformer
admittance follows the
OpenDSS terminal primitive: common leakage base, winding voltage/tap maps,
actual WYE/DELTA coil incidence, explicit neutral impedance, and explicit
per-coil excitation shunts. Zero leakage (an ideal voltage constraint),
transformers with more than two windings, ideal zero-impedance regulators,
finite source impedance, and unsupported controls
are rejected with an error rather than approximated.
The direct OpenDSS YPrim comparison, including YY/DD/YD/DY fixed-tap cases and
per-winding delta-roll cases, is frozen in
[`docs/evidence/mc-pf-yprim`](evidence/mc-pf-yprim) and exercised by the
transformer unit tests.

Raw BMOPF inputs must use scalar or uniform taps. An `n_winding` raw record
cannot carry a tap because PowerIO 0.11.0 drops that field; a canonical typed
two-winding record can carry its fixed winding taps.
Non-constant-power loads must carry explicit nominal branch voltages.
Constant-power loads may omit them: the solver assigns each line-connected
voltage zone a line-to-neutral base from transformer winding ratings or other
explicit nameplates, falling back to its ideal source voltage when no rated
anchor exists. It converts that base to the load's WYE, DELTA, or single-phase
branch voltage. Missing or conflicting anchors are rejected instead of guessed.
Nonzero legacy `g_no_load`/`b_no_load` values on isolating transformers require
normalization to an explicit `no_load_shunt`; this avoids choosing between the
tagged schema and BMOPFTools legacy reference sides. Exact-zero legacy fields
are accepted as no-ops, and `single_phase_autotransformer` defines nonzero
legacy excitation across its from winding unambiguously. An OPF instance can
be projected to PF through PowerIO's typed conversion; its objective and active
constraints are discarded by that conversion and PowerIO emits the standard
projection diagnostic; the convenience raw solve proceeds with the projected
PF instance.

A declared solid transformer neutral (`r=x=0`) is returned as an exact fixed
zero-voltage terminal constraint; it is not replaced by a large admittance. A
negative canonical `r_neutral` means floating. Grounded bus terminals remain
the same exact constraint representation.

Within one prepared solve, the sparse factor is built once and reused for
every fixed-point iteration while topology, taps, nominal admittances, and
fixed terminals are unchanged. A solve with no unknown terminals has no
factorization to perform.

## Load voltage models

Each load branch uses its complex terminal-to-terminal voltage `U`, including
neutral displacement for an explicit WYE neutral and line-to-line voltage for
a DELTA branch. Let `r = abs(U) / v_nom`, with nominal powers `P0` and `Q0`.

| BMOPF model | Active power | Reactive power |
| --- | --- | --- |
| `constant_power` | `P0` | `Q0` |
| `constant_current` | `P0 * r` | `Q0 * r` |
| `constant_impedance` | `P0 * r^2` | `Q0 * r^2` |
| `zip` | `P0 * (alpha_z*r^2 + alpha_i*r + alpha_p)` | `Q0 * (beta_z*r^2 + beta_i*r + beta_p)` |
| `exponential` | `P0 * r^gamma_p` | `Q0 * r^gamma_q` |

For nonzero branch voltage, consumed current is `conj((P + j*Q) / U)`.
Constant-current loads therefore track the branch-voltage angle and retain
their power factor; they are not fixed complex current phasors. ZIP coefficients
are used as supplied, without silently normalizing their sums. Active and
reactive coefficients or exponents may differ.
With the default voltage envelope, all non-impedance load models use nominal
impedance below `v_low_pu`, linearly interpolate complex current between the
impedance and constant-power values from `v_low_pu` to `v_min_pu`, use the
declared model from `v_min_pu` through `v_max_pu`, and use the impedance fixed
at `v_max_pu` above it. This is the convergence fallback used by OpenDSS, with
the normal band widened here to the solver-wide 0.85–1.15 pu defaults. Constant
impedance is unchanged. Disabling `voltage_envelope` restores the raw equations;
at exactly zero voltage, raw laws without a continuous limit return a controlled
error. Non-finite currents or absorbed powers are always rejected.

The piecewise regions follow the [OpenDSS convergence-model
description](https://opendss.epri.com/LoadModelsThatNearlyAlwaysConver.html)
and its [`Load.pas`
implementation](https://github.com/tshort/OpenDSS/blob/master/Source/PCElements/Load.pas).
Explicit nominal voltages, ZIP coefficients and exponential exponents may be
supplied once for all branches or separately per branch. Unknown model names,
incomplete parameter sets and conflicting coefficient families are rejected;
they are not interpreted through PowerIO's permissive model fallback.

The nominal admittance remains fixed. Each iteration compensates its current
against the selected load law, so these models retain the single-factorization
algorithm. Reported load currents and powers use the operating voltage, rather
than merely copying nominal powers.

Convergence and voltage validity are reported separately. `voltage_valid` is
true only when every load branch lies within the configured `v_min_pu` and
`v_max_pu` band. `min_voltage_pu`, `max_voltage_pu`, and
`voltage_violations` identify the observed range and each offending load branch.

These are the BMOPF voltage laws plus the OpenDSS low/high-voltage fallback,
not the complete OpenDSS load engine. ZIPV cutoff, time-dependent load
multipliers, and control behavior are not added by this extension. Equation-only
OpenDSS comparisons disable the envelope or remain within the matching range.
The frozen load references use OpenDSS model 2 for impedance, model 5 for
current, model 8 for ZIP with cutoff disabled, and model 4 with specified CVR
exponents for exponential loads. This comparison does not add support for
OpenDSS model-number input or its time-series/load-status semantics.
