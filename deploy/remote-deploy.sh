#!/usr/bin/env bash
# Deploy a digest, promote it after public health, or recover the exact previous
# deployment. State snapshots include the env and both Compose files.
set -euo pipefail

die() { echo "==> $*" >&2; exit 1; }

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
	[[ "$1" =~ ^ghcr\.io/[a-z0-9._/-]+@sha256:[0-9a-f]{64}$ ]]
}

validate_portable_absolute_path() {
	local value="$1" label="$2" part
	case "$value" in
		/|*//*|/*[!a-zA-Z0-9._/-]*|[!/]*)
			echo "$label must be a non-root absolute path without empty or unsafe components" >&2
			return 1
			;;
	esac
	IFS=/ read -r -a parts <<< "${value#/}"
	for part in "${parts[@]}"; do
		case "$part" in
			""|.|..)
				echo "$label contains a forbidden path component: $part" >&2
				return 1
				;;
		esac
	done
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
	die "refusing $MODE of $IMAGE: expected a GHCR digest"
fi
if [ -n "$EXPECTED_CURRENT_IMAGE" ] && ! validate_image "$EXPECTED_CURRENT_IMAGE"; then
	die "refusing invalid rollback guard"
fi

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
DEPLOY_ROOT_INPUT="${DEPLOY_ROOT:-$(cd "$SCRIPT_DIR/.." && pwd -P)}"
validate_portable_absolute_path "$DEPLOY_ROOT_INPUT" "deploy root" || exit 1
DEPLOY_ROOT="$(cd -- "$DEPLOY_ROOT_INPUT" && pwd -P)" \
	|| die "deploy root not found: $DEPLOY_ROOT_INPUT"
[ "$DEPLOY_ROOT" != / ] || die "canonical deploy root may not be /"

DEPLOY_BUNDLE_INPUT="${TELLEGEN_DEPLOY_BUNDLE_DIR:-$DEPLOY_ROOT/deploy}"
validate_portable_absolute_path "$DEPLOY_BUNDLE_INPUT" "deploy bundle" || exit 1
DEPLOY_BUNDLE_DIR="$(cd -- "$DEPLOY_BUNDLE_INPUT" && pwd -P)" \
	|| die "deploy bundle not found: $DEPLOY_BUNDLE_INPUT"
