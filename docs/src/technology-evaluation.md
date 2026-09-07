# Technology choices

Keep Svelte for controls and MapLibre GL JS for geographic rendering. Rust remains the numerical engine, compiled for native execution and WASM. This recommendation follows the capabilities below, checked on September 7, 2026.

| Project | Use in Tellegen |
| --- | --- |
| [MapLibre Rust](https://github.com/maplibre/maplibre-rs) | Its published feature list still lacks GeoJSON, text, and a TypeScript API. Replacing the current renderer would remove capabilities used by case import and equipment selection. |
| [MapLibre Native](https://github.com/maplibre/maplibre-native) | Appropriate for a future native map application. It does not replace browser controls, accessibility, or persistent Study management. |
| [Martin](https://github.com/maplibre/martin) | Consider for a large shared geographic catalogue or self-hosted basemaps. Local case files do not require a tile server. |
| [MapLibre Tile](https://github.com/maplibre/maplibre-tile-spec) | Useful for large tiled geographic collections. Keep editable equipment geometry in GeoJSON: tile encoders can reorder features, and equipment identity must remain explicit. |
| [wasm-pack](https://github.com/wasm-bindgen/wasm-pack) | Already builds the engine package. Keep reproducible versions and exercise the installed package in CI. |
| [Walrus](https://github.com/wasm-bindgen/walrus) | Already participates through wasm-bindgen's tooling. Add a custom module-processing step only for a measured requirement; it is not a replacement for the application. |
| [wgpu](https://docs.rs/wgpu/latest/wgpu/) | Supports WebGPU and optional WebGL on WASM. A GPU drawing experiment could help unusually large networks, but needs measurements against the current map before adoption. |
| [Verus](https://github.com/verus-lang/verus) | Worth a small PowerIO experiment on byte-range arithmetic or identity-index invariants. Its supported Rust subset makes whole-parser adoption a separate research effort. Existing parser, binding, and cross-tool tests remain necessary. |

A useful next graphics measurement separates parsing, numerical solving, moving result arrays to JavaScript, layer updates, and map rendering. Record first-load bytes and time, memory growth, interaction latency, and frame time on Texas7k and a larger case. Typed arrays and selective layer updates should be evaluated before changing renderers. GPU support alone does not establish faster double-precision sparse power-system solves.

## AC OPF in the browser

[Pounce's current browser implementation](https://github.com/jkitchin/pounce/blob/04bdf2917fb4999b09d213448afc0b80df2b24b2/docs/src/wasm.md) uses `wasm32-wasip1` and a small JavaScript WASI implementation for clocks, output, and randomness. The worker receives an AMPL NL model and returns solution data. Tellegen's engine uses `wasm32-unknown-unknown`; the two modules can coexist in separate workers.

A local feasibility check instantiated Pounce's published WASM module under Node with its browser WASI implementation. The two-variable nonlinear test `nonconvex_qcqp.nl` returned `SolveSucceeded`, objective approximately -2, in 59 iterations. This verifies execution of the published module, not an AC OPF implementation or a build from that source revision. The evidence packet records the downloaded module's hash and the input revision.

A Pounce AC OPF addition should first use the existing native model implementation with its analytic Jacobian and Hessian tests. Then compare native and browser results on small power-system cases, checking the original electrical equations and limits independently of solver-reported residuals. A dedicated worker allows cancellation by terminating the calculation. BMOPF multiconductor AC OPF additionally needs verified winding, neutral, device-control, objective, and per-terminal limit equations. The fixed-point AC power flow does not claim that support.

## PowerIO and geographic data

PowerIO already represents a geographic document as `PioModule<PioValue::GeoLayer>`, and its module-aware `apply_geo_layer` supports both electrical network types. Keep GeoJSON as the exchange document and PioModule as the owner of source descriptions, diagnostics, and history. A second combined network/geography container would duplicate those responsibilities.

Tellegen retains the imported multiconductor module and its GeoLayer together, applies geographic files to the selected case, and reports matched buses, routes, and unmatched features. A geographic edit retains the calculation input and can be saved as generation-2 IR. Geographic coordinates and diagram positions keep their declared coordinate type. Tests use BMOPFTools' `bus_from` and `bus_to` route identities and check unchanged electrical results after attachment and reload.

The remaining useful upstream simplification is consistent module-aware extraction and application for calculation inputs and solutions as well as bare networks. Any such addition must preserve explicit device settings and numerical results. It should build on the existing functions without changing published ABI 7 or replacing BMOPF's geographic work.
