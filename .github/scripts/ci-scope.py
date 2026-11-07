#!/usr/bin/env python3
"""Choose the CI gates a change needs.

CI's scope job runs this and appends its stdout, one `<gate>=true|false` line
per gate, to $GITHUB_OUTPUT:

    ci-scope.py --base <commit>   diff <commit>..HEAD and decide
    ci-scope.py                   no base: run every gate

ci.yml chooses the base. A pull request diffs its merge commit against the
first parent, the branch it merges into. A push to main diffs against the last
main commit whose CI passed, so a change whose own run failed or was replaced
by a newer one still counts.

The gates:

  rust, js, image  Run unless every changed path is one these tables name as
                   unable to affect the gate. A path the tables do not name
                   runs the gate.
  python, macos    Python job, Linux and Windows wheels and sdist; the macOS
                   wheel legs. These run only for the paths that go into the
                   wheel (macos and python) or that lint, type-check or test it
                   (python).
  docs             The pull_request `paths` filter of docs-pages.yml, repeated
                   here so test-ci-scope.py can check the two agree. Nothing
                   reads this output.

Every gate runs when the base is missing or the diff fails or is empty. Every
gate but docs runs when the change touches this file or ci.yml.

A release PR that only moves the engine's version needs only the Rust gate,
which runs cargo-semver-checks and packages the crate. A manifest or lockfile
change counts as such a bump when it differs from the base only in the
`[package]` version of crates/tellegen or crates/tellegen-py (and their
Cargo.lock entries). This reads the change, not the branch name, so it applies
on a push to main too, and a branch named release-plz-* gets no special case.

The release workflows call the gates without inputs, which runs them in full.
Nothing here applies to them.

Standard library only, Python 3.9 or newer.
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
import traceback

GATES = ("rust", "js", "python", "macos", "image", "docs")
# The gates a change to the scoping itself runs: all of them except docs,
# which docs-pages.yml filters on its own.
CI_GATES = ("rust", "js", "python", "macos", "image")

# Changing how CI picks its gates runs every one of them.
SCOPING = frozenset({".github/workflows/ci.yml", ".github/scripts/ci-scope.py"})

ENGINE_MANIFEST = "crates/tellegen/Cargo.toml"
WHEEL_MANIFEST = "crates/tellegen-py/Cargo.toml"
LOCKFILE = "Cargo.lock"
VERSIONED_PACKAGES = frozenset({"tellegen", "tellegen-py"})


def _glob(pattern: str) -> re.Pattern:
    """GitHub's path glob: `**` crosses directories, `*` and `?` do not."""
    out = []
    i = 0
    while i < len(pattern):
        if pattern.startswith("**/", i):
            out.append("(?:.*/)?")
            i += 3
        elif pattern.startswith("**", i):
            out.append(".*")
            i += 2
        elif pattern[i] == "*":
            out.append("[^/]*")
            i += 1
        elif pattern[i] == "?":
            out.append("[^/]")
            i += 1
        else:
            out.append(re.escape(pattern[i]))
            i += 1
    return re.compile("".join(out) + r"\Z")


class Rules:
    """Ordered (glob, verdict) pairs. The first match decides; else `default`."""

    def __init__(self, default: bool, *groups: tuple[tuple[str, bool], ...]):
        self.default = default
        self.rules = tuple(rule for group in groups for rule in group)
        self.compiled = tuple((_glob(glob), verdict) for glob, verdict in self.rules)

    def __call__(self, path: str) -> bool:
        for pattern, verdict in self.compiled:
            if pattern.match(path):
                return verdict
        return self.default


def skip(*globs: str) -> tuple[tuple[str, bool], ...]:
    return tuple((glob, False) for glob in globs)


def run(*globs: str) -> tuple[tuple[str, bool], ...]:
    return tuple((glob, True) for glob in globs)


