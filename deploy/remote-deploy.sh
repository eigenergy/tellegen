#!/usr/bin/env bash
# Deploy, promote, or roll back the tellegen app stack on the host. CI copies
# this script and the compose files to a per-run bundle below the deploy path.
#
#   remote-deploy.sh <image@sha256:digest> [data-dir]
#   remote-deploy.sh --promote <image@sha256:digest>
#   remote-deploy.sh --rollback [expected-current-image@sha256:digest]
#
# A deploy must pass local health before returning. It becomes the durable
# last-known-good deployment only after the caller observes public health and
# invokes --promote. Shared proxy deployments route to tellegen over the
# external `edge` Docker network.
set -euo pipefail

die() {
	echo "==> $*" >&2
	exit 1
}

usage() {
	cat >&2 <<'EOF'
usage:
  remote-deploy.sh <image-ref@sha256:...> [data-dir]
  remote-deploy.sh --promote <image-ref@sha256:...>
  remote-deploy.sh --rollback [expected-current-image-ref@sha256:...]
EOF
	exit 2
}

validate_image() {
	local image="$1"
	# Accept a digest only. A tag can be repointed after the push. This pattern
	# also prevents a newline or shell metacharacter from reaching an env file.
	[[ "$image" =~ ^ghcr\.io/[a-z0-9._/-]+@sha256:[0-9a-f]{64}$ ]]
}

MODE=deploy
IMAGE=""
EXPECTED_CURRENT_IMAGE=""
DATA_DIR=""
case "${1:-}" in
	--promote)
		[ "$#" -eq 2 ] || usage
		MODE=promote
		IMAGE="$2"
		;;
	--rollback)
		[ "$#" -le 2 ] || usage
		MODE=rollback
		EXPECTED_CURRENT_IMAGE="${2:-}"
		;;
	-*) usage ;;
	"") usage ;;
	*)
		[ "$#" -le 2 ] || usage
		IMAGE="$1"
		DATA_DIR="${2:-${TELLEGEN_DATA_DIR:-}}"
		;;
esac

if { [ "$MODE" = deploy ] || [ "$MODE" = promote ]; } && ! validate_image "$IMAGE"; then
	echo "refusing $MODE of $IMAGE: expected ghcr.io/<name>@sha256:<64 hex>" >&2
	exit 2
fi
if [ -n "$EXPECTED_CURRENT_IMAGE" ] && ! validate_image "$EXPECTED_CURRENT_IMAGE"; then
	echo "refusing rollback guard $EXPECTED_CURRENT_IMAGE: expected a GHCR digest" >&2
	exit 2
fi

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
DEPLOY_ROOT_INPUT="${DEPLOY_ROOT:-$(cd "$SCRIPT_DIR/.." && pwd)}"
DEPLOY_ROOT="$(cd -- "$DEPLOY_ROOT_INPUT" && pwd -P)" \
	|| die "deploy root not found: $DEPLOY_ROOT_INPUT"
DEPLOY_BUNDLE_INPUT="${TELLEGEN_DEPLOY_BUNDLE_DIR:-$DEPLOY_ROOT/deploy}"
DEPLOY_BUNDLE_DIR="$(cd -- "$DEPLOY_BUNDLE_INPUT" && pwd -P)" \
	|| die "deploy bundle not found: $DEPLOY_BUNDLE_INPUT"