case "$DEPLOY_BUNDLE_DIR" in
	"$DEPLOY_ROOT"/*) ;;
	*) die "deploy bundle must resolve below $DEPLOY_ROOT" ;;
esac

CURRENT_ENV="$DEPLOY_ROOT/.env"
STATE_ROOT="$DEPLOY_ROOT/.deploy-state"
SNAPSHOTS_DIR="$STATE_ROOT/snapshots"
LAST_GOOD_POINTER="$STATE_ROOT/last-known-good"
PENDING_POINTER="$STATE_ROOT/pending"

need_regular_file() {
	[ -f "$1" ] && [ ! -L "$1" ] || {
		echo "missing or unsafe regular file: $1" >&2
		return 1
	}
}

safe_destination() {
	local destination="$1"
	[ ! -L "$destination" ] || { echo "refusing symlink destination: $destination" >&2; return 1; }
	[ ! -d "$destination" ] || { echo "refusing directory destination: $destination" >&2; return 1; }
	[ ! -e "$destination" ] || [ -f "$destination" ] || {
		echo "refusing non-regular destination: $destination" >&2
		return 1
	}
}

atomic_copy() {
	local source="$1" destination="$2" mode="$3" temp
	need_regular_file "$source" || return 1
	safe_destination "$destination" || return 1
	temp="$(mktemp "${destination}.tmp.XXXXXX")" || return 1
	if ! install -m "$mode" -- "$source" "$temp" || ! mv -fT -- "$temp" "$destination"; then
		rm -f -- "$temp"
		return 1
	fi
}

write_env_file() {
	local destination="$1" image="$2" data_dir="$3" temp
	safe_destination "$destination" || return 1
	temp="$(mktemp "${destination}.tmp.XXXXXX")" || return 1
	chmod 600 "$temp"
	if ! printf 'TELLEGEN_IMAGE=%s\nTELLEGEN_DATA_DIR=%s\n' "$image" "$data_dir" > "$temp" \
		|| ! mv -fT -- "$temp" "$destination"; then
		rm -f -- "$temp"
		return 1
	fi
}

validate_data_dir() {
	local input="$1"
	validate_portable_absolute_path "$input" "data directory" || return 1
	[ -d "$input" ] && [ ! -L "$input" ] || {
		echo "data directory is missing or is a symlink: $input" >&2
		return 1
	}
	VALIDATED_DATA_DIR="$(cd -- "$input" && pwd -P)" || return 1
	case "$VALIDATED_DATA_DIR" in
		"$DEPLOY_ROOT"/*) ;;
		*) echo "data directory must resolve below $DEPLOY_ROOT" >&2; return 1 ;;
	esac
}

# Load exactly the values written above; never source deployment state.
load_env_file() {
	local file="$1" line saw_image=0 saw_data=0
	STATE_IMAGE=""
	STATE_DATA_DIR=""
	need_regular_file "$file" || return 1
	while IFS= read -r line || [ -n "$line" ]; do
		case "$line" in
			TELLEGEN_IMAGE=*)
				[ "$saw_image" -eq 0 ] || return 1
				saw_image=1
				STATE_IMAGE="${line#TELLEGEN_IMAGE=}"
				;;
			TELLEGEN_DATA_DIR=*)
				[ "$saw_data" -eq 0 ] || return 1
				saw_data=1
				STATE_DATA_DIR="${line#TELLEGEN_DATA_DIR=}"
				;;
			*) echo "unexpected deployment state in $file" >&2; return 1 ;;
		esac
	done < "$file"
	[ "$saw_image" -eq 1 ] && [ "$saw_data" -eq 1 ] || return 1
	validate_image "$STATE_IMAGE" || return 1
	validate_data_dir "$STATE_DATA_DIR" || return 1
	STATE_DATA_DIR="$VALIDATED_DATA_DIR"
}

read_pointer() {
	local file="$1" extra
	POINTER_NAME=""
	POINTER_DIR=""
	need_regular_file "$file" || return 1
	IFS= read -r POINTER_NAME < "$file" || return 1
	extra="$(tail -n +2 -- "$file")"
	[ -z "$extra" ] || { echo "extra data in pointer $file" >&2; return 1; }
	[[ "$POINTER_NAME" =~ ^snapshot\.[A-Za-z0-9]+$ ]] || {
		echo "invalid deployment pointer: $file" >&2
		return 1
	}
	POINTER_DIR="$SNAPSHOTS_DIR/$POINTER_NAME"
	[ -d "$POINTER_DIR" ] && [ ! -L "$POINTER_DIR" ] || return 1
	need_regular_file "$POINTER_DIR/.env" || return 1
	need_regular_file "$POINTER_DIR/docker-compose.prod.yml" || return 1
	need_regular_file "$POINTER_DIR/docker-compose.edge.yml" || return 1
}

write_pointer() {
	local name="$1" destination="$2" temp
	[[ "$name" =~ ^snapshot\.[A-Za-z0-9]+$ ]] || return 1
	safe_destination "$destination" || return 1
	temp="$(mktemp "${destination}.tmp.XXXXXX")" || return 1
	chmod 600 "$temp"
	if ! printf '%s\n' "$name" > "$temp" || ! mv -fT -- "$temp" "$destination"; then
		rm -f -- "$temp"
		return 1
	fi
}

compose_for() {
	local env_file="$1" bundle="$2"
	compose=(
		docker compose -p tellegen --env-file "$env_file"
		-f "$bundle/docker-compose.prod.yml"
		-f "$bundle/docker-compose.edge.yml"
	)
}

validate_compose() {
	local services
	timeout 60 "${compose[@]}" config >/dev/null || return 1
	services="$(timeout 60 "${compose[@]}" config --services)" || return 1
	[ "$services" = tellegen ] || { echo "unexpected compose services: $services" >&2; return 1; }
}

health_payload_ok() {
	[[ "$1" == *'"status":"ok"'* && "$1" != *'"cases":[]'* ]]
}

current_container_is_healthy() {
	local expected="$1" actual state health edge payload
	actual="$(timeout 20 docker inspect tellegen --format '{{.Config.Image}}' 2>/dev/null || true)"
	[ "$actual" = "$expected" ] || return 1
	edge="$(timeout 20 docker inspect tellegen --format '{{if index .NetworkSettings.Networks "edge"}}edge{{end}}' 2>/dev/null || true)"
	[ "$edge" = edge ] || return 1
	state="$(timeout 20 docker inspect tellegen --format '{{.State.Status}}' 2>/dev/null || true)"
	[ "$state" = running ] || return 1
	health="$(timeout 20 docker inspect tellegen --format '{{if .State.Health}}{{.State.Health.Status}}{{else}}none{{end}}' 2>/dev/null || true)"
	{ [ "$health" = healthy ] || [ "$health" = none ]; } || return 1
	payload="$(curl --proto '=http' --connect-timeout 3 --max-time 10 -fsS \
		http://127.0.0.1:8000/api/health 2>/dev/null || true)"
	health_payload_ok "$payload"
}

wait_for_local_health() {
	local expected="$1" attempt state health edge payload actual
	edge="$(timeout 20 docker inspect tellegen --format '{{if index .NetworkSettings.Networks "edge"}}edge{{end}}' 2>/dev/null || true)"
	[ "$edge" = edge ] || { echo "tellegen is not attached to edge" >&2; return 1; }
	for attempt in $(seq 1 150); do
		state="$(timeout 20 docker inspect tellegen --format '{{.State.Status}}' 2>/dev/null || echo missing)"
		case "$state" in missing|exited|dead) return 1 ;; esac
		health="$(timeout 20 docker inspect tellegen --format '{{if .State.Health}}{{.State.Health.Status}}{{else}}none{{end}}' 2>/dev/null || echo none)"
		if [ "$health" = healthy ] || { [ "$health" = none ] && [ "$state" = running ]; }; then break; fi
		[ "$attempt" -lt 150 ] || return 1
		sleep 5
	done
	actual="$(timeout 20 docker inspect tellegen --format '{{.Config.Image}}' 2>/dev/null || true)"
	[ "$actual" = "$expected" ] || return 1
	for attempt in $(seq 1 90); do
		payload="$(curl --proto '=http' --connect-timeout 3 --max-time 10 -fsS \
			http://127.0.0.1:8000/api/health 2>/dev/null || true)"
		if health_payload_ok "$payload"; then echo "==> tellegen host health ok"; return 0; fi
		[ "$attempt" -lt 90 ] || break
		sleep 10
	done
	return 1
}

logs() {
	[ "${#compose[@]}" -eq 0 ] || timeout 60 "${compose[@]}" logs --tail=200 tellegen >&2 || true
	timeout 60 docker logs --tail=200 tellegen >&2 || true
}

remove_snapshot() {
	local directory="$1"
	case "$directory" in "$SNAPSHOTS_DIR"/snapshot.*) ;; *) return 1 ;; esac
	rm -f -- "$directory/.env" "$directory/docker-compose.prod.yml" \
		"$directory/docker-compose.edge.yml"
	rmdir -- "$directory" 2>/dev/null || true
}

create_snapshot() {
	local image="$1" data_dir="$2" bundle="$3" building final suffix
	need_regular_file "$bundle/docker-compose.prod.yml" || return 1
	need_regular_file "$bundle/docker-compose.edge.yml" || return 1
	building="$(mktemp -d "$SNAPSHOTS_DIR/.building.XXXXXX")" || return 1
	suffix="${building##*.}"
	final="$SNAPSHOTS_DIR/snapshot.$suffix"
	if ! write_env_file "$building/.env" "$image" "$data_dir" \
		|| ! install -m 644 -- "$bundle/docker-compose.prod.yml" "$building/docker-compose.prod.yml" \
		|| ! install -m 644 -- "$bundle/docker-compose.edge.yml" "$building/docker-compose.edge.yml" \
		|| ! mv -T -- "$building" "$final"; then
		rm -f -- "$building/.env" "$building/docker-compose.prod.yml" "$building/docker-compose.edge.yml"
		rmdir -- "$building" 2>/dev/null || true
		return 1
	fi
	compose_for "$final/.env" "$final"
	if ! validate_compose; then remove_snapshot "$final"; return 1; fi
	CREATED_SNAPSHOT="${final##*/}"
}

