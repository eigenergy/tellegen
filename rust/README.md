# tellegen Rust

This crate builds the browser WebAssembly module used by the frontend and the
native Rust server used in deployment. It depends on powerio for case and
display parsing.

## Exports

- `parse_case(text, format)`: parse a supported case format and return the
  powerio network JSON with parse warnings.
- `ingest_case(text, format)`: return summary counts, load, capacity, parse
  warnings, and map geometry when the file carries coordinates.
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

## Server

From the repository root:

```sh
cargo run --manifest-path rust/Cargo.toml --bin tellegen-server
```

The server reads staged TAMU data from `TELLEGEN_DATA` or `data/`. Set
`TELLEGEN_ALLOW_FALLBACK=1` for local smoke checks without staged TAMU files.
