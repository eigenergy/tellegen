# WebMCP

WebMCP lets a web page hand your AI assistant a toolbox. Tellegen's map
offers tools such as `inspect_case`, `query_network` and `update_case`, so an
agent reads and changes the case through them instead of reading the screen
and clicking. Each call shows in the **Agent** panel and in a ribbon below the
header. The tools read and update the state shown in the interface and run the
same WebAssembly solver.

WebMCP is a new browser standard, in an origin trial in Chrome and Edge.
Browsers without `document.modelContext` run the application without these
tools, and manual use works as before.

## Getting started (no setup knowledge needed) {#getting-started}

Open **Agent**, then **Connect**, on the map. The panel checks your browser,
tells you what, if anything, to change, and has copy buttons for every command
and prompt on this page.

### 1. Use a browser that speaks WebMCP

| What the Connect tab says   | What to do                                                                                                                                                                                                                                                    |
| --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Ready for agents            | Nothing. Go to step 2.                                                                                                                                                                                                                                        |
| One switch away             | Paste `chrome://flags/#enable-webmcp-testing` (`edge://flags/...` in Edge) into the address bar, set it to Enabled and relaunch. Chrome 149 and later need no flag on https://tellegen.dev, which carries an origin-trial token; local and preview copies do. |
| Almost there                | Update Chrome or Edge to version 149 or later.                                                                                                                                                                                                                |
| … doesn't speak WebMCP yet  | Firefox and Safari have no WebMCP. Use Chrome or Edge on a computer, or an agent that brings its own browser (step 2).                                                                                                                                        |
| Agents work from a computer | The agents that use page tools run on desktop.                                                                                                                                                                                                                |

The origin-trial token covers https://tellegen.dev only, not other hosts.

### 2. Connect an agent

Pick one. Only the last option touches remote debugging, and you don't need it.

**ChatGPT desktop app (no setup).** Open https://tellegen.dev in the app's
built-in browser and ask for what you want. The agent finds the tools itself;
the **Site tools** button in the address bar lists them. This works in the
desktop app's browser only, not in the Codex CLI, the IDE extension or the
Chrome connector.

**Claude Code with Claude in Chrome.** Install the
[Claude in Chrome](https://code.claude.com/docs/en/chrome) extension, sign in
to Claude Code with `/login`, and start it with:

```sh
claude --chrome
```

Claude does not look for WebMCP tools on its own yet, so tell it where they
are. The Connect tab's **Copy prompt for Claude** button copies a prompt that
starts:

> Open https://tellegen.dev in Chrome. The page offers WebMCP tools: list them
> by running `await document.modelContext.getTools()` in the page, and call
> them with `document.modelContext.executeTool()` instead of clicking.

This uses your own Chrome, so the origin-trial token applies and no flag is
needed.

**Claude Code, Codex CLI, Cursor or another MCP client with Chrome DevTools
MCP.** Add Google's
[Chrome DevTools MCP](https://github.com/ChromeDevTools/chrome-devtools-mcp)
server once, with its experimental WebMCP tools turned on:

```sh
# Claude Code
claude mcp add chrome-devtools --scope user -- npx chrome-devtools-mcp@latest --categoryExperimentalWebmcp=true --chromeArg=--enable-features=WebMCP
# Codex CLI
codex mcp add chrome-devtools -- npx chrome-devtools-mcp@latest --categoryExperimentalWebmcp=true --chromeArg=--enable-features=WebMCP
```

Then ask the agent to open https://tellegen.dev and list its WebMCP tools. It
gets `list_webmcp_tools` and `execute_webmcp_tool`. The server starts its own
Chrome window with a separate profile, which is why the command passes
`--enable-features=WebMCP`: flags you set at `chrome://flags` in your everyday
profile do not carry over.

**Agents built into the browser.** Gemini in Chrome and Copilot in Edge do not
call page tools yet.

### 3. Ask

With the tools ready, the Connect tab shows a prompt that explains the tools
and a few requests to try, each with a copy button. For example: "Which lines
are congested right now, and which buses pay the highest prices?"

### Questions

**Do I need to turn on remote debugging?** No. Remote debugging
(`chrome://inspect/#remote-debugging`) is needed only to attach Chrome
DevTools MCP to the Chrome you already have open, with its `--autoConnect` or
`--browserUrl` options. The default DevTools MCP setup above starts its own
window instead. If you do attach, that Chrome needs WebMCP itself (version 149
or later on tellegen.dev, or the flag), because `--chromeArg` applies only to
a browser the server starts.

**Do I need to add an MCP server?** Only for the Chrome DevTools MCP route,
with the one command above. The ChatGPT app and `claude --chrome` need none.
To work on case files without a browser, Tellegen has a separate stdio server;
see [MCP server](mcp.md).

**How do I know it's working?** The Connect tab says **Ready for agents** and
counts the tools this tab offers. After the first call it says **Agent
connected**, and the call appears under **Activity**.

The first-visit introduction at the bottom of the map opens the Connect tab
and links to the application changelog.

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
leave it out. The web app registers it only in a development build or when the
page URL has `?debug`.

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
