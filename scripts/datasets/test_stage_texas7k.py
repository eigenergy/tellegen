"""Failure-path tests: never replace a manifest with unverified case data."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('stage', Path(__file__).with_name('stage-texas7k.py'))
stage = importlib.util.module_from_spec(spec)
spec.loader.exec_module(stage)


class StageTests(unittest.TestCase):
    def test_checksum_failure_preserves_manifest_and_leaves_no_bundle(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = root / 'distribution-cases.json'
            previous = '[{"id":"existing","file":"existing.pio.json","name":"Existing"}]\n'
            manifest.write_text(previous)
            with patch.object(stage.subprocess, 'check_output', return_value=b'changed source'):
                with self.assertRaisesRegex(ValueError, 'checksum mismatch'):
                    stage.stage(root, root, root)
            self.assertEqual(manifest.read_text(), previous)
            self.assertEqual(list(root.iterdir()), [manifest])

    def test_duplicate_ids_fail_before_running_conversion(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = root / 'distribution-cases.json'
            previous = json.dumps([{'id':'same'}, {'id':'same'}])
            manifest.write_text(previous)
            with patch.object(stage.subprocess, 'check_output') as read:
                with self.assertRaisesRegex(ValueError, 'duplicate IDs'):
                    stage.stage(root, root, root)
                read.assert_not_called()
            self.assertEqual(manifest.read_text(), previous)


if __name__ == '__main__':
    unittest.main()
