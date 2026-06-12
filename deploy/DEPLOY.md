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

The container binds to 127.0.0.1:8000 only, so nothing is public until a reverse proxy fronts it. The first build is slow: one stage builds powerio (Rust) and the wasm module from source, another compiles JuMP and Ipopt. Both are cached layers, so source changes rebuild in seconds.

## TLS and reverse proxy

Any reverse proxy works. `deploy/Caddyfile` is a ready config: set your domain, copy it to the host, and run Caddy (host package or a container with ports 80 and 443). ACME certificates are automatic.

## Sizing

The staged cases (200, 500, and 2000 buses) fit in about 3 GB resident; the 2000 bus case dominates with its 32 MB sensitivity cache and Ipopt workspace. `docker-compose.yml` caps the container at 6 GB; adjust `mem_limit` to your machine and add swap on small hosts.

Solve requests serialize per case behind a lock. A warm re-solve costs about 100 ms on the small cases and 1 to 2 s on ACTIVSg2000, so a handful of concurrent users needs no queue. The read endpoints serve pre-serialized strings and are safe under load.

## Hardening before a public launch

- Stock Caddy has no per IP rate limiting. Build with `caddy-ratelimit` and enable the limits sketched in the Caddyfile; the sensitivity and solve endpoints are the expensive ones.
- PowerDiff and PowerIO install from git until registered in General; pass `--build-arg POWERDIFF_URL=...` / `POWERIO_JL_URL=...` if the repos move, and `--build-arg POWERIO_URL=...` for the Rust source build.
- If you add POST endpoints (case upload), put body size caps and a bounded queue in front of them first. The drop-a-file feature needs none of this: parsing runs in the visitor's browser and nothing reaches the server.

## Reference: the dev deployment (Hetzner)

The exact recipe behind the public demo, for copy-paste:

1. Hetzner Cloud CPX31 (4 vCPU, 8 GB), Ubuntu LTS, plus a 4 GB swapfile.
2. Docker Engine + compose plugin; nothing else runs on the host.
3. UFW: allow 22, 80, 443 (tcp, plus udp on 443 for HTTP/3), deny the rest.
4. DNS A record at the server IP; Caddy from the host package with the repo Caddyfile.
5. `git clone` to `/opt/tellegen`; `scp` the TAMU distributions over and `scripts/stage-data.sh` them; `docker compose up -d --build`.
