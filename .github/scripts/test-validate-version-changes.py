#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import os
import tempfile
from pathlib import Path


SCRIPT = Path(__file__).with_name("validate-version-changes.py")
SPEC = importlib.util.spec_from_file_location("version_validator", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
validator = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(validator)


def write(path: str) -> None:
    target = Path(path)
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text("generated\n")


def rejects(callable_) -> None:
    try:
        callable_()
    except SystemExit:
        return
    raise AssertionError("invalid generated changes were accepted")


with tempfile.TemporaryDirectory() as directory:
    os.chdir(directory)
    npm_entries = [
        (" M", "package-lock.json"),
        (" M", "packages/engine/package.json"),
        (" M", "packages/engine/CHANGELOG.md"),
        (" M", "packages/webmcp/package.json"),
        (" M", "packages/webmcp/CHANGELOG.md"),
        (" D", ".changeset/safe-change.md"),
    ]
    for status, path in npm_entries:
        if status != " D":
            write(path)
    validator.validate_npm(npm_entries)
    rejects(lambda: validator.validate_npm(npm_entries + [("??", "postinstall.js")]))
    rejects(
        lambda: validator.validate_npm(
            [entry for entry in npm_entries if not entry[1].endswith("package.json")]
        )
    )
    validator.validate_npm(
        [
            (" M", "package-lock.json"),
            (" M", "packages/webmcp/package.json"),
            (" M", "packages/webmcp/CHANGELOG.md"),
            (" D", ".changeset/webmcp-change.md"),
        ]
    )

    crate_entries = [
        (" M", "Cargo.lock"),
        (" M", "crates/tellegen/CHANGELOG.md"),
        (" M", "crates/tellegen/Cargo.toml"),
    ]
    for _, path in crate_entries:
        write(path)
    validator.validate_crate(crate_entries)
    rejects(lambda: validator.validate_crate(crate_entries[:-1]))
    os.chmod("Cargo.lock", 0o755)
    rejects(lambda: validator.validate_crate(crate_entries))

print("version change validator tests passed")
