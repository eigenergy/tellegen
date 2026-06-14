# tellegen Rust

This crate builds the browser WebAssembly module used by the frontend. It
depends on powerio for case and display parsing.

## Exports

- `parse_case(text, format)`: parse a supported case format and return the
  powerio network JSON with parse warnings.
- `ingest_case(text, format)`: return summary counts, load, capacity, parse
  warnings, and map geometry when coordinates are present.
- `parse_display(bytes, format)`: parse a binary display file. The current
  supported format is PowerWorld `.pwd`.

Case format tokens follow powerio: `m`, `raw`, `aux`, and the JSON variants.
Display parsing uses `pwd`.

## Build

From `frontend/`:

```sh
npm run wasm
```

or directly from this directory:

```sh
wasm-pack build --target web --out-dir ../frontend/src/lib/wasm-pkg
```

The frontend imports the generated package lazily when a file is dropped.
