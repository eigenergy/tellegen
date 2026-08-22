#!/usr/bin/env bash
set -Eeuo pipefail

# Every check below is a bare `[ ... ]` under `set -e`, so a break otherwise exits
# 1 with no output and CI shows only the exit code. Name the line that failed.
trap 'echo "assertion failed at ${BASH_SOURCE[0]}:$LINENO" >&2' ERR

REPOSITORY_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
TEST_ROOT="$(mktemp -d /tmp/tellegen-deploy-test.XXXXXX)"
trap 'rm -rf -- "$TEST_ROOT"' EXIT

MOCK_BIN="$TEST_ROOT/bin"
mkdir -p "$MOCK_BIN"

cat > "$MOCK_BIN/timeout" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" == --signal=* ]]; then shift; fi
shift
exec "$@"
EOF

cat > "$MOCK_BIN/flock" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF

cat > "$MOCK_BIN/curl" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' '{"status":"ok","cases":["case"]}'
EOF

cat > "$MOCK_BIN/docker" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
state="${MOCK_STATE_DIR:?}"
mkdir -p "$state"

if [ "${1:-}" = compose ]; then
	shift
	if [ "${1:-}" = version ]; then exit 0; fi
	env_file=""
	files=()
	command=""
	services=false
	while [ "$#" -gt 0 ]; do
		case "$1" in
			--env-file) env_file="$2"; shift 2 ;;
			-f) files+=("$2"); shift 2 ;;
			-p) shift 2 ;;
			config)
				command=config
				shift
				[ "${1:-}" = --services ] && services=true
				break
				;;
			pull|up|logs) command="$1"; shift; break ;;
			*) shift ;;
		esac
	done
	case "$command" in
		config) $services && echo tellegen; exit 0 ;;
		pull|logs) exit 0 ;;
		up)
			image="$(sed -n 's/^TELLEGEN_IMAGE=//p' "$env_file")"
			printf '%s\n' "$image" > "$state/current-image"
			(IFS=,; printf '%s\n' "${files[*]}") > "$state/config-files"
			printf '%s %s\n' "$image" "$(cksum "${files[0]}" | awk '{print $1}')" >> "$state/up-log"
			exit 0
			;;
	esac
fi

if [ "${1:-}" = network ] && [ "${2:-}" = inspect ]; then exit 0; fi
if [ "${1:-}" = logs ]; then exit 0; fi
if [ "${1:-}" = rm ]; then
	rm -f -- "$state/current-image" "$state/config-files"
	exit 0
fi
if [ "${1:-}" = inspect ]; then
	template="${4:-}"
	image="$(cat "$state/current-image" 2>/dev/null || true)"
	fail="$(cat "$state/fail-image" 2>/dev/null || true)"
	case "$template" in
		*Config.Image*) printf '%s\n' "$image" ;;
		*project.config_files*) cat "$state/config-files" 2>/dev/null || printf '%s\n' '<no value>' ;;
		*NetworkSettings.Networks*) [ -n "$image" ] && printf '%s\n' edge ;;
		*State.Status*)
			if [ -z "$image" ]; then printf '%s\n' missing
			elif [ "$image" = "$fail" ]; then printf '%s\n' exited
			else printf '%s\n' running
			fi
			;;
		*State.Health*) printf '%s\n' healthy ;;
	esac
	exit 0
fi

