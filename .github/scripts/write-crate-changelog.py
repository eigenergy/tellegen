#!/usr/bin/env python3
"""Write the tellegen crate's newest changelog section from merged pull requests.

release-plz writes a changelog section for a new version from commit
subjects. This repository merges pull requests with merge commits, so what
release-plz sees of a pull request is "Merge pull request #N from ...", and
it walks the history along one parent at a time, which can miss a pull
request entirely. A merge commit's message is the pull request's title, so
the title is already in git. This script replaces the body of the newest
section with one entry per pull request that changed the crate, read from
the first-parent history since the previous release. It needs no network
access, and nobody edits the result by hand.

Run it after `release-plz update` and before the version patch is validated:

    python3 .github/scripts/write-crate-changelog.py

`--from`, `--to`, `--version` and `--print` let a dry run pick its range.
"""

from __future__ import annotations

import argparse
import datetime as dt
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

CRATE_DIR = "crates/tellegen/"
CRATE_MANIFEST = "crates/tellegen/Cargo.toml"
CHANGELOG = "crates/tellegen/CHANGELOG.md"
ROOT_MANIFEST = "Cargo.toml"

# Bots and release machinery. Their merges change versions and lockfiles,
# not the crate's behaviour.
SKIP_BRANCH_PREFIXES = ("dependabot/", "release-plz-", "changeset-release/")

# Root manifest keys outside the dependency tables that reach the published
# crate through `*.workspace = true`.
WORKSPACE_PACKAGE_KEYS = {"edition", "rust-version", "license"}

GROUP_ORDER = ("Breaking", "Added", "Changed", "Fixed")

PR_MERGE = re.compile(r"^Merge pull request #(\d+) from ([^/\s]+)/(\S+)$")
CONVENTIONAL = re.compile(
    r"^(?P<kind>[A-Za-z]+)(?:\([^)]*\))?(?P<bang>!)?:\s*(?P<text>\S.*)$"
)
CHANGED_KEY = re.compile(r"^[+-]\s*([A-Za-z0-9_-]+)\s*[.=]")
HEADING = re.compile(r"^## \[")


@dataclass
class Entry:
    group: str
    text: str
    number: int | None


def git(*args: str, cwd: Path) -> str:
    return subprocess.run(
        ["git", *args], cwd=cwd, check=True, capture_output=True, text=True
    ).stdout


def is_ancestor(ancestor: str, rev: str, cwd: Path) -> bool:
    result = subprocess.run(
        ["git", "merge-base", "--is-ancestor", ancestor, rev],
        check=False,
        cwd=cwd,
        capture_output=True,
    )
    return result.returncode == 0


def package_version(manifest: str) -> str:
    section = None
    for line in manifest.splitlines():
        stripped = line.strip()
        if stripped.startswith("["):
            section = stripped.strip("[]").strip()
            continue
        if section == "package":
            match = re.match(r'^version\s*=\s*"([^"]+)"', stripped)
            if match:
                return match.group(1)
    raise SystemExit(f"no [package] version in {CRATE_MANIFEST}")


def dependency_names(manifest: str) -> set[str]:
    """Names in the crate's normal dependency tables, which users build against."""
    names: set[str] = set()
    section = ""
    for line in manifest.splitlines():
        stripped = line.split("#", 1)[0].strip()
        if not stripped:
            continue
        if stripped.startswith("["):
            section = stripped.strip("[]").strip()
            table = re.match(r"^(?:target\..+\.)?dependencies\.([A-Za-z0-9_-]+)$", section)
            if table:
                names.add(table.group(1))
            continue
        if re.match(r"^(?:target\..+\.)?dependencies$", section):
            key = re.match(r"^([A-Za-z0-9_-]+)\s*[.=]", stripped)
            if key:
                names.add(key.group(1))
    return names


def changed_keys(diff: str) -> list[str]:
    keys = []
    for line in diff.splitlines():
        if line.startswith(("+++", "---")):
            continue
        match = CHANGED_KEY.match(line)
        if match:
            keys.append(match.group(1))
        elif line.startswith(("+", "-")) and line[1:].strip() and not line[1:].lstrip().startswith("#"):
            keys.append("")
    return keys


def touches_crate(rev: str, dependencies: set[str], cwd: Path) -> bool:
    parent = f"{rev}^1"
    for path in git("diff", "--name-only", parent, rev, cwd=cwd).splitlines():
        if path == CHANGELOG:
            continue
        if path == CRATE_MANIFEST:
            keys = changed_keys(git("diff", "-U0", parent, rev, "--", path, cwd=cwd))
            # A version bump alone is release machinery.
            if any(key != "version" for key in keys):
                return True
            continue
        if path.startswith(CRATE_DIR):
            return True
        if path == ROOT_MANIFEST:
            keys = changed_keys(git("diff", "-U0", parent, rev, "--", path, cwd=cwd))
            if any(key in dependencies or key in WORKSPACE_PACKAGE_KEYS for key in keys):
                return True
    return False


