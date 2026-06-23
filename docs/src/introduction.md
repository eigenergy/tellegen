<p class="hero-image">
  <img src="assets/hero.svg" alt="tellegen reactive power flow visualization" />
</p>

# Introduction

tellegen is a reactive visualization interface for power flow cases. The name
refers to Tellegen's theorem and the adjoint sensitivity calculations.

The app uses a gradient preview, exact commit interaction model. Perturbations
update the display from KKT sensitivity columns. Exact DC OPF commits run in
the tellegen frontend through Clarabel and WebAssembly. Case parsing uses
[powerio](https://github.com/eigenergy/powerio).

## Demo

The public demo serves three TAMU ACTIVSg synthetic grids at the geographic
coordinates stored in their PowerWorld aux exports. These are fictional grids
on geographic footprints, not surveyed infrastructure:

| case | territory | buses | branches |
| --- | --- | ---: | ---: |
| ACTIVSg200 | central Illinois | 200 | 245 |
| ACTIVSg500 | South Carolina | 500 | 597 |
| ACTIVSg2000 | Texas | 2000 | 3206 |

Each case is an islanded DC OPF instance. Bus color shows locational marginal
price. Selecting a bus shows the dLMP/dd column for a demand perturbation at
that bus. Moving the demand slider applies the local sensitivity immediately;
releasing it computes the exact solution with Clarabel in WebAssembly. Bundled
cases can fall back to the tellegen backend if browser solve is unavailable.

## Local Files

Dropped `.m`, `.raw`, and `.aux` files are parsed in the browser by the
WebAssembly build of powerio. Files with coordinates render on the map. Files
without coordinates can be placed by clicking the map, or paired with local
geographic files in `.csv`, `.json`, or `.geojson` form. A dropped PowerWorld
`.pwd` file is decoded as display data and rendered as approximate substation
positions. Parsed local case files solve in the browser and are not uploaded.

## API

- `GET /api/health`
- `GET /api/cases`
- `GET /api/cases/{id}/case`
- `GET /api/cases/{id}/network`
- `GET /api/cases/{id}/solution`
- `GET /api/cases/{id}/sensitivity/lmp/d/{bus}`
- `GET /api/cases/{id}/solve`

The sensitivity and solve endpoints accept `?d=bus:mw,bus:mw`, where each value
is a MW delta from the base case. The solve stream emits `status`, `solution`,
optional `sensitivity`, and `done` events.