case "$DEPLOY_BUNDLE_DIR" in
	"$DEPLOY_ROOT"/*) ;;
	*) die "deploy bundle must resolve below $DEPLOY_ROOT: $DEPLOY_BUNDLE_DIR" ;;
esac

CURRENT_ENV="$DEPLOY_ROOT/.env"
LAST_GOOD_ENV="$DEPLOY_ROOT/.env.last-known-good"

need_file() {
	[ -f "$1" ] || die "missing required file: $1"
}

validate_data_dir() {
	local input="$1"
	case "$input" in
		/) echo "refusing data dir /" >&2; return 1 ;;
		/*[!a-zA-Z0-9._/-]*) echo "data dir has an unexpected character: $input" >&2; return 1 ;;
		/*) ;;
		*) echo "data dir must be an absolute path: $input" >&2; return 1 ;;
	esac
	[ -d "$input" ] || { echo "data directory not found: $input" >&2; return 1; }
	VALIDATED_DATA_DIR="$(cd -- "$input" && pwd -P)" \
		|| { echo "cannot canonicalize data directory: $input" >&2; return 1; }
	case "$VALIDATED_DATA_DIR" in
		"$DEPLOY_ROOT"/*) ;;
		*) echo "data directory must resolve below $DEPLOY_ROOT: $VALIDATED_DATA_DIR" >&2; return 1 ;;
	esac
}

# Load exactly the two values this script writes. Never source an env file.
load_env_file() {
	local file="$1" line saw_image=0 saw_data_dir=0
	STATE_IMAGE=""
	STATE_DATA_DIR=""
	[ -f "$file" ] || { echo "missing deployment state: $file" >&2; return 1; }
	while IFS= read -r line || [ -n "$line" ]; do
		case "$line" in
			TELLEGEN_IMAGE=*)
				[ "$saw_image" -eq 0 ] || { echo "duplicate image in $file" >&2; return 1; }
				saw_image=1
				STATE_IMAGE="${line#TELLEGEN_IMAGE=}"
				;;
			TELLEGEN_DATA_DIR=*)
				[ "$saw_data_dir" -eq 0 ] || { echo "duplicate data dir in $file" >&2; return 1; }
				saw_data_dir=1
				STATE_DATA_DIR="${line#TELLEGEN_DATA_DIR=}"
				;;
			*) echo "unexpected deployment state in $file" >&2; return 1 ;;
		esac
	done < "$file"
	validate_image "$STATE_IMAGE" || { echo "invalid image in $file" >&2; return 1; }
	validate_data_dir "$STATE_DATA_DIR" || return 1
	STATE_DATA_DIR="$VALIDATED_DATA_DIR"
}

write_env_temp() {
	local image="$1" data_dir="$2" temp
	temp="$(mktemp "$DEPLOY_ROOT/.env.tmp.XXXXXX")" || return 1
	chmod 600 "$temp"
	if ! printf 'TELLEGEN_IMAGE=%s\nTELLEGEN_DATA_DIR=%s\n' "$image" "$data_dir" > "$temp"; then
		rm -f -- "$temp"
		return 1
	fi
	WRITTEN_ENV="$temp"
}

install_env_atomically() {
	local image="$1" data_dir="$2" destination="$3"
	write_env_temp "$image" "$data_dir" || return 1
	if ! mv -f -- "$WRITTEN_ENV" "$destination"; then
		rm -f -- "$WRITTEN_ENV"
		return 1
	fi
}

compose_for_env() {
	local env_file="$1"
	compose=(
		docker compose -p tellegen --env-file "$env_file"
		-f "$DEPLOY_BUNDLE_DIR/docker-compose.prod.yml"
		-f "$DEPLOY_BUNDLE_DIR/docker-compose.edge.yml"
	)
}

validate_compose() {
	local services
	"${compose[@]}" config >/dev/null || return 1
	services="$("${compose[@]}" config --services)" || return 1
	[ "$services" = tellegen ] || {
		echo "unexpected compose services: $services" >&2
		return 1
	}
}

logs() {
	if [ "${#compose[@]}" -gt 0 ]; then
		"${compose[@]}" logs --tail=200 tellegen >&2 || true
	fi
	docker logs --tail=200 tellegen >&2 || true
}

health_payload_ok() {
	local payload="$1"
	[[ "$payload" == *'"status":"ok"'* && "$payload" != *'"cases":[]'* ]]
}

current_container_is_healthy() {
	local expected_image="$1" container_image state health edge_membership payload
	container_image="$(docker inspect tellegen --format '{{.Config.Image}}' 2>/dev/null || true)"
	[ "$container_image" = "$expected_image" ] || return 1
	edge_membership="$(docker inspect tellegen --format '{{if index .NetworkSettings.Networks "edge"}}edge{{end}}' 2>/dev/null || true)"
	[ "$edge_membership" = edge ] || return 1
	state="$(docker inspect --format '{{.State.Status}}' tellegen 2>/dev/null || true)"
	[ "$state" = running ] || return 1
	health="$(docker inspect --format '{{if .State.Health}}{{.State.Health.Status}}{{else}}none{{end}}' tellegen 2>/dev/null || true)"
	{ [ "$health" = healthy ] || [ "$health" = none ]; } || return 1
	payload="$(curl -fsS http://127.0.0.1:8000/api/health 2>/dev/null || true)"
	health_payload_ok "$payload"
}

wait_for_local_health() {
	local expected_image="$1" attempt state health edge_membership payload container_image
	edge_membership="$(docker inspect tellegen --format '{{if index .NetworkSettings.Networks "edge"}}edge{{end}}' 2>/dev/null || true)"
	[ "$edge_membership" = edge ] || {
		echo "tellegen container is not attached to the edge network" >&2
		return 1
	}

	for attempt in $(seq 1 150); do
		state="$(docker inspect --format '{{.State.Status}}' tellegen 2>/dev/null || echo missing)"
		case "$state" in
			missing|exited|dead)
				echo "tellegen container is $state before becoming healthy" >&2
				return 1
				;;
		esac
		health="$(docker inspect --format '{{if .State.Health}}{{.State.Health.Status}}{{else}}none{{end}}' tellegen 2>/dev/null || echo none)"
		if [ "$health" = healthy ] || { [ "$health" = none ] && [ "$state" = running ]; }; then
			echo "==> Docker reports tellegen $health"
			break
		fi
		if [ "$attempt" -eq 150 ]; then
			echo "tellegen did not become healthy in time" >&2
			return 1
		fi
		sleep 5
	done

	# Make sure Compose actually recreated the container from the selected digest.
	container_image="$(docker inspect tellegen --format '{{.Config.Image}}' 2>/dev/null || true)"
	[ "$container_image" = "$expected_image" ] || {
		echo "running container image does not match deployment state" >&2
		return 1
	}

	echo "==> Checking host health payload"
	for attempt in $(seq 1 90); do
		payload="$(curl -fsS http://127.0.0.1:8000/api/health 2>/dev/null || true)"
		if health_payload_ok "$payload"; then
			echo "==> tellegen host health ok: $payload"
			return 0
		fi
		if [ -n "$payload" ]; then
			echo "unexpected health payload: $payload" >&2
		fi
		sleep 10
	done
	echo "host health did not report status ok with at least one case" >&2
	return 1
}

cleanup_old_bundles() {
	local staging_root="$DEPLOY_ROOT/.deploy-staging"
	case "$DEPLOY_BUNDLE_DIR" in
		"$staging_root"/*/deploy) ;;
		*) return ;;
	esac
	local current="${DEPLOY_BUNDLE_DIR%/deploy}"
	while IFS= read -r -d '' candidate; do
		[ "$candidate" = "$current" ] && continue
		if ! rm -f -- \
			"$candidate/deploy/docker-compose.prod.yml" \
			"$candidate/deploy/docker-compose.edge.yml" \
			"$candidate/deploy/remote-deploy.sh" \
			|| ! rmdir -- "$candidate/deploy" "$candidate"; then
			echo "warning: could not remove old deploy bundle $candidate" >&2
		fi
	done < <(find "$staging_root" -mindepth 1 -maxdepth 1 -type d -mtime +7 -print0)
}

