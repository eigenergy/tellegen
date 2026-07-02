# HTTP API

The demo server's surface. Data endpoints are always served; the compute
endpoints ship disabled and answer 403 unless `TELLEGEN_SERVER_COMPUTE=1`
(see [Deployment](deployment.md)).

## Data

- `GET /api/health` — liveness and the served case ids.
- `GET /api/compute` — `{"enabled": bool}`, whether the compute endpoints are on.
- `GET /api/cases` — case summaries.
- `GET /api/cases/{id}/case` — the raw powerio network JSON the browser engine consumes.
- `GET /api/cases/{id}/network` — the map view (buses, branches, coordinates).
- `GET /api/cases/{id}/solution` — the cached base DC OPF solution, computed once at startup.

## Compute

- `GET /api/cases/{id}/sensitivity/lmp/d/{bus}` — the ∂LMP/∂demand column at a bus.
- `GET /api/cases/{id}/sensitivity/lmp/fmax/{branch}` — the ∂LMP/∂rating column at a branch.
- `GET /api/cases/{id}/solve` — a DC OPF solve streamed over server-sent events:
  `status`, `solution`, optional `sensitivity`, and `done`.

The sensitivity and solve endpoints accept `?d=bus:mw,bus:mw`, each value a MW
delta from the base case.

## Limits

Solve work is bounded by `TELLEGEN_SOLVER_CONCURRENCY` (default 2) and
`TELLEGEN_SOLVER_TIMEOUT_SECS` (default 30). Compute routes are rate limited
per client: 5 solve and 25 sensitivity requests per 10 seconds by default,
tuned with `TELLEGEN_RATE_LIMIT_WINDOW_SECS`,
`TELLEGEN_SOLVE_RATE_LIMIT_EVENTS`, and
`TELLEGEN_SENSITIVITY_RATE_LIMIT_EVENTS`.
