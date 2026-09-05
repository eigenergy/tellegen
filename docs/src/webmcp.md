# WebMCP

Tellegen exposes the solved case in the current browser tab through structured
tools. They read and update the state shown in the interface and run the same
WebAssembly solver.

Browsers without `document.modelContext` run the application without these
tools.

## Tools

| Tool                    | Behavior                                                                                                                             |
| ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| `inspect_case`          | Read the active case, formulation, solve summary, edit counts, case ID, and revision.                                                |
| `query_network`         | Return a bounded set of buses or branches by stable identity or solved metric.                                                       |
| `analyze_sensitivity`   | Compute an LMP sensitivity column for the current DC OPF solution.                                                                   |
| `focus_network`         | Select a bus or branch in the visible interface.                                                                                     |
| `preview_case_update`   | Predict demand or rating edits from cloned state.                                                                                    |
| `update_case`           | Solve and commit a bounded batch of demand or rating edits.                                                                          |
| `reset_case`            | Solve and commit the base state with the current formulation.                                                                        |
| `propose_capacity_plan` | Use implicit derivatives to choose bounded capacity increase trials, verify them with exact solves, and stage an unapplied proposal. |
| `apply_capacity_plan`   | Apply an approved proposal after checking its session, case, and revision.                                                           |

The general OPF tools register for the page lifecycle. `inspect_case` reports
when no solvable case is active; the other general tools require one.
`propose_capacity_plan` registers when the active formulation supports it.
`apply_capacity_plan` registers only while a staged proposal matches the current
state.

## Revisions and mutations

Every page load receives a random session identifier. Each case has a
monotonic revision within that session. A committed edit, reset, formulation
change, or visible network selection advances the revision and expires pending
proposals and approvals.

Preview, standalone sensitivity analysis, and planning operations use cloned
state. Electrical mutations enter one queue, check their expected revision
inside the queue, solve a candidate state, and commit once after the solve
succeeds. A failed solve leaves the case, formulation, revision, proposal, and
approval unchanged.

Tool callback options are optional because WebMCP clients do not all provide
an `AbortSignal`. When a caller supplies one, reads and planning stop at safe
boundaries. The browser reports dynamic registration failures instead of
claiming that a missing tool group registered successfully.

## Capacity planning

`propose_capacity_plan` accepts a weighted LMP objective, stable candidate
branch identities, a total MW budget, a maximum increase per branch, a fixed
increment, a global line count, and an exact solve budget. The solve budget
counts the baseline and every accepted or rejected trial.

The planner computes a vector product through the solved DC OPF KKT system,
orders capacity increases by that local direction, and checks each trial with an
exact solve. It recomputes the direction after every accepted trial. Each solved
trial records its first order prediction and exact result; a failed trial records
the failure reason without an exact delta.

The proposal changes no electrical state. A visible control grants one use
approval for its proposal ID and base revision. Applying it repeats every
identity and revision check inside the mutation queue. A failed or stale
application consumes neither the proposal nor its approval.

## Persistence

The activity panel records the last 100 completed tool calls in an experiment
journal, including validated requests, bounded results, failures, and elapsed
time. **Export journal** downloads that record with the retained capacity
planning trials and their decisions. The export identifies its browser session
and reports how many older records were discarded. Invalid requests are omitted
from the recorded input.

When a preview and a successful update name the same case, revision, formulation,
and edits, the panel shows the predicted objective change beside the exact
change and absolute error. A failed update leaves the comparison empty. The
relative error is undefined when the exact change is zero.

Journal exports contain data only; importing one never authorizes or executes
an edit. Proposals and approvals stay in memory. Saving a case writes
its materialized PowerIO module. Loading that module starts a fresh runtime
`Study`; its history records transformations applied to the stored value.
Saving an exact result writes a PowerIO solution module containing the
amended instance that was solved.

## Package boundary

`@tellegen/webmcp` owns tool descriptions, JSON Schemas, runtime validation,
annotations, bounded result shaping, and registration lifecycle. It depends on
a `TellegenWebMcpAdapter`, not on Svelte or the Rust engine. The application
adapter connects that interface to the active controller and visible activity
panel.

Imported labels and identifiers are treated as untrusted content. Every
mutation requires the active case ID and revision. The package rejects unknown
fields, nonfinite values, duplicate edits, excessive arrays, and unsupported
enum values at the execute boundary as well as in its schemas.

## Testing

Run the package and application checks from the repository root:

```sh
npm run check:webmcp
npm run check:web
npm run build:web
npm run test:browser
```

The tests cover registration and teardown, calls with and without callback
options, validation, output bounds, stale revisions, one use approval,
transaction rollback, queued mutations, cancellation, and visible state.
Playwright uses a test host with the WebMCP API shape. Before release, invoke
the tools through native WebMCP in the in-app browser as a separate bridge
check.

## References

- [WebMCP specification](https://github.com/webmachinelearning/webmcp)
- [Chrome WebMCP documentation](https://developer.chrome.com/docs/ai/webmcp)
- [Imperative API](https://developer.chrome.com/docs/ai/webmcp/imperative-api)
- [Tool security](https://developer.chrome.com/docs/ai/webmcp/secure-tools)
- [WebMCP evaluations](https://developer.chrome.com/docs/ai/webmcp/evals)