bootstrap_last_known_good() {
	if [ -f "$LAST_GOOD_ENV" ]; then
		load_env_file "$LAST_GOOD_ENV" \
			|| die "last-known-good deployment state is invalid"
		return
	fi
	if [ ! -f "$CURRENT_ENV" ]; then
		echo "==> No previous deployment is available for rollback"
		return
	fi
	if ! load_env_file "$CURRENT_ENV"; then
		die "current deployment state is invalid"
	fi
	if current_container_is_healthy "$STATE_IMAGE"; then
		install_env_atomically "$STATE_IMAGE" "$STATE_DATA_DIR" "$LAST_GOOD_ENV" \
			|| die "could not record the existing deployment as last known good"
		echo "==> Recorded the existing healthy deployment as last known good"
	else
		echo "==> Existing deployment is not healthy; it will not be a rollback target"
	fi
}

restore_last_known_good() {
	local rollback_image rollback_data
	load_env_file "$LAST_GOOD_ENV" || return 1
	rollback_image="$STATE_IMAGE"
	rollback_data="$STATE_DATA_DIR"
	echo "==> Rolling back to $rollback_image"
	# Validate and pull from the durable state before changing the active env.
	compose_for_env "$LAST_GOOD_ENV"
	validate_compose || return 1
	"${compose[@]}" pull tellegen || return 1
	install_env_atomically "$rollback_image" "$rollback_data" "$CURRENT_ENV" || return 1
	compose_for_env "$CURRENT_ENV"
	"${compose[@]}" up -d || return 1
	wait_for_local_health "$rollback_image"
}

