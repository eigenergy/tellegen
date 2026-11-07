#!/usr/bin/env python3
"""Exercise the crate changelog writer on throwaway repositories."""

from __future__ import annotations

import importlib.util
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).with_name("write-crate-changelog.py").resolve()
spec = importlib.util.spec_from_file_location("write_crate_changelog", SCRIPT)
assert spec is not None and spec.loader is not None
writer = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = writer
spec.loader.exec_module(writer)

ROOT_MANIFEST = """[workspace]
members = ["crates/*"]

[workspace.package]
edition = "2021"
repository = "https://github.com/example/tellegen"

[workspace.dependencies]
powerio = "1.0"
axum = "0.8"
"""

CRATE_MANIFEST = """[package]
name = "tellegen"
version = "{version}"
edition.workspace = true

[dependencies]
powerio.workspace = true

[dev-dependencies]
proptest = "1"
"""

RELEASE_PLZ_SECTION = """# Changelog

Header prose.

## [0.1.1](https://github.com/example/tellegen/compare/v0.1.0...v0.1.1) - 2026-01-02

### Other

- Merge pull request #10 from example/feat/thing
- Some commit subject

## [0.1.0](https://github.com/example/tellegen/releases/tag/v0.1.0) - 2026-01-01

- First release.
"""


class Repo:
    def __init__(self, root: Path) -> None:
        self.root = root
        self.git("init", "-q", "-b", "main")
        self.git("config", "user.name", "Test")
        self.git("config", "user.email", "test@example.com")
        self.git("config", "commit.gpgsign", "false")
        self.git("config", "tag.gpgsign", "false")

    def git(self, *args: str) -> str:
        return subprocess.run(
            ["git", *args], cwd=self.root, check=True, capture_output=True, text=True
        ).stdout

    def write(self, path: str, text: str) -> None:
        target = self.root / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text)

    def commit(self, message: str) -> None:
        self.git("add", "-A")
        self.git("commit", "-q", "-m", message)

    def pull_request(self, number: int, branch: str, title: str, path: str, text: str) -> None:
        self.git("checkout", "-q", "-b", branch)
        self.write(path, text)
        self.commit(f"Work on {branch}")
        self.git("checkout", "-q", "main")
        message = f"Merge pull request #{number} from example/{branch}\n\n{title}"
        self.git("merge", "-q", "--no-ff", "-m", message, branch)


def initial(repo: Repo, version: str = "0.1.0") -> None:
    repo.write("Cargo.toml", ROOT_MANIFEST)
    repo.write("crates/tellegen/Cargo.toml", CRATE_MANIFEST.format(version=version))
    repo.write("crates/tellegen/src/lib.rs", "// lib\n")
    repo.write("crates/tellegen/CHANGELOG.md", "# Changelog\n")
    repo.write("docs/index.md", "docs\n")
    repo.commit("Initial commit")


