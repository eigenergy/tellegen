# @tellegen/engine

## 0.3.0

### Minor Changes

- [#97](https://github.com/eigenergy/tellegen/pull/97) [`5d93591`](https://github.com/eigenergy/tellegen/commit/5d93591d74ecbba1b80fc1e0f9a9fc8e4de4cc31) Thanks [@samtalki](https://github.com/samtalki)! - Use generation-2 PowerIO IR as the sole case boundary, add weighted sensitivity
  and capacity planning, and expose the browser workflows through
  `@tellegen/webmcp`.

  Record validated WebMCP requests in a bounded activity timeline, compare
  matching previews with exact committed results, and export the browser study
  activity together with capacity planning trials.

  Add persistent, portable Studies with versioned goals, branching state history,
  structured evidence and explicit application of reviewed recommendations. Share
  Rust-generated contracts across browser, WebMCP and native CLI operations.

  Compose scalar objectives over supported prices, dispatch, flows and voltage,
  and explore bounded capacity changes or demand placement and redistribution
  using combined implicit derivatives and exact candidate solves. Save completed
  operations atomically and preserve the best verified candidate on cancellation.

  Persist cumulative bus demand edits, retain the original network base case, and
  prepare an exactly solved reset without deleting exploration history. Add
  first-visit help, agent connection instructions and a changelog in the web app.

  Separate Study goals, network states and the activity timeline into focused tabs.
  Combine agent connection help and activity in one panel, and keep map controls
  clear of the panels on desktop and mobile.

## 0.2.0

### Minor Changes

- [#60](https://github.com/eigenergy/tellegen/pull/60) [`75021aa`](https://github.com/eigenergy/tellegen/commit/75021aa821ec5a93f35ff63ca87506aea52b42cf) Thanks [@samtalki](https://github.com/samtalki)! - Add one-pass typed JSON ingestion and bound browser file-drop batches by count
  and bytes.

- [#83](https://github.com/eigenergy/tellegen/pull/83) [`670958b`](https://github.com/eigenergy/tellegen/commit/670958b95cc05c7f24fe04694c51dfc9dd907e9a) Thanks [@samtalki](https://github.com/samtalki)! - A saved study now states the powerio release that wrote it, and a study saved by
  an earlier build no longer loads. Open the source case and save the study again.
  Case uploads are parsed as bytes, so a `.raw` or `.aux` exported in CP1252 is
  refused rather than silently mangled.

- [#60](https://github.com/eigenergy/tellegen/pull/60) [`bf35bb3`](https://github.com/eigenergy/tellegen/commit/bf35bb3988a03efc43c1201ae960859c152d26b4) Thanks [@samtalki](https://github.com/samtalki)! - Parse dropped cases from bytes, preserving decoding errors and enabling
  PowerWorld `.pwb`. Add `ingestModelJson` and route `.epc` files through the PSLF
  reader.

### Patch Changes

- [#60](https://github.com/eigenergy/tellegen/pull/60) [`4d5c2a4`](https://github.com/eigenergy/tellegen/commit/4d5c2a44a7bef2e2d0bfd0c16ab6a57a8c3daebb) Thanks [@samtalki](https://github.com/samtalki)! - Retire the wasm instance after a trap instead of serving the next request from
  it, bound the string entry points at the same 128 MiB limit as their byte
  counterparts, frame a single-point selection instead of clamping to maximum
  zoom, and keep the parsing indicator up while dropped JSON cases materialize.

- [#60](https://github.com/eigenergy/tellegen/pull/60) [`bd043f1`](https://github.com/eigenergy/tellegen/commit/bd043f15de3be6d3b78c236285bca570d0f3e8e6) Thanks [@samtalki](https://github.com/samtalki)! - Include lowered three-winding rows in rendered transmission topology and mark
  display-only rows as non-editable. Report canonical and rendered analysis row
  counts separately.
