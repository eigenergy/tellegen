# Deployment

tellegen deploys as one container: the tellegen backend with the built tellegen
frontend copied into the image. Production can run behind an existing Caddy edge
proxy that owns ports 80 and 443.

## Requirements

- Docker Engine with the Compose plugin
- 4 GB RAM minimum; 8 GB recommended for the bundled cases
- external Docker network `edge`, owned by the Caddy edge stack
- staged demo data under the deploy path's `data/` directory
- GHCR pull access on the host, or a public `ghcr.io/eigenergy/tellegen` package

## Shared Edge Layout

On a host with an existing reverse proxy, tellegen joins external Docker
network `edge` under container name `tellegen`. The proxy should route the
public hostname to `tellegen:8000`. The tellegen workflow does not mutate the
proxy config; it only deploys the app container and ensures that container
joins `edge`.

The staged case data should live under the deploy path, for example:

```sh
$TELLEGEN_DEPLOY_PATH/data
```

For the full demo geometry, stage these files:

```text
ACTIVSg200/case_ACTIVSg200.m
ACTIVSg200/ACTIVSg200.aux
ACTIVSg500/case_ACTIVSg500.m
ACTIVSg500/ACTIVSg500.aux
ACTIVSg7000/Texas7k_20210804.m
ACTIVSg7000/Texas7k_lat_long.csv
CATS/CaliforniaTestSystem.m
CATS/CATS_buses.csv
CATS/CATS_lines.json
```

The server serves the staged subset. If no complete case pair is staged, the
container exits unless `TELLEGEN_ALLOW_FALLBACK=1` is set for a CI or local
smoke check. Production deploys should stage the intended public case set before
enabling the workflow.

`CATS/CATS_gens.csv` is also staged when available. It is source metadata for
generator locations; the current map does not draw a separate generator layer.

Treat every staged case as public. The browser fetches the full staged network
JSON through `/api/cases/{id}/case` so it can build browser studies and exact
solves locally.

## Server Compute

The compute endpoints (`/api/cases/{id}/solve` over SSE and the
`/api/cases/{id}/sensitivity/...` routes) ship disabled and answer 403
`server compute is disabled`. Set `TELLEGEN_SERVER_COMPUTE=1` to enable them as
the fallback for browsers that cannot run the WebAssembly engine; the rate
limits and solver concurrency caps then apply. `/api/compute` reports the gate
(`{"enabled": bool}`) so the frontend picks honest fallback copy and skips
requests that would 403. The data endpoints, including the cached base
`/solution`, are always served.

## Local Build Deploy

For a host that builds from source:

```sh
git clone <repo> /opt/tellegen
cd /opt/tellegen
scripts/stage-data.sh /path/to/datasets
docker compose -f docker-compose.yml -f deploy/docker-compose.edge.yml up -d --build
```

`docker-compose.yml` binds the service to `127.0.0.1:8000`. The edge overlay
also joins the app to Docker network `edge` under container name `tellegen`, so
the shared proxy can route to it. Set `TELLEGEN_ALLOW_FALLBACK=1` only for CI
or local smoke checks that intentionally use the two pglib cases with synthetic
coordinates.

## Image Deploy

The production compose file consumes an image built by GitHub Actions:

```sh
TELLEGEN_IMAGE=ghcr.io/eigenergy/tellegen:<sha>
TELLEGEN_DATA_DIR=/opt/tellegen/data
docker compose -p tellegen --env-file .env \
  -f deploy/docker-compose.prod.yml \
  -f deploy/docker-compose.edge.yml up -d
```

The production compose file binds `127.0.0.1:8000`, mounts staged data read
only, runs with a read only root filesystem, drops Linux capabilities, blocks
new privileges, caps process count, and sets the memory limit. The edge overlay
adds the fixed container name and external `edge` network membership needed by
the Caddy route. The fixed Compose project name matters because the shared Caddy
edge stack is a separate project.

Use the host deploy script for normal deploys and rollbacks:

```sh
bash deploy/remote-deploy.sh ghcr.io/eigenergy/tellegen:<sha> "$TELLEGEN_DEPLOY_PATH/data"
```

The script validates Docker, Compose, the external `edge` network, that at
least one case directory exists, and the compose config. It pulls the selected
image before recreating the container, then waits for Docker health and
`/api/health`. It does not use `--remove-orphans`; the shared edge proxy is
owned by a separate stack.