# Paths that none of rust, js and image read. Each gate's own rules come
# first, so a gate that does read one of these says so before this list.
NOT_BUILT = skip(
    ".changeset/**",
    "docs/**",
    "book.toml",
    "python/**",
    "pyproject.toml",
    "ruff.toml",
    "mypy.ini",
    "scripts/build-docs.sh",
    "scripts/derive-*.py",
    "scripts/stage-data.sh",
    ".github/ISSUE_TEMPLATE/**",
    ".github/dependabot.yml",
    ".github/workflows/deploy.yml",
    ".github/workflows/docs-pages.yml",
    ".github/workflows/package-inspect.yml",
    ".github/workflows/powerio-candidate.yml",
    ".github/workflows/release-crate.yml",
    ".github/workflows/release-npm.yml",
    "release-plz.toml",
    "justfile",
    "CITATION.cff",
    # No build or test reads markdown outside the paths each gate names first.
    "**/*.md",
)

# Release-time and CI-test scripts that none of the three gates run.
RELEASE_SCRIPTS = skip(
    ".github/scripts/sync_python_version.py",
    ".github/scripts/validate-version-changes.py",
    ".github/scripts/write-crate-changelog.py",
    ".github/scripts/test-*.py",
)

# gates-rust.yml. cargo package reads each crate's own README and CHANGELOG,
# so markdown under crates/ counts. It runs scripts/acopf-release.py,
# scripts/epl-guard.sh and .github/scripts/powerio-pin.py, which no rule skips.
# No Rust code or test reads docs/ (the benchmarks crate writes
# docs/src/benchmark-results.md only when asked to), scripts/, apps/,
# packages/ or examples/.
RUST = Rules(
    True,
    run("crates/**"),
    NOT_BUILT,
    RELEASE_SCRIPTS,
    skip(
        "apps/**",
        "packages/**",
        "examples/**",
        "package.json",
        "package-lock.json",
        "deploy/**",
        "Dockerfile",
        ".dockerignore",
        "docker-compose.yml",
        "licenses/**",
        "scripts/build-acopf-wasi.sh",
        "scripts/*.mjs",
        "scripts/test-acopf-release.py",
        ".github/actions/**",
        ".github/scripts/npm-audit.sh",
        ".github/workflows/gates-js.yml",
        ".github/workflows/gates-python.yml",
    ),
)

# gates-js.yml builds the wasm module and the CLI from crates/, so Rust counts.
# Nothing builds tellegen-py or benchmarks there, and nothing includes a crate's
# markdown. The browser tests read fixtures from crates/tellegen/tests/data.
# `npm run test:evidence` runs docs/evidence/webmcp and scripts/umami-report;
# `npm run wasm:acopf` runs scripts/build-acopf-wasi.sh, which copies licenses/.
JS = Rules(
    True,
    skip("crates/tellegen-py/src/**", "crates/benchmarks/src/**", "crates/**/*.md"),
    run("crates/**", "docs/evidence/webmcp/**", "apps/**", "packages/**", "examples/**"),
    NOT_BUILT,
    RELEASE_SCRIPTS,
    skip(
        "deploy/**",
        "Dockerfile",
        ".dockerignore",
        "docker-compose.yml",
        "deny.toml",
        "rustfmt.toml",
        "scripts/epl-guard.sh",
        ".github/scripts/powerio-pin.py",
        ".github/workflows/gates-rust.yml",
        ".github/workflows/gates-python.yml",
    ),
)

# ci.yml's image job: the deploy and .github/scripts tests, then the Dockerfile
# build, which sees only what the Dockerfile copies: the Cargo workspace,
# scripts/build-acopf-wasi.sh and scripts/acopf-release.py, licenses/, the npm
# workspace manifests and lockfile, apps/web, four packages and two examples.
# It builds tellegen-server, the wasm module and the WASI reactor, never
# tellegen-py or benchmarks.
IMAGE = Rules(
    True,
    skip("crates/tellegen-py/src/**", "crates/benchmarks/src/**", "crates/**/*.md"),
    run("crates/**", "apps/**", "packages/**", "examples/**", ".github/scripts/**"),
    NOT_BUILT,
    skip(
        "deny.toml",
        "rustfmt.toml",
        "docker-compose.yml",
        "scripts/epl-guard.sh",
        "scripts/*.mjs",
        "scripts/test-acopf-release.py",
        ".github/actions/**",
        ".github/workflows/gates-rust.yml",
        ".github/workflows/gates-js.yml",
        ".github/workflows/gates-python.yml",
    ),
)

