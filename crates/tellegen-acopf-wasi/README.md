# Experimental AC OPF WASI adapter

`npm run wasm:acopf` writes `tellegen_acopf_wasi.wasm` under
`target/experimental-acopf/`. An app build includes it only when
`PUBLIC_TELLEGEN_ACOPF_WASM_URL` is explicitly set.

The crate is inert in default workspace builds. Build or test the reactor with
`cargo build -p tellegen-acopf-wasi --features acopf --target wasm32-wasip1`
or `cargo test -p tellegen-acopf-wasi --features acopf`. Keep explicit AC OPF
builds separate from shipping-adapter builds: Cargo unifies dependency features
across packages selected in the same invocation.

The asset contains EPL-2.0 POUNCE code. Do not distribute it until all release
gates in `docs/src/acopf-pounce-decision.md` are complete.

The opt-in build also emits the POUNCE EPL-2.0 license, a notice, and a
`POUNCE-SOURCE.json` manifest beside the WASM. The manifest identifies the
checksummed 0.13.0 crate sources and the built artifact. These accompany the
asset in opt-in app builds; public distribution still needs release-owner
approval. The build/source guard requires Python 3.11+ (select it with `PYTHON`
if the system `python3` is older).