def entry_for(rev: str, cwd: Path) -> Entry | None:
    parents, subject, body = git(
        "show", "-s", "--format=%P%x00%s%x00%b", rev, cwd=cwd
    ).split("\x00", 2)
    number = None
    title = subject.strip()
    merge = PR_MERGE.match(title)
    if merge and len(parents.split()) > 1:
        branch = merge.group(3)
        if branch.startswith(SKIP_BRANCH_PREFIXES):
            return None
        number = int(merge.group(1))
        lines = [line.strip() for line in body.splitlines() if line.strip()]
        title = lines[0] if lines else branch
    if not title:
        return None

    group = "Changed"
    conventional = CONVENTIONAL.match(title)
    if conventional:
        kind = conventional.group("kind").lower()
        if conventional.group("bang"):
            group = "Breaking"
        elif kind == "feat":
            group = "Added"
        elif kind == "fix":
            group = "Fixed"
        title = conventional.group("text")
    title = title[:1].upper() + title[1:]
    return Entry(group, title, number)


def release_commits(previous: str, to: str, cwd: Path) -> list[str]:
    """First-parent commits that landed after the release that `previous` tags.

    The tag can sit on a release branch that merged into main later, so a
    commit belongs to the new release when its first parent already
    contains the tag, not merely when the tag doesn't contain it.
    """
    commits = []
    for rev in git("rev-list", "--first-parent", to, cwd=cwd).split():
        parents = git("show", "-s", "--format=%P", rev, cwd=cwd).split()
        if not parents or not is_ancestor(previous, parents[0], cwd):
            break
        commits.append(rev)
    commits.reverse()
    return commits


def render(entries: list[Entry], repository: str) -> str:
    blocks = []
    for group in GROUP_ORDER:
        lines = []
        for entry in entries:
            if entry.group != group:
                continue
            if entry.number is None:
                lines.append(f"- {entry.text}")
            else:
                link = f"{repository}/pull/{entry.number}"
                lines.append(f"- {entry.text} ([#{entry.number}]({link}))")
        if lines:
            blocks.append(f"### {group}\n\n" + "\n".join(lines))
    return "\n\n".join(blocks)


def write_section(changelog: str, version: str, heading: str, body: str) -> str:
    lines = changelog.splitlines()
    own = re.compile(rf"^## \[{re.escape(version)}\]")
    start = next((i for i, line in enumerate(lines) if own.match(line)), None)
    if start is None:
        # release-plz wrote no section: put one above the previous release.
        insert = next((i for i, line in enumerate(lines) if HEADING.match(line)), len(lines))
        new_lines = lines[:insert] + ["", heading, "", *body.splitlines(), ""] + lines[insert:]
    else:
        end = next(
            (i for i in range(start + 1, len(lines)) if HEADING.match(lines[i])),
            len(lines),
        )
        new_lines = lines[: start + 1] + ["", *body.splitlines(), ""] + lines[end:]
    text = "\n".join(new_lines)
    text = re.sub(r"\n{3,}", "\n\n", text)
    return text.rstrip("\n") + "\n"


def repository_url(root_manifest: str) -> str:
    match = re.search(r'^repository\s*=\s*"([^"]+)"', root_manifest, re.MULTILINE)
    if not match:
        raise SystemExit("no workspace repository URL in Cargo.toml")
    return match.group(1).rstrip("/")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    parser.add_argument("--root", type=Path, default=Path.cwd(), help="repository root")
    parser.add_argument("--from", dest="previous", help="tag of the previous release")
    parser.add_argument("--to", default="HEAD", help="last commit of the new release")
    parser.add_argument("--version", help="new version (default: the crate manifest's)")
    parser.add_argument("--date", help="release date, YYYY-MM-DD (default: today, UTC)")
    parser.add_argument("--print", action="store_true", help="print the section, write nothing")
    args = parser.parse_args(argv)
    root = args.root.resolve()

    manifest = (root / CRATE_MANIFEST).read_text()
    version = args.version or package_version(manifest)
    previous = args.previous or git(
        "describe", "--tags", "--abbrev=0", "--match", "v[0-9]*", args.to, cwd=root
    ).strip()
    if previous == f"v{version}":
        print(f"tellegen {version} is already tagged; nothing to write")
        return 0

    dependencies = dependency_names(manifest)
    entries = []
    for rev in release_commits(previous, args.to, root):
        if not touches_crate(rev, dependencies, root):
            continue
        entry = entry_for(rev, root)
        if entry is not None:
            entries.append(entry)
    if not entries:
        print(f"no pull request since {previous} changed the crate; leaving release-plz's section")
        return 0

    repository = repository_url((root / ROOT_MANIFEST).read_text())
    date = args.date or dt.datetime.now(dt.timezone.utc).date().isoformat()
    heading = f"## [{version}]({repository}/compare/{previous}...v{version}) - {date}"
    body = render(entries, repository)
    if args.print:
        print(f"{heading}\n\n{body}")
        return 0

    path = root / CHANGELOG
    path.write_text(write_section(path.read_text(), version, heading, body))
    print(f"wrote {len(entries)} entries for tellegen {version} since {previous}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
