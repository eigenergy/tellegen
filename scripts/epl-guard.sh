#!/usr/bin/env bash
set -euo pipefail

check_no_pounce() {
	local label="$1"
	shift
	local tree
	if ! tree="$(cargo tree "$@" 2>&1)"; then
		printf '%s\n' "$tree" >&2
		echo "cargo tree failed for $label" >&2
		return 1
	fi
	if grep -qi pounce <<<"$tree"; then
		echo "EPL pounce backend leaked into: $label" >&2
		return 1
	fi
}

check_has_pounce() {
	local label="$1"
	shift
	local tree
	if ! tree="$(cargo tree "$@" 2>&1)"; then
		printf '%s\n' "$tree" >&2
		echo "cargo tree failed for $label" >&2
		return 1
	fi
	if ! grep -qi pounce <<<"$tree"; then
		echo "acopf feature did not include the pinned pounce backend: $label" >&2
		return 1
	fi
}

check_no_pounce tellegen-default -p tellegen
check_no_pounce tellegen-wasm -p tellegen-wasm --target wasm32-unknown-unknown
check_no_pounce tellegen-server -p tellegen-server
check_no_pounce tellegen-cli -p tellegen-cli
check_no_pounce tellegen-py -p tellegen-py
check_has_pounce tellegen-acopf -p tellegen --no-default-features --features acopf
check_has_pounce tellegen-acopf-wasi -p tellegen-acopf-wasi
echo "ok: pounce is confined to the opt-in tellegen/acopf feature and development WASI adapter"