discover_legacy_bundle() {
	local label path canonical directory saw_prod=0 saw_edge=0
	label="$(timeout 20 docker inspect tellegen \
		--format '{{ index .Config.Labels "com.docker.compose.project.config_files" }}' \
		2>/dev/null || true)"
	if [ -z "$label" ] || [ "$label" = '<no value>' ]; then
		LEGACY_BUNDLE="$DEPLOY_ROOT/deploy"
	else
		IFS=, read -r -a files <<< "$label"
		[ "${#files[@]}" -eq 2 ] || return 1
		LEGACY_BUNDLE=""
		for path in "${files[@]}"; do
			validate_portable_absolute_path "$path" "legacy compose file" || return 1
			need_regular_file "$path" || return 1
			canonical="$(cd -- "$(dirname -- "$path")" && pwd -P)/$(basename -- "$path")"
			case "$canonical" in "$DEPLOY_ROOT"/*) ;; *) return 1 ;; esac
			directory="${canonical%/*}"
			[ -z "$LEGACY_BUNDLE" ] && LEGACY_BUNDLE="$directory"
			[ "$LEGACY_BUNDLE" = "$directory" ] || return 1
			case "${canonical##*/}" in
				docker-compose.prod.yml) saw_prod=1 ;;
				docker-compose.edge.yml) saw_edge=1 ;;
				*) return 1 ;;
			esac
		done
		[ "$saw_prod" -eq 1 ] && [ "$saw_edge" -eq 1 ] || return 1
	fi
	need_regular_file "$LEGACY_BUNDLE/docker-compose.prod.yml" || return 1
	need_regular_file "$LEGACY_BUNDLE/docker-compose.edge.yml" || return 1
}

