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
detailed terminal results, and load-model qualification. A canonical portable
PowerIO solution is included only when the original load model is verified.
Reopening validates the saved input/result association without running a new factorization.
Use **Save result** to retain it locally, **Export** to download the snapshot,
and **Import** or the **Saved study** selector to reopen it. Browser storage is
local to the app's origin; export a snapshot to move it to another browser.

The corresponding engine APIs are `solveMcStudy` and `replayMcStudy`;
Rust exposes `solve_mc_study_json` and `replay_mc_study_json`. These simulation
snapshots are distinct from the balanced-network `StudyDocument`.

After the first solve, selecting a bus exposes its load-branch P and Q values.
Changing either value automatically warm-solves the same fixed-point
current-injection problem. Rapid changes are coalesced, and **Reset** removes
that branch's override. The edited powers and matching result are included in
subsequent saved snapshots. Large portable input/solution modules are
materialized only when saving, exporting, attaching geography, or explicitly
rebuilding the calculation. An ordinary edit sends the edit set to the worker
and receives a constant-size summary in one round trip; the Studies panel
fetches the selected bus's terminals and one page of equipment results on
demand.

The interactive API is a retained `McPfSession` in Rust and
`createMcPfSession(moduleJson, options)` in TypeScript. A session keeps the
prepared passive network, compensation matrix, sparse LU factorization, and
last converged voltage. `replace_load_powers`/`replaceLoadPowers` accepts the
complete set of absolute load-branch overrides relative to the session's base
input; an empty set restores the base powers. Failed validation or convergence
leaves the previous solved operating point intact.

Edits are an overlay on the base input. They replace the prepared load laws
(physical nominal admittance follows the edit; the compensation reference and
factor stay frozen) and never copy or rebuild the network. Each solve produces
an `McPfSummary`, computed from the converged phasors with the same output
finiteness checks as the complete `McPfResult`, which is built only on request
(`build_result` in Rust, `result()` in TypeScript). `edited_instance`,
`input_module_json`, and `snapshot` build the edited network on demand, and
`materialization_count` counts how often. Detail views use `detail` (one bus
and a page of equipment ports) and the interleaved terminal voltage and
current arrays. The session retains the stored module's records rather than
its JSON text, so portable output needs no re-parse and is byte-identical to
re-parsing. `profile` and `cold_profile` time load evaluation, KCL and
matrix-vector work, retained LU solves, and the summary, plus parse, prepare,
and factor at creation; WASM builds time them with `performance.now`. Topology, taps, source
voltages, load connection maps, load voltage-model parameters, and solver
options are structural session data: changing them requires a new session.

The multiconductor PF entry point is `tellegen::solve_bmopf_json`. It accepts
raw BMOPF JSON, validates fields that PowerIO 0.11.0 would otherwise collapse,
then parses the document through PowerIO and solves the resulting typed
`McAcPfInstance`. The native runner has the same path:

```text
cargo run -p tellegen --example mc_pf --features mc-pf -- case.json
```

The retained-session example scales every load branch and reports cold/warm
timings and phase profiles, iteration counts, and the factorization and
materialization counts:

```text
cargo run --profile release-py -p tellegen --example mc_pf_session --features mc-pf -- case.json 1.001
```

The generated feeder benchmark measures latency, payload, and memory at
32,412 to 676,530 terminals, natively and in the browser, and CI runs its
~10,000-bus preset as a deterministic regression; see
`docs/src/mc-pf-performance.md` and `crates/benchmarks/README.md`.

The browser build exposes the same operation as `solve_mc_bmopf(text,
options_json)`. The TypeScript transport is `solveMcBmopf(text, options)`.
For a serialized PowerIO MC module, use `solve_mc_module_json` in Rust,
`solve_mc_module` in WASM, or `solveMcModule` in TypeScript. Typed Rust callers
can pass an `McAcPfInstance` directly to `solve_mc_ac_pf_instance`.

