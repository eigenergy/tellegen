# Persistent Studies

A Study records a goal, the electrical states explored in pursuit of it, and the
exact evidence behind a recommendation. Open **Studies** in the network view,
state a goal, resolve the equipment and weights, and review the interpretation
before creating the Study. The editable JSON is the numerical request.

The starting PowerIO calculation instance preserves the inner OPF objective and
constraints. The Study objective is a separate scalar expression over its solved
prices, dispatch, flows or voltages. Expressions compose weighted observables,
sums, scaling, squared target deviations and direct intervention penalties.
The engine evaluates the complete derivative with a combined adjoint solve,
including the direct penalty terms. It does not build a dense matrix containing
all observable/decision pairs.

## Explore, compare and apply

Capacity decisions use MW for DC OPF and MVA for SOCWR. Active-demand decisions
use MW. Each decision states a stable element identity, bounds and an increment.
The shared feasible set adds a weighted absolute-change budget and a maximum
number of changed elements. A goal retains its starting anchor: continuing from
a candidate measures cumulative changes from that anchor and does not reset the
budget. Placement allocates the declared total additional load; redistribution
uses paired transfers that preserve total demand.

**Find a proposal** ranks feasible moves with the current gradient, solves a
bounded beam exactly, keeps a verified improvement and recomputes the direction.
Every attempted solve consumes the budget, including baseline reconstruction and
failed trials. The returned recommendation is the best verified candidate found,
not a certificate of global optimality. Active-set changes can invalidate a
local derivative. Trial evidence records predictions, exact values, changed
active constraints, failures and termination. Derivative evidence includes its
regularization and refinement settings.

The Study keeps three separate state pointers:

| Pointer | Meaning |
|---|---|
| Inspecting | The saved state shown on the map and used for the next action |
| Recommended | The best verified candidate from the current proposal |
| Applied | The state explicitly accepted through the Apply action |

Selecting a state or branching from it changes the view. Applying a proposal is
an explicit action. Approval binds the proposal to its goal, base state and
revision; changing any of them expires the approval. A goal revision retains the
previous interpretation and its evidence. Comparisons can evaluate saved
candidates under a selected goal revision. Inspection and attached evidence do
not invent a new electrical state. A challenge can name the recommendation it
assesses with `assessed_recommendation`.

The saved-state map belongs to the Study. **Return to live case** returns to the
interactive case loaded outside it. Importing or inspecting a Study does not
execute imported text or silently overwrite that live case.

## Demand edits and base-case reset

**Solve demand edit** adds a signed MW increment at a permitted bus in the
selected saved state. Successive edits accumulate across buses and survive
export and reload. Bounds, increments, the absolute-change budget and the
number of changed buses apply to the cumulative changes from the goal anchor.
A partial placement or redistribution remains a saved candidate until its total
is satisfied; it cannot be applied as a recommendation yet. Edit evidence
includes the sparse demand-change vector relative to the original network data.

**Prepare base-case reset** exactly solves the original network input and saves
a reset candidate. Applying that candidate restores the original demand and
ratings without deleting the goal revisions, earlier states or activity. The
original input is distinct from the Study's starting point, which may already
contain live edits. Native `CreateStudy` accepts optional `base_input` for that
original PowerIO IR; without it, the supplied `input` is the base. Older imported
bundles without retained base data report that reset is unavailable.

The shared operations are `edit_demand` and `restore_base`. Both leave the
applied pointer unchanged until an explicit Apply action. Observations,
edits and solves, planning trials, and decisions remain distinct in Activity.

## Storage and continuation

Electrical inputs, calculation instances and exact solutions use PowerIO
IR generation 2. Study semantics live in the application document. A portable
bundle contains a deduplicated SHA-256 artifact map, immutable state and goal
records, experiments, evidence references and decisions. Browser storage uses
IndexedDB; the native interface saves the same bundle on the filesystem.
Completed operations save atomically before the controller publishes them.

Export a Study to continue in another browser or through the native CLI.
Import checks document versions, identities, artifact hashes and references,
and verifies that saved solution instances agree with their state inputs.
Approval tokens never enter the bundle. Older experiment journals import as
historical evidence; unavailable electrical states remain explicitly unavailable.

A storage failure reports the failed save and recovery choices. Free storage or
export the current saved Study before retrying. Filesystem writers use a revision
check and an exclusive lock; inspect a leftover lock's process before removing
it. A second writer cannot silently replace a newer revision.

Cancellation finishes the current exact trial, then saves the completed trials,
best candidate and cancelled termination. An exact solve is indivisible, so
cancellation latency depends on that trial. Closing or forcibly killing the
process before the save can lose the current operation; the previous completed
revision remains durable. PowerMCP defaults to a 300-second graceful termination window before forcing a
stopped process to exit. Its runtime and cancellation limits are configurable.

## Native and agent interfaces

```sh
cargo build -p tellegen-cli --features conic
tellegen contract
tellegen study create study.json < create-request.json
tellegen study inspect study.json
tellegen study run study.json < operation-request.json
tellegen study export study.json > portable-study.json
tellegen study import another-study.json < portable-study.json
```

`tellegen contract` returns generated schemas for `CreateStudy`, `StudyRequest`,
`StudyBundle` and the operation result. Rust is the contract authority; the
browser's TypeScript types and runtime schemas are generated from those same
records. `expected_revision` accompanies each mutation.

Browser WebMCP exposes `create_study`, `inspect_study`, `revise_study_goal`,
`branch_study`, `compare_study_states`, `propose_study` and
`record_study_evidence`, `edit_demand` and `restore_base_case`. Inspection returns compact continuation context and
bounded pages of larger records. The browser controls call the same controller.
PowerMCP's `tellegen` adapter invokes the native CLI directly and uses the same
request schemas. Its agent interface leaves application to an explicit user
action.

A sensitivity build supports DC OPF and AC power flow Studies. The `conic`
feature adds SOCWR. Nonlinear AC OPF and multiconductor solving are unavailable;
unsupported objective/formulation combinations fail before exploration.

The capacity WebMCP tools create persistent Studies and use the same bounded
search as `propose_study`. Their compact response includes `study_id` for
continuation. The creation solve and every planning solve count toward the
capacity call's budget. A human capacity approval binds the Study's goal, starting
state and recommended state. Goal changes expire it; recording inspection evidence
does not. Failed persistence leaves the approval available for a retry.

When a matching live case is open, case inspection, network queries and sensitivity
queries attach evidence to its captured Study state. They create no electrical
state. Queries about a different or edited live case remain independent until a
new Study captures that case. If a different case finishes loading while an
approved capacity change is being saved, the saved Study remains the authoritative
result and the tool reports `case_updated: false`; the new live case is untouched.

Reproducible large-case declarations and an installed-CLI runner live in
[`evidence/studies`](https://github.com/eigenergy/tellegen/tree/codex/webmcp-challenge-v1/evidence/studies).
The original Texas7k cost curves include decreasing piecewise slopes, which the
convex DC OPF formulation rejects. A separately labelled example constructs and
records their lower convex envelope through PowerIO's typed network API. It
changes the inner economic model and does not establish an optimum for the
original nonconvex case.