echo "unexpected docker invocation: $*" >&2
exit 1
EOF
chmod +x "$MOCK_BIN"/*

image() { printf 'ghcr.io/eigenergy/tellegen@sha256:%064d' "$1"; }

make_root() {
	local root="$1" marker="$2"
	mkdir -p "$root/deploy" "$root/data/case" "$root/mock"
	printf 'services: # %s\n' "$marker" > "$root/deploy/docker-compose.prod.yml"
	printf 'services: # edge-%s\n' "$marker" > "$root/deploy/docker-compose.edge.yml"
}

run_script() {
	local root="$1"
	shift
	PATH="$MOCK_BIN:$PATH" MOCK_STATE_DIR="$root/mock" DEPLOY_ROOT="$root" \
		TELLEGEN_DEPLOY_BUNDLE_DIR="$root/deploy" \
		bash "$REPOSITORY_ROOT/deploy/remote-deploy.sh" "$@"
}

assert_image() {
	local root="$1" expected="$2"
	[ "$(cat "$root/mock/current-image")" = "$expected" ]
}

# First deployment, promotion, interrupted candidate recovery, and rollback.
root="$TEST_ROOT/primary"
make_root "$root" v1
image_a="$(image 1)"
image_b="$(image 2)"
image_c="$(image 3)"
image_d="$(image 4)"
run_script "$root" "$image_a" "$root/data"
[ -f "$root/.deploy-state/pending" ] && [ ! -e "$root/.deploy-state/last-known-good" ]
run_script "$root" --promote "$image_a"
[ ! -e "$root/.deploy-state/pending" ]
v1_hash="$(cksum "$root/deploy/docker-compose.prod.yml" | awk '{print $1}')"

printf 'services: # v2\n' > "$root/deploy/docker-compose.prod.yml"
run_script "$root" "$image_b" "$root/data"
run_script "$root" "$image_c" "$root/data"
assert_image "$root" "$image_c"
[ "$(grep -Fc "$image_a $v1_hash" "$root/mock/up-log")" -eq 2 ]
run_script "$root" --promote "$image_c"

printf '%s\n' "$image_d" > "$root/mock/fail-image"
if run_script "$root" "$image_d" "$root/data"; then
	echo "failed candidate unexpectedly succeeded" >&2
	exit 1
fi
assert_image "$root" "$image_c"
[ ! -e "$root/.deploy-state/pending" ]

# A failed first deployment leaves neither a container nor pending state.
first="$TEST_ROOT/first-failure"
make_root "$first" first
printf '%s\n' "$image_d" > "$first/mock/fail-image"
if run_script "$first" "$image_d" "$first/data"; then exit 1; fi
[ ! -e "$first/mock/current-image" ]
[ ! -e "$first/.deploy-state/pending" ]

# Migrate a healthy legacy deployment using the exact labeled Compose bundle.
legacy="$TEST_ROOT/legacy"
make_root "$legacy" legacy
printf 'TELLEGEN_IMAGE=%s\nTELLEGEN_DATA_DIR=%s\n' "$image_a" "$legacy/data" > "$legacy/.env"
printf '%s\n' "$image_a" > "$legacy/mock/current-image"
printf '%s,%s\n' "$legacy/deploy/docker-compose.prod.yml" \
	"$legacy/deploy/docker-compose.edge.yml" > "$legacy/mock/config-files"
run_script "$legacy" "$image_b" "$legacy/data"
[ -f "$legacy/.deploy-state/last-known-good" ]
run_script "$legacy" --rollback "$image_b"
assert_image "$legacy" "$image_a"

# A host whose `.env` this script did not write must still deploy. The strict
# two-key parse rejects any other shape, and adoption is best effort, so an
# unreadable current state degrades to "no rollback target" rather than aborting
# the deployment outright.
unadoptable="$TEST_ROOT/unadoptable"
make_root "$unadoptable" unadoptable
printf 'TELLEGEN_IMAGE=%s\nCOMPOSE_PROJECT_NAME=tellegen\n' "$image_a" > "$unadoptable/.env"
printf '%s\n' "$image_a" > "$unadoptable/mock/current-image"
run_script "$unadoptable" "$image_b" "$unadoptable/data"
assert_image "$unadoptable" "$image_b"
# Nothing was adopted, so this first deploy has no rollback target; promotion is
# what establishes one, and every later deploy recovers normally from there.
[ -f "$unadoptable/.deploy-state/pending" ] && [ ! -e "$unadoptable/.deploy-state/last-known-good" ]
run_script "$unadoptable" --promote "$image_b"
[ ! -e "$unadoptable/.deploy-state/pending" ]
[ -f "$unadoptable/.deploy-state/last-known-good" ]

echo "remote deploy state-machine tests passed"
