#!/usr/bin/env python3
"""Synchronize the wheel's version after the unprivileged release-plz update."""

from __future__ import annotations

import re
import stat
import tomllib
from pathlib import Path


ENGINE_MANIFEST = Path("crates/tellegen/Cargo.toml")
PYTHON_MANIFEST = Path("crates/tellegen-py/Cargo.toml")
LOCKFILE = Path("Cargo.lock")


def fail(message: str) -> "NoReturn":
    raise SystemExit(message)


def read_regular(path: Path) -> str:
    try:
        mode = path.lstat().st_mode
        if not stat.S_ISREG(mode) or mode & 0o111:
            fail(f"expected a regular nonexecutable release file: {path}")
        return path.read_bytes().decode("utf-8")
    except (OSError, UnicodeError) as error:
        fail(f"cannot read release file {path}: {error}")


def parse_toml(text: str) -> dict:
    try:
        return tomllib.loads(text)
    except tomllib.TOMLDecodeError as error:
        fail(f"invalid release TOML: {error}")


def package_version(text: str, name: str) -> str:
    package = parse_toml(text).get("package", {})
    if not isinstance(package, dict) or package.get("name") != name:
        fail(f"expected a package manifest for {name}")
    version = package.get("version")
    if not isinstance(version, str) or re.fullmatch(
        r"[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?", version
    ) is None:
        fail(f"expected an explicit package version for {name}")
    return version


def replace_version_field(text: str, version: str) -> str:
    pattern = re.compile(
        r'''(?m)^(?P<prefix>[ \t]*version[ \t]*=[ \t]*)(?P<quote>["'])'''
        r'''[^"'\n]*(?P=quote)(?P<suffix>[ \t]*(?:\#[^\n]*)?)$'''
    )
    updated, count = pattern.subn(
        lambda match: (
            match.group("prefix") + match.group("quote") + version
            + match.group("quote") + match.group("suffix")
        ),
        text,
    )
    if count != 1:
        fail("expected exactly one explicit version field in the package table")
    return updated


def replace_manifest_version(text: str, version: str) -> str:
    section = re.search(r"(?ms)^\[package\][ \t]*(?:\#[^\n]*)?\n.*?(?=^\[|\Z)", text)
    if section is None:
        fail("cannot locate the package table")
    return (
        text[:section.start()]
        + replace_version_field(section.group(), version)
        + text[section.end():]
    )


def lock_package(text: str, name: str) -> dict:
    packages = parse_toml(text).get("package", [])
    if not isinstance(packages, list):
        fail("expected package entries in Cargo.lock")
    matches = [package for package in packages if package.get("name") == name]
    if len(matches) != 1 or "source" in matches[0] or "checksum" in matches[0]:
        fail(f"Cargo.lock must contain exactly one local package named {name}")
    return matches[0]


def check_lock_versions(text: str, version: str) -> None:
    for name in ("tellegen", "tellegen-py"):
        if lock_package(text, name).get("version") != version:
            fail(f"Cargo.lock {name} version does not match the engine: {version}")


def synchronize(root: Path = Path(".")) -> None:
    engine = read_regular(root / ENGINE_MANIFEST)
    wheel = read_regular(root / PYTHON_MANIFEST)
    lock = read_regular(root / LOCKFILE)
    version = package_version(engine, "tellegen")
    previous = package_version(wheel, "tellegen-py")
    if lock_package(lock, "tellegen").get("version") != version:
        fail("release-plz must update the engine lock entry before wheel synchronization")
    if lock_package(lock, "tellegen-py").get("version") != previous:
        fail("the wheel manifest and lock entry disagree before synchronization")
    if version == previous:
        return

    updated_wheel = replace_manifest_version(wheel, version)
    # Do not regenerate the dependency graph or update dependencies. Only the
    # unique local Python package record may differ from release-plz's lock.
    blocks = list(re.finditer(r"(?ms)^\[\[package\]\]\n.*?(?=^\[\[package\]\]|\Z)", lock))
    matches = [
        block for block in blocks
        if parse_toml(block.group())["package"][0].get("name") == "tellegen-py"
    ]
    if len(matches) != 1:
        fail("cannot locate exactly one Python package lock record")
    block = matches[0]
    updated_lock = (
        lock[:block.start()] + replace_version_field(block.group(), version)
        + lock[block.end():]
    )
    # Validate both prospective files before writing either of them.
    if package_version(updated_wheel, "tellegen-py") != version:
        fail("the synchronized wheel version does not match the engine")
    check_lock_versions(updated_lock, version)
    (root / PYTHON_MANIFEST).write_text(updated_wheel)
    (root / LOCKFILE).write_text(updated_lock)


if __name__ == "__main__":
    synchronize()
