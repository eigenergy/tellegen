# `@tellegen/webmcp`

WebMCP tools for interactive Tellegen cases that do not depend on a UI
framework. The package owns tool descriptors, JSON Schemas, runtime validation,
security annotations, bounded responses, registration lifecycle, and an
adapter contract. It has no dependency on Svelte or `@tellegen/engine`.

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

`createTellegenTools(adapter)` returns the general OPF descriptors.
`createTellegenPlanningTools(planning)` returns the proposal and application
descriptors in their dynamic registration groups. Tests and browser runners
can call every descriptor's `execute` function with or without an
`AbortSignal`.

Pass `onActivity` to `createTellegenTools` or a registration helper to observe
bounded start and finish events. Events contain the tool identity and response.
Request recording is opt-in through `recordValidatedInput: true`; only inputs
that pass validation are copied into completed events.

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

The adapter exposes case, network, sensitivity, and edit operations. Hosts keep
PowerIO v0.11 generation-2 `pio-ir` modules, typed state selection, and
calculation instances behind that interface.

See the [WebMCP guide](https://eigenergy.github.io/tellegen/webmcp.html) for the
tool contract, security model, and test workflow.