bootstrap_last_known_good() {
	if [ -e "$LAST_GOOD_POINTER" ]; then
		read_pointer "$LAST_GOOD_POINTER" || die "invalid last-known-good pointer"
		return
	fi
	[ -e "$CURRENT_ENV" ] || return 0
	load_env_file "$CURRENT_ENV" || die "invalid current deployment state"
	local image="$STATE_IMAGE" data_dir="$STATE_DATA_DIR"
	if current_container_is_healthy "$image"; then
		discover_legacy_bundle || die "cannot locate the running deployment's Compose files"
		create_snapshot "$image" "$data_dir" "$LEGACY_BUNDLE" || die "cannot snapshot the running deployment"
		write_pointer "$CREATED_SNAPSHOT" "$LAST_GOOD_POINTER" || die "cannot record last known good"
		echo "==> Migrated the healthy running deployment into durable state"
	fi
}

restore_last_known_good() {
	local name directory image
	read_pointer "$LAST_GOOD_POINTER" || return 1
	name="$POINTER_NAME"
	directory="$POINTER_DIR"
	load_env_file "$directory/.env" || return 1
	image="$STATE_IMAGE"
	compose_for "$directory/.env" "$directory"
	validate_compose || return 1
	timeout 900 "${compose[@]}" pull tellegen || return 1
	timeout 300 "${compose[@]}" up -d || return 1
	wait_for_local_health "$image" || return 1
	atomic_copy "$directory/.env" "$CURRENT_ENV" 600 || return 1
	echo "==> Restored $image from $name"
}

clear_pending() {
	local expected="$1" lkg=""
	read_pointer "$PENDING_POINTER" || return 1
	[ "$POINTER_NAME" = "$expected" ] || return 1
	if [ -e "$LAST_GOOD_POINTER" ]; then
		read_pointer "$LAST_GOOD_POINTER" || return 1
		lkg="$POINTER_NAME"
	fi
	rm -f -- "$PENDING_POINTER"
	[ "$expected" = "$lkg" ] || remove_snapshot "$SNAPSHOTS_DIR/$expected"
}

remove_first_candidate() {
	local name="$1" image="$2" actual
	actual="$(timeout 20 docker inspect tellegen --format '{{.Config.Image}}' 2>/dev/null || true)"
	if [ "$actual" = "$image" ]; then timeout 120 docker rm -f tellegen >/dev/null || return 1; fi
	if [ -e "$CURRENT_ENV" ] && load_env_file "$CURRENT_ENV" && [ "$STATE_IMAGE" = "$image" ]; then
		rm -f -- "$CURRENT_ENV"
	fi
	clear_pending "$name"
}

