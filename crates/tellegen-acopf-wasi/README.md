# Experimental AC OPF WASI adapter

`npm run wasm:acopf` writes `tellegen_acopf_wasi.wasm` under
`target/experimental-acopf/`. An app build includes it only when
`PUBLIC_TELLEGEN_ACOPF_WASM_URL` is explicitly set.

The asset contains EPL-2.0 POUNCE code. Do not distribute it until all release
gates in `docs/src/acopf-pounce-decision.md` are complete.