Options are JSON fields `tolerance`, `max_iterations`, `damping`,
`zero_voltage_tolerance`, `absolute_kcl_tolerance`,
`relative_kcl_tolerance`, `voltage_envelope`, `v_low_pu`, `v_min_pu`, and
`v_max_pu`; defaults are `1e-4 V`, `100`, `1.0`, `1e-9 V`, `1e-4 A`, `1e-5`,
`true`, `0.5`, `0.85`, and `1.15`. Convergence requires both the
voltage-change tolerance and the physical KCL test: the residual must be below
`absolute_kcl_tolerance + relative_kcl_tolerance * incident_current_scale`.
Voltages are volts, currents are amperes, and terminal powers are VA. Complex
values cross JSON as `{ "re": ..., "im": ... }`.

The solver supports voltage-dependent loads, ideal voltage sources,
lines, passive shunts, and finite-leakage two-winding transformers, including
phase-to-phase `single_phase` records, two-winding `n_winding` records with
explicit BMOPF delta rolls, and fixed-tap `single_phase_autotransformer`
regulator snapshots. It also supports the three coupled windings into which
PowerIO lowers a BMOPF `center_tap` transformer, including anti-series
split-phase polarity, fixed taps, excitation, and neutral impedance.
Transformer admittance follows the
OpenDSS terminal primitive: common leakage base, winding voltage/tap maps,
actual WYE/DELTA coil incidence, explicit neutral impedance, and explicit
per-coil excitation shunts. Zero leakage (an ideal voltage constraint),
arbitrary transformers with more than two windings, ideal zero-impedance
regulators, finite source impedance, and unsupported controls
are rejected with an error rather than approximated.
The direct OpenDSS YPrim comparison for YY/DD/YD/DY fixed-tap and per-winding
delta-roll cases is frozen in
[`docs/evidence/mc-pf-yprim`](evidence/mc-pf-yprim). Transformer unit tests add
a fixed-tap, excited centre-tap YPrim comparison, while the loaded centre-tap
oracle also checks terminal voltages, currents, powers, losses, and polarity.

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
every fixed-point iteration while topology, taps, the frozen compensation
admittance, and fixed terminals are unchanged. A retained session also reuses
that factor after load P/Q changes and starts from the previous converged
voltage. A solve with no unknown terminals has no factorization to perform.

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

The compensation admittance stamped into the linear operator remains fixed.
Each iteration compensates its current against the selected load law, so these
models retain the single-factorization algorithm. A retained session updates
the physical nominal admittance used by impedance and voltage-envelope laws
when P/Q changes, while leaving only the algebraic compensation reference
frozen. Reported load currents and powers use the operating voltage, rather
than merely copying nominal powers.

Convergence and voltage validity are reported separately. `voltage_valid` is
true only when every load branch lies within the configured `v_min_pu` and
`v_max_pu` band. This is a load-model band, not an engineering voltage limit
or a test of whether the load equations were modified. `min_voltage_pu`, `max_voltage_pu`, and
`voltage_violations` identify the observed range and each offending load branch.

These are the BMOPF voltage laws plus the OpenDSS low/high-voltage fallback,
not the complete OpenDSS load engine. ZIPV cutoff, time-dependent load
multipliers, and control behavior are not added by this extension. Equation-only
OpenDSS comparisons disable the envelope or remain within the matching range.
The frozen load references use OpenDSS model 2 for impedance, model 5 for
current, model 8 for ZIP with cutoff disabled, and model 4 with specified CVR
exponents for exponential loads. This comparison does not add support for
OpenDSS model-number input or its time-series/load-status semantics.


## Settings and model qualification

