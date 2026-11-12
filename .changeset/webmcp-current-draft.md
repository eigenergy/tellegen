---
"@tellegen/webmcp": minor
"@tellegen/svelte": minor
---

The WebMCP tools follow the current WebMCP draft and can now open and export cases.

Every tool declares an `outputSchema`: the `{ ok, data }` or `{ ok, error }` envelope it resolves with, with the fields each tool reports. `schemaErrors` checks a response against it, so a host can test that its adapter matches. The `ToolAnnotations` type gains `consequentialHint` and `debugging`; no tellegen tool is consequential, because every change stays in the browser tab and can be reset.

Two new tools sit on an optional `files` adapter capability, which `@tellegen/svelte` provides. `export_case` writes the displayed case through PowerIO (MATPOWER, PSS/E, PowerWorld, PSLF, PowerModels, pandapower, or a `.pio.json` module), downloads it for the user, and returns the file name, size and PowerIO's conversion findings. `import_case` opens case text the way a dropped file opens and returns the new case IDs and PowerIO's parse findings.

Registration takes an `exposedTo` list of origins allowed to call the tools from an embedding page, and an opt-in `debugTool` that registers `inspect_webmcp_registration`, a read-only tool annotated `debugging`. A rejected registration now throws `TellegenRegistrationError`, which names the tool and the DOMException the browser gave (`InvalidStateError` for a duplicate name, `NotAllowedError`, `SecurityError`, `TypeError`), so its message changed. A finished activity event sets `callerCancelled` when the agent cancelled a call that still completed. A `TellegenToolDefinition` now carries `outputSchema`, so code that builds tool definitions by hand must add one.
