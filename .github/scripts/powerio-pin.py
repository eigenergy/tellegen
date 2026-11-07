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
    "powerio-synth",
    "powerio-tnd",
    "powerio-tx",
)
OPTIONAL_POWERIO_CRATES = ("powerio-archive",)
REVISION = re.compile(r"[0-9a-f]{40}")
SUPPORTED_VERSION = re.compile(r"0\.(11|12)\.\d+(?:-[0-9A-Za-z.-]+)?")


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
    if not any(crate in patches for crate in POWERIO_CRATES + OPTIONAL_POWERIO_CRATES):
        return None

    revisions: set[str] = set()
    crates = POWERIO_CRATES + tuple(crate for crate in OPTIONAL_POWERIO_CRATES if crate in patches)
    for crate in crates:
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
    crates = POWERIO_CRATES + tuple(
        crate for crate in OPTIONAL_POWERIO_CRATES
        if any(package.get("name") == crate for package in packages)
    )
    for crate in crates:
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


def candidate_configuration(candidate_manifest: Path) -> tuple[str, tuple[str, ...]]:
    """Read the selected checkout, never guessing its version from a branch name."""
    manifest = read_toml(candidate_manifest)
    version = manifest.get("workspace", {}).get("package", {}).get("version")
    if not isinstance(version, str) or SUPPORTED_VERSION.fullmatch(version) is None:
        fail("candidate must declare a PowerIO 0.11.x or 0.12.x workspace version")
    dependencies = manifest.get("workspace", {}).get("dependencies", {})
    crates = POWERIO_CRATES + tuple(
        crate for crate in OPTIONAL_POWERIO_CRATES if crate in dependencies
    )
    for crate in crates:
        entry = dependencies.get(crate)
        if not isinstance(entry, dict) or entry.get("version") != version:
            fail(f"candidate must declare one version for {crate}")
    return version, crates


def set_manifest_revision(
    manifest_path: Path, revision: str, *, version: str | None = None,
    crates: tuple[str, ...] = POWERIO_CRATES,
) -> None:
    if REVISION.fullmatch(revision) is None:
        fail("PowerIO revision must be a full lowercase 40-character commit SHA")

    current = manifest_revision(manifest_path)
    text = manifest_path.read_text()
    if version is not None:
        if SUPPORTED_VERSION.fullmatch(version) is None:
            fail("unsupported PowerIO candidate version")
        # A git patch whose release is 0.12 cannot satisfy ^0.11.9. Change only
        # this disposable checkout's PowerIO constraints to the exact candidate.
        dependencies = read_toml(manifest_path).get("workspace", {}).get("dependencies", {})
        for crate in POWERIO_CRATES:
            if crate not in dependencies:
                continue
            pattern = re.compile(
                rf'(?m)^(?P<prefix>{re.escape(crate)}\s*=\s*(?:\{{\s*version\s*=\s*)?)'
                r'"[^"\n]+"'
            )
            text, count = pattern.subn(lambda m: m.group("prefix") + f'"={version}"', text)
            if count != 1:
                fail(f"could not select exactly one workspace dependency for {crate}")
    section_match = re.search(
        r"(?ms)^\[patch\.crates-io\]\n(?P<body>.*?)(?=^\[|\Z)", text
    )
    if current is None:
        entries = "".join(
            f'{crate} = {{ git = "{POWERIO_REPOSITORY}", rev = "{revision}" }}\n'
            for crate in crates
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
    for crate in crates:
        if crate in OPTIONAL_POWERIO_CRATES and crate not in read_toml(manifest_path).get("patch", {}).get("crates-io", {}):
            body = body.rstrip() + f'\n{crate} = {{ git = "{POWERIO_REPOSITORY}", rev = "{revision}" }}\n'
            continue
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
    parser.add_argument("--candidate-manifest", type=Path,
                        help="PowerIO checkout manifest used to select its exact release")
    parser.add_argument("--github-env", type=Path,
                        help="append validated candidate features to the Actions environment")
    args = parser.parse_args()

    if args.revision is not None:
        version, crates = (
            candidate_configuration(args.candidate_manifest)
            if args.candidate_manifest else (None, POWERIO_CRATES)
        )
        set_manifest_revision(args.manifest, args.revision, version=version, crates=crates)
        if args.github_env:
            features = []
            if version and version.startswith("0.12."):
                features.append("powerio-012")
                wasm = read_toml(args.manifest.parent / "crates/tellegen-wasm/Cargo.toml")
                if "powerio-collections" in wasm.get("features", {}):
                    features.append("powerio-collections")
            with args.github_env.open("a") as output:
                output.write(f'TELLEGEN_POWERIO_FEATURES={",".join(features)}\n')
        print(f"PowerIO manifest pin set to {args.revision}")
        return

    if args.candidate_manifest or args.github_env:
        parser.error("--candidate-manifest and --github-env require --set")

    revision = manifest_revision(args.manifest)
    check_lock(args.lock, revision)
    source = revision if revision is not None else "one checksummed crates.io release"
    print(f"PowerIO manifest and lockfile agree on {source}")


if __name__ == "__main__":
    main()
