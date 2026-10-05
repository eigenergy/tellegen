#!/usr/bin/env python3
"""Fail closed unless a version generator changed only release outputs."""

from __future__ import annotations

import os
import re
import stat
import subprocess
import sys

from sync_python_version import (
    ENGINE_MANIFEST,
    LOCKFILE,
    PYTHON_MANIFEST,
    check_lock_versions,
    package_version,
    read_regular,
    replace_manifest_version,
)


def fail(message: str) -> "NoReturn":
    raise SystemExit(message)


def status_entries() -> list[tuple[str, str]]:
    output = subprocess.check_output(
        ["git", "status", "--porcelain=v1", "-z", "--untracked-files=all"]
    )
    entries: list[tuple[str, str]] = []
    for raw in output.split(b"\0"):
        if not raw:
            continue
        if len(raw) < 4 or raw[2:3] != b" ":
            fail("unexpected git status entry")
        try:
            status = raw[:2].decode("ascii")
            path = os.fsdecode(raw[3:])
        except UnicodeError as error:
            fail(f"invalid git status entry: {error}")
        entries.append((status, path))
    return entries


def require_regular_nonexecutable(path: str) -> None:
    try:
        mode = os.lstat(path).st_mode
    except OSError as error:
        fail(f"cannot inspect generated file {path}: {error}")
    if not stat.S_ISREG(mode):
        fail(f"generated path is not a regular file: {path}")
    if mode & 0o111:
        fail(f"generated file became executable: {path}")


def validate_npm(entries: list[tuple[str, str]]) -> None:
    # `changeset version` rewrites the dependency range of every workspace
    # consumer, including the three in the `ignore` list whose own versions it
    # leaves alone. Their manifests are therefore expected output, not an
    # escape: the release surfaces below still gate what may change.
    allowed_modified = {
        "package-lock.json",
        "packages/engine/CHANGELOG.md",
        "packages/engine/package.json",
        "packages/engine/src/generated/contracts.ts",
        "packages/svelte/CHANGELOG.md",
        "packages/svelte/package.json",
        "packages/webmcp/CHANGELOG.md",
        "packages/webmcp/package.json",
        "apps/web/package.json",
        "examples/browser-minimal/package.json",
        "examples/svelte-minimal/package.json",
    }
    package_manifests = {
        "packages/engine/package.json",
        "packages/svelte/package.json",
        "packages/webmcp/package.json",
    }
    changed_manifests: set[str] = set()
    deleted_changesets = 0

    for status, path in entries:
        if path in allowed_modified:
            if status not in {" M", "M "}:
                fail(f"unexpected status for generated file {path}: {status!r}")
            require_regular_nonexecutable(path)
            if path in package_manifests:
                changed_manifests.add(path)
            continue
        if re.fullmatch(r"\.changeset/[A-Za-z0-9_-]+\.md", path):
            if path == ".changeset/README.md" or status not in {" D", "D "}:
                fail(f"unexpected changeset change {path}: {status!r}")
            deleted_changesets += 1
            continue
        fail(f"npm versioning changed an unapproved path: {path} ({status!r})")

    if not changed_manifests:
        fail("npm versioning did not change a package manifest")
    if deleted_changesets == 0:
        fail("npm versioning did not consume a changeset")


def validate_crate(entries: list[tuple[str, str]]) -> None:
    required = {
        "Cargo.lock",
        "crates/tellegen/CHANGELOG.md",
        "crates/tellegen/Cargo.toml",
    }
    actual = {path for _, path in entries}
    # The wheel may already carry the next engine version. Keep the original
    # three release outputs mandatory, with this one precisely bounded addition.
    allowed = required | {str(PYTHON_MANIFEST)}
    if actual not in (required, allowed):
        fail(
            "crate versioning changed the wrong paths: "
            f"expected {sorted(required)!r}, optionally {str(PYTHON_MANIFEST)!r}; "
            f"got {sorted(actual)!r}"
        )
    for status, path in entries:
        if status not in {" M", "M "}:
            fail(f"unexpected status for generated file {path}: {status!r}")
        require_regular_nonexecutable(path)

    engine_version = package_version(read_regular(ENGINE_MANIFEST), "tellegen")
    wheel = read_regular(PYTHON_MANIFEST)
    if package_version(wheel, "tellegen-py") != engine_version:
        fail("the wheel version does not match the engine")
    check_lock_versions(read_regular(LOCKFILE), engine_version)
    if str(PYTHON_MANIFEST) in actual:
        # A fourth path is not permission to change Python dependencies, build
        # settings, or comments. Check the exact version-only edit against the
        # checkout base, both before sealing and after applying the staged patch.
        original = subprocess.check_output(
            ["git", "show", f"HEAD:{PYTHON_MANIFEST}"]
        ).decode("utf-8")
        package_version(original, "tellegen-py")
        if wheel != replace_manifest_version(original, engine_version):
            fail("crate versioning may only synchronize the Python package version")


def main() -> None:
    if len(sys.argv) != 2 or sys.argv[1] not in {"npm", "crate"}:
        fail("usage: validate-version-changes.py npm|crate")
    entries = status_entries()
    if not entries:
        fail("version generator produced no changes")
    if sys.argv[1] == "npm":
        validate_npm(entries)
    else:
        validate_crate(entries)


if __name__ == "__main__":
    main()
