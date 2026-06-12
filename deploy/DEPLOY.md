# Deploying tellegen

tellegen ships as one container: the Julia API server with the built frontend baked in. Any host that runs Docker can serve it; a small VPS is enough for the bundled cases.

## Requirements

- Docker Engine with the compose plugin
- 4 GB of RAM (8 GB gives headroom for larger cases)
- a domain name pointed at the server, if you want TLS

## Run

```sh
git clone <repo> /opt/tellegen
cd /opt/tellegen
scripts/stage-data.sh /path/to/tamu-datasets   # copies ~9 MB into data/
docker compose up -d --build
```

The TAMU ACTIVSg distributions are downloaded by the operator from [electricgrids.engr.tamu.edu](https://electricgrids.engr.tamu.edu/) and staged into `data/`, which the compose file mounts read only. Stage real files, not symlinks; bind mounts do not follow host symlinks. Without staged data the server falls back to pglib cases with synthetic coordinates.

The container binds to 127.0.0.1:8000 only, so nothing is public until a reverse proxy fronts it. The first build is slow: one stage builds the wasm module (powerio from crates.io), another compiles JuMP and Ipopt. Both are cached layers, so source changes rebuild in seconds.

## TLS and reverse proxy

Any reverse proxy works. `deploy/Caddyfile` is a ready config: set your domain, copy it to the host, and run Caddy (host package or a container with ports 80 and 443). ACME certificates are automatic.

## Sizing

The staged cases (200, 500, and 2000 buses) fit in about 3 GB resident; the 2000 bus case dominates with its 32 MB sensitivity cache and Ipopt workspace. `docker-compose.yml` caps the container at 6 GB; adjust `mem_limit` to your machine and add swap on small hosts.

Solve requests serialize per case behind a lock. A warm re-solve costs about 100 ms on the small cases and 1 to 2 s on ACTIVSg2000, so a handful of concurrent users needs no queue. The read endpoints serve pre-serialized strings and are safe under load.

## Hardening before a public launch

- Stock Caddy has no per IP rate limiting. Build with `caddy-ratelimit` and enable the limits sketched in the Caddyfile; the sensitivity and solve endpoints are the expensive ones.
- PowerDiff and PowerIO install from git at revs pinned in the Dockerfile until registered in General; bump `POWERIO_JL_REV` / `POWERDIFF_REV` (or override with `--build-arg`) when upstream moves. The powerio binary arrives as PowerIO.jl's bundled artifact; the wasm build takes powerio from crates.io.
- If you add POST endpoints (case upload), put body size caps and a bounded queue in front of them first. The drop-a-file feature needs none of this: parsing runs in the visitor's browser and nothing reaches the server.

## Sharing a host with an existing reverse proxy

When another stack's proxy already owns ports 80/443, run tellegen beside it instead of adding a second proxy: bring the container up joined to the proxy's Docker network with `deploy/docker-compose.edge.yml` (rename the network there to match the host's), then add a vhost to the proxy that forwards to `tellegen:8000` by container name. With Caddy that vhost is the site block from `deploy/Caddyfile` with `reverse_proxy tellegen:8000`; reload the proxy and TLS follows automatically once DNS resolves. The public demo at [tellegen.dev](https://tellegen.dev) runs this way; with the staged cases it holds about 3 GB resident beside whatever else the host serves. (`.dev` is HSTS preloaded, so the site is HTTPS-only by construction; automatic TLS covers it.)
