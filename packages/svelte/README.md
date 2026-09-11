# @tellegen/svelte

Svelte components for tellegen maps, case panels, local case files, and browser solves.

```sh
npm install @tellegen/svelte
```

Import the component and stylesheet in your app:

```svelte
<script lang="ts">
  import { TellegenViewer } from "@tellegen/svelte";
  import "@tellegen/svelte/styles.css";
</script>

<TellegenViewer />
```

Use local files only by disabling bundled case loading:

```svelte
<script lang="ts">
  import { TellegenViewer } from "@tellegen/svelte";
  import "@tellegen/svelte/styles.css";
</script>

<TellegenViewer loadDefaultCases={false} showFooter={false} />
```

The viewer accepts at most 32 files in one ingestion batch. Each file must be
at most 128 MiB, and the aggregate batch must also be at most 128 MiB. Files
remain in the browser on this path.

`TellegenViewer` accepts:

- `apiBase`, default `/api`
- `loadDefaultCases`, default `true`
- `docsHref`
- `orgHref`
- `orgLabel`
- `showFooter`, default `true`

Use `TellegenProvider` and `TellegenShell` when state should survive route changes:

```svelte
<script lang="ts">
  import { TellegenProvider, TellegenShell } from "@tellegen/svelte";
  import "@tellegen/svelte/styles.css";
</script>

<TellegenProvider>
  <TellegenShell />
</TellegenProvider>
```

## Panel layout

The shell docks Network and Studies on the left, and Solver and Agent on the
right. The most recently opened panel gets more room in its dock; click another
heading to expand it. Drag a panel heading to detach it; use the corner handle to resize it.
Panel options dock it on either side, and **Reset layout** restores the default
positions. Focus a heading or resize handle and use arrow keys for keyboard
control; hold Shift for larger steps. Narrow windows show one active drawer.
Panel positions are stored in the browser, independently of case and Study data.

Custom panels can join the same layout inside `TellegenProvider`:

```svelte
<TellegenProvider>
  <TellegenShell />
  <PanelFrame id="notes" title="Notes" side="right" order={30}>
    <p>Case notes</p>
  </PanelFrame>
</TellegenProvider>
```

Import `PanelFrame` from `@tellegen/svelte`. Its optional `open` prop is bindable;
`width` sets its preferred width, and the `headerActions` snippet adds compact
controls beside its title.

## Engine Reexports

The package reexports the browser engine helpers, including `ingestCase`,
`classifyJson`, and `ingestJsonDrop`, for custom file imports. These APIs use
byte input: pass `Uint8Array` to `ingestCase`, await
`classifyJson(bytes)` and read its `{ kind, format }` result, and use
`ingestJsonDrop(bytes)` to classify and parse in one call. `isStudyPackageText`
was removed; stored PowerIO documents report `kind === "module"`.
`JsonDropKind` also includes `transmission`, `distribution`, `ambiguous`, and
`unknown`; BMOPF and PMD are distribution `format` values rather than separate
kinds. Every solvable ingest payload carries the retained module in
`module_json`; the viewer uses that generation-2 IR for Study construction and
geographic edits without keeping a second serialized network.

Drawing layers use a plain canvas with their own x/y coordinates. Saved Studies
retain the drawing center and scale separately from the geographic map camera.

## Release

Build and inspect the package from the repository root:

```sh
npm ci
npm run wasm
npm run build:engine
npm run build:svelte
npm run pack:svelte
npm run test:svelte-packed
```

`@tellegen/svelte` is published with `@tellegen/engine` in the first framework
release. The package ships only `dist`, the README, the MIT license text, and
package metadata. The packed smoke test installs the generated tarballs into a
temporary Svelte consumer and builds it so missing exports, styles, or wasm
assets fail before publish.
