<p class="hero-image">
  <img src="assets/hero.svg" alt="tellegen reactive power flow visualization" />
</p>

# Introduction

tellegen is a reactive visualization interface for power flow cases. The name
refers to Tellegen's theorem and the adjoint sensitivity calculations used by
PowerDiff.jl.

The app uses a gradient preview, exact commit interaction model. Perturbations
update the display from KKT sensitivity columns computed by
[PowerDiff.jl](https://github.com/grid-opt-alg-lab/PowerDiff.jl). Exact DC OPF
solutions stream back from the server. Case parsing uses
[powerio](https://github.com/eigenergy/powerio) in both Rust/WebAssembly and
Julia.

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
releasing it sends the perturbation to the server and streams Ipopt iterations
until the exact solution returns.

## Local Files

Dropped `.m`, `.raw`, and `.aux` files are parsed in the browser by the
WebAssembly build of powerio. Files with coordinates render on the map. Files
without coordinates can be placed by clicking the map, or paired with local
coordinate sidecars in `.csv`, `.json`, or `.geojson` form. A dropped PowerWorld
`.pwd` file is decoded as display data and rendered as approximate substation
positions.

Dropped files are not uploaded.

## API

- `GET /api/health`
- `GET /api/cases`
- `GET /api/cases/{id}/network`
- `GET /api/cases/{id}/solution`
- `GET /api/cases/{id}/sensitivity/lmp/d/{bus}`
- `GET /api/cases/{id}/solve`

The sensitivity and solve endpoints accept `?d=bus:mw,bus:mw`, where each value
is a MW delta from the base case. The solve stream emits `status`, `iteration`,
`solution`, optional `sensitivity`, and `done` events.
