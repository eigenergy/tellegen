# Third Party Notices

This page records attribution sources for the repository and public demo. It
does not replace the license metadata in `Cargo.lock`, `package-lock.json`, or
the source distributions for each dependency.

## Project Code

The project, including the Rust crates and the npm packages
`@tellegen/engine` and `@tellegen/svelte`, is MIT licensed. See `LICENSE`.

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

### Optional POUNCE backend

The non-default native `tellegen/acopf` feature depends on
[POUNCE](https://github.com/jkitchin/pounce), a pure-Rust nonlinear solver
licensed under EPL-2.0. It is pinned to revision
`925e75fbd036de309929e398159f946d42d0d94b`. The default engine and the
published WASM, server, CLI, and Python adapters do not enable this feature.
`scripts/epl-guard.sh` checks that boundary from Cargo's resolved dependency
trees. The unpublished `tellegen-acopf-wasi` development adapter deliberately
does enable it; its generated asset is gitignored and ordinary builds and
deployments do not copy it.

Before any binary containing POUNCE is distributed, the release owner must
approve the EPL-2.0 obligations, ship the POUNCE license and notices, and state
where the corresponding POUNCE source can be obtained. This repository does
not modify POUNCE. The current dependency exception records a review boundary;
it is not release approval.

## Populated Places

The synthetic grid generator ships `crates/tellegen-synth/data/populated_places.json`
(copied into `@tellegen/engine` as `data/populated_places.json`): name,
longitude, latitude, `pop_max` and country of the Natural Earth v5.1.2 1:10m
populated places, derived by `scripts/derive-populated-places.py` from
`ne_10m_populated_places_simple.geojson`, whose SHA-256 the table records.
Natural Earth data is in the public domain
([naturalearthdata.com](https://www.naturalearthdata.com/about/terms-of-use/)).

It also ships `crates/tellegen-synth/data/national_demand.json`, compiled
into the engine: each country's annual electricity demand and population,
derived by `scripts/derive-national-demand.py` from Our World in Data's
energy dataset ([github.com/owid/energy-data](https://github.com/owid/energy-data)),
whose `electricity_demand` column comes from Ember's Yearly Electricity Data.
Ember, *Yearly Electricity Data*, and Our World in Data, *Energy dataset*, are
licensed under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/); the
table keeps each country's latest year, so changes were made. The
`population` column is Our World in Data's population series, compiled from
the sources listed at
[ourworldindata.org/population-sources](https://ourworldindata.org/population-sources)
and subject to their terms. The table records the SHA-256 of the CSV it was
derived from.

`crates/tellegen-synth/data/country_areas.json`, also compiled into the
engine, gives each country's area, measured by
`scripts/derive-country-areas.py` from Natural Earth's 1:50m admin-0
countries (public domain). `crates/tellegen-synth/data/voltage_ladders.json`
lists national transmission voltage levels with the operator, regulator and
standards documents each entry was taken from; it copies no dataset.

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

The web app's basemap comes from one of two providers, depending on whether
the deployment has a CARTO basemap key. CARTO's terms ask for one; keys come
from [carto.com/basemaps/apikey](https://carto.com/basemaps/apikey).

- With a key, the basemap is CARTO's vector basemap (`basemaps.cartocdn.com`),
  with OpenStreetMap data, and [OpenFreeMap](https://openfreemap.org/) is the
  fallback.
- Without a key, OpenFreeMap serves it (styles, vector tiles, fonts and
  sprites from `tiles.openfreemap.org`), and the keyless CARTO style is the
  fallback.

OpenFreeMap's style and tile schema are [OpenMapTiles](https://openmaptiles.org/)
(BSD 3-Clause for the schema, CC BY 4.0 for the design), and the map data is
© [OpenStreetMap](https://www.openstreetmap.org/copyright) contributors under
the ODbL. The app modifies the OpenFreeMap styles when it renders them: road
lines start at zoom 8, and in the dark theme water, waterways and land fills
are recoloured.

The attribution control shows the credits of whichever provider is on screen
and cannot be switched off. Keep it visible in any hosted demo or derivative
deployment.

## OpenStreetMap Mode

A grid generated with `source: "osm"` contains data derived from
OpenStreetMap, © [OpenStreetMap](https://www.openstreetmap.org/copyright)
contributors, available under the Open Database License (ODbL) 1.0. Check the
ODbL's attribution and share-alike terms before redistributing such a module
or its exports.