## GitHub Actions Deploy

`.github/workflows/deploy.yml` runs on `push` to `main` and
`workflow_dispatch`, but every job is gated by repository variable:

```text
TELLEGEN_DEPLOY_ENABLED=true
```

Leave that variable unset until the host has staged data, an `edge` network,
and GHCR pull access. Once enabled, the workflow:

1. runs the tellegen backend and tellegen frontend checks;
2. builds the Docker image;
3. starts the image in Actions and checks `/api/health` with fallback data;
4. pushes `ghcr.io/eigenergy/tellegen:<sha>` and `ghcr.io/eigenergy/tellegen:main`;
5. copies `docker-compose.prod.yml`, `docker-compose.edge.yml`, and
   `remote-deploy.sh` to the host;
6. runs the host deploy script with the immutable SHA image;
7. checks `${TELLEGEN_DEMO_URL}/api/health` from Actions.

Required repository secrets:

- `TELLEGEN_DEPLOY_HOST`
- `TELLEGEN_DEPLOY_USER`
- `TELLEGEN_DEPLOY_SSH_KEY`
- `TELLEGEN_DEPLOY_PATH`, for example `/opt/tellegen`
- `TELLEGEN_DEMO_URL`, for example `https://tellegen.dev`
- `TELLEGEN_DEPLOY_KNOWN_HOSTS`: the pinned SSH host key for the deploy host,
  the verified output of `ssh-keyscan -H <host>` or an equivalent known hosts
  entry. The workflow refuses to deploy without this secret.

The workflow does not send the GitHub Actions token to the host. The host must
already be able to pull the GHCR image, or the GHCR package must be public.

## Preflight

Read only checks for a configured host:

```sh
ssh "$DEPLOY_HOST" docker network inspect edge
ssh "$DEPLOY_HOST" find "$TELLEGEN_DEPLOY_PATH/data" -maxdepth 2 -type f
ssh "$DEPLOY_HOST" curl -fsS http://127.0.0.1:8000/api/health
```

After a deploy:

```sh
ssh "$DEPLOY_HOST" docker inspect tellegen --format '{{if index .NetworkSettings.Networks "edge"}}edge{{end}}'
curl -fsS "${TELLEGEN_DEMO_URL%/}/api/health"
```

Both health checks should report `status: "ok"` and a nonempty `cases` array.
For the full public demo, that array should contain `case200`, `case500`,
`case7000`, and `cats`.

## Reverse Proxy

`deploy/Caddyfile` is a sample route for a shared edge proxy. Tellegen CI does
not mutate proxy config files.

The sample route sets HSTS, CSP, permissions policy, referrer policy, frame
ancestor denial, and content type sniffing protection. The CSP allows the
current SvelteKit bootstrap, WebAssembly compilation, MapLibre blob workers, and
CARTO tile images.

A Caddy image with `github.com/mholt/caddy-ratelimit` is needed for the sample
rate limits; stock Caddy does not include that module. The tellegen backend also
enforces solve and sensitivity rate limits with the same defaults, so the
container has a guard even when the proxy module is absent. Keep solve endpoints
compatible with SSE: do not buffer responses, and do not use short write
timeouts on `/api/cases/*/solve`.

## Capacity

Staged cases are parsed at boot, and the base DC OPF solution is cached for
each case. Browser WebAssembly handles the normal exact solve path; the
tellegen backend recomputes fallback solves on demand. Read endpoints serve
prebuilt case payloads.

## Public Hardening

- Keep the Caddy security headers and rate limits on solve and sensitivity
  endpoints.
- Keep the backend rate limits enabled. Defaults are 5 solve requests and 25
  sensitivity requests per client per 10 seconds. Set
  `TELLEGEN_SOLVE_RATE_LIMIT_EVENTS=0` or
  `TELLEGEN_SENSITIVITY_RATE_LIMIT_EVENTS=0` only for local debugging.
- Keep the image running as the bundled unprivileged user and keep the compose
  read only filesystem, dropped capabilities, no new privileges, process cap,
  and memory cap.
- Keep staged data mounted read only.
- Stage only cases that are cleared for public distribution; the demo serves the
  full network JSON for each staged case.
- Add request body limits before adding any tellegen backend upload endpoint.
- Current file drop parsing runs in the browser and does not reach the tellegen
  backend.
