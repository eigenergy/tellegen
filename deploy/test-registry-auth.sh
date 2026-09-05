#!/usr/bin/env bash
set -euo pipefail

root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
test_root="$(mktemp -d /tmp/tellegen-registry-test.XXXXXX)"
trap 'rm -rf -- "$test_root"' EXIT
mkdir -p "$test_root/bin" "$test_root/existing"
printf '%s\n' 'existing credentials' > "$test_root/existing/config.json"
export TEST_REGISTRY_ROOT="$test_root"

cat > "$test_root/bin/docker" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
[ "$*" = 'login ghcr.io --username release-user --password-stdin' ]
[ "$(stat -c %a "$DOCKER_CONFIG")" = 700 ]
[ "$(cat)" = 'test-registry-token' ]
printf '%s\n' "$DOCKER_CONFIG" > "$TEST_REGISTRY_ROOT/config-path"
printf '%s\n' 'temporary credentials' > "$DOCKER_CONFIG/config.json"
exit "${TEST_LOGIN_EXIT:-0}"
EOF
cat > "$test_root/bin/check-command" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
[ -f "$DOCKER_CONFIG/config.json" ]
[ "$1" = 'argument with spaces' ]
[ -z "$(cat)" ]
touch "$TEST_REGISTRY_ROOT/command-ran"
exit "${TEST_COMMAND_EXIT:-0}"
EOF
chmod +x "$test_root/bin/"*
export PATH="$test_root/bin:$PATH"
export DOCKER_CONFIG="$test_root/existing"

run_case() {
	local login_exit="$1" command_exit="$2" expected_exit="$3" actual_exit=0
	rm -f "$test_root/command-ran"
	printf '%s' 'test-registry-token' |
		TEST_LOGIN_EXIT="$login_exit" TEST_COMMAND_EXIT="$command_exit" \
		bash "$root/with-registry-auth.sh" release-user check-command 'argument with spaces' \
		> "$test_root/output" 2>&1 || actual_exit="$?"
	[ "$actual_exit" -eq "$expected_exit" ]
	[ ! -e "$(cat "$test_root/config-path")" ]
	[ "$(cat "$DOCKER_CONFIG/config.json")" = 'existing credentials' ]
	! grep -q 'test-registry-token' "$test_root/output"
	if [ "$login_exit" -eq 0 ]; then
		[ -f "$test_root/command-ran" ]
	else
		[ ! -e "$test_root/command-ran" ]
	fi
}

run_case 0 0 0
run_case 0 37 37
run_case 42 0 42
echo 'Registry authentication: success, command failure, and login failure passed.'
