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