command -v docker >/dev/null 2>&1 || die "docker is not installed"
command -v curl >/dev/null 2>&1 || die "curl is not installed"
command -v flock >/dev/null 2>&1 || die "flock is not installed"
docker compose version >/dev/null || die "docker compose plugin is not available"
docker network inspect edge >/dev/null || die "external Docker network 'edge' is missing"

# The Actions mutex covers normal runs. This host lock also covers a canceled
# SSH session whose remote shell continues, plus direct operator invocations.
exec 9>"$DEPLOY_ROOT/.deploy.lock"
flock -w 900 9 || die "timed out waiting for another deployment"

need_file "$DEPLOY_BUNDLE_DIR/docker-compose.prod.yml"
need_file "$DEPLOY_BUNDLE_DIR/docker-compose.edge.yml"
compose=()
umask 077

case "$MODE" in
	rollback)
		if [ -n "$EXPECTED_CURRENT_IMAGE" ]; then
			load_env_file "$CURRENT_ENV" || die "current deployment state is invalid"
			[ "$STATE_IMAGE" = "$EXPECTED_CURRENT_IMAGE" ] \
				|| die "current deployment changed; refusing stale rollback"
		fi
		if restore_last_known_good; then
			echo "==> Rollback is healthy"
			cleanup_old_bundles
			exit 0
		fi
		logs
		die "rollback failed"
		;;
	promote)
		load_env_file "$CURRENT_ENV" || die "current deployment state is invalid"
		[ "$STATE_IMAGE" = "$IMAGE" ] || die "current deployment changed; refusing stale promotion"
		compose_for_env "$CURRENT_ENV"
		validate_compose || die "current compose config is invalid"
		if ! wait_for_local_health "$IMAGE"; then
			logs
			die "current deployment is not healthy; refusing promotion"
		fi
		install_env_atomically "$STATE_IMAGE" "$STATE_DATA_DIR" "$LAST_GOOD_ENV" \
			|| die "could not promote deployment state"
		echo "==> Promoted $IMAGE as last known good"
		cleanup_old_bundles
		exit 0
		;;
esac

if [ -z "$DATA_DIR" ]; then
	DATA_DIR="$DEPLOY_ROOT/data"
fi
validate_data_dir "$DATA_DIR" || exit 1
DATA_DIR="$VALIDATED_DATA_DIR"

# The server serves whatever cases are staged under DATA_DIR. Require at least
# one case directory without coupling deployment to a hardcoded case list.
[ -n "$(find "$DATA_DIR" -mindepth 1 -maxdepth 1 -type d -print -quit 2>/dev/null)" ] \
	|| die "no case data staged under $DATA_DIR"

write_env_temp "$IMAGE" "$DATA_DIR" || die "could not create candidate deployment state"
CANDIDATE_ENV="$WRITTEN_ENV"
trap 'rm -f -- "${CANDIDATE_ENV:-}"' EXIT
compose_for_env "$CANDIDATE_ENV"

echo "==> Validating compose config"
validate_compose || die "candidate compose config is invalid"
echo "==> Pulling $IMAGE"
"${compose[@]}" pull tellegen || die "could not pull candidate image"

# Capture a healthy pre-automation deployment once. Later candidates are only
# promoted after the workflow verifies public health.
bootstrap_last_known_good

mv -f -- "$CANDIDATE_ENV" "$CURRENT_ENV" || die "could not install candidate deployment state"
CANDIDATE_ENV=""
trap - EXIT
compose_for_env "$CURRENT_ENV"

echo "==> Starting tellegen"
if ! "${compose[@]}" up -d || ! wait_for_local_health "$IMAGE"; then
	echo "==> Candidate failed local health; restoring last known good" >&2
	logs
	if restore_last_known_good; then
		echo "==> Previous deployment restored and healthy" >&2
	else
		logs
		echo "==> No healthy rollback could be completed" >&2
	fi
	exit 1
fi

echo "==> Candidate passed local health; awaiting public-health promotion"
cleanup_old_bundles
