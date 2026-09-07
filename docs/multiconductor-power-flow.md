# Multiconductor fixed-point power flow

In the web app, open **Studies** and choose **Load four-wire example**, or
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
snapshots are distinct from the optimization-oriented `StudyDocument` format.

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

Options are JSON fields `tolerance`, `max_iterations`, `damping`, and
`zero_voltage_tolerance`, `absolute_kcl_tolerance`, and
`relative_kcl_tolerance`; defaults are `1e-8`, `100`, `1.0`, `1e-9`,
`1e-6 A`, and `1e-8`. Convergence requires both the voltage-change tolerance
and the physical KCL test: the residual must be below
`absolute_kcl_tolerance + relative_kcl_tolerance * incident_current_scale`.
Voltages are volts, currents are amperes, and terminal powers are VA. Complex
values cross JSON as `{ "re": ..., "im": ... }`.

The solver supports voltage-dependent loads, ideal voltage sources,
lines, passive shunts, and finite-leakage two-winding transformers, including
two-winding `n_winding` records with explicit BMOPF delta rolls. Transformer
admittance follows the
OpenDSS terminal primitive: common leakage base, winding voltage/tap maps,
actual WYE/DELTA coil incidence, explicit neutral impedance, and explicit
per-coil excitation shunts. Zero leakage (an ideal voltage constraint),
transformers with more than two windings, finite source impedance, and unsupported controls
are rejected with an error rather than approximated.
The direct OpenDSS YPrim comparison, including YY/DD/YD/DY fixed-tap cases and
per-winding delta-roll cases, is frozen in
[`docs/evidence/mc-pf-yprim`](evidence/mc-pf-yprim) and exercised by the
transformer unit tests.

Raw BMOPF inputs must use scalar or uniform taps. An `n_winding` raw record
cannot carry a tap because PowerIO 0.11.0 drops that field; a canonical typed
two-winding record can carry its fixed winding taps.
Loads must carry explicit nominal branch voltages; the solver
does not invent a nominal voltage for a two-wire or other branch connection.
Legacy `g_no_load`/`b_no_load` values require
normalization to an explicit `no_load_shunt`; this avoids choosing between the
tagged schema and BMOPFTools legacy reference sides. An OPF instance can be
projected to PF through PowerIO's typed conversion; its objective and active
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
At exactly zero voltage, laws with a continuous zero-current limit return zero;
nonzero constant-power or constant-current contributions return a controlled
error. Tiny nonzero voltages remain usable for bounded current laws. Non-finite
currents or absorbed powers are rejected.
Explicit nominal voltages, ZIP coefficients and exponential exponents may be
supplied once for all branches or separately per branch. Unknown model names,
incomplete parameter sets and conflicting coefficient families are rejected;
they are not interpreted through PowerIO's permissive model fallback.

The nominal admittance remains fixed. Each iteration compensates its current
against the selected load law, so these models retain the single-factorization
algorithm. Reported load currents and powers use the operating voltage, rather
than merely copying nominal powers.

These are the BMOPF voltage laws, not the complete OpenDSS load engine.
OpenDSS low/high-voltage fallback, ZIPV cutoff, time-dependent load multipliers,
and control behavior are not added by this extension. OpenDSS comparisons must
disable those differences or remain within the matching operating range.
The frozen load references use OpenDSS model 2 for impedance, model 5 for
current, model 8 for ZIP with cutoff disabled, and model 4 with specified CVR
exponents for exponential loads. This comparison does not add support for
OpenDSS model-number input or its time-series/load-status semantics.
