# Deployment

tellegen deploys as one container: the tellegen backend with the built tellegen
frontend copied into the image. Production can run behind an existing Caddy edge
proxy that owns ports 80 and 443.

## Requirements

- Docker Engine with the Compose plugin
- 4 GB RAM minimum; 8 GB recommended for the bundled cases
- external Docker network `edge`, owned by the Caddy edge stack
- staged TAMU data under the deploy path's `data/` directory
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

It must contain all six files:

```text
ACTIVSg200/case_ACTIVSg200.m
ACTIVSg200/ACTIVSg200.aux
ACTIVSg500/case_ACTIVSg500.m
ACTIVSg500/ACTIVSg500.aux
ACTIVSg2000/case_ACTIVSg2000.m
ACTIVSg2000/ACTIVSg2000.aux
```

Without all three staged cases, production deploy should fail before replacing
the working container.

## Local Build Deploy

For a host that builds from source:

```sh
git clone <repo> /opt/tellegen
cd /opt/tellegen
scripts/stage-data.sh /path/to/tamu-datasets
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
only, and sets the memory limit. The edge overlay adds the fixed container name
and external `edge` network membership needed by the Caddy route. The fixed
Compose project name matters because the shared Caddy edge stack is a separate
project.

Use the host deploy script for normal deploys and rollbacks:

```sh
bash deploy/remote-deploy.sh ghcr.io/eigenergy/tellegen:<sha> "$TELLEGEN_DEPLOY_PATH/data"
```

The script validates Docker, Compose, the external `edge` network, the staged
TAMU files, and the compose config. It pulls the selected image before
recreating the container, then waits for Docker health and `/api/health`. It
does not use `--remove-orphans`; the shared edge proxy is owned by a separate
stack.

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

Optional but recommended:

- `TELLEGEN_DEPLOY_KNOWN_HOSTS`: the pinned SSH host key for the deploy host,
  the output of `ssh-keyscan <host>`. When set, the workflow uses it instead of
  scanning the host key fresh each run.

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

Both health checks should report `case200`, `case500`, and `case2000`.

## Reverse Proxy

`deploy/Caddyfile` is a sample route for a shared edge proxy. Tellegen CI does
not mutate proxy config files.

A Caddy image with `github.com/mholt/caddy-ratelimit` is needed for the sample
rate limits; stock Caddy does not include that module. Keep solve endpoints
compatible with SSE: do not buffer responses, and do not use short write
timeouts on `/api/cases/*/solve`.

## Capacity

The staged 200, 500, and 2000 bus cases are parsed at boot, and the base DC OPF
solution is cached for each case. Browser WebAssembly handles the normal exact
solve path; the tellegen backend recomputes fallback solves on demand. Read
endpoints serve prebuilt case payloads.

## Public Hardening

- Keep the Caddy rate limits on solve and sensitivity endpoints.
- Keep staged data mounted read only.
- Add request body limits before adding any tellegen backend upload endpoint.
- Current file drop parsing runs in the browser and does not reach the tellegen
  backend.
