# WebMCP

WebMCP is a browser feature that lets a web page offer tools to an AI agent.
Tellegen's page offers tools such as `inspect_case`, `query_network` and
`update_case`, so an agent reads and changes the case you see by calling them,
instead of reading the screen and clicking. Every call shows in the **Agent**
panel and runs the same solver in your browser as the interface does.

There is no server to add and nothing to install in Tellegen. You need two
things: a browser that has WebMCP, and an agent that can use that browser.

## Getting started {#getting-started}

### 1. Open Tellegen in a browser that has WebMCP

Open <https://tellegen.dev> in Chrome 149 or later, or in Brave, on a computer.
Then open **Agent** and its **Connect** tab.

<!-- webmcp:status:start -->
| The Connect tab says | What to do |
| --- | --- |
| Loading… | Wait. It changes within a few seconds. |
| Connected | The tools are registered. Press **Copy prompt** and go to step 2. |
| Off in this browser | The browser has WebMCP but this page may not use it. Copy the address shown (`chrome://flags/#enable-webmcp-testing`), paste it into the address bar, set it to **Enabled** and relaunch. |
| Needs Chrome 149+ (or Edge 150+) | Update the browser to the version named. |
| Not available in this browser | Firefox, Safari and phones have no WebMCP. Use Chrome, Edge or Brave on a computer. |
| Tools failed to load | Press **Reload**. **Details** shows the error; report it if it comes back. |
<!-- webmcp:status:end -->

On tellegen.dev, Chrome and Brave need no setting: the site takes part in
Chrome's WebMCP trial, which runs until 30 March 2027. Edge has its own trial
that tellegen.dev has not joined, so Edge 150 or later needs the flag. Any
other address, such as a copy you run yourself, needs the flag in every
browser.

When the tab says **Connected** it also shows how many tools the page offers.
Two tools appear only in some states, and the tab lists the reason when one is
missing: `simulate_dynamics` needs Dynamics turned on in Settings, and
`propose_capacity_plan` needs a solved DC OPF on the live case.

### 2. Connect an agent

An agent can call the tools only if it controls a browser tab with Tellegen
open. Three ways to do that:

**ChatGPT desktop app.** Open <https://tellegen.dev> in the app's built-in
browser and ask. It finds the tools by itself; the **Site tools** button in
the address bar lists them. If the button is missing, check **Settings >
Browser > Permissions > Enable site tools**. This does not work in the Codex
CLI, the IDE extension or the Chrome connector.