# What goes into the wheel, the case its tests solve, and its gate. tellegen-py
# depends on the engine alone among the workspace crates; any other crate's
# manifest still counts, since Cargo loads the whole workspace.
WHEEL = Rules(
    False,
    skip("crates/**/*.md"),
    run(
        "crates/tellegen/**",
        "crates/tellegen-py/**",
        "crates/*/Cargo.toml",
        "python/**",
        "Cargo.toml",
        "Cargo.lock",
        "pyproject.toml",
        "rust-toolchain.toml",
        "docs/evidence/studies/case3.m",
        ".github/workflows/gates-python.yml",
        ".github/workflows/release-python.yml",
        "scripts/check-python-sdist.py",
    ),
)

# What the Python job lints, type-checks or tests without changing the wheel.
# ruff.toml's `include` keeps `ruff check .` to python/, and mypy and pytest read
# only python/, so a .py file elsewhere (CI and evidence scripts) is not linted.
PYTHON_LINT = Rules(False, run("python/**", "ruff.toml", "mypy.ini"))

# Mirrors `on.pull_request.paths` in docs-pages.yml: what mdBook and
# scripts/build-docs.sh read. Nothing under docs/src includes a file from
# outside it.
DOCS_PATHS = (
    "book.toml",
    "docs/src/**",
    "docs/theme/**",
    "docs/assets/**",
    "scripts/build-docs.sh",
    ".github/workflows/docs-pages.yml",
)
DOCS = Rules(False, run(*DOCS_PATHS))


def affected(path: str, bumped: frozenset = frozenset(), versions_agree: bool = True) -> set:
    """The gates one changed path needs."""
    gates = {"docs"} if DOCS(path) else set()
    if path in SCOPING:
        return gates | set(CI_GATES)
    if path in bumped:
        gates.add("rust")
        # The Python job's first step refuses a wheel whose version is not the
        # engine's. Run it when a bump leaves the two apart.
        if not versions_agree:
            gates.add("python")
        return gates
    for gate, rules in (("rust", RUST), ("js", JS), ("image", IMAGE), ("macos", WHEEL)):
        if rules(path):
            gates.add(gate)
    if "macos" in gates or PYTHON_LINT(path):
        gates.add("python")
    return gates


def decide(
    paths: list,
    bumped: frozenset = frozenset(),
    versions_agree: bool = True,
) -> tuple[dict, dict]:
    """Each gate's verdict, and the first path that set it (for the log)."""
    if not paths:
        return {gate: True for gate in GATES}, {gate: "(no changed paths)" for gate in GATES}
    result = {gate: False for gate in GATES}
    reason = {}
    for path in paths:
        for gate in affected(path, bumped, versions_agree):
            if not result[gate]:
                result[gate] = True
                reason[gate] = path
    return result, reason


# --- git -------------------------------------------------------------------


class GitError(Exception):
    pass


def git(*args: str) -> bytes:
    try:
        return subprocess.run(
            ("git",) + args, check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE
        ).stdout
    except OSError as error:
        raise GitError(f"git {' '.join(args)}: {error}") from None
    except subprocess.CalledProcessError as error:
        detail = (error.stderr or b"").decode(errors="replace").strip()
        raise GitError(f"git {' '.join(args)}: {detail or f'exit {error.returncode}'}") from None


def show(rev: str, path: str) -> str | None:
    try:
        return git("show", f"{rev}:{path}").decode()
    except (GitError, UnicodeDecodeError):
        return None


