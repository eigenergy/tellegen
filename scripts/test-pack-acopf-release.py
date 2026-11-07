"""Release packaging rejects mismatched source, development builds, and altered files."""

import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location("pack_acopf", Path(__file__).with_name("pack-acopf-release.py"))
PACK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PACK)


class PackagingTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.source = self.root / "source"
        self.output = self.root / "output"
        self.assets = self.source / "target/experimental-acopf"
        for directory in [self.assets, self.source / "licenses", self.source / "crates/tellegen", self.source / "packages/engine"]:
            directory.mkdir(parents=True)
        (self.source / "crates/tellegen/Cargo.toml").write_text('[package]\nversion = "0.6.1"\n')
        (self.source / "packages/engine/package.json").write_text('{"version":"0.7.0"}')
        self.wasm = b"\0asm\x01\0\0\0"
        (self.assets / PACK.FILES[0]).write_bytes(self.wasm)
        for name in PACK.FILES[1:3]:
            (self.assets / name).write_text(name)
            (self.source / "licenses" / name).write_text(name)
        self.revision = "a" * 40
        self.manifest = {
            "development_only": False,
            "tellegen_worktree_dirty": None,
            "tellegen_source": f"https://github.com/eigenergy/tellegen/tree/{self.revision}",
            "artifact_sha256": hashlib.sha256(self.wasm).hexdigest(),
            "artifact_bytes": len(self.wasm),
            "license": "EPL-2.0",
            "crates": [{"name": "pounce-rs", "version": "0.15.0"}],
        }

    def package(self, tag="v0.6.1"):
        (self.assets / "POUNCE-SOURCE.json").write_text(json.dumps(self.manifest))
        PACK.package(self.source, self.output, tag, self.revision)

    def test_release_contains_checksums_and_exact_source(self):
        self.package()
        manifest = json.loads((self.output / "POUNCE-SOURCE.json").read_text())
        self.assertEqual(manifest["release_tag"], "v0.6.1")
        self.assertEqual(manifest["abi"], "tellegen-acopf-wasi/1")
        self.assertIn(self.revision, manifest["tellegen_source_archive"])
        for row in (self.output / "ACOPF-SHA256SUMS.txt").read_text().splitlines():
            digest, name = row.split("  ")
            self.assertEqual(digest, hashlib.sha256((self.output / name).read_bytes()).hexdigest())

    def test_rejects_development_dirty_mismatched_and_unlicensed_artifacts(self):
        for key, value in [("development_only", True), ("tellegen_worktree_dirty", True),
                           ("tellegen_source", "wrong"), ("artifact_sha256", "0" * 64),
                           ("artifact_bytes", 1), ("license", "MIT"), ("crates", [])]:
            with self.subTest(key=key):
                previous = self.manifest[key]
                self.manifest[key] = value
                with self.assertRaises(ValueError):
                    self.package()
                self.manifest[key] = previous

    def test_rejects_wrong_release_and_changed_notices(self):
        with self.assertRaisesRegex(ValueError, "crate version"):
            self.package("v0.6.2")
        (self.assets / "POUNCE-NOTICE.txt").write_text("altered")
        with self.assertRaisesRegex(ValueError, "released source"):
            self.package()


if __name__ == "__main__":
    unittest.main()
