# @tellegen/webmcp

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
