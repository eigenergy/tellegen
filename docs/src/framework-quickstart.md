# Framework Quickstart

The first integration reference is [`examples/browser-minimal`](https://github.com/eigenergy/tellegen/tree/main/examples/browser-minimal). It is a plain Vite and TypeScript app that imports `@tellegen/engine`; it does not import from `apps/web`.

## Install

For a downstream app:

```sh
npm install @tellegen/engine
```

For local development in this repository:

```sh
npm ci
npm run wasm
npm run build:engine
npm --workspace @tellegen/example-browser-minimal run dev
```

The package expects a bundler that understands wasm assets imported with `?url`. Vite and SvelteKit handle that path.

## Tiny Case Flow

```ts
import { browserWasmTransport, formatOf } from "@tellegen/engine";

const format = formatOf("case14.m");
if (!format) throw new Error("unsupported case format");

const parsed = await browserWasmTransport.ingestCase(caseText, format);
const study = await browserWasmTransport.createStudy(parsed.network_json, "dcopf");

try {
  const preview = study.preview({ 3: 25 });
  const committed = study.commit(parsed.name, { 3: 25 }, 3);
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

Dropped files stay in the browser when the browser wasm transport is used. The parser, solve, preview, commit, and sensitivity paths all run in WebAssembly in the page. A host app only sends data to a server if it chooses an HTTP transport or writes its own upload path.
