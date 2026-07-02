# Transports

The transport boundary is the line between a host app and the tellegen engine.

## Browser Wasm

`@tellegen/engine` ships the browser wasm transport today. It wraps the `wasm-pack` output from `crates/tellegen-wasm` and exposes direct functions plus `browserWasmTransport`.

Use this transport when cases should stay local to the browser. Dropped case
files are parsed in WebAssembly, and solves, `Study.preview`, `Study.commit`,
and sensitivity requests run there as well. No case text or network JSON leaves
the browser unless the host app sends it.

The transport has one wasm package carrying parsing, all solves, `Study`,
capabilities, and generalized sensitivity requests.

The loader is lazy. Host apps can call `preloadEngine()` to control when the browser downloads and initializes wasm.

## HTTP

The hosted demo also uses HTTP for bundled case metadata and native server fallback paths. That server is a demo consumer, not a requirement for using `@tellegen/engine`.

An HTTP transport can implement the same shape as `EngineTransport`: parse or
fetch a network, call a native `solve_json` endpoint, keep a server side study
handle, and return the same generated TypeScript contract shapes.

That transport is optional for apps that need server sized cases, audit logs, or native deployment.

## Tauri

The desktop and mobile path can use the same transport contract with a Tauri command layer. The UI keeps the `Study` workflow; only the call boundary changes from browser wasm to native commands.

The Rust contract stays the source of truth. New transports should return the generated `SolveResponse`, `ProblemCaps`, and sensitivity matrix shapes instead of defining parallel types.
