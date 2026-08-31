# `@tellegen/webmcp`

Framework-independent WebMCP tools for interactive tellegen cases. The package
owns tool descriptors, JSON Schemas, runtime validation, security annotations,
bounded responses, registration lifecycle, and an adapter contract. It has no
dependency on Svelte or `@tellegen/engine`.

```sh
npm install @tellegen/webmcp
```

Provide a `TellegenWebMcpAdapter`, then register it in a browser component:

```ts
import { registerDocumentTellegenWebMcp } from '@tellegen/webmcp';

const lifecycle = new AbortController();
const registration = await registerDocumentTellegenWebMcp(document, adapter, {
  signal: lifecycle.signal
});

// Unregister on route or component teardown.
lifecycle.abort();
registration.dispose();
```

`createTellegenTools(adapter)` returns the same seven plain descriptors without
registering them. Tests and headless browser runners can call their `execute`
functions directly with an `AbortSignal`.

Pass `onActivity` to `createTellegenTools` or a registration helper to observe
bounded start and finish events. Events contain the tool identity and response,
while omitting raw input. A host can use them for visible agent activity,
auditing, and exact before/after comparisons without changing tool behavior.

The adapter uses case, network, sensitivity, and edit operations rather than
the current engine `Study` class. This is the compatibility seam for PowerIO
1.0 `PioModule` values, typed state selection, and calculation instances.

See the [WebMCP guide](https://eigenergy.github.io/tellegen/webmcp.html) for the
tool contract, security model, and test workflow.
