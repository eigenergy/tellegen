# tellegen

Reactive visualization for power systems optimization. Named for Tellegen's theorem, the basis of adjoint sensitivity analysis in circuits.

The interaction model is gradient preview, exact commit: perturbations update the display instantly through KKT sensitivity columns computed by [PowerDiff.jl](https://github.com/grid-opt-alg-lab/PowerDiff.jl), and exact OPF re-solves stream in behind them. Case file parsing uses [powerio](https://github.com/eigenergy/powerio).

## The demo

Two synthetic networks share one map: ACTIVSg200 sits on Illinois, ACTIVSg500 on South Carolina. Each is an islanded DC OPF instance on the backend; the header switcher pans the shared view between them. Both cases have quadratic generator costs, which keeps dLMP/dd nonzero across the interior of the feasible region (linear cost cases have piecewise constant LMPs whose gradient is zero almost everywhere).

Bus colors are locational marginal prices on a sequential ramp. Click a bus and the map switches to its dLMP/dd column on a diverging ramp: one exact KKT column, no re-solve. Drag the demand slider and prices preview instantly along that gradient; release it and the exact solution streams back over SSE with the interior point iterations drawn live. The panel then scores the preview: predicted objective change next to the exact one.

## Exact gradients

The sensitivities are exact derivatives of the KKT system at the optimum, not finite differences or fits. `backend/test/runtests.jl` holds that claim to numbers: dLMP/dd columns from PowerDiff match central finite differences of full re-solves to better than 0.1% relative error, and the residual is finite difference truncation, not gradient error.

```sh
julia --project=backend backend/test/runtests.jl
```

The objective prediction in the UI uses the envelope theorem (the LMP times the demand step) plus the second order term from the dLMP/dd diagonal, so the preview is exact through second order in the perturbation.

## Layout

- `backend/` Julia API server (Oxygen.jl + PowerDiff.jl)
- `frontend/` SvelteKit 5 static app (MapLibre GL + deck.gl)
- `wasm/` powerio WASM wrapper for in browser case file parsing (phase 2)
- `deploy/` deployment guide and Caddy config

## Development

Backend (port 8000):

```sh
cd backend
julia --project=. bootstrap.jl
```

PowerDiff.jl and PowerIO.jl are not yet in the General registry. Until they are, the backend environment uses local clones:

```sh
julia --project=backend -e 'using Pkg;
  Pkg.develop([PackageSpec(path="../Research/PowerIO.jl"),
               PackageSpec(path="../Research/PowerDiff.jl")])'
```

Frontend (port 5173, proxies `/api` to 8000):

```sh
cd frontend
npm install
npm run dev
```

## API

- `GET /api/health`
- `GET /api/cases`
- `GET /api/cases/{id}/network` buses, branches, synthetic coordinates
- `GET /api/cases/{id}/solution` LMPs, flows, dispatch from the DC OPF
- `GET /api/cases/{id}/sensitivity/lmp/d/{bus}` dLMP/dd column at a bus
- `GET /api/cases/{id}/solve` SSE re-solve stream

Both of the last two accept `?d=bus:mw,bus:mw`, demand deltas in MW from the base case, so gradients and solves are taken at the client's operating point. The solve stream emits `status`, then `iteration` events as Ipopt walks in (iterate, objective, primal and dual infeasibility), then `solution`, then a refreshed `sensitivity` column when `?sens={bus}` is passed, then `done`. It is a GET because EventSource only speaks GET.

## Roadmap

The demo is deliberately a slice. The wider toolchain it draws from suggests where it goes next:

- AC OPF sensitivities and voltage/reactive operands (PowerDiff.jl computes them today; the UI needs operand switching)
- synthetic grid generation, so users can spawn networks onto any part of the map (powerio)
- in browser case parsing via the powerio WASM build in `wasm/`
- energy burden and demographic overlays
- DER screening workflows

## License

MIT
