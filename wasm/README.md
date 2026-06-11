# tellegen-wasm

powerio compiled to WASM for client side case file parsing. A dropped file is parsed in the browser; nothing is uploaded unless the user requests a server solve.

Phase 2. Not wired into the MVP build.

## Build

```sh
cargo install wasm-pack
wasm-pack build --target web --release
```

Output lands in `pkg/`; import from the frontend with a dynamic `import()` so the ~80 KB (gzip) module loads only when a file is dropped.

Notes from the powerio survey: wrap `parse_str` (not `parse_file`, which touches the filesystem), and leave `powerio-matrix` out until its `rayon` and file I/O paths are feature gated. Function names here (`parse_str`, `to_json`) track powerio's public API; adjust if the crate surface differs at integration time.
