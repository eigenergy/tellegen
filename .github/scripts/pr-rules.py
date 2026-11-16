#!/usr/bin/env python3
"""Check a pull request against the repository's release and size rules.

    pr-rules.py --base <commit> --head <commit> [--title T] [--labels a,b]

Three rules, each lifted by a label that only a maintainer can apply:

  release:minor  Without it, a changeset may only declare `patch`, and no
                 commit subject, commit body or pull request title may mark a
                 breaking change (`type!:` or a `BREAKING CHANGE:` footer).
                 release-plz bumps the crate's minor version on such a commit,
                 and changesets bumps a package's minor version on a `minor`
                 changeset, so either one changes the next release number.
  large-pr       Without it, a pull request may add at most MAX_ADDED lines,
                 not counting lockfiles, changelogs, generated files and test
                 data.

The diff is taken from the merge base of the two commits, so it holds only what
the pull request changes. Exits 1 with one line per broken rule.

Standard library only, Python 3.9 or newer.
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys

MINOR_LABEL = "release:minor"
LARGE_LABEL = "large-pr"
MAX_ADDED = 1500

BREAKING_SUBJECT = re.compile(r"^[A-Za-z]+(\([^)]*\))?!:")
BREAKING_FOOTER = re.compile(r"^BREAKING[ -]CHANGE:", re.MULTILINE)
CHANGESET_BUMP = re.compile(r"""^\s*["']?([^"':]+)["']?\s*:\s*(patch|minor|major)\s*$""")

# Added lines in these paths do not count towards MAX_ADDED: nobody writes
# them by hand, or they are recorded data.
UNCOUNTED = (
    re.compile(r"(^|/)(Cargo\.lock|package-lock\.json|Manifest\.toml)$"),
    re.compile(r"(^|/)CHANGELOG\.md$"),
    re.compile(r"(^|/)generated/"),
    re.compile(r"(^|/)tests/data/"),
    re.compile(r"(^|/)fixtures/"),
    re.compile(r"^validation/.*\.(json|jsonl|csv)$"),
    re.compile(r"\.snap$"),
)


def git(*args: str) -> str:
    return subprocess.run(
        ["git", *args], check=True, capture_output=True, text=True
    ).stdout


def changeset_bumps(text: str) -> list[tuple[str, str]]:
    """The (package, bump) pairs in a changeset file's front matter."""
    lines = text.splitlines()
    if not lines or lines[0].strip() != "---":
        return []
    bumps = []
    for line in lines[1:]:
        if line.strip() == "---":
            break
        match = CHANGESET_BUMP.match(line)
        if match:
            bumps.append((match.group(1).strip(), match.group(2)))
    return bumps


def is_breaking(subject: str, body: str = "") -> bool:
    return bool(BREAKING_SUBJECT.match(subject) or BREAKING_FOOTER.search(body))


def counted(path: str) -> bool:
    return not any(pattern.search(path) for pattern in UNCOUNTED)


def added_lines(numstat: str) -> int:
    """Sum the added lines of `git diff --numstat` output over counted paths."""
    total = 0
    for line in numstat.splitlines():
        parts = line.split("\t")
        if len(parts) < 3 or parts[0] == "-":
            continue
        # A rename is printed as `old => new` or `dir/{old => new}/file`.
        path = re.sub(r"\{[^{}]* => ([^{}]*)\}", r"\1", parts[2])
        path = path.split(" => ")[-1].replace("//", "/")
        if counted(path):
            total += int(parts[0])
    return total


def check(
    *,
    changesets: dict[str, str],
    commits: list[tuple[str, str, str]],
    title: str,
    numstat: str,
    labels: set[str],
) -> list[str]:
    """Return one message per broken rule.

    `changesets` maps a changeset path to its text; `commits` holds
    (short hash, subject, body) for each commit of the pull request.
    """
    problems = []
    if MINOR_LABEL not in labels:
        for path, text in sorted(changesets.items()):
            for package, bump in changeset_bumps(text):
                if bump != "patch":
                    problems.append(
                        f"{path}: {package} is declared `{bump}`; changesets are "
                        f"`patch` unless a maintainer adds the `{MINOR_LABEL}` label"
                    )
        for sha, subject, body in commits:
            if is_breaking(subject, body):
                problems.append(
                    f"commit {sha} ({subject!r}) is marked as a breaking change, "
                    f"which bumps the crate's minor version; reword it, or a "
                    f"maintainer adds the `{MINOR_LABEL}` label"
                )
        if is_breaking(title):
            problems.append(
                f"the pull request title is marked as a breaking change; reword "
                f"it, or a maintainer adds the `{MINOR_LABEL}` label"
            )
    if LARGE_LABEL not in labels:
        added = added_lines(numstat)
        if added > MAX_ADDED:
            problems.append(
                f"the pull request adds {added} lines (limit {MAX_ADDED}, not "
                f"counting lockfiles, changelogs, generated files and test data); "
                f"split it, or a maintainer adds the `{LARGE_LABEL}` label"
            )
    return problems


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--base", required=True)
    parser.add_argument("--head", required=True)
    parser.add_argument("--title", default="")
    parser.add_argument("--labels", default="")
    args = parser.parse_args()

    span = f"{args.base}...{args.head}"
    changed = git(
        "diff", "--name-only", "--diff-filter=AM", span, "--", ".changeset"
    ).splitlines()
    changesets = {
        path: git("show", f"{args.head}:{path}")
        for path in changed
        if path.endswith(".md") and not path.endswith("README.md")
    }
    commits = []
    log = git(
        "log", "--no-merges", "--format=%h%x1f%s%x1f%b%x1e", f"{args.base}..{args.head}"
    )
    for record in log.split("\x1e"):
        if record.strip():
            sha, subject, body = record.strip("\n").split("\x1f")
            commits.append((sha, subject, body))

    problems = check(
        changesets=changesets,
        commits=commits,
        title=args.title,
        numstat=git("diff", "--numstat", "-M", span),
        labels={label.strip() for label in args.labels.split(",") if label.strip()},
    )
    for problem in problems:
        print(f"::error::{problem}")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
