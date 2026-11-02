"""Exercise source manifests in the Git-free Docker build context."""

import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


class AcOpfReleaseTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        (self.root / 'scripts').mkdir()
        for name in ['Cargo.toml', 'Cargo.lock', 'scripts/acopf-release.py']:
            shutil.copyfile(ROOT / name, self.root / name)
        self.asset = self.root / 'tellegen_acopf_wasi.wasm'
        self.asset.write_bytes(b'\x00asm\x01\x00\x00\x00')

    def manifest(self, revision):
        return subprocess.run(
            [sys.executable, str(self.root / 'scripts/acopf-release.py'),
             '--asset', str(self.asset)],
            env={**os.environ, 'TELLEGEN_SOURCE_REVISION': revision},
            capture_output=True, text=True,
        )

    def test_container_records_supplied_revision_and_artifact_hash(self):
        revision = 'a' * 40
        result = self.manifest(revision)
        self.assertEqual(result.returncode, 0, result.stderr)
        source = json.loads((self.root / 'POUNCE-SOURCE.json').read_text())
        self.assertEqual(source['tellegen_source'],
                         f'https://github.com/eigenergy/tellegen/tree/{revision}')
        self.assertIsNone(source['tellegen_worktree_dirty'])
        self.assertEqual(source['artifact_sha256'], hashlib.sha256(self.asset.read_bytes()).hexdigest())
        self.assertTrue(source['crates'])

    def test_manual_container_does_not_invent_a_revision(self):
        result = self.manifest('')
        self.assertEqual(result.returncode, 0, result.stderr)
        source = json.loads((self.root / 'POUNCE-SOURCE.json').read_text())
        self.assertEqual(source['tellegen_source'], 'https://github.com/eigenergy/tellegen')
        self.assertIsNone(source['tellegen_worktree_dirty'])

    def test_invalid_revision_is_rejected(self):
        result = self.manifest('not-a-git-sha')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('must be a full Git commit SHA', result.stderr)


if __name__ == '__main__':
    unittest.main()