The [user guide](src/multiconductor-pf.md#solver-settings-and-opendss-comparison) lists every exposed default, explains the new tolerance values, and compares their units and acceptance criteria with OpenDSS.


**Power flow settings** in Studies exposes the load-model envelope and its
`v_low_pu`, `v_min_pu`, and `v_max_pu` thresholds separately from numerical
settings (maximum iterations, damping, and advanced voltage/KCL tolerances).
The form shares one set of next-run settings with the context-sensitive solve
button in both Network and Studies.
Edits and **Restore defaults** take effect on the next explicit run; editing
settings alone does not start a calculation. Invalid settings disable the run
actions. Each rerun creates a new retained session, preserving current load edits.
Reopened snapshots restore their recorded settings. Failed runs preserve the
previous result, whose assessment records the settings actually used for it.

Every new `McPfResult` and `McPfSummary` carries `load_model`:

- `status`: `original`, `modified`, or `unverified`.
- `modified_branch_count`: branches whose final currents differ from the
  original load law, allowing only floating-point roundoff (1e-12 times one
  ampere plus the larger current magnitude).
- `original_kcl_residual` (A) and `original_scaled_kcl_residual`: an independent
  check using the original load equations. A scaled residual at most one
  passes the configured KCL tolerances; both fields are null if the original
  equations are undefined or non-finite at the returned voltage.
- `options`: all effective solver settings.

The full result also includes `load_model_changes`: load/bus/branch identities,
branch voltage in pu, actual absorbed power, and original-law power at the same
voltage (W + j var, or null if undefined). A modified result remains inspectable
and saveable. Its warning persists in Studies, the network inspector, diagram
provenance and CSV exports. Numerical convergence alone never establishes an
original-model solution. Legacy results without an assessment are unverified.

Constant-impedance and zero-power branches are not marked modified merely
because they fall outside the voltage band. Envelope use during intermediate
iterations does not qualify the final point. Disabling the envelope can yield
an original-model solution outside the band. A change to the thresholds changes
the equations; it does not waive engineering operating limits.

Simulation snapshots now use version 2. For modified or unverified results,
`solution_module` is null: the snapshot retains its input, options, assessment
and full numerical result without claiming a canonical `McAcPfSolution`.
For verified original-model results, the solution module also records the
assessment in the `tellegen.mc_pf` extension. Direct `to_powerio_solution`
conversion rejects modified or unverified results.

Version 1 snapshots remain readable. Replay validates their contents,
reassesses the original load laws at the saved voltages without re-solving,
and upgrades them to version 2; a misleading canonical solution is removed
when the saved point used modified equations. Version 2 replay verifies the
saved qualification against the input, settings and terminal voltages. Terminal, source and element currents and powers are independently recomputed from the effective network equations; the result is not used as its own reference.
Unconverged iterates are still not returned as solutions.

### Explicit energized-network selection

Import detects bus islands without a voltage source using lines, closed switches
and all transformer windings to establish connectivity. Network and Studies each
show one solve button: **Run power flow** when every island has source coverage,
or **Solve energized network** when unpowered islands are present. Counts and an
expandable list of affected buses and equipment are visible before solving. A
network with no source remains inspectable, with solving disabled and an explanation.

Detection is read-only. Clicking **Solve energized network** explicitly selects
source-covered islands, revalidates the exclusions against the current input, and
records the selection. Importing a replacement case runs detection again. Load and
coordinate edits do not change source coverage. This bus-level check does not
establish conductor-level solvability, change solver settings, or reconnect open
switches. The low-level full-network solver still rejects unpowered islands unless
an explicit selection is recorded.

The original full network remains in the map, diagram and saved input module.
The input's `tellegen.energized_network` extension lists each excluded island's
buses and equipment. Open boundary switches are listed with the excluded island;
a switch joining two excluded islands appears in both lists. Aggregate counts
deduplicate equipment by kind and name. Excluded loads are explicitly unsupplied
and absent from solved totals. Missing results are not synthesized as zero.

Full results carry `network_scope` and `excluded_islands`; compact session summaries
carry only scope counts. Saved Study JSON retains the full input and all exclusions.
A canonical PowerIO solution, when available, covers the selected network and
carries the exclusion record in its module extension. CSV provenance carries both
network scope and load-model qualification. Replay checks exclusions against the
full input topology and checks the result against the selected instance. Load
edits update only the solved network's loads in the retained full input.

This selection is independent of load-model qualification: an energized subset
may solve either the original load equations or modified equations. The scope
and qualification are displayed separately. Importing a prepared input or
reopening a saved Study retains the explicit selection for subsequent runs.

Saved studies compact formatting whitespace in their embedded PowerIO modules.
All values, numeric spellings and string contents are retained, while large
full-network studies fit within the existing browser input limit more readily.
