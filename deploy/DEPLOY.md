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
docker compose up -d --build
```

The container binds to 127.0.0.1:8000 only, so nothing is public until a reverse proxy fronts it. The first build is slow: the dependency layer compiles JuMP and Ipopt. It is keyed on `backend/Project.toml`, so source changes rebuild in seconds.

## TLS and reverse proxy

Any reverse proxy works. `deploy/Caddyfile` is a ready config: set your domain, copy it to the host, and run Caddy (host package or a container with ports 80 and 443). ACME certificates are automatic.

## Sizing

The bundled cases (200 and 500 buses) fit in 2 GB resident; thousands of buses run around 1.7 GB. `docker-compose.yml` caps the container at 6 GB; adjust `mem_limit` to your machine and add swap on small hosts.

Solve requests serialize per case behind a lock, and a warm re-solve costs about 100 ms, so a handful of concurrent users needs no queue. The read endpoints serve pre-serialized strings and are safe under load.

## Hardening before a public launch

- Stock Caddy has no per IP rate limiting. Build with `caddy-ratelimit` and enable the limits sketched in the Caddyfile; the sensitivity and solve endpoints are the expensive ones.
- PowerDiff and PowerIO install from git until registered in General; pass `--build-arg POWERDIFF_URL=...` / `POWERIO_JL_URL=...` if the repos move.
- If you add POST endpoints (case upload), put body size caps and a bounded queue in front of them first.

## Reference: the dev deployment (Hetzner)

The exact recipe behind the public demo, for copy-paste:

1. Hetzner Cloud CPX31 (4 vCPU, 8 GB), Ubuntu LTS, plus a 4 GB swapfile.
2. Docker Engine + compose plugin; nothing else runs on the host.
3. UFW: allow 22, 80, 443 (tcp, plus udp on 443 for HTTP/3), deny the rest.
4. DNS A record at the server IP; Caddy from the host package with the repo Caddyfile.
5. `git clone` to `/opt/tellegen`, `docker compose up -d --build`.
