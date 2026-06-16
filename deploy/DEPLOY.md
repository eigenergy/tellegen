# Deploying tellegen

tellegen deploys as one container: the Julia API server with the built static
frontend copied into the image.

## Requirements

- Docker Engine with the compose plugin
- 4 GB RAM minimum; 8 GB recommended for the bundled cases
- a reverse proxy for TLS
- staged TAMU data under the deploy path's `data/` directory

## Local Build Deploy

For a host that builds from source:

```sh
git clone <repo> /opt/tellegen
cd /opt/tellegen
scripts/stage-data.sh /path/to/tamu-datasets
docker compose up -d --build
```

`docker-compose.yml` binds the service to `127.0.0.1:8000`. A reverse proxy must
front the process before it is public. Without all three staged TAMU cases, the
server exits. Set `TELLEGEN_ALLOW_FALLBACK=1` only for CI or local smoke checks
that intentionally use the two pglib cases with synthetic coordinates.

## Image Deploy

The production compose file consumes an image built by GitHub Actions:

```sh
TELLEGEN_IMAGE=ghcr.io/eigenergy/tellegen:<sha>
TELLEGEN_DATA_DIR=/opt/tellegen/data
docker compose --env-file .env -f deploy/docker-compose.prod.yml up -d
```

`deploy/docker-compose.prod.yml` uses the same port binding, memory limit, data
mount, and restart policy as the local compose file. It does not build from
source on the host.

`/api/health` returns `ok` only after cases load. The image smoke test sets
`TELLEGEN_ALLOW_FALLBACK=1` because Actions does not have TAMU data. The host
and public deploy checks require the staged 200, 500, and 2000 bus cases.

## GitHub Actions Deploy

`.github/workflows/deploy.yml` runs on `push` to `main` and
`workflow_dispatch`. It:

1. runs the frontend and backend checks;
2. builds the Docker image;
3. starts the image in Actions and checks `/api/health`;
4. pushes `ghcr.io/eigenergy/tellegen:<sha>` and `ghcr.io/eigenergy/tellegen:main`;
5. copies `deploy/docker-compose.prod.yml` to the host;
6. writes `.env` with the selected image and data directory;
7. restarts the host service;
8. checks `http://127.0.0.1:8000/api/health` on the host and
   `${TELLEGEN_DEMO_URL}/api/health` from Actions.

Required repository secrets:

- `TELLEGEN_DEPLOY_HOST`
- `TELLEGEN_DEPLOY_USER`
- `TELLEGEN_DEPLOY_SSH_KEY`
- `TELLEGEN_DEPLOY_PATH`
- `TELLEGEN_DEMO_URL`, for example `https://tellegen.dev`

Optional but recommended:

- `TELLEGEN_DEPLOY_KNOWN_HOSTS`: the pinned SSH host key for the deploy host,
  the output of `ssh-keyscan <host>`. When set, the workflow uses it instead of
  scanning the host key fresh each run, closing a trust-on-first-use window
  where a spoofed host could intercept a deploy.

The deploy path must already contain staged case data at `data/`. The workflow
creates the directory if it is missing, but it does not download TAMU data.
The GHCR package should be public, or the host should have Docker credentials
configured outside this repository. The workflow does not send the GitHub
Actions token to the host.

## Reverse Proxy

`deploy/Caddyfile` is a minimal Caddy configuration. Replace the example domain
and forward to `127.0.0.1:8000`. Caddy obtains ACME certificates automatically.

If another proxy already owns ports 80 and 443, run tellegen behind that proxy.
`deploy/docker-compose.edge.yml` joins the service to an external Docker network
and gives it a fixed container name. The proxy can then route to
`tellegen:8000`.

## Capacity

The staged 200, 500, and 2000 bus cases use about 3 GB resident memory on the
current host. ACTIVSg2000 dominates memory and solve time because of its dense
sensitivity cache and Ipopt workspace. Solve requests serialize per case behind
a lock. Read endpoints serve pre-serialized JSON.

## Public Hardening

- Stock Caddy does not include per IP rate limiting. Add a rate limit module or
  equivalent proxy controls before exposing expensive endpoints to broad public
  traffic.
- Keep `/api/cases/*/solve` compatible with SSE; do not buffer responses and do
  not use short write timeouts.
- Add request body limits before adding any server side upload endpoint. Current
  file drop parsing runs in the browser and does not reach the server.
