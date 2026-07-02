# Framework Quickstart

There are two integration references:

- [`examples/svelte-minimal`](https://github.com/eigenergy/tellegen/tree/main/examples/svelte-minimal) imports `@tellegen/svelte` and renders the full viewer for local files only.
- [`examples/browser-minimal`](https://github.com/eigenergy/tellegen/tree/main/examples/browser-minimal) imports `@tellegen/engine` directly and builds its own simple UI.

## Install

For a Svelte app:

```sh
npm install @tellegen/svelte
```

For a custom UI:

```sh
npm install @tellegen/engine
```

For local development in this repository:

```sh
npm ci
npm run wasm
npm run build:engine
npm run build:svelte
npm --workspace @tellegen/example-svelte-minimal run dev
```

The engine package resolves wasm assets relative to its packaged modules with
`new URL(..., import.meta.url)`. Vite and SvelteKit handle that path for the
Svelte package and for custom engine consumers.

## Svelte Viewer

```svelte
<script lang="ts">
  import { TellegenViewer } from "@tellegen/svelte";
  import "@tellegen/svelte/styles.css";
</script>

<TellegenViewer />
```

For local files only:

```svelte
<script lang="ts">
  import { TellegenViewer } from "@tellegen/svelte";
  import "@tellegen/svelte/styles.css";
</script>

<TellegenViewer loadDefaultCases={false} showFooter={false} />
```

Run the local example:

```sh
npm --workspace @tellegen/example-svelte-minimal run dev
```

## Engine Flow

Run the engine example:

```sh
npm --workspace @tellegen/example-browser-minimal run dev
```

```ts
import { createStudy, formatOf, ingestCase } from "@tellegen/engine";

const format = formatOf("case14.m");
if (!format) throw new Error("unsupported case format");

const parsed = await ingestCase(caseText, format);
const study = await createStudy(parsed.network_json, "dcopf");

try {
  const preview = study.preview({ 3: 25 });
  const committed = study.commit(parsed.name, { 3: 25 }, {}, { bus: 3 });
  console.log(preview.objectiveDelta, committed.sensitivity);
} finally {
  study.free();
}
```

The call sequence is:

1. detect the case format;
2. parse the case in browser WebAssembly;
3. create a `Study`;
4. preview a demand edit without a solve;
5. commit the edit with a sensitivity request; and
6. free the `Study`.

## Privacy Boundary

Dropped files stay in the browser when the app uses the browser wasm path. The
parser, solve, preview, commit, and sensitivity paths all run in WebAssembly in
the page. A host app only sends data to a server if it chooses an HTTP path or
writes its own upload path.
