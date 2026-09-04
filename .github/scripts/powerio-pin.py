#!/usr/bin/env python3
"""Keep every temporary PowerIO git patch on one exact revision."""

from __future__ import annotations

import argparse
import re
import tomllib
from pathlib import Path


POWERIO_REPOSITORY = "https://github.com/eigenergy/powerio.git"
POWERIO_CRATES = (
    "powerio",
    "powerio-core",
    "powerio-dist",
    "powerio-matrix",
    "powerio-prob",
    "powerio-tx",
)
REVISION = re.compile(r"[0-9a-f]{40}")


def fail(message: str) -> "NoReturn":
    raise SystemExit(message)


def read_toml(path: Path) -> dict:
    try:
        return tomllib.loads(path.read_text())
    except (OSError, tomllib.TOMLDecodeError) as error:
        fail(f"cannot read {path}: {error}")


def manifest_revision(manifest_path: Path) -> str:
    manifest = read_toml(manifest_path)
    try:
        patches = manifest["patch"]["crates-io"]
    except (KeyError, TypeError):
        fail(f"{manifest_path} has no [patch.crates-io] table")

    revisions: set[str] = set()
    for crate in POWERIO_CRATES:
        entry = patches.get(crate)
        if not isinstance(entry, dict):
            fail(f"{manifest_path} has no git patch for {crate}")
        if entry.get("git") != POWERIO_REPOSITORY:
            fail(f"{manifest_path} patches {crate} from an unexpected repository")
        revision = entry.get("rev")
        if not isinstance(revision, str) or REVISION.fullmatch(revision) is None:
            fail(f"{manifest_path} has an invalid revision for {crate}")
        revisions.add(revision)

    if len(revisions) != 1:
        fail(f"{manifest_path} does not pin every PowerIO crate to one revision")
    return revisions.pop()


def check_lock(lock_path: Path, revision: str) -> None:
    lock = read_toml(lock_path)
    packages = lock.get("package")
    if not isinstance(packages, list):
        fail(f"{lock_path} has no package entries")

    expected_source = f"git+{POWERIO_REPOSITORY}?rev={revision}#{revision}"
    for crate in POWERIO_CRATES:
        matches = [
            package
            for package in packages
            if package.get("name") == crate
            and str(package.get("source", "")).startswith(
                f"git+{POWERIO_REPOSITORY}?rev="
            )
        ]
        if len(matches) != 1:
            fail(
                f"{lock_path} must contain exactly one git package named {crate}; "
                f"found {len(matches)}"
            )
        if matches[0].get("source") != expected_source:
            fail(f"{lock_path} does not resolve {crate} at {revision}")


def set_manifest_revision(manifest_path: Path, revision: str) -> None:
    if REVISION.fullmatch(revision) is None:
        fail("PowerIO revision must be a full lowercase 40-character commit SHA")

    text = manifest_path.read_text()
    section_match = re.search(
        r"(?ms)^\[patch\.crates-io\]\n(?P<body>.*?)(?=^\[|\Z)", text
    )
    if section_match is None:
        fail(f"{manifest_path} has no [patch.crates-io] table")

    body = section_match.group("body")
    for crate in POWERIO_CRATES:
        pattern = re.compile(
            rf'(?m)^(?P<prefix>{re.escape(crate)}\s*=\s*\{{[^\n]*\brev\s*=\s*")'
            r"[0-9a-f]{40}"
            r'(?P<suffix>"[^\n]*\}\s*)$'
        )
        body, count = pattern.subn(
            lambda match: match.group("prefix") + revision + match.group("suffix"),
            body,
        )
        if count != 1:
            fail(f"could not update exactly one {crate} patch in {manifest_path}")

    updated = (
        text[: section_match.start("body")]
        + body
        + text[section_match.end("body") :]
    )
    manifest_path.write_text(updated)
    manifest_revision(manifest_path)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--set",
        dest="revision",
        metavar="SHA",
        help="rewrite Cargo.toml to test a candidate PowerIO commit",
    )
    parser.add_argument("--manifest", type=Path, default=Path("Cargo.toml"))
    parser.add_argument("--lock", type=Path, default=Path("Cargo.lock"))
    args = parser.parse_args()

    if args.revision is not None:
        set_manifest_revision(args.manifest, args.revision)
        print(f"PowerIO manifest pin set to {args.revision}")
        return

    revision = manifest_revision(args.manifest)
    check_lock(args.lock, revision)
    print(f"PowerIO manifest and lockfile agree on {revision}")


if __name__ == "__main__":
    main()
