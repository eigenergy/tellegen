#!/usr/bin/env python3
"""Fail closed unless a version generator changed only release outputs."""

from __future__ import annotations

import json
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


PRE_STATE = ".changeset/pre.json"
CHANGESET = re.compile(r"\.changeset/([A-Za-z0-9_-]+)\.md")
PRE_CHANGESET = re.compile(r"\.changeset/pre/([A-Za-z0-9_-]+)\.md")
DELETED = {" D", "D "}
# A changeset moved in pre mode is untracked when the version patch job sees
# it, intent-to-add while that job seals the patch, and staged once the
# version pull request job has applied it.
ADDED = {"??", " A", "A "}


def fail(message: str) -> "NoReturn":
    raise SystemExit(message)


def status_entries() -> list[tuple[str, str]]:
    # Without --no-renames, a staged move into .changeset/pre/ is one rename
    # entry carrying two paths.
    output = subprocess.check_output(
        [
            "git",
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--no-renames",
        ]
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


def committed(path: str) -> bytes | None:
    if not subprocess.check_output(["git", "ls-tree", "-z", "HEAD", "--", path]):
        return None
    return subprocess.check_output(["git", "show", f"HEAD:{path}"])


def committed_pre_mode() -> str | None:
    """`pre` or `exit` while Changesets prerelease mode is committed.

    Read from HEAD: the generator could rewrite the working tree copy.
    """
    raw = committed(PRE_STATE)
    if raw is None:
        return None
    try:
        state = json.loads(raw)
    except ValueError as error:
        fail(f"cannot parse committed {PRE_STATE}: {error}")
    if not isinstance(state, dict) or state.get("mode") not in {"pre", "exit"}:
        fail(f"committed {PRE_STATE} is neither in pre nor in exit mode")
    # The tag names the prerelease and the npm dist-tag it publishes under.
    tag = state.get("tag")
    if not isinstance(tag, str) or not tag or tag == "latest":
        fail(f"committed {PRE_STATE} needs a prerelease tag other than latest")
    return state["mode"]


def validate_npm(entries: list[tuple[str, str]]) -> None:
    # `changeset version` rewrites the dependency range of every workspace
    # consumer, including the three in the `ignore` list whose own versions it
    # leaves alone. Their manifests are therefore expected output, not an
    # escape: the release surfaces below still gate what may change.
    allowed_modified = {
        "package-lock.json",
        "packages/engine/CHANGELOG.md",
        "packages/engine/package.json",
        "packages/engine/src/generated/api-types.ts",
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
    # In pre mode, Changesets 3 leaves pre.json alone and moves each consumed
    # changeset into .changeset/pre/ unchanged, so that exiting can still
    # release it. Exiting removes pre.json and everything in .changeset/pre/.
    pre_mode = committed_pre_mode()
    changed_manifests: set[str] = set()
    deleted_changesets: set[str] = set()
    moved_changesets: set[str] = set()
    deleted_pre_changesets = 0
    deleted_pre_state = False

    for status, path in entries:
        if path in allowed_modified:
            if status not in {" M", "M "}:
                fail(f"unexpected status for generated file {path}: {status!r}")
            require_regular_nonexecutable(path)
            if path in package_manifests:
                changed_manifests.add(path)
            continue
        match = CHANGESET.fullmatch(path)
        if match:
            if path == ".changeset/README.md" or status not in DELETED:
                fail(f"unexpected changeset change {path}: {status!r}")
            deleted_changesets.add(match[1])
            continue
        match = PRE_CHANGESET.fullmatch(path)
        if match and pre_mode == "pre" and status in ADDED:
            require_regular_nonexecutable(path)
            with open(path, "rb") as moved:
                if moved.read() != committed(f".changeset/{match[1]}.md"):
                    fail(f"pre mode changed or invented a changeset: {path}")
            moved_changesets.add(match[1])
            continue
        if match and pre_mode == "exit" and status in DELETED:
            deleted_pre_changesets += 1
            continue
        if path == PRE_STATE and pre_mode == "exit" and status in DELETED:
            deleted_pre_state = True
            continue
        fail(f"npm versioning changed an unapproved path: {path} ({status!r})")

    if not changed_manifests:
        fail("npm versioning did not change a package manifest")
    if pre_mode == "pre" and moved_changesets != deleted_changesets:
        fail("pre mode did not move every consumed changeset into .changeset/pre/")
    if pre_mode == "exit" and not deleted_pre_state:
        fail(f"exiting pre mode did not remove {PRE_STATE}")
    if not deleted_changesets and deleted_pre_changesets == 0:
        fail("npm versioning did not consume a changeset")


def validate_crate(entries: list[tuple[str, str]]) -> None:
    changelog = {"crates/tellegen/CHANGELOG.md"}
    bump = changelog | {str(LOCKFILE), str(ENGINE_MANIFEST)}
    # After release-plz bumps the engine, sync_python_version.py moves the
    # wheel onto the same version, unless the wheel already carries it.
    synchronized = bump | {str(PYTHON_MANIFEST)}
    actual = {path for _, path in entries}
    # A version already raised by hand on main (above the crates.io release)
    # leaves release-plz only the changelog to write.
    if actual not in (changelog, bump, synchronized):
        fail(
            "crate versioning changed the wrong paths: "
            f"expected {sorted(bump)!r}, optionally {str(PYTHON_MANIFEST)!r}, "
            f"or only the changelog; got {sorted(actual)!r}"
        )
    for status, path in entries:
        if status not in {" M", "M "}:
            fail(f"unexpected status for generated file {path}: {status!r}")
        require_regular_nonexecutable(path)

    # Whichever paths changed, both manifests and both lock entries end on
    # the engine's version.
    engine_version = package_version(read_regular(ENGINE_MANIFEST), "tellegen")
    wheel = read_regular(PYTHON_MANIFEST)
    if package_version(wheel, "tellegen-py") != engine_version:
        fail("the wheel version does not match the engine")
    check_lock_versions(read_regular(LOCKFILE), engine_version)
    if str(PYTHON_MANIFEST) in actual:
        # A fourth path is not permission to change Python dependencies, build
        # settings, or comments. Check the exact version-only edit against the
        # checkout base, both before sealing and after applying the staged patch.
        original = committed(str(PYTHON_MANIFEST))
        if original is None:
            fail(f"{PYTHON_MANIFEST} is not in the checkout base")
        try:
            original_text = original.decode("utf-8")
        except UnicodeError as error:
            fail(f"cannot read the committed {PYTHON_MANIFEST}: {error}")
        package_version(original_text, "tellegen-py")
        if wheel != replace_manifest_version(original_text, engine_version):
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
