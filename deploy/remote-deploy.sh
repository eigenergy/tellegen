#!/usr/bin/env bash
# Deploy the tellegen app stack on the host. CI copies this script and the
# compose files to the deploy path, then calls:
#
#   bash deploy/remote-deploy.sh ghcr.io/eigenergy/tellegen:<sha> /opt/tellegen/data
#
# Shared proxy deployments route to tellegen over the `edge` Docker network, so
# this always uses the production compose file plus the edge overlay.
set -euo pipefail

IMAGE="${1:-}"
DATA_DIR="${2:-${TELLEGEN_DATA_DIR:-}}"

if [ -z "$IMAGE" ]; then
	echo "usage: remote-deploy.sh <image-ref> [data-dir]" >&2
	exit 2
fi

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
DEPLOY_ROOT="${DEPLOY_ROOT:-$(cd "$SCRIPT_DIR/.." && pwd)}"
if [ -z "$DATA_DIR" ]; then
	DATA_DIR="$DEPLOY_ROOT/data"
fi

cd "$DEPLOY_ROOT"

die() {
	echo "==> $*" >&2
	exit 1
}

logs() {
	if [ -f .env ]; then
		docker compose --env-file .env \
			-f deploy/docker-compose.prod.yml \
			-f deploy/docker-compose.edge.yml \
			logs --tail=200 tellegen >&2 || true
	fi
	docker logs --tail=200 tellegen >&2 || true
}

fail_with_logs() {
	echo "==> $*" >&2
	logs
	exit 1
}

need_file() {
	[ -f "$1" ] || die "missing required file: $1"
}

command -v docker >/dev/null 2>&1 || die "docker is not installed"
command -v curl >/dev/null 2>&1 || die "curl is not installed"
docker compose version >/dev/null || die "docker compose plugin is not available"
docker network inspect edge >/dev/null || die "external Docker network 'edge' is missing"

need_file deploy/docker-compose.prod.yml
need_file deploy/docker-compose.edge.yml

for case in ACTIVSg200 ACTIVSg500 ACTIVSg2000; do
	need_file "$DATA_DIR/$case/case_$case.m"
	need_file "$DATA_DIR/$case/$case.aux"
done

umask 077
printf 'TELLEGEN_IMAGE=%s\nTELLEGEN_DATA_DIR=%s\n' "$IMAGE" "$DATA_DIR" > .env

compose=(docker compose --env-file .env -f deploy/docker-compose.prod.yml -f deploy/docker-compose.edge.yml)

echo "==> Validating compose config"
"${compose[@]}" config >/dev/null

echo "==> Pulling $IMAGE"
"${compose[@]}" pull tellegen

echo "==> Starting tellegen"
"${compose[@]}" up -d --remove-orphans

edge_membership="$(docker inspect tellegen --format '{{if index .NetworkSettings.Networks "edge"}}edge{{end}}' 2>/dev/null || true)"
[ "$edge_membership" = "edge" ] || fail_with_logs "tellegen container is not attached to the edge network"

echo "==> Waiting for Docker health"
for attempt in $(seq 1 150); do
	state="$(docker inspect --format '{{.State.Status}}' tellegen 2>/dev/null || echo missing)"
	case "$state" in
		missing|exited|dead)
			fail_with_logs "tellegen container is $state before becoming healthy"
			;;
	esac

	health="$(docker inspect --format '{{if .State.Health}}{{.State.Health.Status}}{{else}}none{{end}}' tellegen 2>/dev/null || echo none)"
	if [ "$health" = "healthy" ] || { [ "$health" = "none" ] && [ "$state" = "running" ]; }; then
		echo "==> Docker reports tellegen $health"
		break
	fi
	if [ "$attempt" -eq 150 ]; then
		fail_with_logs "tellegen did not become healthy in time"
	fi
	sleep 5
done

echo "==> Checking host health payload"
for attempt in $(seq 1 90); do
	payload="$(curl -fsS http://127.0.0.1:8000/api/health 2>/dev/null || true)"
	if [[ "$payload" == *'"case200"'* && "$payload" == *'"case500"'* && "$payload" == *'"case2000"'* ]]; then
		echo "==> tellegen host health ok"
		exit 0
	fi
	if [ -n "$payload" ]; then
		echo "unexpected health payload: $payload" >&2
	fi
	sleep 10
done

fail_with_logs "host health did not report case200, case500, and case2000"