rollback_pending() {
	local expected="$1" name directory image actual
	if [ ! -e "$PENDING_POINTER" ]; then
		if [ -n "$expected" ] && [ -e "$LAST_GOOD_POINTER" ]; then
			read_pointer "$LAST_GOOD_POINTER" || return 1
			load_env_file "$POINTER_DIR/.env" || return 1
			if current_container_is_healthy "$STATE_IMAGE"; then
				if [ "$STATE_IMAGE" = "$expected" ]; then
					echo "==> Deployment was already promoted"
				else
					echo "==> Candidate was already rolled back"
				fi
				return 0
			fi
		fi
		if [ -n "$expected" ] && [ ! -e "$LAST_GOOD_POINTER" ]; then
			actual="$(timeout 20 docker inspect tellegen --format '{{.Config.Image}}' 2>/dev/null || true)"
			if [ "$actual" != "$expected" ]; then
				echo "==> No pending candidate is active"
				return 0
			fi
		fi
		[ -z "$expected" ] || return 1
		restore_last_known_good
		return
	fi
	read_pointer "$PENDING_POINTER" || return 1
	name="$POINTER_NAME"
	directory="$POINTER_DIR"
	load_env_file "$directory/.env" || return 1
	image="$STATE_IMAGE"
	[ -z "$expected" ] || [ "$image" = "$expected" ] || return 1
	if [ -e "$LAST_GOOD_POINTER" ]; then
		restore_last_known_good || return 1
	else
		remove_first_candidate "$name" "$image" || return 1
		return 0
	fi
	clear_pending "$name"
}

recover_interrupted_deploy() {
	local pending_name lkg_name="" pending_dir pending_image
	[ -e "$PENDING_POINTER" ] || return 0
	read_pointer "$PENDING_POINTER" || die "invalid pending deployment pointer"
	pending_name="$POINTER_NAME"
	pending_dir="$POINTER_DIR"
	load_env_file "$pending_dir/.env" || die "invalid pending deployment state"
	pending_image="$STATE_IMAGE"
	if [ -e "$LAST_GOOD_POINTER" ]; then
		read_pointer "$LAST_GOOD_POINTER" || die "invalid last-known-good pointer"
		lkg_name="$POINTER_NAME"
	fi
	if [ "$pending_name" = "$lkg_name" ] && current_container_is_healthy "$pending_image"; then
		atomic_copy "$pending_dir/.env" "$CURRENT_ENV" 600 || die "cannot finalize promoted state"
		rm -f -- "$PENDING_POINTER"
		echo "==> Finalized an interrupted promotion"
		return
	fi
	echo "==> Recovering an interrupted unpromoted deployment"
	rollback_pending "$pending_image" || die "interrupted deployment recovery failed"
}

cleanup_snapshots() {
	local lkg="" pending="" directory name
	if [ -e "$LAST_GOOD_POINTER" ]; then read_pointer "$LAST_GOOD_POINTER" || return 1; lkg="$POINTER_NAME"; fi
	if [ -e "$PENDING_POINTER" ]; then read_pointer "$PENDING_POINTER" || return 1; pending="$POINTER_NAME"; fi
	while IFS= read -r -d '' directory; do
		name="${directory##*/}"
		[ "$name" = "$lkg" ] || [ "$name" = "$pending" ] || remove_snapshot "$directory"
	done < <(find "$SNAPSHOTS_DIR" -mindepth 1 -maxdepth 1 -type d -name 'snapshot.*' -print0)
}

