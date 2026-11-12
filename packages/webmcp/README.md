# `@tellegen/webmcp`

WebMCP tools for interactive Tellegen cases that do not depend on a UI
framework. The package owns tool descriptors, JSON Schemas, runtime validation,
security annotations, bounded responses, registration lifecycle, and an
adapter interface. It has no dependency on Svelte or `@tellegen/engine`.

```sh
npm install @tellegen/webmcp
```

Provide a `TellegenWebMcpAdapter`, then register it in a browser component:

```ts
import { registerDocumentTellegenWebMcp } from "@tellegen/webmcp";

const lifecycle = new AbortController();
const registration = await registerDocumentTellegenWebMcp(document, adapter, {
  signal: lifecycle.signal,
});

// Unregister on route or component teardown.
lifecycle.abort();
registration.dispose();
```

`createTellegenTools(adapter)` returns the general OPF descriptors. Hosts that
provide `adapter.cases` also register `list_cases` and `select_case`. Listing
is paginated with `offset`, `limit`, and `next_offset`; selection takes a
`case_id` and an optional `expected_revision` for the displayed view. Case
selection changes the view without applying a proposal or deleting saved work.

Inspection and queries describe the displayed network, including saved Study
states. Results identify `case_id`, `study_id`, `state_id`, `formulation`, and
units. A saved state's view revision follows its immutable state identity;
`study_revision` separately identifies the document revision. Live editing
requires returning to the live case through `select_case`.
`createTellegenPlanningTools(planning)` returns the proposal and application
descriptors in their dynamic registration groups. Tests and browser runners
can call every descriptor's `execute` function with or without an
`AbortSignal`.

Pass `onActivity` to `createTellegenTools` or a registration helper to observe
bounded start and finish events. Events contain the tool identity and response.
Request recording is opt-in through `recordValidatedInput: true`; only inputs
that pass validation are copied into completed events.
Requests larger than 16,384 JSON characters are omitted from the record;
execution is unchanged.

Use `ExperimentJournal` to retain those events and compare a preview with an
exact update of the same case, revision, formulation, and edits:

```ts
import {
  ExperimentJournal,
  registerDocumentTellegenWebMcp,
} from "@tellegen/webmcp";

const journal = new ExperimentJournal(crypto.randomUUID());
const registration = await registerDocumentTellegenWebMcp(document, adapter, {
  recordValidatedInput: true,
  onActivity: (event) => journal.record(event),
});
const savedJournal = journal.export(); // call when the user chooses to save
```

The journal keeps up to 100 completed calls by default and reports discarded
records in its export. Its data cannot authorize or execute edits.

Every descriptor carries the WebMCP `outputSchema` of the response it
resolves with: `{ ok: true, data }` or `{ ok: false, error: { code, message } }`.
Each tool's `data` schema names the fields it reports without requiring them,
so a response trimmed to the output budget still validates. `schemaErrors`
checks a value against a schema; use it in a host's tests to confirm the
adapter's results match:

```ts
import { schemaErrors } from "@tellegen/webmcp";

const response = await tool.execute(input);
expect(schemaErrors(tool.outputSchema, response)).toEqual([]);
```

Hosts that provide `adapter.files` also register `export_case`, which writes
the displayed case through PowerIO to one of `files.exportFormats` and
downloads it for the user, and `import_case`, which opens case text the way a
dropped file opens. Both return PowerIO's findings, not the file text.

Registration options:

- `exposedTo`: origins other than the document's own that may discover and
  call the tools, such as a page that embeds the host in an iframe. The
  embedding page must opt in too.
- `debugTool: true`: also register `inspect_webmcp_registration`, a read-only
  tool annotated `debugging` that reports the registered tools, the last
  registration error, and the optional WebMCP features the browser offers.

A rejected `registerTool()` call throws `TellegenRegistrationError`, whose
`reason` is the DOMException name the browser gave, such as
`InvalidStateError` for a duplicate tool name.

The adapter exposes case, network, sensitivity, and edit operations. Hosts keep
PowerIO v0.11 generation-2 `pio-ir` modules, typed state selection, and
calculation instances behind that interface.

See the [WebMCP guide](https://eigenergy.github.io/tellegen/webmcp.html) for the
tool definitions, security model, and test workflow.

## Citing

If you use this package in published work, please cite the tellegen paper:
Samuel Talkington, Frederik Geth, Qian Zhang, Le Xie, and Skyler Liu. 2026.
*Interactive Power Flow in the Browser*. The Inaugural ACM Conference on Digital
Transformation (ACM DXConf 2026), Ann Arbor, MI, USA.
[doi:10.1145/3847238.3849038](https://doi.org/10.1145/3847238.3849038). The
[repository README](https://github.com/eigenergy/tellegen#citing) has the BibTeX
entry.
