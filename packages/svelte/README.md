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

## Engine Reexports

The package reexports the browser engine helpers, including `ingestCase`,
`classifyJson`, and `ingestJsonDrop`, for custom drop surfaces. In 0.2 these
APIs use byte input: pass `Uint8Array` to `ingestCase`, await
`classifyJson(bytes)` and read its `{ kind, format }` result, and use
`ingestJsonDrop(bytes)` to classify and parse in one call. `isStudyPackageText`
was removed; test for `kind === "balanced-package"` instead. `JsonDropKind` now
includes `transmission`, `distribution`, `ambiguous`, and `unknown`; BMOPF and
PMD are distribution `format` values rather than separate kinds.

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
