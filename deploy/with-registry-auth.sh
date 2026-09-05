#!/usr/bin/env bash
# Read a registry token from stdin and keep its Docker configuration private
# to this command. The caller's Docker credentials remain untouched.
set -euo pipefail

if [ "$#" -lt 2 ] || [ -z "$1" ]; then
	echo "usage: with-registry-auth.sh <registry-user> <command> [args...]" >&2
	exit 2
fi
registry_user="$1"
shift

umask 077
registry_config="$(mktemp -d "${TMPDIR:-/tmp}/tellegen-registry.XXXXXX")"
trap 'rm -rf -- "$registry_config"' EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM
export DOCKER_CONFIG="$registry_config"

docker login ghcr.io --username "$registry_user" --password-stdin
"$@" </dev/null
