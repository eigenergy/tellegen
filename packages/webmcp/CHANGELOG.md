# @tellegen/webmcp

## 0.4.2

### Patch Changes

- [#436](https://github.com/eigenergy/tellegen/pull/436) [`16b60f3`](https://github.com/eigenergy/tellegen/commit/16b60f32e23aa32f03cc2729bfca0f9316e463f9) Thanks [@samtalki](https://github.com/samtalki)! - Decide an infeasible solve from the engine's error code and a permanent wasm load failure from the error type, reading the message text only when those are missing. Reading a Study's `bundle` or the journal's `records` copies less, and `@tellegen/svelte` exports the `plain` and `signed` number formatters.

## 0.4.1

### Patch Changes

- [#402](https://github.com/eigenergy/tellegen/pull/402) [`8c867c4`](https://github.com/eigenergy/tellegen/commit/8c867c45b1e38f901eec1a8d8d47f04a060abe29) Thanks [@samtalki](https://github.com/samtalki)! - `TELLEGEN_ERROR_CODES` lists every error code a tool can return, with when it happens and what to do next, and `TellegenErrorCode` is its type.

## 0.4.0

### Minor Changes

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`81ed298`](https://github.com/eigenergy/tellegen/commit/81ed2984bb313a90fd9275d2c0be81dec1da72a6) Thanks [@samtalki](https://github.com/samtalki)! - A host can keep `simulate_dynamics` behind a setting: with `dynamicsAvailable()` on the adapter, the tool is registered only while it returns true.

- [#397](https://github.com/eigenergy/tellegen/pull/397) [`a7e6f57`](https://github.com/eigenergy/tellegen/commit/a7e6f57d28df48a39119f51ec1ee031aeceac352) Thanks [@samtalki](https://github.com/samtalki)! - Add the `simulate_dynamics` tool, which runs an RMS simulation of the displayed case after up to 16 events and returns its verdict and key numbers. `import_case` accepts `.dyr` text for the displayed case.

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`a508e37`](https://github.com/eigenergy/tellegen/commit/a508e371985e0c98cedbf25cc7fbda30a8b7d876) Thanks [@samtalki](https://github.com/samtalki)! - New `onToolsChange` registration option reports the tool names whenever a dynamics or capacity planning tool comes or goes, so a host's tool count stays current.

### Patch Changes

- [#400](https://github.com/eigenergy/tellegen/pull/400) [`2bcc94d`](https://github.com/eigenergy/tellegen/commit/2bcc94dd25bae08764bd98caeb80e6c1bc97d347) Thanks [@samtalki](https://github.com/samtalki)! - Results show as labelled numbers, such as Operational cost and Lines at limit, and Settings is a list of short rows grouped by topic that apply and rerun at once. New export: `StatBoxes`.

- [#376](https://github.com/eigenergy/tellegen/pull/376) [`aafc65f`](https://github.com/eigenergy/tellegen/commit/aafc65f4e0c3fbe1d2c10d2a75a92276ae3fc6fb) Thanks [@samtalki](https://github.com/samtalki)! - The engine reads and writes cases with PowerIO 0.12.0, so saved files carry version 0.12.0 and PowerIO 0.11 readers refuse them; files from earlier releases still open. An unrated branch is now saved as an infinite rating, and the multiconductor solvers follow PowerIO 0.12 for inverter power factor signs and type-A regulator taps.

## 0.3.0

### Minor Changes

- [#375](https://github.com/eigenergy/tellegen/pull/375) [`999d23e`](https://github.com/eigenergy/tellegen/commit/999d23ee53a791d3aba930ff6456ab8399d4a876) Thanks [@samtalki](https://github.com/samtalki)! - The WebMCP tools follow the current WebMCP draft and can now open and export cases.

  Every tool declares an `outputSchema`: the `{ ok, data }` or `{ ok, error }` envelope it resolves with, with the fields each tool reports. `schemaErrors` checks a response against it, so a host can test that its adapter matches. The `ToolAnnotations` type gains `consequentialHint` and `debugging`; no tellegen tool is consequential, because every change stays in the browser tab and can be reset.

  Two new tools sit on an optional `files` adapter capability, which `@tellegen/svelte` provides. `export_case` writes the displayed case through PowerIO (MATPOWER, PSS/E, PowerWorld, PSLF, PowerModels, pandapower, or a `.pio.json` module), downloads it for the user, and returns the file name, size and PowerIO's conversion findings. `import_case` opens case text the way a dropped file opens and returns the new case IDs and PowerIO's parse findings.

  Registration takes an `exposedTo` list of origins allowed to call the tools from an embedding page, and an opt-in `debugTool` that registers `inspect_webmcp_registration`, a read-only tool annotated `debugging`. A rejected registration now throws `TellegenRegistrationError`, which names the tool and the DOMException the browser gave (`InvalidStateError` for a duplicate name, `NotAllowedError`, `SecurityError`, `TypeError`), so its message changed. A finished activity event sets `callerCancelled` when the agent cancelled a call that still completed. A `TellegenToolDefinition` now carries `outputSchema`, so code that builds tool definitions by hand must add one.

### Patch Changes

- [#378](https://github.com/eigenergy/tellegen/pull/378) [`bfb463b`](https://github.com/eigenergy/tellegen/commit/bfb463b60a089048a51dcfeb80034ef18c28458e) Thanks [@samtalki](https://github.com/samtalki)! - Activity events for the settings tools now carry each tool's own title (for example "Run selected calculation" and "Inspect settings") instead of a lowercase fragment such as "run settings".

## 0.2.0

### Minor Changes

- [#341](https://github.com/eigenergy/tellegen/pull/341) [`4ed2a0b`](https://github.com/eigenergy/tellegen/commit/4ed2a0b2282551535bcc5dee4d1b9cf992ffb351) Thanks [@samtalki](https://github.com/samtalki)! - Add a common Settings panel and a Rust-owned calculation settings contract across TypeScript, WebMCP, native, CLI and HTTP interfaces. Applying settings stages the next explicit solve without replacing the accepted result. Retained balanced Studies keep their effective options and active constraint selections. Saved PowerIO modules record the calculation and only the options that differ from the engine defaults; reopening a file written by another engine version drops options this engine does not know and logs a warning instead of refusing the file. The generated settings types name calculations with the `Calculation` union. Browser display preferences persist separately, and API mutations check case and settings revisions. `tellegen solve-mc` now resolves its options through the same contract, so an unknown option key is an error rather than being silently ignored.

- [#341](https://github.com/eigenergy/tellegen/pull/341) [`425fb53`](https://github.com/eigenergy/tellegen/commit/425fb5345735eded7ef179bb1dbe45482591a5b5) Thanks [@samtalki](https://github.com/samtalki)! - Recognize multiconductor entries in the hosted case catalogue and load their
  portable PowerIO modules into the distribution viewer and browser AC power-flow
  workflow, with lazy loading, browser-local removal, and restore support.

  Hosted cases can supply source/association metadata and PF defaults. Preserve
  those defaults across UI/WebMCP solves and saved-study reopen; compact
  multiconductor exports keep large studies within the replay input limit.
  The public demo lists the Texas7k distribution case once an operator stages
  its bundle; deploys do not require it.

  Cancel hosted requests on controller disposal, discard late ingest completions,
  and report missing backends without starting a request.

  Keep hosted inputs parsed in the engine and initialize the shared solver settings
  from catalogue defaults, including saved-study replay. Free retained inputs when
  hosted requests are superseded or the catalogue is reloaded.

## 0.1.0

Initial release with typed WebMCP tools for network inspection, sensitivity
analysis, exact edits, capacity planning and persistent Studies. Includes
runtime request validation, revision checks, cancellation, proposal approval
and an exportable activity timeline. Hosts supply their solver and storage
adapters; the package has no UI framework dependency.
