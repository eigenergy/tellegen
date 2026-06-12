# tellegen-wasm

powerio compiled to WebAssembly for client side case file parsing. A dropped file is parsed in the browser; it never leaves the machine.

## Exports

- `parse_case(text, format)`: full powerio network as JSON, with parse warnings.
- `ingest_case(text, format)`: counts, total load and capacity, parse warnings, and a map-ready `view` of buses and branches in the shape the backend serves, placed at the substation coordinates in the file (PowerWorld complete case aux exports). `view` is null when the file has none. One parse per dropped file.

Format tokens follow powerio: `m`, `raw`, `aux`, and the JSON variants.

## Build

```sh
cargo install wasm-pack
wasm-pack build --target web --out-dir ../frontend/src/lib/wasm-pkg
```

The output (~150 KB gzip) is imported lazily by the frontend, so the module loads only when a file is dropped.
