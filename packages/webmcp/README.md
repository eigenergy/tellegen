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
bounded start and finish events. Events contain the tool identity and response,
while omitting raw input. A host can use them for visible agent activity and
bounded result summaries without changing tool behavior.

The adapter exposes case, network, sensitivity, and edit operations. Hosts keep
PowerIO v0.11 generation-2 `pio-ir` modules, typed state selection, and
calculation instances behind that interface.

See the [WebMCP guide](https://eigenergy.github.io/tellegen/webmcp.html) for the
tool contract, security model, and test workflow.
