#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import os
import subprocess
import tempfile
from pathlib import Path


SCRIPT = Path(__file__).with_name("validate-version-changes.py")
SPEC = importlib.util.spec_from_file_location("version_validator", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
validator = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(validator)


def write(path: str, text: str = "generated\n") -> None:
    target = Path(path)
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(text)


def rejects(callable_) -> None:
    try:
        callable_()
    except SystemExit:
        return
    raise AssertionError("invalid generated changes were accepted")


def git(*args: str) -> None:
    subprocess.run(
        [
            "git",
            "-c",
            "user.name=test",
            "-c",
            "user.email=test@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=/dev/null",
            *args,
        ],
        check=True,
        stdout=subprocess.DEVNULL,
    )


def commit(files: dict[str, str]) -> None:
    for path, text in files.items():
        write(path, text)
    git("add", "--all")
    git("commit", "--quiet", "--allow-empty", "--message", "fixture")


CHANGESET = '---\n"@tellegen/engine": minor\n---\n\nA prerelease change.\n'

with tempfile.TemporaryDirectory() as directory:
    os.chdir(directory)
    # The npm validator reads the committed pre state, so it needs a HEAD.
    git("init", "--quiet", "--initial-branch=main")
    commit({})
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
    # Outside pre mode, the pre state and .changeset/pre/ are not outputs.
    write(".changeset/pre/safe-change.md")
    rejects(
        lambda: validator.validate_npm(
            npm_entries + [("??", ".changeset/pre/safe-change.md")]
        )
    )
    write(".changeset/pre.json")
    rejects(lambda: validator.validate_npm(npm_entries + [("??", ".changeset/pre.json")]))
    os.remove(".changeset/pre.json")
    os.remove(".changeset/pre/safe-change.md")

    # Pre mode: `changeset version` moves the consumed changeset into
    # .changeset/pre/ unchanged and leaves pre.json alone.
    commit(
        {
            ".changeset/pre.json": '{\n  "mode": "pre",\n  "tag": "next"\n}\n',
            ".changeset/next-change.md": CHANGESET,
        }
    )
    os.remove(".changeset/next-change.md")
    write(".changeset/pre/next-change.md", CHANGESET)
    status = validator.status_entries()
    assert (" D", ".changeset/next-change.md") in status, status
    assert ("??", ".changeset/pre/next-change.md") in status, status
    git("add", "--all", "--", ".changeset")
    # Staged, as the version pull request job sees the applied patch: a
    # deletion and an addition, not one rename entry with two paths.
    status = validator.status_entries()
    assert ("D ", ".changeset/next-change.md") in status, status
    assert ("A ", ".changeset/pre/next-change.md") in status, status
    git("reset", "--quiet")
    pre_entries = [
        (" M", "package-lock.json"),
        (" M", "packages/engine/package.json"),
        (" M", "packages/engine/CHANGELOG.md"),
        (" D", ".changeset/next-change.md"),
        ("??", ".changeset/pre/next-change.md"),
    ]
    validator.validate_npm(pre_entries)
    validator.validate_npm(pre_entries[:-1] + [(" A", ".changeset/pre/next-change.md")])
    validator.validate_npm(pre_entries[:-1] + [("A ", ".changeset/pre/next-change.md")])
    # Dropped: deleted without the move.
    rejects(lambda: validator.validate_npm(pre_entries[:-1]))
    # Pre state rewritten by the generator.
    rejects(lambda: validator.validate_npm(pre_entries + [(" M", ".changeset/pre.json")]))
    # Nothing consumed.
    rejects(lambda: validator.validate_npm(pre_entries[:3]))
    # Invented: a pre changeset with no committed original.
    write(".changeset/pre/other-change.md", CHANGESET)
    rejects(
        lambda: validator.validate_npm(
            pre_entries + [("??", ".changeset/pre/other-change.md")]
        )
    )
    os.remove(".changeset/pre/other-change.md")
    # Changed in transit.
    write(".changeset/pre/next-change.md", CHANGESET + "Injected.\n")
    rejects(lambda: validator.validate_npm(pre_entries))
    write(".changeset/pre/next-change.md", CHANGESET)

    # A prerelease must never take the latest dist-tag.
    commit({".changeset/pre.json": '{\n  "mode": "pre",\n  "tag": "latest"\n}\n'})
    rejects(lambda: validator.validate_npm(pre_entries))

    # Exit mode: the stable release consumes .changeset/pre/ and removes the
    # pre state.
    commit({".changeset/pre.json": '{\n  "mode": "exit",\n  "tag": "next"\n}\n'})
    os.remove(".changeset/pre.json")
    os.remove(".changeset/pre/next-change.md")
    exit_entries = [
        (" M", "package-lock.json"),
        (" M", "packages/engine/package.json"),
        (" M", "packages/engine/CHANGELOG.md"),
        (" D", ".changeset/pre.json"),
        (" D", ".changeset/pre/next-change.md"),
    ]
    validator.validate_npm(exit_entries)
    # The pre state must go.
    rejects(lambda: validator.validate_npm(exit_entries[:3] + exit_entries[4:]))
    # Nothing moves into .changeset/pre/ on exit.
    write(".changeset/pre/next-change.md", CHANGESET)
    rejects(
        lambda: validator.validate_npm(
            exit_entries[:4] + [("??", ".changeset/pre/next-change.md")]
        )
    )

    crate_entries = [
        (" M", "Cargo.lock"),
        (" M", "crates/tellegen/CHANGELOG.md"),
        (" M", "crates/tellegen/Cargo.toml"),
    ]
    def manifest(name: str, version: str) -> str:
        return f'[package]\nname = "{name}"\nversion = "{version}"\n'

    def lock(engine: str, wheel: str) -> str:
        return (
            f'[[package]]\nname = "tellegen"\nversion = "{engine}"\n\n'
            f'[[package]]\nname = "tellegen-py"\nversion = "{wheel}"\n'
        )

    # The checkout base: engine and wheel agree on 0.3.0.
    commit(
        {
            "crates/tellegen/CHANGELOG.md": "# Changelog\n",
            "crates/tellegen/Cargo.toml": manifest("tellegen", "0.3.0"),
            "crates/tellegen-py/Cargo.toml": manifest("tellegen-py", "0.3.0"),
            "Cargo.lock": lock("0.3.0", "0.3.0"),
        }
    )
    # A version already raised by hand: release-plz writes only the changelog.
    write("crates/tellegen/CHANGELOG.md", "# Changelog\n\n## 0.3.0\n")
    validator.validate_crate([(" M", "crates/tellegen/CHANGELOG.md")])
    # The wheel must still agree with the engine.
    write("crates/tellegen-py/Cargo.toml", manifest("tellegen-py", "0.2.0"))
    rejects(lambda: validator.validate_crate([(" M", "crates/tellegen/CHANGELOG.md")]))
    rejects(
        lambda: validator.validate_crate(
            [(" M", "crates/tellegen/CHANGELOG.md"), (" M", "crates/tellegen-py/Cargo.toml")]
        )
    )

    # A release-plz bump to 0.3.1, then the wheel synchronized to it.
    write("crates/tellegen/Cargo.toml", manifest("tellegen", "0.3.1"))
    write("crates/tellegen-py/Cargo.toml", manifest("tellegen-py", "0.3.1"))
    write("Cargo.lock", lock("0.3.1", "0.3.1"))
    synchronized = crate_entries + [(" M", "crates/tellegen-py/Cargo.toml")]
    validator.validate_crate(synchronized)
    rejects(lambda: validator.validate_crate(crate_entries[:-1]))
    rejects(lambda: validator.validate_crate(crate_entries[1:]))
    rejects(lambda: validator.validate_crate([(" M", "Cargo.lock")]))
    rejects(
        lambda: validator.validate_crate(
            [(" M", "crates/tellegen/CHANGELOG.md"), (" M", "crates/tellegen-py/Cargo.toml")]
        )
    )
    # Anything beyond the version in the wheel's manifest is refused.
    write(
        "crates/tellegen-py/Cargo.toml",
        manifest("tellegen-py", "0.3.1") + "publish = true\n",
    )
    rejects(lambda: validator.validate_crate(synchronized))
    write("crates/tellegen-py/Cargo.toml", manifest("tellegen-py", "0.3.1"))
    # A lock entry left behind is refused.
    write("Cargo.lock", lock("0.3.1", "0.3.0"))
    rejects(lambda: validator.validate_crate(synchronized))
    write("Cargo.lock", lock("0.3.1", "0.3.1"))
    os.chmod("Cargo.lock", 0o755)
    rejects(lambda: validator.validate_crate(crate_entries))

print("version change validator tests passed")
