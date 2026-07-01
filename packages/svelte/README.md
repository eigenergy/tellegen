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
