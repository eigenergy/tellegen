"""Reject altered reconstruction inputs before any conversion work."""
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('prepare', Path(__file__).with_name('prepare-texas7k.py'))
prepare = importlib.util.module_from_spec(spec)
spec.loader.exec_module(prepare)


class PreparationTests(unittest.TestCase):
    def fixture(self, root, path='Master.dss'):
        source = root / 'test/data/Texas7k/p1uhs0_1247'
        source.mkdir(parents=True)
        (source / 'Master.dss').write_bytes(b'original')
        (source.parent / 'source_manifest.json').write_text(json.dumps([
            {'path': path, 'size': 8, 'sha256': hashlib.sha256(b'original').hexdigest()}]))
        return source

    def test_verified_source_proof(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root)
            with patch.object(prepare.subprocess, 'check_output', side_effect=['pinned\n', '']):
                proof = prepare.verify_sources(root, {'workflow_revision': 'pinned'})
            self.assertEqual(proof['source_file_count'], 1)
            self.assertEqual(proof['source_bytes'], 8)

    def test_altered_source_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = self.fixture(root)
            (source / 'Master.dss').write_bytes(b'modified')
            with patch.object(prepare.subprocess, 'check_output', side_effect=['pinned\n', '']):
                with self.assertRaisesRegex(ValueError, 'Source checksum mismatch'):
                    prepare.verify_sources(root, {'workflow_revision': 'pinned'})

    def test_manifest_cannot_escape_source(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root, '../outside.dss')
            with patch.object(prepare.subprocess, 'check_output', side_effect=['pinned\n', '']):
                with self.assertRaisesRegex(ValueError, 'Source checksum mismatch'):
                    prepare.verify_sources(root, {'workflow_revision': 'pinned'})

    def test_changed_workflow_and_revision_are_rejected(self):
        for responses in (['other\n'], ['pinned\n', ' M scripts/Texas7k/normalize.py\n']):
            with self.subTest(responses=responses):
                with patch.object(prepare.subprocess, 'check_output', side_effect=responses):
                    with self.assertRaises(ValueError):
                        prepare.verify_sources(Path('/unused'), {'workflow_revision': 'pinned'})


if __name__ == '__main__':
    unittest.main()
