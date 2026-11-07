#!/usr/bin/env python3
"""Pin which CI gates each kind of change runs (.github/scripts/ci-scope.py)."""

from __future__ import annotations

import importlib.util
import os
import re
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
SCRIPT = HERE / "ci-scope.py"
SPEC = importlib.util.spec_from_file_location("ci_scope", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
scope = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(scope)

CI = ("rust", "js", "python", "macos", "image")


def gates(*names: str) -> dict:
    unknown = set(names) - set(scope.GATES)
    assert not unknown, unknown
    return {gate: gate in names for gate in scope.GATES}


RELEASE_PLZ = [
    "Cargo.lock",
    "crates/tellegen-py/Cargo.toml",
    "crates/tellegen/CHANGELOG.md",
    "crates/tellegen/Cargo.toml",
]
BUMPED = frozenset({"Cargo.lock", "crates/tellegen-py/Cargo.toml", "crates/tellegen/Cargo.toml"})

# The files changeset-release/main changed in #330.
CHANGESET_RELEASE = [
    ".changeset/osm-model-reinforcement.md",
    ".changeset/svelte-declutter-markers.md",
    "apps/web/package.json",
    "examples/browser-minimal/package.json",
    "examples/svelte-minimal/package.json",
    "package-lock.json",
    "packages/engine/CHANGELOG.md",
    "packages/engine/package.json",
    "packages/engine/src/generated/api-types.ts",
    "packages/svelte/CHANGELOG.md",
    "packages/svelte/package.json",
]

# name: (changed paths, expected gates)
CASES = {
    "svelte-only PR": (
        [
            "packages/svelte/src/lib/BusMarker.svelte",
            "packages/svelte/tests/markers.test.ts",
            ".changeset/svelte-declutter-markers.md",
        ],
        gates("js", "image"),
    ),
    "web-app-only PR": (["apps/web/src/routes/+page.svelte"], gates("js", "image")),
    "docs-only PR": (
        ["docs/src/architecture.md", "docs/assets/hero.svg", "README.md", "CONTRIBUTING.md"],
        gates("docs"),
    ),
    "evidence data": (["docs/evidence/synth/README.md", "docs/evidence/synth/cats/runs.json"], gates()),
    "evidence script": (["docs/evidence/synth/run.py"], gates("python")),
    "webmcp evidence harness": (["docs/evidence/webmcp/run.mjs"], gates("js")),
    "the wheel's test case": (["docs/evidence/studies/case3.m"], gates("python", "macos")),
    "python-only PR": (
        ["python/tellegen/__init__.py", "python/tests/test_tellegen.py"],
        gates("python", "macos"),
    ),
    "python lint config": (["ruff.toml"], gates("python")),
    "binding crate": (["crates/tellegen-py/src/lib.rs"], gates("rust", "python", "macos")),
    "engine crate": (["crates/tellegen/src/lib.rs"], gates(*CI)),
    "synth crate": (["crates/tellegen-synth/src/site.rs"], gates("rust", "js", "image")),
    "benchmarks crate": (["crates/benchmarks/src/main.rs"], gates("rust")),
    "crate readme": (["crates/tellegen/README.md"], gates("rust")),
    "lockfile-only dependency bump": (["Cargo.lock"], gates(*CI)),
    "toolchain": (["rust-toolchain.toml"], gates(*CI)),
    "move out of crates/": (["crates/tellegen/src/old.rs", "docs/src/old.rs"], gates(*CI, "docs")),
    "npm lockfile": (["package-lock.json"], gates("js", "image")),
    "changeset-release PR": (CHANGESET_RELEASE, gates("js", "image")),
    "Dockerfile": (["Dockerfile"], gates("image")),
    "deploy script": (["deploy/remote-deploy.sh"], gates("image")),
    "POUNCE notices": (["licenses/POUNCE-NOTICE.txt"], gates("js", "image")),
    "cargo-deny config": (["deny.toml"], gates("rust")),
    "release script": ([".github/scripts/write-crate-changelog.py"], gates("python", "image")),
    "PowerIO pin guard": ([".github/scripts/powerio-pin.py"], gates("rust", "python", "image")),
    "npm audit script": ([".github/scripts/npm-audit.sh"], gates("js", "image")),
    "js setup action": ([".github/actions/js-setup/action.yml"], gates("js")),
    "rust gate workflow": ([".github/workflows/gates-rust.yml"], gates("rust")),
    "js gate workflow": ([".github/workflows/gates-js.yml"], gates("js")),
    "python gate workflow": ([".github/workflows/gates-python.yml"], gates("python", "macos")),
    "docs workflow": ([".github/workflows/docs-pages.yml"], gates("docs")),
    "release workflow": ([".github/workflows/release-crate.yml"], gates()),
    "ci.yml": ([".github/workflows/ci.yml"], gates(*CI)),
    "the scope script": ([".github/scripts/ci-scope.py"], gates(*CI)),
    "the scope test": ([".github/scripts/test-ci-scope.py"], gates("python", "image")),
    "unknown path": (["tools/lint.sh"], gates("rust", "js", "image")),
    "unknown root file": ([".gitattributes"], gates("rust", "js", "image")),
}


class Rules(unittest.TestCase):
    def test_cases(self) -> None:
        for name, (paths, expected) in CASES.items():
            with self.subTest(name):
                self.assertEqual(scope.decide(paths)[0], expected)

    def test_release_plz_pr(self) -> None:
        self.assertEqual(scope.decide(RELEASE_PLZ, BUMPED)[0], gates("rust"))
        # A version raised by hand on main leaves release-plz only the changelog.
        self.assertEqual(scope.decide(["crates/tellegen/CHANGELOG.md"])[0], gates("rust"))
        # The Python job refuses a wheel whose version is not the engine's.
        self.assertEqual(
            scope.decide(RELEASE_PLZ, BUMPED, versions_agree=False)[0], gates("rust", "python")
        )
        # A version bump among other changes is just a change.
        self.assertEqual(
            scope.decide(RELEASE_PLZ + ["crates/tellegen/src/lib.rs"], BUMPED)[0], gates(*CI)
        )

    def test_nothing_changed_runs_everything(self) -> None:
        self.assertEqual(scope.decide([])[0], gates(*scope.GATES))

    def test_docs_filter_matches_docs_pages(self) -> None:
        text = (ROOT / ".github/workflows/docs-pages.yml").read_text()
        block = re.search(r"\n  pull_request:\n    paths:\n((?:      - .*\n)+)", text)
        self.assertIsNotNone(block, "docs-pages.yml has no pull_request paths filter")
        listed = tuple(line.strip()[2:].strip("\"'") for line in block.group(1).splitlines())
        self.assertEqual(listed, scope.DOCS_PATHS)


def tracked() -> list:
    out = subprocess.check_output(["git", "ls-files", "-z"], cwd=ROOT)
    return [path.decode() for path in out.split(b"\0") if path]


def commands(name: str, job: str | None = None) -> str:
    """What a workflow (or one of its jobs) runs: its `run` scripts and the
    `uses` lines, without comments, descriptions or messages around them."""
    text = (ROOT / ".github/workflows" / name).read_text()
    if job is not None:
        text = re.search(rf"(?ms)^  {re.escape(job)}:\n.*?(?=^  [\w-]+:\n|\Z)", text).group(0)
    out = []
    block = None
    for line in text.splitlines():
        indent = len(line) - len(line.lstrip())
        if block is not None and (not line.strip() or indent > block):
            if not line.lstrip().startswith("#"):
                out.append(line)
            continue
        block = None
        key = re.match(r"\s*(?:- )?(run|uses):\s*(.*)$", line)
        if key and key.group(2) in ("|", "|-", ">", ">-"):
            block = indent
        elif key:
            out.append(key.group(2))
    return "\n".join(out)


def referenced(text: str, files: list) -> set:
    """The tracked files a workflow or manifest names, directories expanded."""
    found = set()
    for token in re.findall(r"[\w.-]+(?:/[\w.*-]+)+", text):
        token = token[2:] if token.startswith("./") else token
        pattern = scope._glob(token)
        found.update(f for f in files if f == token or f.startswith(token + "/") or pattern.match(f))
    return found


class Drift(unittest.TestCase):
    """The tables still describe the repository the gates run on."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.files = tracked()

    def check(self, rules, paths, label: str) -> None:
        self.assertTrue(paths, f"{label}: found nothing to check")
        for path in sorted(paths):
            with self.subTest(label, path=path):
                self.assertTrue(rules(path), f"{label} reads {path}, which the scope skips")

    def test_every_pattern_matches_a_tracked_file(self) -> None:
        # A typo in a rule would silently change what it decides.
        for name in ("RUST", "JS", "IMAGE", "WHEEL", "PYTHON_LINT", "DOCS"):
            for glob, _ in getattr(scope, name).rules:
                with self.subTest(name, glob=glob):
                    pattern = scope._glob(glob)
                    self.assertTrue(any(pattern.match(f) for f in self.files), glob)

    def test_rust_gate_inputs(self) -> None:
        self.check(scope.RUST, referenced(commands("gates-rust.yml"), self.files), "gates-rust.yml")
        self.check(scope.RUST, [f for f in self.files if f.startswith("crates/")], "the workspace")

    def test_js_gate_inputs(self) -> None:
        scripts = (ROOT / "package.json").read_text()
        self.check(scope.JS, referenced(commands("gates-js.yml"), self.files), "gates-js.yml")
        self.check(scope.JS, referenced(scripts, self.files), "package.json scripts")
        workspaces = [f for f in self.files if f.split("/")[0] in ("apps", "packages", "examples")]
        self.check(scope.JS, workspaces, "the npm workspaces")

    def test_image_inputs(self) -> None:
        job = commands("ci.yml", "image")
        self.check(scope.IMAGE, referenced(job, self.files), "ci.yml's image job")
        # Everything the Dockerfile copies, except what no stage compiles:
        # crate markdown and the two crates the image never builds.
        unbuilt = scope.Rules(
            False, scope.run("crates/**/*.md", "crates/tellegen-py/src/**", "crates/benchmarks/src/**")
        )
        copied = set()
        for line in (ROOT / "Dockerfile").read_text().splitlines():
            words = line.split()
            if not words or words[0] != "COPY" or any(w.startswith("--from") for w in words):
                continue
            sources = [w for w in words[1:-1] if not w.startswith("--")]
            copied |= referenced(" ".join(f"./{s}" for s in sources), self.files)
            copied |= {s for s in sources if s in self.files}
        self.check(scope.IMAGE, {f for f in copied if not unbuilt(f)}, "the Dockerfile")

    def test_wheel_inputs(self) -> None:
        # The version check names crates/tellegen as a directory, which brings
        # its markdown along; no build reads that.
        named = referenced(commands("gates-python.yml"), self.files)
        self.check(scope.WHEEL, {f for f in named if not f.endswith(".md")}, "gates-python.yml")


# --- the command, against a throwaway repository -----------------------------

MANIFEST = """[package]
name = "{name}"
version = "{version}" # keep this comment
edition = "2021"

[dependencies.serde]
version = "{serde}"

[[example]]
name = "demo"
"""

LOCK = """# This file is automatically @generated by Cargo.
version = 4

[[package]]
name = "serde"
version = "{serde}"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "{checksum}"

[[package]]
name = "tellegen"
version = "{engine}"
dependencies = [
 "serde",
]

[[package]]
name = "tellegen-py"
version = "{wheel}"
dependencies = [
 "tellegen",
]
"""


class Command(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.repo = Path(self.tmp.name)
        self.git("init", "-q")
        self.release("0.6.0", "0.6.0")
        self.write("packages/svelte/src/Map.svelte", "<div></div>\n")
        self.write("docs/src/intro.md", "# Intro\n")
        self.green = self.commit("green on main")

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def git(self, *args: str) -> str:
        env = dict(os.environ, GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL=os.devnull)
        return subprocess.check_output(
            [
                "git",
                "-c", "user.name=ci-scope",
                "-c", "user.email=ci-scope@example.invalid",
                "-c", "commit.gpgsign=false",
                "-c", "core.hooksPath=/dev/null",
                *args,
            ],
            cwd=self.repo,
            env=env,
            text=True,
        ).strip()

    def write(self, path: str, text: str) -> None:
        target = self.repo / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text)

    def commit(self, message: str) -> str:
        self.git("add", "-A")
        self.git("commit", "-q", "--allow-empty", "-m", message)
        return self.git("rev-parse", "HEAD")

    def release(self, engine: str, wheel: str, serde: str = "1.0.0", changelog: str = "") -> None:
        self.write("crates/tellegen/Cargo.toml", MANIFEST.format(name="tellegen", version=engine, serde="1"))
        self.write("crates/tellegen-py/Cargo.toml", MANIFEST.format(name="tellegen-py", version=wheel, serde="1"))
        self.write(
            "Cargo.lock",
            LOCK.format(engine=engine, wheel=wheel, serde=serde, checksum=serde.replace(".", "") * 8),
        )
        self.write("crates/tellegen/CHANGELOG.md", f"# Changelog\n{changelog}")

    def run_scope(self, *args: str) -> dict:
        out = subprocess.run(
            [sys.executable, str(SCRIPT), *args],
            cwd=self.repo,
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        ).stdout
        result = dict(line.split("=", 1) for line in out.splitlines())
        self.assertEqual(set(result), set(scope.GATES))
        return {gate: value == "true" for gate, value in result.items()}

    def test_release_plz_pr(self) -> None:
        self.release("0.6.1", "0.6.1", changelog="\n## 0.6.1\n")
        self.commit("Release tellegen")
        self.assertEqual(self.run_scope("--base", "HEAD^1"), gates("rust"))

    def test_version_bump_that_leaves_the_wheel_behind(self) -> None:
        self.release("0.6.1", "0.6.0")
        self.commit("Bump the engine alone")
        self.assertEqual(self.run_scope("--base", "HEAD^1"), gates("rust", "python"))

    def test_lockfile_dependency_bump(self) -> None:
        self.release("0.6.1", "0.6.1", serde="1.0.1")
        self.commit("Bump serde with the release")
        self.assertEqual(self.run_scope("--base", "HEAD^1"), gates(*CI))

    def test_manifest_dependency_bump(self) -> None:
        text = (self.repo / "crates/tellegen/Cargo.toml").read_text()
        self.write("crates/tellegen/Cargo.toml", text.replace('version = "1"', 'version = "1.1"'))
        self.commit("Raise the serde floor")
        self.assertEqual(self.run_scope("--base", "HEAD^1"), gates(*CI))

    def test_push_to_main_counts_every_change_since_green(self) -> None:
        # A svelte change whose push run failed or was superseded, then a docs
        # change: the second push still owes the svelte change its gates.
        self.write("packages/svelte/src/Map.svelte", "<div>map</div>\n")
        self.commit("Merge the svelte change")
        self.write("docs/src/intro.md", "# Introduction\n")
        self.commit("Merge the docs change")
        self.assertEqual(self.run_scope("--base", self.green), gates("js", "image", "docs"))
        self.assertEqual(self.run_scope("--base", "HEAD^1"), gates("docs"))

    def test_rename_counts_both_sides(self) -> None:
        self.git("mv", "crates/tellegen/CHANGELOG.md", "docs/src/changelog.md")
        self.commit("Move the changelog")
        self.assertEqual(self.run_scope("--base", "HEAD^1"), gates("rust", "docs"))

    def test_uncertainty_runs_everything(self) -> None:
        everything = gates(*scope.GATES)
        self.assertEqual(self.run_scope(), everything)
        self.assertEqual(self.run_scope("--base", "HEAD"), everything)
        self.assertEqual(self.run_scope("--base", "0" * 40), everything)
        self.assertEqual(self.run_scope("--base", "HEAD^5"), everything)


if __name__ == "__main__":
    unittest.main(verbosity=1)