cleanup_old_bundles() {
	local staging="$DEPLOY_ROOT/.deploy-staging" current candidate
	case "$DEPLOY_BUNDLE_DIR" in "$staging"/*/deploy) ;; *) return ;; esac
	[ -d "$staging" ] || return
	current="${DEPLOY_BUNDLE_DIR%/deploy}"
	while IFS= read -r -d '' candidate; do
		[ "$candidate" = "$current" ] && continue
		rm -f -- "$candidate/deploy/docker-compose.prod.yml" \
			"$candidate/deploy/docker-compose.edge.yml" "$candidate/deploy/remote-deploy.sh"
		rmdir -- "$candidate/deploy" "$candidate" 2>/dev/null || true
	done < <(find "$staging" -mindepth 1 -maxdepth 1 -type d -mtime +7 -print0)
}

for command in docker curl flock timeout; do command -v "$command" >/dev/null 2>&1 || die "$command is not installed"; done
need_regular_file "$DEPLOY_BUNDLE_DIR/docker-compose.prod.yml" || exit 1
need_regular_file "$DEPLOY_BUNDLE_DIR/docker-compose.edge.yml" || exit 1
[ ! -L "$STATE_ROOT" ] && { [ ! -e "$STATE_ROOT" ] || [ -d "$STATE_ROOT" ]; } || die "unsafe deployment state directory"
[ ! -L "$SNAPSHOTS_DIR" ] && { [ ! -e "$SNAPSHOTS_DIR" ] || [ -d "$SNAPSHOTS_DIR" ]; } || die "unsafe snapshot directory"
install -m 700 -d "$STATE_ROOT" "$SNAPSHOTS_DIR"
safe_destination "$DEPLOY_ROOT/.deploy.lock" || die "unsafe deploy lock"
timeout 30 docker compose version >/dev/null || die "docker compose plugin is unavailable"
timeout 30 docker network inspect edge >/dev/null || die "external Docker network edge is missing"

exec 9>"$DEPLOY_ROOT/.deploy.lock"
flock -w 900 9 || die "timed out waiting for another deployment"
compose=()
umask 077

case "$MODE" in
	rollback)
		if rollback_pending "$EXPECTED_CURRENT_IMAGE"; then
			cleanup_snapshots
			cleanup_old_bundles
			echo "==> Rollback is healthy"
			exit 0
		fi
		logs
		die "rollback failed"
		;;
	promote)
		read_pointer "$PENDING_POINTER" || die "no pending deployment to promote"
		pending_name="$POINTER_NAME"
		pending_dir="$POINTER_DIR"
		load_env_file "$pending_dir/.env" || die "invalid pending deployment state"
		[ "$STATE_IMAGE" = "$IMAGE" ] || die "pending deployment changed; refusing promotion"
		compose_for "$pending_dir/.env" "$pending_dir"
		validate_compose || die "pending compose config is invalid"
		wait_for_local_health "$IMAGE" || { logs; die "pending deployment is not healthy"; }
		write_pointer "$pending_name" "$LAST_GOOD_POINTER" || die "cannot promote deployment"
		atomic_copy "$pending_dir/.env" "$CURRENT_ENV" 600 || die "cannot record current deployment"
		rm -f -- "$PENDING_POINTER"
		cleanup_snapshots
		cleanup_old_bundles
		echo "==> Promoted $IMAGE"
		exit 0
		;;
esac

recover_interrupted_deploy
bootstrap_last_known_good

if [ -z "$DATA_DIR" ]; then DATA_DIR="$DEPLOY_ROOT/data"; fi
validate_data_dir "$DATA_DIR" || exit 1
DATA_DIR="$VALIDATED_DATA_DIR"
[ -n "$(find "$DATA_DIR" -mindepth 1 -maxdepth 1 -type d -print -quit 2>/dev/null)" ] \
	|| die "no case data staged under $DATA_DIR"

create_snapshot "$IMAGE" "$DATA_DIR" "$DEPLOY_BUNDLE_DIR" || die "cannot create candidate deployment snapshot"
candidate_name="$CREATED_SNAPSHOT"
candidate_dir="$SNAPSHOTS_DIR/$candidate_name"
compose_for "$candidate_dir/.env" "$candidate_dir"
echo "==> Pulling $IMAGE"
timeout 900 "${compose[@]}" pull tellegen || { remove_snapshot "$candidate_dir"; die "cannot pull candidate"; }
write_pointer "$candidate_name" "$PENDING_POINTER" || { remove_snapshot "$candidate_dir"; die "cannot record pending deployment"; }

echo "==> Starting tellegen"
if ! timeout 300 "${compose[@]}" up -d || ! wait_for_local_health "$IMAGE"; then
	echo "==> Candidate failed local health; restoring previous deployment" >&2
	logs
	rollback_pending "$IMAGE" || echo "==> No healthy rollback could be completed" >&2
	exit 1
fi
atomic_copy "$candidate_dir/.env" "$CURRENT_ENV" 600 || { rollback_pending "$IMAGE" || true; die "cannot record current candidate state"; }
cleanup_snapshots
cleanup_old_bundles
echo "==> Candidate passed local health; awaiting public-health promotion"