**Claude Code with the Claude in Chrome extension.** Install
[Claude in Chrome](https://code.claude.com/docs/en/chrome), sign it in to the
same account as Claude Code, and start Claude Code with `claude --chrome`. It
needs a Claude plan signed in with `/login`, not an API key. Claude does not
look for page tools by itself, so press **Copy prompt** in the Connect tab and
paste it: the prompt tells Claude how to list and call the tools.

**Any MCP client with Chrome DevTools MCP** (Claude Code, Codex CLI, Cursor).
Add Google's
[Chrome DevTools MCP](https://github.com/ChromeDevTools/chrome-devtools-mcp)
server with its WebMCP tools turned on:

```sh
# Claude Code
claude mcp add chrome-devtools --scope user -- npx chrome-devtools-mcp@latest --categoryExperimentalWebmcp=true --chromeArg=--enable-features=WebMCP
# Codex CLI
codex mcp add chrome-devtools -- npx chrome-devtools-mcp@latest --categoryExperimentalWebmcp=true --chromeArg=--enable-features=WebMCP
```

It needs Node.js and Chrome 150 or later. The server starts its own browser
window with its own profile; `--enable-features=WebMCP` turns WebMCP on in
that window for every address, so this route also works for a local copy.
To use Brave or another Chromium browser instead of Chrome, add
`--executablePath` with the path to it. Ask the agent to open
<https://tellegen.dev> and list its WebMCP tools.

Gemini in Chrome and Copilot in Edge do not call page tools yet.

### 3. Ask

- Which lines are congested right now, and which buses pay the highest prices?
- If demand at the most expensive bus rises by 50 MW, how do prices and the
  total cost change? Preview it before applying anything.
- Propose the cheapest line capacity upgrade that relieves congestion, and wait
  for my approval before applying it.

Each call appears under **Activity** in the Agent panel, with its input and
result.

### Check it without an agent

The tools are ordinary page objects, so you can try them from the browser's
developer console on the Tellegen tab:

```js
const tools = await document.modelContext.getTools();
tools.map((tool) => tool.name);
const inspect = tools.find((tool) => tool.name === "inspect_case");
JSON.parse(await document.modelContext.executeTool(inspect, {}));
```

`executeTool` takes the tool object from `getTools()` and a plain object of
arguments. If `document.modelContext` is `undefined`, the browser has not
turned WebMCP on for this page; see the table in step 1.

## Running it locally {#local}

It needs Rust, Node.js, Python 3.11 or later and
[just](https://github.com/casey/just).

```sh
just dev-setup   # once: install, build the wasm modules and the packages
just dev         # the case server on :8000 and the app on http://localhost:5173
```

A local address is not covered by the trial, so turn WebMCP on in the browser:
open `chrome://flags/#enable-webmcp-testing` (the Connect tab shows the address
to copy), choose **Enabled** and relaunch. Starting the browser with
`--enable-features=WebMCP` does the same for one run. A development build also
registers `inspect_webmcp_registration`, which reports the registered tools and
the last registration error.

## Tools

| Tool                          | Behavior                                                                                                                             |
| ----------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| `list_cases`                  | List configured and imported cases, availability, and stable case IDs.                                                               |
| `select_case`                 | Switch the displayed case without deleting saved Studies or applying proposals.                                                      |
| `import_case`                 | Open case file text as a new case, the way a dropped file opens, and report PowerIO's parse findings.                                |
| `export_case`                 | Write the displayed case through PowerIO to an exchange format or a `.pio.json` module, download it, and report conversion findings. |
| `inspect_case`                | Read the displayed case or saved state, formulation, solve summary, IDs, units, and revision.                                        |
| `query_network`               | Return a bounded set of buses or branches by stable identity or solved metric.                                                       |
| `analyze_sensitivity`         | Compute an LMP sensitivity column for the displayed state under its formulation (not AC power flow).                                 |
| `focus_network`               | Select a bus or branch in the visible interface.                                                                                     |
| `preview_case_update`         | Predict demand or rating edits from cloned state.                                                                                    |
| `update_case`                 | Solve and commit a bounded batch of demand or rating edits.                                                                          |
| `reset_case`                  | Solve and commit the base state with the current formulation.                                                                        |
| `inspect_settings`            | Read the selected and accepted calculation settings, or this browser's display preferences.                                          |
| `update_settings`             | Validate and apply a settings patch atomically without solving.                                                                      |
| `reset_settings`              | Return the selected calculation's settings to their defaults without solving.                                                        |
| `run_calculation`             | Calculate with the selected settings; a failed calculation keeps the preceding result.                                               |
| `solve_multiconductor_pf`     | Solve the displayed distribution case's multiconductor AC power flow.                                                                |
| `simulate_dynamics`           | Run an RMS simulation of the displayed case with its `.dyr` data after events and return the screening verdict and figures.          |
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

`simulate_dynamics` takes up to 16 events (`bus_fault`, `line_fault` cleared
by opening the line, `branch_trip`, `generator_trip`, `load_change`) with
buses as numbers, branches as `from-to#circuit` (the circuit may be left out
when it is unique) and units as `bus#id`; an unknown or ambiguous id returns
`INVALID_INPUT` naming the candidates. It answers with the verdict, the COI
frequency nadir, RoCoF and final value, the largest angle separation, the
lowest voltage and its recovery, counts of islands, held units and notes,
and, with `trace_points`, up to four traces (`coi:f`, `f:<bus>`, `vm:<bus>`,
`omega:<unit>`, `dcoi:<unit>`, `pl:<bus>`) of at most 40 points. It does not
change the case; the run shows in the Dynamics panel, marked as the agent's,
unless `show_in_app` is false. `import_case` with a `.dyr` file attaches it to
the displayed case and returns what it linked.

`export_case` downloads the file to the user's computer and returns its name,
size, and PowerIO's findings about what the target format cannot represent; the
file text does not go to the agent. `import_case` reads text formats by
extension (`.m`, `.raw`, `.rawx`, `.aux`, `.epc`, `.dss`, `.json`, `.xml`,
`.xiidm`, `.uct`, `.ucte`, `.cdf`, `.dyr`) up to 4,000,000 characters. Binary
formats, archives, and geographic sidecars still need the file picker. A case
without coordinates opens as a diagram, or waits for placement, exactly as a
dropped file does; the result reports that as `availability`.

## Tool contract

Tellegen follows the WebMCP draft of November 2026: tools register on
`document.modelContext` with `registerTool()` and an `AbortSignal` that
unregisters them.

Every tool declares an `inputSchema` and an `outputSchema`. Each response is
JSON in one object, `{ "ok": true, "data": … }` or
`{ "ok": false, "error": { "code": …, "message": … } }`, and the output
schema describes that object and the fields each tool's `data` reports. The
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
leave it out. The web app registers it only in a development build or when the
page URL has `?debug`.

## Error codes {#errors}

A failed call returns `{ "ok": false, "error": { "code": "...", "message": "..." } }`.
The message says what to do for that call; the code is stable.

<!-- webmcp:errors:start -->
| Code | When | What to do next |
| --- | --- | --- |
| `INVALID_INPUT` | An argument is missing, has the wrong type or is out of range. | Fix the argument the message names and call again. |
| `INVALID_OUTPUT` | The tool produced a result that is not valid JSON for its schema. | Report it; this is a bug in the page, not in the call. |
| `OUTPUT_TOO_LARGE` | The result is longer than the output limit. | Ask for less: lower `limit` or name fewer elements. |
| `TIMEOUT` | The tool did not finish within its time limit. | Call again, or use a smaller case or a shorter simulation. |
| `CANCELLED` | The agent or the person stopped the call. | Call again only if the result is still wanted. |
| `BUSY` | Another tool call that changes the case is still running. | Wait for it to finish, then call again. |
| `TOOL_FAILED` | The tool failed for a reason no other code covers. | Read the message; call `inspect_case` to see the current state. |
| `INVALID_CONFIGURATION` | The page set up the tools with an invalid option. | Report it; an agent cannot fix this. |
| `NO_ACTIVE_CASE` | No case that can be solved is shown. | Call `list_cases`, then `select_case`. |
| `CASE_NOT_FOUND` | The case ID is not one `list_cases` returned. | Call `list_cases` and use one of its IDs. |
| `CASE_UNAVAILABLE` | The case exists but cannot be opened, for example its data failed to load. | Choose another case from `list_cases`. |
| `CASE_NOT_READY` | The case is still loading. | Wait a moment, then call `inspect_case` again. |
| `CASE_SOLVING` | A solve is still running. | Wait for it to finish, then call again. |
| `CASE_DATA_UNAVAILABLE` | The page does not hold the case data the tool needs. | Ask the person to reload the page. |
| `STALE_CASE` | The case shown changed since `case_id` was read. | Call `inspect_case` and use the new `case_id`. |
| `STALE_REVISION` | The case, its settings or the display changed since the revision was read. | Read the state again with the tool the message names and use the new revision. |
| `SAVED_STATE_READ_ONLY` | A saved Study state is shown, which cannot be edited. | Use the Study tools, or call `select_case` to return to the live case. |
| `ELEMENT_NOT_FOUND` | The bus or branch ID is not in the case. | Find a valid ID with `query_network`. |
| `EDIT_NOT_SUPPORTED` | The element cannot be edited, for example a bus that is shown but not modelled. | Choose an element that `query_network` reports as editable. |
| `EDIT_OUT_OF_RANGE` | The change is outside the limits for that element. | Choose a value inside the range the message gives. |
| `FORMULATION_UNAVAILABLE` | The calculation is not available for this case. | Call `inspect_settings` to list the calculations that are. |
| `METRIC_UNAVAILABLE` | The calculation shown does not produce the requested quantity, such as a price from a power flow. | Ask for a quantity the message lists, or run a calculation that produces it. |
| `INVALID_SORT` | `sort_by` does not apply to that kind of element. | Choose another `sort_by`. |
| `SENSITIVITY_UNAVAILABLE` | The calculation shown has no sensitivity for the requested target. | Run a calculation that has one (DC OPF for prices), or change the target. |
| `PREVIEW_UNAVAILABLE` | The calculation shown cannot predict the effect of an edit. | Apply the edit with `update_case`, which solves it exactly. |
| `DYNAMICS_OFF` | Dynamic simulation is turned off in this build. | Tell the person dynamic simulation is not available in this build. |
| `DYNAMICS_ATTACHED` | The case already has dynamic data and the import would replace it. | Ask the person, then call `import_case` with `replace_dynamics: true`. |
| `TRACE_UNAVAILABLE` | The browser cannot compute the checksum that identifies the case data. | Use a browser with Web Crypto, on an https page or localhost. |
| `IMPORT_FAILED` | The file could not be read as a case. | Check the file name's extension and the text; the message says what failed. |
| `EXPORT_UNAVAILABLE` | The case cannot be written in the requested format. | Choose a format the `export_case` schema lists for this case. |
| `PLANNING_UNAVAILABLE` | Capacity planning cannot run on this case or this branch. | Solve the case as DC OPF, and choose branches `query_network` returns. |
| `UNKNOWN_PROPOSAL` | No proposal with that ID is waiting. | Call `propose_capacity_plan` first. |
| `STALE_PROPOSAL` | The case changed after the proposal was computed. | Call `propose_capacity_plan` again. |
| `APPROVAL_REQUIRED` | The person has not approved the proposal in the Agent panel. | Ask them to review it and press Approve, then call again. |
<!-- webmcp:errors:end -->

## Enabling WebMCP

Chrome 149 and Edge 150 run separate WebMCP origin trials, each with its own
tokens; https://tellegen.dev carries a Chrome token. For local development,
enable `chrome://flags/#enable-webmcp-testing`. A deployment that registers
for the trial sets `PUBLIC_WEBMCP_ORIGIN_TRIAL_TOKEN` at build time, and the
prerendered pages carry each token in its own `origin-trial` meta tag. To cover
both browsers, put the Chrome and Edge tokens in the one setting, separated by a
comma or whitespace.

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

While an agent works, a ribbon below the header names the running tool and its
elapsed time, then shows a one-line result or the failure for a few seconds.
It appears with the Agent panel closed and in zen mode, so people watching see
each call; clicking it opens the Agent panel. `focus_network`,
`preview_case_update`, `update_case` and `analyze_sensitivity` also fly the map
to the bus or branch they work on. Only `focus_network` changes the selection;
the others move the camera, so the revision an agent holds stays valid.

The activity panel records the last 100 completed tool calls in an experiment
journal, including validated requests, bounded results, failures, and elapsed
time. **Export** downloads that record with the retained capacity
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
