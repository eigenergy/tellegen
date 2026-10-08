# Deployment

tellegen deploys as one container: the tellegen backend with the built tellegen
frontend copied into the image. Production can run behind an existing Caddy edge
proxy that owns ports 80 and 443.

## Requirements

- Docker Engine with the Compose plugin
- `flock` (normally provided by util-linux)
- 4 GB RAM minimum; 8 GB recommended for the bundled cases
- external Docker network `edge`, owned by the Caddy edge stack
- staged demo data under the deploy path's `data/` directory
- For manual deployment, GHCR pull access on the host or a public
  `ghcr.io/eigenergy/tellegen` package; the workflow authenticates its own pulls

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

## Hosted Distribution Cases

Stage portable PowerIO multiconductor modules alongside the balanced demos and
add `distribution-cases.json` at the root of `TELLEGEN_DATA` (normally `data/`):

```json
[
  {
    "id": "my-feeder",
    "name": "My distribution feeder",
    "file": "feeders/my-feeder.pio.json"
  }
]
```

Each `file` is relative to the data directory and must stay inside it, including
when resolving symlinks. IDs must be unique, use ASCII letters, digits, `-` or
`_`, and cannot reuse a built-in demo ID or the browser's `local-`/`dist-`
prefixes. Names must be nonempty. The manifest is optional; omitting it retains
the existing demo catalogue.

Files must be PowerIO `pio-ir` modules containing a multiconductor network,
AC PF/OPF instance, or AC PF/OPF solution. Geometry and diagnostics are retained.
For a feeder already solved in the browser, export its distribution Study and
extract its portable input module:

```sh
mkdir -p data/feeders
jq -r '.input_module' tellegen-study.json > data/feeders/my-feeder.pio.json
```

Raw OpenDSS, BMOPF, and PMD files must first be imported into a PowerIO module;
they are not accepted directly by this manifest. A Tellegen Study export is a
wrapper, so stage its `input_module`, not the entire export.

Rebuild once to install a server/frontend version with distribution hosting.
Subsequent manifest and case changes need only a server restart and browser
reload. The normal Docker data mount also serves these files. No frontend
registration or staging-script change is needed.

The catalogue reports `model: "multiconductor"`. Selecting a feeder fetches
`/api/cases/{id}/case` and opens the existing distribution viewer. Coordinates
select the map or diagram view. Supported cases can run AC power flow in browser
WebAssembly; unsupported electrical models remain viewable with an explanation.
They also appear in WebMCP `list_cases`/`select_case`. Closing a hosted feeder
hides it in that browser; Restore defaults brings it back.

Distribution cases do not have server DC OPF solutions, snapshots, or sensitivity
endpoints. Their health check validates module loading, not AC power-flow
convergence. Missing or invalid files remain listed as unavailable and make
`/api/health` return 503; if no case can load, startup fails. A malformed manifest
or duplicate ID fails startup. All hosted modules are downloadable by clients.

A distribution-only data directory is supported and does not require the four
balanced demos or `TELLEGEN_ALLOW_FALLBACK`. When balanced demo files are also
staged, their existing requirement that all configured demos load still applies.
The public-demo deployment health assertion expects the four transmission cases
and the Texas7k distribution pilot described below. Custom deployments can use
their own manifest and catalogue expectations.

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
TELLEGEN_IMAGE=ghcr.io/eigenergy/tellegen@sha256:<digest>
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
bash deploy/remote-deploy.sh \
  ghcr.io/eigenergy/tellegen@sha256:<digest> \
  "$TELLEGEN_DEPLOY_PATH/data"
```

The script validates Docker, Compose, the external `edge` network, that at
least one case directory exists below the canonical deploy root, and the
compose config. A host `flock` serializes workflow and operator invocations. The
script pulls the selected digest before recreating the container, then waits
for Docker health and `/api/health`. It does not use `--remove-orphans`; the
shared edge proxy is owned by a separate stack.

Before changing the container, the script records a pending snapshot containing
the digest, data path, and exact two Compose files. Promotion atomically makes
that snapshot the last known good state. Rollback therefore restores the prior
Compose configuration as well as its image, and a later invocation recovers an
unpromoted candidate left by runner cancellation or SSH loss. The first run
migrates a healthy existing deployment from Docker's Compose file labels.

## GitHub Actions Deploy

`.github/workflows/deploy.yml` runs after a successful `CI` workflow for a push
to `main`, or by `workflow_dispatch` from `main`. Every job is gated by the
repository variable:

```text
TELLEGEN_DEPLOY_ENABLED=true
```

Leave that variable unset until the host has staged data and an `edge` network.
Once enabled, the workflow builds and smoke-tests the
exact commit whose CI passed, then checks that it is still the latest successful
CI push for `main`. Superseded runs stop without publishing; a newer failing or
in-progress commit does not suppress the last green deployment. The current run
pushes the already smoke-tested local image, resolves its immutable registry
digest, and repeats the latest-green check after any environment wait. It then
copies the deploy files into a unique per-run bundle, invokes the locked
host deploy with that bundle and digest, and checks
`${TELLEGEN_DEMO_URL}/api/health`. A manual run redeploys the latest green
commit. It checks latest-green state again immediately before promotion; a
candidate superseded during health checks is rolled back.

All demo runs share one non-canceling workflow mutex with the maximum pending
queue. Dispatch and start order are not a publication guarantee, so the
repeated latest-green checks remain the source of truth for which commit may
publish or reach the host. Per-run bundles keep an interrupted remote process
from observing files uploaded by its successor; the host lock serializes their
use. After a healthy deploy, bundles older than seven days are removed.

Required repository secrets:

- `TELLEGEN_DEPLOY_HOST`
- `TELLEGEN_DEPLOY_USER`
- `TELLEGEN_DEPLOY_SSH_KEY`
- `TELLEGEN_DEPLOY_PATH`, for example `/opt/tellegen`
- `TELLEGEN_DEMO_URL`, for example `https://tellegen.dev`
- `TELLEGEN_DEPLOY_KNOWN_HOSTS`: the pinned SSH host key for the deploy host,
  the verified output of `ssh-keyscan -H <host>` or an equivalent known hosts
  entry. The workflow refuses to deploy without this secret.

