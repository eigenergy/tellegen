#!/usr/bin/env python3
"""Verify one PowerIO candidate revision or one published crates.io release."""

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


def manifest_revision(manifest_path: Path) -> str | None:
    manifest = read_toml(manifest_path)
    patches = manifest.get("patch", {}).get("crates-io", {})
    if not any(crate in patches for crate in POWERIO_CRATES):
        return None

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


def check_lock(lock_path: Path, revision: str | None) -> None:
    lock = read_toml(lock_path)
    packages = lock.get("package")
    if not isinstance(packages, list):
        fail(f"{lock_path} has no package entries")

    expected_source = (
        f"git+{POWERIO_REPOSITORY}?rev={revision}#{revision}"
        if revision is not None
        else "registry+https://github.com/rust-lang/crates.io-index"
    )
    versions: set[str] = set()
    for crate in POWERIO_CRATES:
        matches = [
            package
            for package in packages
            if package.get("name") == crate
        ]
        if len(matches) != 1:
            fail(
                f"{lock_path} must contain exactly one package named {crate}; "
                f"found {len(matches)}"
            )
        if matches[0].get("source") != expected_source:
            fail(f"{lock_path} resolves {crate} from an unexpected source")
        versions.add(str(matches[0].get("version", "")))
        if revision is None and re.fullmatch(
            r"[0-9a-f]{64}", str(matches[0].get("checksum", ""))
        ) is None:
            fail(f"{lock_path} has no crates.io checksum for {crate}")
    if len(versions) != 1 or "" in versions:
        fail(f"{lock_path} mixes PowerIO component versions")


def set_manifest_revision(manifest_path: Path, revision: str) -> None:
    if REVISION.fullmatch(revision) is None:
        fail("PowerIO revision must be a full lowercase 40-character commit SHA")

    current = manifest_revision(manifest_path)
    text = manifest_path.read_text()
    section_match = re.search(
        r"(?ms)^\[patch\.crates-io\]\n(?P<body>.*?)(?=^\[|\Z)", text
    )
    if current is None:
        entries = "".join(
            f'{crate} = {{ git = "{POWERIO_REPOSITORY}", rev = "{revision}" }}\n'
            for crate in POWERIO_CRATES
        )
        if section_match is None:
            updated = text.rstrip() + "\n\n[patch.crates-io]\n" + entries
        else:
            end = section_match.end("body")
            updated = text[:end].rstrip() + "\n" + entries + "\n" + text[end:]
        manifest_path.write_text(updated)
        manifest_revision(manifest_path)
        return

    assert section_match is not None
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
    source = revision if revision is not None else "one checksummed crates.io release"
    print(f"PowerIO manifest and lockfile agree on {source}")


if __name__ == "__main__":
    main()
