# @tellegen/engine

Parses cases, runs WebAssembly solves, and exposes `Study` preview, commit, and
sensitivity calls in browsers.

The package has no SvelteKit dependency. Host apps import the top level package and use either the direct functions or the `browserWasmTransport` facade:

```ts
import {
  browserWasmTransport,
  createStudy,
  ingestJsonDrop,
  solveModule,
} from "@tellegen/engine";
```

The wasm files are resolved relative to the package module. Host apps must serve
package asset files from `node_modules`.

## Browser Ingestion

Case and JSON entry points accept the original bytes:

```ts
import { ingestCase, ingestJsonDrop } from "@tellegen/engine";

const bytes = new Uint8Array(await file.arrayBuffer());
const casePayload = await ingestCase(bytes, "raw");
const jsonPayload = await ingestJsonDrop(bytes);
```

Every solvable ingest payload includes `module_json`, a retained PowerIO
module. Pass that value to `createStudy` or `solveModule`. `network_json` is a
derived view used by display and geographic transforms; it is not a solver
input or persistence format.

`ingestJsonDrop(bytes)` classifies and parses JSON in one call. Its
`IngestedJsonDrop` result is discriminated by `kind`: PowerIO modules and model
JSON have a `null` format; transmission and distribution results carry the
selected format; `ambiguous` and `unknown` results have a `null` payload.

Every API that accepts a byte buffer rejects inputs larger than
`MAX_ENGINE_INPUT_BYTES` (128 MiB) before worker dispatch.

## Migrating To 0.2

- Pass `Uint8Array` to `ingestCase`. For an existing string, use
  `new TextEncoder().encode(text)`.
- Replace `classifyJson(text)` with `await classifyJson(bytes)`. It now returns
  `{ kind, format }` instead of a kind string.
- If migrating code that imported `isStudyPackageText` from `@tellegen/svelte`,
  use `await ingestJsonDrop(bytes)`. A stored document reports `kind: "module"`;
  its payload determines whether it contains a supported balanced or
  multiconductor value.
- Update `JsonDropKind` handling: `bmopf` and `pmd` are now
  `kind: "distribution"` with a `format`; `not-json` is `unknown`; and
  `transmission` and `ambiguous` are additional outcomes.

## Contracts

The public contract version is `CONTRACT_VERSION`, which matches the package version. `CONTRACT_SOURCE_SHA256` records the `crates/tellegen/src/api.rs` content used to generate `src/generated/contracts.ts`.

Run the generator after Rust API changes:

```sh
npm run contracts
```

CI runs `npm run build:engine` from the repository root, and that runs
`contracts:check`. A stale generated contract fails the build.

Breaking contract changes:

- Removing or renaming exported request or response fields.
- Changing field units, enum tags, formulation ids, solve status tags, or sensitivity operand/parameter shapes.
- Tightening a field from optional to required.

Nonbreaking changes:

- Adding optional fields.
- Adding new formulation ids, solve statuses, operands, or parameters when existing values keep their meaning.

## Release

Build and inspect the package from the repository root:

```sh
npm ci
npm run wasm
npm run build:engine
npm run pack:engine
```

`@tellegen/engine` is the lower level browser package for custom UIs and for the
Svelte package. It releases through Changesets with the other npm packages. The
hosted demo under `apps/web` is private and consumes `@tellegen/svelte` through
the same workspace file dependency used by the local examples.
