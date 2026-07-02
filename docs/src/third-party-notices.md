# Third Party Notices

This page records attribution sources for the repository and public demo. It
does not replace the license metadata in `Cargo.lock`, `package-lock.json`, or
the source distributions for each dependency.

## Project Code

The project, including the Rust crates and the npm packages
`@tellegen/engine` and `@tellegen/svelte`, is licensed under either
Apache-2.0 or MIT, at your option. See `LICENSE-APACHE` and `LICENSE-MIT`.

The engine uses the W space SOCWR formulation as implemented in PowerModels.jl
as a formulation reference. PowerModels.jl is BSD 3-Clause licensed. The
tellegen implementation is independent.

## Direct Software Dependencies

Rust dependencies are resolved by Cargo and governed by `deny.toml`. Direct
runtime dependencies include `powerio`, `clarabel`, `faer`, `num-complex`,
`serde`, `serde_json`, `wasm-bindgen`, `console_error_panic_hook`, `axum`,
`tokio`, `tokio-stream`, `tower-http`, `tracing`, and `tracing-subscriber`.

The web app dependencies are resolved by npm. Direct dependencies include
SvelteKit, Svelte, Vite, TypeScript, deck.gl, MapLibre GL JS, Prettier,
`@fontsource-variable/bricolage-grotesque`, and `@fontsource/ibm-plex-mono`.

The repository does not modify or vendor those dependency sources. License and
notice files from dependencies are distributed through their normal package
archives.

## Demo Case Data

The demo data is staged by the operator and is not vendored in this repository.

ACTIVSg synthetic grids come from the Texas A&M Electric Grid Test Case
Repository. The case file headers request this citation:

> A. B. Birchfield, T. Xu, K. M. Gegner, K. S. Shetye, and T. J. Overbye, "Grid
> Structural Characteristics as Validation Criteria for Synthetic Networks,"
> IEEE Transactions on Power Systems, 2017.

CATS comes from the WISPO POP CATS California Test System repository and is BSD
3 Clause licensed in that repository. The source repository requests this
citation:

> S. Taylor, A. Rangarajan, N. Rhodes, J. Snodgrass, B. C. Lesieutre, and L. A.
> Roald, "California Test System (CATS): A Geographically Accurate Test System
> Based on the California Grid," IEEE Transactions on Energy Markets, Policy and
> Regulation, vol. 2, no. 1, pp. 107-118, 2024.
> doi:10.1109/TEMPR.2023.3338568.

Embedded fallback cases are from PGLib OPF and are used only when
`TELLEGEN_ALLOW_FALLBACK=1` is set. PGLib OPF data is CC BY 4.0; the PGLib
software is MIT licensed.

## Maps

The web app uses CARTO map tiles with OpenStreetMap attribution. Keep the map
attribution visible in any hosted demo or derivative deployment.
