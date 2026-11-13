# New Zealand OpenStreetMap regression

`new-zealand-overpass.json.gz` is the public Overpass response retrieved on
2026-11-06 for `new-zealand-request.json`, through MapToGrid's Overpass mirror.
The response records its own OpenStreetMap base timestamp. The request uses
the application's Natural Earth 110m outline, seed 2026 and population demand.

The fixture reproduces the 417-bus, 722-branch model whose DC OPF was infeasible
and AC power flow did not converge. The WASM regression checks the unchanged
mapped-circuit mode and the explicitly synthetic reinforced model, offline.

OpenStreetMap data © OpenStreetMap contributors, licensed under the
[Open Database License 1.0](https://www.openstreetmap.org/copyright).
Natural Earth country outline: public domain.

## Portugal

`portugal-overpass.json.gz` was retrieved on 2026-11-07 with
`portugal-request.json`. Its OpenStreetMap base timestamp is
2026-11-02T00:59:53Z. It reproduced a 490 km inferred connection between
two 132 kV substations and 45 buses outside 0.9–1.1 pu after AC convergence.
The regression retains the full demand, checks that inferred connections
stay local, and requires converged AC voltages inside the planning band.
The same OpenStreetMap and Natural Earth licences above apply.

## Generated-grid hand-off

`handoff-request.json` is the request behind the hand-off benchmarks of
#261 (`scripts/bench-synth-handoff.mjs` and the opt-in browser spec
`apps/web/tests/synth-handoff-bench.spec.ts`): a synthetic, network-free
grid over eastern China, seed 2026. The benchmarks add `target_bus_count`
(3,000 by default), so every run builds the same grid.
