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
The public-demo deployment health assertion requires the four transmission
cases. It accepts the Texas7k distribution pilot described below without
requiring it. Custom deployments can use their own manifest and catalogue
expectations.

## Server Compute

The website build includes the experimental nonlinear AC OPF solver at
`/experimental-acopf/tellegen_acopf_wasi.wasm`, with its EPL-2.0 license,
notice, and checksummed source manifest alongside it. The Docker build compiles
this separate WASI reactor and copies it into the frontend. It does not enable
AC OPF in the native server. Pass `--build-arg TELLEGEN_SOURCE_REVISION=$(git rev-parse HEAD)`
when manually building an image to record the source commit; CI and deployment
workflows supply it automatically.

DC OPF remains the default calculation. The AC OPF dropdown option enables
only after the browser successfully probes the solver, and only for an
unedited balanced case. The browser runs each AC OPF solve in a dedicated,
terminable worker, reused between solves. There is no server fallback. AC OPF
returns nodal prices and supports sensitivity requests through the engine API;
edits and saving/exporting results remain unavailable.

Local website builds require `npm run wasm:acopf` before `npm run build:web`
or the Vite development server. `PUBLIC_TELLEGEN_ACOPF_WASM_URL` can override
the bundled solver URL; an unset or empty value uses the bundled asset.
Public distribution still requires the release review recorded in
[the POUNCE decision](deployment.md#release-architecture-separate-ac-opf-download).

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
- `TELLEGEN_DEPLOY_KNOWN_HOSTS`: the recorded SSH host key for the deploy host,
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

Both health checks should report `status: "ok"`, an empty `unavailable` array
and a nonempty `cases` array. For the full public demo, that array should
contain `case200`, `case500`, `case7000`, and `cats`. It also lists
`texas7k-p1uhs0_1247` once that case's bundle and `distribution-cases.json` are
staged (see [Texas7k distribution demo](#texas7k-distribution-demo)). The deploy
workflow requires the four transmission cases and allows, but does not require,
the Texas7k case.

## Local full-data browser check

Build the web app and start a native server from the repository root with all four cases staged.
The suite checks that the server is serving the current production build. Build the WebAssembly
engine first if the workspace has no generated Wasm package.

```sh
npm run build:engine
npm run build:web
TELLEGEN_DATA="$PWD/data" TELLEGEN_FRONTEND_BUILD="$PWD/apps/web/build" \
  TELLEGEN_HOST=127.0.0.1 TELLEGEN_PORT=4216 cargo run --release -p tellegen-server
```

In a second terminal, wait until the health check reports `status: "ok"`, then run the suite
against the same server.

```sh
curl -fsS http://127.0.0.1:4216/api/health
TELLEGEN_FULL_DATA=1 TELLEGEN_PREVIEW_PORT=4216 \
  npm --workspace tellegen-frontend run test:browser -- full-data-release.spec.ts
```

The checks load every demo case, save and reopen Texas7k results, exercise browser sensitivity and
planning, verify that an ACTIVSg200 geographic attachment preserves the result, and follow CATS
transfer and thermal limit results into the equipment inspector.

## Reverse Proxy

`deploy/Caddyfile` is a sample route for a shared edge proxy. Tellegen CI does
not mutate proxy config files.

The sample route sets HSTS, CSP, permissions policy, referrer policy, frame
ancestor denial, and content type sniffing protection. The CSP allows the
current SvelteKit bootstrap, WebAssembly compilation, MapLibre blob workers, and
the basemap hosts: `https://tiles.openfreemap.org` (the OpenFreeMap style,
vector tiles, fonts and sprites, all fetched, so `connect-src` must carry it)
and `https://*.cartocdn.com` (CARTO, the primary with a basemap key and the
fallback without one), in both `img-src` and
`connect-src`. The policy that ships with the build lives in
`apps/web/vite.config.ts`; a proxy policy must list the same hosts or the map
stays blank behind it. A deployment that points `TellegenMap` at another style
adds that host in both places.

To lead with CARTO's basemap, set the repository variable
`CARTO_BASEMAP_KEY` to a CARTO basemap key from
[carto.com/basemaps/apikey](https://carto.com/basemaps/apikey). Keys are free
for non-commercial use, and no account is needed. The deploy workflow passes
the key to the frontend build as `PUBLIC_CARTO_BASEMAP_KEY`; a local build
reads the same variable from the environment. The key ships in the page and
is sent with every tile request, so it is public by design: keep it in a
repository variable, not a secret, and restrict it to the deployment's hosts
in CARTO's key dashboard. Without the variable, the build has no key:
OpenFreeMap leads and CARTO is the fallback. The CSP is the same either way.

Agents reach the app's WebMCP tools through `document.modelContext`. Chrome and
Edge offer it on an origin with a WebMCP origin trial token, so visitors do not
need to enable a browser flag. Register the deployment's origin for the trial
and set the repository variable `WEBMCP_ORIGIN_TRIAL_TOKEN` to the token. The
deploy workflow passes it to the frontend build as
`PUBLIC_WEBMCP_ORIGIN_TRIAL_TOKEN`, and the prerendered pages carry it in an
`origin-trial` meta tag. A token is valid for one origin only, so a staging host
needs its own. To let other sites call the tools when they embed the app, set
`WEBMCP_EXPOSED_TO` to a comma-separated list of their origins, such as
`https://maptogrid.example`; the workflow passes it as
`PUBLIC_WEBMCP_EXPOSED_TO`, and the embedding page must also opt in. Both values
ship in the page, so keep them in repository variables, not secrets. Without
them the build adds no meta tag and the tools stay same-origin. A local build
reads both variables from the environment.

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

The intended public example is `texas7k-p1uhs0_1247`, the main energized island
of the six-feeder substation associated with Texas7k. It contains 22,076 modeled
buses and all 15,299 loads. Two original unsupplied line sections (five buses,
three lines) are removed; retained electrical values and coordinates are
unchanged. The prepared source and notices are checked against recorded checksums in
`scripts/datasets/texas7k-main-island-source.json`.

Reproduce the PowerIO 0.11.7 conversion and extraction using
`validation/evidence/studies/texas7k-reconversion/README.md`, build the browser engine,
then prepare the hosting data:

```sh
npm run wasm
npm run build:engine
python3 scripts/datasets/stage-texas7k.py /path/to/prepared-main-island data --main-island
```

Alternatively set `TELLEGEN_TEXAS7K_MAIN_ISLAND=/path/to/prepared-main-island`
when invoking `scripts/stage-data.sh`. Staging verifies all prepared input
checksums, reduces duplicate representation, converts to portable PowerIO, and
compares original/reduced geometry and complete WASM PF results. Source labels
remain in a source information sidecar. The reduction removes no further equipment and
rounds no electrical values. It requires Python 3 and Node and does not download
data. The historical full-pilot path remains available through
`TELLEGEN_BMOPF_REPO` and is used only when no main-island path is supplied.

The script preserves other distribution entries and atomically registers an
immutable bundle under `data/distribution/`. Each bundle contains license and
source notices, reduction source information, solver options, source revision/checksums,
and validation evidence. The case appears in the demo once the complete bundle
and `distribution-cases.json` are staged under the deployment data directory and
readable by the container user (uid 10001); restart the server after staging.
Deploys do not wait for it: the public deployment gate requires only the four
transmission cases. A manifest whose bundle is missing or unreadable makes the
case unavailable and fails that gate, so stage the bundle before the manifest.
Data is an operator staging step; the image deployment workflow does not upload
it. Retain the old manifest and bundle for data rollback.

The optional manifest `metadata` object supports `description`, an absolute
HTTPS `source_url`, a built-in transmission `related_case_id`, and `pf_options`.
`pf_options` accepts any of the ten multiconductor power-flow options listed in
[Settings](settings.md), checked by the same Rust resolver as every other
caller. Unknown `metadata` fields fail manifest parsing; an unknown or invalid
`pf_options` entry makes the case unavailable. Missing values retain engine
defaults. Explicit solve options override case defaults, and successful solves
retain their effective options for later runs and Study export/reopen. Hosted
distribution cases appear in WebMCP with the same solver defaults as the UI.

The pilot uses constant-power loads without the bounded-voltage envelope,
frozen regulator taps and nominal loads; it has voltage drops and overloads.
The association is case-level, not a coupled transmission/distribution solve.
The catalogue link does not assert a specific transmission bus mapping.

The case endpoint negotiates gzip, serving a copy compressed once when the
server loads the case. Measure the served PowerIO payload, not the
reduced BMOPF file, when estimating downloads. The current hosted module is
36.69 MB uncompressed and 1.26 MB gzipped; its BMOPF companion is 13.69 MB and
1.10 MB respectively. See `validation/evidence/studies/texas7k-reconversion/SHIPPING.md`.
The main island remains a
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
before changing the recorded source, converter, solver or demo bundle.

The historical full-pilot desktop acceptance run loaded in 5.34 s and solved in 1.78 s over
loopback. Combined Chromium RSS was about 1.65 GB after solving and 4.80 GB after
the export/reopen/rerun workflow (checkpoint samples, not exclusive allocation
or a continuous peak). Those timings were not rerun for the main-island bundle.
The six-feeder case is intended for desktop evaluation;
low-memory/mobile operation remains unqualified. See
`validation/evidence/studies/texas7k-reduction/BROWSER.md` for methodology and limits.


<a id="release-architecture"></a>

<a id="release-architecture-release-architecture"></a>

## Release Architecture

The public browser surfaces are `@tellegen/engine`, `@tellegen/svelte`,
`@tellegen/webmcp` and `@tellegen/analytics`.

Stable release surfaces:

- `crates/tellegen` Rust API layer, including the serde request and response
  shapes in `src/api.rs`;
- `crates/tellegen-wasm` wasm adapter;
- `packages/engine` TypeScript package, generated TypeScript types, and browser
  wasm entry points;
- `packages/svelte` Svelte component package;
- `packages/webmcp` WebMCP tool definitions and runtime validation without a UI
  framework dependency;
- `packages/analytics` allowlisted usage analytics with a first-party Umami
  Cloud sender, no dependencies;
- examples under `examples/`.

The hosted demo under `apps/web` is one consumer of the packages. It keeps
routes, SEO, credits, privacy, and deployment specific behavior.

<a id="release-architecture-javascript-workspace"></a>

### JavaScript Workspace

The repository uses npm workspaces with one root `package-lock.json` for:

- `packages/engine`;
- `packages/webmcp`;
- `packages/analytics`;
- `packages/svelte`;
- `apps/web`;
- `examples/browser-minimal`; and
- `examples/svelte-minimal`.

Install JavaScript dependencies from the repository root:

```sh
npm ci
```

Root scripts define the package order:

- `npm run wasm` builds the one wasm module (features `conic` and `synth`) into
  `packages/engine`;
- `npm run build:engine` builds `@tellegen/engine`;
- `npm run build:webmcp` builds `@tellegen/webmcp`;
- `npm run build:analytics` builds `@tellegen/analytics`;
- `npm run build:svelte` builds `@tellegen/svelte`;
- `npm run build:example` builds both downstream examples;
- `npm run check:web`, `npm run build:web`, and `npm run smoke:web` gate the
  hosted demo;
- `npm run pack:engine` previews the engine npm package contents;
- `npm run pack:webmcp` previews the WebMCP npm package contents;
- `npm run pack:analytics` previews the analytics npm package contents;
- `npm run pack:svelte` previews the Svelte npm package contents.

<a id="release-architecture-versioning"></a>

### Versioning

`@tellegen/engine`, `@tellegen/svelte`, `@tellegen/webmcp` and
`@tellegen/analytics` use independent package versions.

Before `1.0`, releases can refine public APIs while preserving the examples and
hosted demo behavior. After `1.0`, breaking public TypeScript, Svelte prop, or
Rust API changes require a semver major version.

Examples of breaking changes after `1.0`:

- removing or renaming public exports;
- removing or renaming request or response fields;
- changing enum tags, formulation ids, solve status tags, operand tags, or
  parameter tags;
- changing field units or meanings;
- tightening optional fields to required fields; and
- changing serialized request or response shapes.

Nonbreaking changes can ship in a minor version:

- adding optional fields;
- adding formulations, operands, parameters, statuses, or helper exports while
  preserving existing meanings; and
- adding component props with defaults.

Patch versions are for bug fixes and docs that do not change public APIs.

<a id="release-architecture-saved-cases"></a>

#### Saved cases

Tellegen saves the materialized network as a stored PowerIO module. The module
keeps applicable records, severs source linkage invalidated by edits, and adds
descriptive edit history. Loading it starts a fresh runtime `Study` at the
saved point. Its history records transformations applied to the stored value.

Exact OPF results are stored as PowerIO solution modules containing the
instance that was solved.

<a id="release-architecture-releasing"></a>

### Releasing

Package versions and the crate version move independently, so two release bots
drive the pipeline. A release that touches one surface publishes that surface
only.

Nobody cuts a tag by hand. This repository stores no registry token. Both
registries authenticate with OIDC, and each exchange runs in a deployment
environment (`crates-io`, `npm`). Reviewing and merging the generated version
pull request is the final human publication gate. Publication after that merge
is unattended.

Each environment allows deployments from `main` only and has no required
reviewer. Set the same environment on the registry's trusted publisher, along
with the repository and workflow filename. The workflow also rejects non-main
refs; the publisher binding makes the registry enforce that identity.

<a id="release-architecture-enabling-the-pipeline"></a>

#### Enabling the pipeline

The release workflows do nothing until the `TELLEGEN_RELEASE_ENABLED` repository
variable is `true`. What they need lives outside the repository. Set the
variable after you make all of these:

1. a `crates-io` environment and an `npm` environment, each with a deployment
   branch rule limited to `main` and no required reviewers;
2. a crates.io trusted publisher for `tellegen`, set to this repository,
   `release-crate.yml`, **and the `crates-io` environment**;
3. an npm trusted publisher for `@tellegen/engine`, `@tellegen/svelte`,
   `@tellegen/webmcp` and `@tellegen/analytics`, each set to this repository,
   `release-npm.yml`, **and the `npm` environment**. The publisher binds to the workflow filename. If you
   rename the workflow, publishing stops until you change the publisher;
4. a GitHub App on this repository with `contents: write` and
   `pull-requests: write`. Put its id in the `RELEASE_PLZ_APP_ID` variable and
   its private key in the `release-bot` environment's
   `RELEASE_PLZ_APP_PRIVATE_KEY` secret. Restrict that environment to `main`.
   Both release
   bots use it so their version pull requests start CI. Pull requests opened by
   `GITHUB_TOKEN` do not start workflows.

Keep deployment credentials in the main-only `demo` environment. No deployment
or Release App credentials belong at repository scope; only the jobs bound to
their environments receive them. The temporary migration workflow is removed
after its environment-only aliases prove that all copied credentials match.

The current `main` ruleset requires pull requests, merge commits and CI checks;
it blocks force pushes and deletion. It does not require the branch to be
up to date before merging. It requires zero
approvals and does not require approval of the last push, so a sole maintainer
can release after reviewing the final changes and checks. The version
pull-request merge is the publication gate. If independent approval becomes
required, set at least one approval and dismiss stale approvals or require
approval of the last reviewable push. Do not give the Release App a ruleset bypass.

For npm and crates.io, bootstrap a missing package before configuring its
trusted publisher. PyPI supports a pending publisher that creates the project
on the first OIDC upload; its setup is described below.

Check all four npm publishers with `npm trust list @tellegen/<name>`. A package
previously published by hand still needs its own publisher; configuring the
engine does not configure WebMCP. After `npm login`, the command is
`npm trust github @tellegen/<name> --file release-npm.yml --repo eigenergy/tellegen --env npm --allow-publish`.
It requires npm's interactive two-factor confirmation. A partial publish can
be retried: already published versions are retained, and remaining packages
publish through the same workflow.

<a id="release-architecture-separate-ac-opf-download"></a>

#### Separate AC OPF download

Each stable crate release also builds the isolated POUNCE WASI reactor from
that exact tag and attaches it to the existing GitHub release. The download
contains `tellegen_acopf_wasi.wasm`, `POUNCE-EPL-2.0.txt`, `POUNCE-NOTICE.txt`,
`POUNCE-SOURCE.json` and `ACOPF-SHA256SUMS.txt`. It remains separate from the MIT
engine packages. The manifest records the reactor ABI, engine version, full
source revision, source archive and POUNCE crate checksums. This preserves the
license and corresponding-source information distributed with the website.

Consumers specify a release URL and file checksum, rather than the changing demo
URL. The publisher never replaces existing release assets: a retry verifies
identical bytes and uploads only missing files. A source or toolchain change
needs a new release. To backfill an existing stable tag, dispatch
`release-acopf.yml` on `main` with `tag=vX.Y.Z`; it accepts only an existing
release whose tagged commit is part of `main`.

Delete any `NPM_TOKEN` or `CARGO_REGISTRY_TOKEN` repository secrets. The
workflows do not read long-lived registry credentials.

<a id="release-architecture-packages"></a>

#### Packages

`@tellegen/engine`, `@tellegen/svelte`, `@tellegen/webmcp` and
`@tellegen/analytics` release through changesets.

A change to any of these packages needs a changeset. Run `npm run changeset`
and commit the file it writes. See `.changeset/README.md`.

On a push to `main`, `.github/workflows/release-npm.yml` keeps a "Release
packages" pull request open. That pull request applies every pending changeset:
version bumps, changelog entries, the engine `ENGINE_VERSION`, and the
lockfile. Merge it to run the gates and publish. Tags take the form
`@tellegen/<name>@X.Y.Z`.

The workflow selects version or publish mode before it requests privileged
permissions. Only publish mode runs the Rust and JavaScript gates; version mode
refreshes the pull request, whose CI runs them. Versioning runs without the GitHub App, validates an exact output
allowlist, and passes a SHA-256-verified patch to a fresh runner. That runner
revalidates and commits the patch before minting the App token; only `git` and
`gh` run afterward. Publishing builds immutable tarballs in an unprivileged
job, then gives only the final npm job the `npm` environment and OIDC
permission. Merging the version pull request is therefore the only manual
release action.

`@tellegen/svelte` resolves `@tellegen/engine` from the registry, so a release
that moves both publishes the engine first.

<a id="release-architecture-crate"></a>

#### Crate

`tellegen` is the only crate that publishes to crates.io. `tellegen-wasm`,
`tellegen-server`, `tellegen-cli`, `tellegen-py`, and `benchmarks` carry
`publish = false`, and the release-plz workspace defaults to `release = false`.
A new crate needs an explicit package opt-in.

On a push to `main`, `.github/workflows/release-crate.yml` keeps a pull request
open that bumps the version. Merge it to run the gates, continue the existing
`vX.Y.Z` tag series, make the GitHub release, and publish. The configuration
sets `release_always = false`, limiting publication to a merged release-plz pull
request. The unprivileged gate runs `cargo package --locked --features sensitivity,mc-pf,conic,acopf,mc-opf,schema`, so every
shipping numerical feature builds against crates.io; the OIDC-enabled
release skips build verification because those unprivileged gates have already
verified the package against that exact commit. release-plz obtains its
short-lived crates.io credential directly from OIDC.

While the workspace resolves PowerIO from a temporary Git dependency (a
`[patch.crates-io]` section, used only until a PowerIO release reaches
crates.io), this packaging check fails, because packaging ignores `[patch]`;
that keeps the crate from publishing ahead of its PowerIO dependency. The
`powerio-012` feature is a no-op and is not in the list.
The release API comparison also uses `--only-explicit-features` with that list:
`cargo-semver-checks` otherwise enables non-default features heuristically.
`release-plz` itself has `semver_check = false` because this read-only gate owns
API comparison. Ordinary Cargo publication and docs builds use default features.

The workflow's first job decides whether the push can publish: its engine
version has no `v<version>` tag yet, and the push writes
`crates/tellegen/CHANGELOG.md`, which only the release pull request does. Only
then do the Rust gates and the publish job run. Any other push, including every
push between a version raised by hand and its release, only refreshes the
version pull request, which runs CI itself. A manual run, or a tag list or diff
that cannot be read, plans a publish and leaves the decision to release-plz.
While a release publishes, the version update waits for the gates, as it always
has.

The crate update uses the same sealed-patch privilege boundary as package
versioning. After `release-plz update`, an unprivileged step
(`.github/scripts/sync_python_version.py`) sets the Python wheel's package
version and its `Cargo.lock` entry to the engine's, without resolving or
updating any dependency, and `cargo metadata --locked` checks the result. The
patch then holds one of three path sets: the engine changelog alone (a version
already raised by hand on `main`); the engine manifest, its changelog, and the
lockfile; or those three and `crates/tellegen-py/Cargo.toml`, in which only the
package version may differ from the checkout. In every case the validator
requires both manifests and both lock entries to carry the engine's version,
before the patch is sealed and again after it is applied, before the App token
is minted. A wheel already on the new version needs no edit, and an update with
no release stays a no-op.

After a crate release succeeds, `release-crate.yml` dispatches
`release-python.yml` on main with the published tag. It resolves the tag to an
immutable commit, confirms it belongs to main and has a published GitHub
release, and runs the Python gates against that source. The gates check the
installed extension's type stubs, test Python 3.11 and 3.13, build five platform
wheels, and build and solve a case from the source archive. Test fixtures are
excluded from the source archive; the package includes its MIT licence and
all solver build inputs.

Only the final, checkout-free job has `id-token: write`. Its `pypi` environment
is restricted to main, and its trusted publisher is `eigenergy/tellegen`,
workflow `release-python.yml`, environment `pypi`. PyPI does not support a
reusable publishing workflow, so the crate workflow dispatches this standalone
workflow instead of calling it. A pending publisher configured at
<https://pypi.org/manage/account/publishing/> creates the project on its first
OIDC upload; no bootstrap API token or manual first upload is necessary.

To retry an interrupted publication, dispatch `release-python.yml` on main
with the same stable tag. Existing distributions are skipped; published
versions and their source commits are never replaced. Add the public install
command to the README and Python guide only after the first upload is verified.

Its fixed `release-plz-main` branch is intentional: with
`release_always = false`, release-plz recognizes a release commit by the
`release-plz-` head prefix. Merge this generated pull request with a normal
merge commit, not squash, so its release commit remains unambiguous if another
change reaches `main` nearby.

An emergency repair to a generated crate release must retain its
`release-plz-*` branch name; that is how `release_always = false` recognizes the
merged release commit.

<a id="release-architecture-inspecting-an-artifact"></a>

#### Inspecting an artifact

`.github/workflows/package-inspect.yml` is a manual run. It builds the artifacts
a release would upload, `cargo package` and all four `npm pack` tarballs, and
attaches them to the run. It publishes nothing.

<a id="release-architecture-changelogs"></a>

#### Changelogs

Each surface keeps its own generated changelog:
`crates/tellegen/CHANGELOG.md`, `packages/engine/CHANGELOG.md`,
`packages/svelte/CHANGELOG.md`, `packages/webmcp/CHANGELOG.md` and
`packages/analytics/CHANGELOG.md`. The root `CHANGELOG.md` is the archive of
the releases before the split. Nobody edits a changelog by hand, including on a
release pull request.

The crate changelog lists the pull requests merged since the previous release,
one entry each, under its title. release-plz writes a section for the new
version from commit subjects, but on this repository most of those read "Merge
pull request #N", and it walks the history along one parent at a time, which
can miss a pull request. So the crate release workflow then runs
`.github/scripts/write-crate-changelog.py`, which replaces that section's body.
It reads first-parent merges since the last `v*` tag and includes changes to
the published crate, the Python wheel, or a workspace dependency the crate uses.
The PR title supplies the entry. If a one-commit PR has an empty merge message,
the generator uses that commit’s subject. It records both solver versions when
Dependabot updates the paired POUNCE pins. Other Dependabot updates and version
merges are skipped. A `feat:` title goes under Added and a `fix:` title under
Fixed. The section itself must stay, because
`release-plz release` takes the GitHub release body from it, so the
`commit_parsers` in `release-plz.toml` keep merge commits.

<a id="release-architecture-ci-gates"></a>

### CI Gates

`.github/workflows/gates-rust.yml` and `.github/workflows/gates-js.yml` define
what green means. CI calls both on every pull request and every push to `main`.
The crate release calls the Rust gates, and the npm release calls both before
it publishes. Add a gate to those files, not to a caller.

CI runs only the gates a change can affect. Its scope job runs
`.github/scripts/ci-scope.py` on the changed paths: a pull request's against
the branch it merges into, a push's against the last `main` commit whose CI
passed. The Rust and JavaScript gates and the image build run unless every
changed path is one the script names as unable to affect them; a path it does
not name runs them. A version bump that only moves `tellegen` and `tellegen-py`
in their manifests and `Cargo.lock` runs only the Rust gates. A missing base, a
failed or empty diff, or a change to the scoping itself runs every gate. A gate
that does not apply still reports its required check, as skipped. The release
workflows call the gates without this scope, so a release always runs them in
full. `.github/scripts/test-ci-scope.py` pins the rules, and checks them
against the files the gates and the Dockerfile read. Change both when a gate
starts reading a new path.

The Rust gates are `cargo fmt --check`, clippy with warnings denied on the
shipping crates, `cargo-deny`, the EPL guard, the workspace tests, the engine's
`conic` path, `tellegen-wasm` built with `conic` and `synth`, and
`cargo package --locked --features sensitivity,mc-pf,conic,acopf,mc-opf,schema` for the published crate. The wasm test matters
because `tellegen-wasm` declares `default = []`, so the workspace run skips the
tests that assert an untrusted package or case rejects rather than panicking.

The JavaScript gates install once from the root lockfile, check and pack
`packages/webmcp` and `packages/analytics`, build and check `packages/engine`
before `packages/svelte`,
run the engine import smoke test, check the Svelte package through a packed
temporary consumer, build the hosted demo, and run its browser tests. `just ci`
runs everything locally.
