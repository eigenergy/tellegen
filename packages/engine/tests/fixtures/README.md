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
