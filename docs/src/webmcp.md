# WebMCP

Open **Agent**, then **Connect**, on the map for connection status, a copyable example
request and the connection guide. A connected agent must discover the WebMCP
tools exposed by this browser tab. A regular browser without WebMCP still
supports manual interaction. The first-visit introduction links to the
application changelog.

Tellegen exposes the solved case in the current browser tab through structured
tools. They read and update the state shown in the interface and run the same
WebAssembly solver.

Browsers without `document.modelContext` run the application without these
tools.

## Tools

| Tool                          | Behavior                                                                                                                             |
| ----------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| `list_cases`                  | List configured and imported cases, availability, and stable case IDs.                                                               |
| `select_case`                 | Switch the displayed case without deleting saved Studies or applying proposals.                                                      |
| `import_case`                 | Open case file text as a new case, the way a dropped file opens, and report PowerIO's parse findings.                                |
| `export_case`                 | Write the displayed case through PowerIO to an exchange format or a `.pio.json` module, download it, and report conversion findings. |
| `inspect_case`                | Read the displayed case or saved state, formulation, solve summary, IDs, units, and revision.                                        |
| `query_network`               | Return a bounded set of buses or branches by stable identity or solved metric.                                                       |
| `analyze_sensitivity`         | Compute an LMP sensitivity column for the current DC OPF solution.                                                                   |
| `focus_network`               | Select a bus or branch in the visible interface.                                                                                     |
| `preview_case_update`         | Predict demand or rating edits from cloned state.                                                                                    |
| `update_case`                 | Solve and commit a bounded batch of demand or rating edits.                                                                          |
| `reset_case`                  | Solve and commit the base state with the current formulation.                                                                        |
| `inspect_settings`            | Read the selected and accepted calculation settings, or this browser's display preferences.                                          |
| `update_settings`             | Validate and apply a settings patch atomically without solving.                                                                      |
| `reset_settings`              | Return the selected calculation's settings to their defaults without solving.                                                       |
| `run_calculation`             | Calculate with the selected settings; a failed calculation keeps the preceding result.                                               |
| `solve_multiconductor_pf`     | Solve the displayed distribution case's multiconductor AC power flow.                                                                |
| `create_study`                | Save the current case and its results as a Study.                                                                                    |
| `inspect_study`               | Read a page of a Study's goals, states, or evidence, or open a saved Study.                                                          |
| `revise_study_goal`           | Save a new goal revision.                                                                                                            |
| `branch_study`                | Continue exploring from a saved state.                                                                                               |
| `compare_study_states`        | Compare two saved operating points, optionally under a goal.                                                                         |
| `propose_study`               | Search for verified capacity or demand improvements and save the best one unapplied.                                                 |
| `edit_demand`                 | Add bounded demand increments to a saved state.                                                                                      |
| `restore_base_case`           | Solve the original network data and save it as a reset candidate.                                                                    |
| `record_study_evidence`       | Attach a stated rationale and evidence to a saved state.                                                                             |
| `propose_capacity_plan`       | Use implicit derivatives to choose bounded capacity increase trials, verify them with exact solves, and stage an unapplied proposal. |
| `apply_capacity_plan`         | Apply an approved proposal after checking its session, case, and revision.                                                           |
| `inspect_webmcp_registration` | Developer check: registered tools, the last registration error, exposed origins, and the browser's WebMCP features.                  |

The general tools register for the page lifecycle. `list_cases` and `select_case`
also cover local files and distribution cases. Distribution queries return connected
load and equipment data; they do not claim computed LMPs or flows. Inspection and
queries use the saved state when one is displayed. Live edit tools require
returning to the live case through `select_case`.
`propose_capacity_plan` registers when the active formulation supports it.
`apply_capacity_plan` registers only while a staged proposal matches the current
state.

`export_case` downloads the file to the user's computer and returns its name,
size, and PowerIO's findings about what the target format cannot represent; the
file text does not go to the agent. `import_case` reads text formats by
extension (`.m`, `.raw`, `.rawx`, `.aux`, `.epc`, `.dss`, `.json`, `.xml`,
`.xiidm`, `.uct`, `.ucte`, `.cdf`) up to 4,000,000 characters. Binary
formats, archives, and geographic sidecars still need the file picker. A case
without coordinates opens as a diagram, or waits for placement, exactly as a
dropped file does; the result reports that as `availability`.

## Tool contract

Tellegen follows the WebMCP draft of November 2026: tools register on
`document.modelContext` with `registerTool()` and an `AbortSignal` that
unregisters them.

Every tool declares an `inputSchema` and an `outputSchema`. Each response is
JSON in one envelope, `{ "ok": true, "data": … }` or
`{ "ok": false, "error": { "code": …, "message": … } }`, and the output
schema states that envelope and the fields each tool's `data` reports. The
data fields are typed but not required, because a response that exceeds the
output budget is trimmed: a committed change whose details do not fit reports
`completed` and `output_truncated` instead. An agent that rejects responses
that fail their schema never rejects one of tellegen's. The browser tests
check the responses their tool calls receive against these schemas.

Annotations: read-only tools set `readOnlyHint`. Every tool except
`inspect_webmcp_registration` sets `untrustedContentHint`, because case names
and identifiers come from imported files. No tool sets `consequentialHint`:
every change stays in the tab and can be reset, and a capacity proposal
applies only after the user approves it in the interface.
`inspect_webmcp_registration` sets `debugging`, so agents for end users can
leave it out.

## Enabling WebMCP

Chrome 149 and Edge 150 run WebMCP origin trials; for local development,
enable `chrome://flags/#enable-webmcp-testing`. A deployment that registers
for the trial sets `PUBLIC_WEBMCP_ORIGIN_TRIAL_TOKEN` at build time, and the
prerendered pages carry the token in an `origin-trial` meta tag.

The tools are same-origin by default. To let a page that embeds tellegen call
them, list its origins, comma separated, in `PUBLIC_WEBMCP_EXPOSED_TO`; the
embedding page must also opt in from its side. A browser that refuses an
origin rejects the registration with a `SecurityError`, which the **Agent**
panel reports.

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
boundaries. A mutation that has passed its commit point finishes even if the
caller cancels; the agent then receives no result, so the activity panel
marks the call as cancelled after completion. The browser reports dynamic
registration failures, naming the tool and the browser's reason, instead of
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

`@tellegen/webmcp` owns tool descriptions, input and output JSON Schemas,
runtime validation, annotations, bounded result shaping, and registration
lifecycle. It depends on
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
Playwright uses a test host that follows the WebMCP registration and
execution algorithms: unique names, schemas serialized at registration, and
JSON text in both directions. Before release, invoke
the tools through native WebMCP in the in-app browser as a separate bridge
check.

## References

- [WebMCP specification](https://webmachinelearning.github.io/webmcp/)
  ([repository](https://github.com/webmachinelearning/webmcp))
- [Chrome WebMCP documentation](https://developer.chrome.com/docs/ai/webmcp)
- [Imperative API](https://developer.chrome.com/docs/ai/webmcp/imperative-api)
- [Tool security](https://developer.chrome.com/docs/ai/webmcp/secure-tools)
- [WebMCP evaluations](https://developer.chrome.com/docs/ai/webmcp/evals)
