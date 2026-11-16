#!/usr/bin/env python3
"""Tests for pr-rules.py. Run directly: python3 .github/scripts/test-pr-rules.py"""

from __future__ import annotations

import importlib.util
import pathlib
import unittest

_spec = importlib.util.spec_from_file_location(
    "pr_rules", pathlib.Path(__file__).with_name("pr-rules.py")
)
assert _spec and _spec.loader
rules = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(rules)

PATCH = '---\n"@tellegen/engine": patch\n---\n\nA fix.\n'
MINOR = '---\n"@tellegen/engine": patch\n"@tellegen/svelte": minor\n---\n\nNew API.\n'


def check(**overrides):
    args = {
        "changesets": {},
        "commits": [],
        "title": "fix: a thing",
        "numstat": "",
        "labels": set(),
    }
    args.update(overrides)
    return rules.check(**args)


class Changesets(unittest.TestCase):
    def test_patch_passes(self):
        self.assertEqual(check(changesets={".changeset/a.md": PATCH}), [])

    def test_minor_fails_and_names_the_package(self):
        problems = check(changesets={".changeset/a.md": MINOR})
        self.assertEqual(len(problems), 1)
        self.assertIn("@tellegen/svelte", problems[0])

    def test_major_fails(self):
        text = "---\n'@tellegen/webmcp': major\n---\n"
        self.assertEqual(len(check(changesets={".changeset/a.md": text})), 1)

    def test_label_allows_minor(self):
        self.assertEqual(
            check(changesets={".changeset/a.md": MINOR}, labels={"release:minor"}), []
        )

    def test_the_word_minor_in_the_note_is_ignored(self):
        text = '---\n"@tellegen/engine": patch\n---\n\nscope: minor\n'
        self.assertEqual(check(changesets={".changeset/a.md": text}), [])


class Breaking(unittest.TestCase):
    def test_bang_subject_fails(self):
        commits = [("abc1234", "refactor(mc-pf)!: new primitives", "")]
        self.assertEqual(len(check(commits=commits)), 1)

    def test_footer_fails(self):
        commits = [("abc1234", "feat: x", "Body.\n\nBREAKING CHANGE: gone\n")]
        self.assertEqual(len(check(commits=commits)), 1)

    def test_title_fails(self):
        self.assertEqual(len(check(title="feat!: x")), 1)

    def test_plain_subjects_pass(self):
        commits = [
            ("abc1234", "feat(acopf): sensitivities", ""),
            ("abc1235", "fix: stop! a typo: here", ""),
        ]
        self.assertEqual(check(commits=commits), [])

    def test_label_allows_breaking(self):
        commits = [("abc1234", "feat!: x", "")]
        self.assertEqual(check(commits=commits, labels={"release:minor"}), [])


class Size(unittest.TestCase):
    def test_under_the_limit_passes(self):
        self.assertEqual(check(numstat=f"{rules.MAX_ADDED}\t0\tsrc/a.rs\n"), [])

    def test_over_the_limit_fails(self):
        numstat = f"{rules.MAX_ADDED}\t0\tsrc/a.rs\n1\t0\tsrc/b.rs\n"
        self.assertEqual(len(check(numstat=numstat)), 1)

    def test_uncounted_paths(self):
        numstat = "\n".join(
            f"9999\t0\t{path}"
            for path in (
                "Cargo.lock",
                "package-lock.json",
                "crates/tellegen/CHANGELOG.md",
                "packages/engine/src/generated/api-types.ts",
                "crates/tellegen/tests/data/rms/reference.json",
                "packages/engine/tests/fixtures/case.m",
                "validation/dc-variants/case300-report.json",
            )
        )
        self.assertEqual(check(numstat=numstat), [])

    def test_binary_files_are_skipped(self):
        self.assertEqual(check(numstat="-\t-\tdocs/assets/a.png\n"), [])

    def test_a_rename_counts_under_its_new_path(self):
        numstat = "9999\t0\tevals/{dc-variants => fixtures}/report.json\n"
        self.assertEqual(check(numstat=numstat), [])
        numstat = "9999\t0\told/fixtures/a.py => src/a.py\n"
        self.assertEqual(len(check(numstat=numstat)), 1)

    def test_label_allows_a_large_change(self):
        self.assertEqual(
            check(numstat="9999\t0\tsrc/a.rs\n", labels={"large-pr"}), []
        )


if __name__ == "__main__":
    unittest.main()