class WriteCrateChangelogTest(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.repo = Repo(Path(self.tmp.name))

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def run_writer(self, *args: str) -> str:
        code = writer.main(["--root", str(self.repo.root), "--date", "2026-01-02", *args])
        self.assertEqual(code, 0)
        return (self.repo.root / "crates/tellegen/CHANGELOG.md").read_text()

    def build_history(self) -> None:
        initial(self.repo)
        self.repo.git("tag", "v0.1.0")
        r = self.repo
        r.pull_request(10, "feat/thing", "feat(engine): add a thing", "crates/tellegen/src/a.rs", "a\n")
        r.pull_request(11, "dependabot/cargo/powerio-1.1", "chore(deps): bump powerio",
                       "Cargo.toml", ROOT_MANIFEST.replace('powerio = "1.0"', 'powerio = "1.1"'))
        r.pull_request(12, "docs/guide", "Write a guide", "docs/guide.md", "guide\n")
        r.pull_request(13, "fix/powerio", "fix: Read cases with PowerIO 1.2",
                       "Cargo.toml", ROOT_MANIFEST.replace('powerio = "1.0"', 'powerio = "1.2"'))
        r.pull_request(14, "deps/axum", "Update axum for the server",
                       "Cargo.toml", ROOT_MANIFEST.replace('powerio = "1.0"', 'powerio = "1.2"')
                       .replace('axum = "0.8"', 'axum = "0.9"'))
        r.pull_request(15, "api/remove", "feat!: remove the old API", "crates/tellegen/src/b.rs", "b\n")
        r.pull_request(16, "test/props", "Test the solver with proptest",
                       "crates/tellegen/Cargo.toml",
                       CRATE_MANIFEST.format(version="0.1.0").replace('proptest = "1"', 'proptest = "2"'))
        r.write("crates/tellegen/src/lib.rs", "// lib, fixed\n")
        r.commit("Fix a comment in the solver")
        r.pull_request(17, "release/bump", "Version the crate 0.1.1", "crates/tellegen/Cargo.toml",
                       CRATE_MANIFEST.format(version="0.1.1").replace('proptest = "1"', 'proptest = "2"'))

    def test_replaces_release_plz_section_with_one_entry_per_pull_request(self) -> None:
        self.build_history()
        self.repo.write("crates/tellegen/CHANGELOG.md", RELEASE_PLZ_SECTION)
        text = self.run_writer()
        self.assertEqual(
            text,
            """# Changelog

Header prose.

## [0.1.1](https://github.com/example/tellegen/compare/v0.1.0...v0.1.1) - 2026-01-02

### Breaking

- Remove the old API ([#15](https://github.com/example/tellegen/pull/15))

### Added

- Add a thing ([#10](https://github.com/example/tellegen/pull/10))

### Changed

- Test the solver with proptest ([#16](https://github.com/example/tellegen/pull/16))
- Fix a comment in the solver

### Fixed

- Read cases with PowerIO 1.2 ([#13](https://github.com/example/tellegen/pull/13))

## [0.1.0](https://github.com/example/tellegen/releases/tag/v0.1.0) - 2026-01-01

- First release.
""",
        )

    def test_inserts_a_section_when_release_plz_wrote_none(self) -> None:
        self.build_history()
        self.repo.write(
            "crates/tellegen/CHANGELOG.md",
            "# Changelog\n\n## [0.1.0](https://github.com/example/tellegen/releases/tag/v0.1.0) - 2026-01-01\n\n- First release.\n",
        )
        text = self.run_writer()
        self.assertTrue(text.startswith(
            "# Changelog\n\n## [0.1.1](https://github.com/example/tellegen/compare/v0.1.0...v0.1.1) - 2026-01-02\n\n### Breaking\n"
        ))
        self.assertIn("\n- First release.\n", text)
        self.assertNotIn("\n\n\n", text)

    def test_tag_on_a_release_branch_counts_from_its_merge(self) -> None:
        initial(self.repo)
        r = self.repo
        # release-plz tags the commit on its own branch, which merges later.
        r.git("checkout", "-q", "-b", "release-plz-main")
        r.write("crates/tellegen/Cargo.toml", CRATE_MANIFEST.format(version="0.2.0"))
        r.commit("Release tellegen")
        r.git("tag", "v0.2.0")
        r.git("checkout", "-q", "main")
        # Merged after the release branch was cut but before it merged: in 0.2.0.
        r.pull_request(20, "fix/early", "fix: land before the release", "crates/tellegen/src/c.rs", "c\n")
        r.git("merge", "-q", "--no-ff", "-m",
              "Merge pull request #21 from example/release-plz-main\n\nRelease tellegen 0.2.0",
              "release-plz-main")
        r.pull_request(22, "feat/late", "feat: land after the release", "crates/tellegen/src/d.rs", "d\n")
        entries = [
            writer.entry_for(rev, r.root)
            for rev in writer.release_commits("v0.2.0", "HEAD", r.root)
            if writer.touches_crate(rev, {"powerio"}, r.root)
        ]
        self.assertEqual([(e.group, e.text, e.number) for e in entries if e],
                         [("Added", "Land after the release", 22)])

    def test_nothing_to_write_when_the_version_is_already_tagged(self) -> None:
        initial(self.repo)
        self.repo.git("tag", "v0.1.0")
        before = (self.repo.root / "crates/tellegen/CHANGELOG.md").read_text()
        self.assertEqual(self.run_writer(), before)

    def test_dependency_names_cover_tables_and_targets(self) -> None:
        manifest = """[package]
name = "tellegen"

[dependencies]
serde = { workspace = true }
powerio.workspace = true # comment

[dependencies.schemars]
workspace = true

[target.'cfg(target_arch = "wasm32")'.dependencies]
getrandom = "0.3"

[dev-dependencies]
proptest = "1"
"""
        self.assertEqual(writer.dependency_names(manifest), {"serde", "powerio", "schemars", "getrandom"})


if __name__ == "__main__":
    unittest.main()
