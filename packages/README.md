# packages/

Shared TypeScript packages for the tellegen monorepo.

Reserved for **`@tellegen/engine`** — a thin wrapper over the `tellegen-wasm`
wasm-pack output plus the TypeScript contract generated from the Rust types
(tsify) and a transport abstraction (in-browser wasm `Study` / HTTP server /
Tauri `invoke`). It is populated in the frontend-wiring phase, once `apps/web`
actually consumes it — and a second consumer (the desktop app) makes the shared
package pay for itself, rather than being indirection for a single importer.

Until then, `apps/web` loads the wasm directly from `apps/web/src/lib`, and no
JavaScript workspace manager is introduced (the Rust side is a Cargo workspace;
cross-language tasks are orchestrated by the root `justfile`).