def changed_paths(base: str) -> list:
    git("rev-parse", "--verify", "--quiet", f"{base}^{{commit}}")
    # --no-renames lists both sides of a move, so moving a file out of crates/
    # still counts as a change to crates/.
    raw = git("diff", "--name-only", "--no-renames", "-z", base, "HEAD", "--")
    return [path.decode() for path in raw.split(b"\0") if path]


# A table header, `[name]` or `[[name]]`.
TABLE = re.compile(r"\s*\[\[?([^\[\]]+)\]\]?\s*(#.*)?$")
PACKAGE_VERSION = re.compile(r'^(\s*version\s*=\s*)"[^"]*"', re.MULTILINE)


def mask_manifest(text: str) -> str:
    """The manifest with its [package] version blanked out."""
    out = []
    table = None
    for line in text.splitlines(keepends=True):
        header = TABLE.match(line)
        if header:
            table = header.group(1).strip()
        elif table == "package":
            line = PACKAGE_VERSION.sub(r'\1"*"', line)
        out.append(line)
    return "".join(out)


def mask_lock(text: str) -> str:
    """The lockfile with the tellegen and tellegen-py versions blanked out."""
    blocks = re.split(r"(?m)^(?=\[\[package\]\]\s*$)", text)
    out = []
    for block in blocks:
        name = re.search(r'(?m)^name = "([^"]*)"$', block)
        local = not re.search(r"(?m)^source = ", block)
        if name and name.group(1) in VERSIONED_PACKAGES and local:
            block = PACKAGE_VERSION.sub(r'\1"*"', block, count=1)
        out.append(block)
    return "".join(out)


def version_bumps(base: str, paths: list) -> frozenset:
    """The changed paths that only move the engine's and the wheel's version."""
    bumped = set()
    for path, mask in (
        (ENGINE_MANIFEST, mask_manifest),
        (WHEEL_MANIFEST, mask_manifest),
        (LOCKFILE, mask_lock),
    ):
        if path not in paths:
            continue
        old, new = show(base, path), show("HEAD", path)
        if old is not None and new is not None and old != new and mask(old) == mask(new):
            bumped.add(path)
    return frozenset(bumped)


def manifest_version(text: str | None) -> str | None:
    if text is None:
        return None
    table = None
    for line in text.splitlines():
        header = TABLE.match(line)
        if header:
            table = header.group(1).strip()
            continue
        match = re.match(r'\s*version\s*=\s*"([^"]*)"', line)
        if table == "package" and match:
            return match.group(1)
    return None


def versions_agree() -> bool:
    engine = manifest_version(show("HEAD", ENGINE_MANIFEST))
    wheel = manifest_version(show("HEAD", WHEEL_MANIFEST))
    return engine is not None and engine == wheel


def scope(base: str | None) -> tuple[dict, dict]:
    if not base:
        return {gate: True for gate in GATES}, {gate: "(no base commit)" for gate in GATES}
    try:
        paths = changed_paths(base)
        bumped = version_bumps(base, paths)
        agree = versions_agree() if bumped else True
    except GitError as error:
        print(f"::warning::cannot diff against {base}, running every gate: {error}", file=sys.stderr)
        return {gate: True for gate in GATES}, {gate: "(diff failed)" for gate in GATES}
    print(f"{len(paths)} changed path(s) since {base}", file=sys.stderr)
    if bumped:
        print(f"version-only: {', '.join(sorted(bumped))}", file=sys.stderr)
    return decide(paths, bumped, agree)


def main(argv: list | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--base", help="commit to diff HEAD against; omit to run every gate")
    args = parser.parse_args(argv)
    try:
        result, reason = scope(args.base)
    except Exception:
        # A bug here must not skip a gate.
        traceback.print_exc()
        print("::warning::ci-scope.py failed, running every gate", file=sys.stderr)
        result, reason = {gate: True for gate in GATES}, {gate: "(scope failed)" for gate in GATES}
    for gate in GATES:
        print(f"{gate}={'true' if result[gate] else 'false'}")
        why = f"needed for {reason[gate]}" if result[gate] else "skipped: no change reaches it"
        print(f"  {gate}: {why}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