The deployment job has read-only package access. It sends its temporary
`GITHUB_TOKEN` through SSH stdin for `docker login`, with a private temporary
Docker configuration used only by that deployment or rollback command.
The helper removes the configuration on success, failure, or a handled signal,
and the job token expires when the job finishes. Existing host credentials
remain untouched. No registry token needs to be stored as a repository secret
or kept on the host.

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

## Texas7k distribution demo

The public demo also stages `texas7k-p1uhs0_1247`, the six-feeder substation
associated with Texas7k. The source is pinned in
`scripts/datasets/texas7k-source.json`. Obtain a BMOPFDraftData checkout with
that commit, build the browser engine, then prepare the data:

```sh
npm run wasm
npm run build:engine
python3 scripts/datasets/stage-texas7k.py /path/to/BMOPFDraftData data
```

Alternatively set `TELLEGEN_BMOPF_REPO=/path/to/BMOPFDraftData` when invoking
`scripts/stage-data.sh`. Staging reads committed source bytes, verifies checksums,
reduces duplicate representation, converts to portable PowerIO, and compares
original/reduced geometry and complete WASM PF results. It requires Git, Python
3 and Node; it does not download data. Electrical equipment is not pruned.

The script preserves other distribution entries and atomically registers an
immutable bundle under `data/distribution/`. Each bundle contains license and
source notices, reduction provenance, solver options, source revision/checksums,
and validation evidence. Copy the complete bundle and manifest to the deployment
data directory before deploying this version: the public deployment gate expects
the four transmission cases plus this distribution case. Data is an operator
staging step; the image deployment workflow does not upload it. Retain the old
manifest and bundle for data rollback. Restart the server after staging.

The optional manifest `metadata` object supports `description`, an absolute
HTTPS `source_url`, a built-in transmission `related_case_id`, and `pf_options`.
Supported option overrides are positive `tolerance`, positive
`absolute_kcl_tolerance`, positive integer `max_iterations`, and boolean
`voltage_envelope`. Unknown fields fail manifest parsing; invalid values make
the case unavailable. Missing values retain engine defaults. Explicit solve
options override case defaults, and successful solves retain their effective
options for later runs and Study export/reopen. Hosted distribution cases appear
in WebMCP with the same solver defaults as the UI.

The pilot uses constant-power loads without the bounded-voltage envelope,
frozen regulator taps and nominal loads; it has voltage drops and overloads.
The association is case-level, not a coupled transmission/distribution solve.
The catalogue link does not assert a specific transmission bus mapping.

The case endpoint negotiates gzip. Measure the served PowerIO payload, not the
reduced BMOPF file, when estimating downloads. The complete pilot remains a
large browser workload. Run the opt-in browser check against a server serving
both the production build and staged data:

```sh
TELLEGEN_DATA=data TELLEGEN_FRONTEND_BUILD=apps/web/build \
  TELLEGEN_PORT=4173 cargo run --release --locked -p tellegen-server
# In another terminal; Playwright reuses the running server on port 4173:
TELLEGEN_TEXAS7K=1 npm run test:browser -- texas7k-distribution.spec.ts --workers=1
```

The test checks actual API metadata, geography, UI/WebMCP solves, study round-trip
options/results, and a load edit. It writes timing, transfer-size and screenshot
artifacts. Routine browser CI uses small fixtures; run this full-data check
before changing the pinned source, converter, solver or demo bundle.

The recorded desktop acceptance run loaded in 5.34 s and solved in 1.78 s over
loopback. Combined Chromium RSS was about 1.65 GB after solving and 4.80 GB after
the export/reopen/rerun workflow (checkpoint samples, not exclusive allocation
or a continuous peak). The full pilot is suitable for desktop evaluation;
low-memory/mobile operation remains unqualified. See
`evidence/studies/texas7k-reduction/BROWSER.md` for methodology and limits.
