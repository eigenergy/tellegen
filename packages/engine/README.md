# @tellegen/engine

Browser package for tellegen case ingestion, wasm solves, Study lifecycle calls, previews, commits, and sensitivity queries.

The package has no SvelteKit dependency. Host apps import the top level package and use either the direct functions or the `browserWasmTransport` facade:

```ts
import { browserWasmTransport, createStudy, solveJson } from "@tellegen/engine";
```

The wasm files are resolved relative to the package module. Host apps must serve
package asset files from `node_modules`.

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

The first framework release publishes this package and `@tellegen/svelte`.
`@tellegen/engine` is the lower level browser package for custom UIs and for the
Svelte package. The hosted demo under `apps/web` is private and consumes
`@tellegen/svelte` through the same workspace file dependency used by the local
examples.
