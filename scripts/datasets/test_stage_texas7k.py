"""Failure-path tests: never replace a manifest with unverified case data."""
import importlib.util
import json
import os
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('stage', Path(__file__).with_name('stage-texas7k.py'))
stage = importlib.util.module_from_spec(spec)
spec.loader.exec_module(stage)


class StageTests(unittest.TestCase):
    def test_staging_wrapper_prefers_prepared_main_island(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            tools = root / 'bin'
            tools.mkdir()
            capture = root / 'arguments'
            python = tools / 'python3'
            python.write_text('#!/bin/sh\nprintf \'%s\\n\' "$@" > "$STAGE_TEST_CAPTURE"\n')
            python.chmod(0o755)
            main, legacy, destination = root / 'main', root / 'legacy', root / 'data'
            env = {**os.environ, 'PATH': str(tools) + os.pathsep + os.environ['PATH'],
                   'STAGE_TEST_CAPTURE': str(capture), 'TELLEGEN_TEXAS7K_MAIN_ISLAND': str(main),
                   'TELLEGEN_BMOPF_REPO': str(legacy)}
            subprocess.run(['sh', str(Path(stage.__file__).parents[1] / 'stage-data.sh'),
                            str(root / 'datasets'), str(destination)],
                           env=env, capture_output=True, text=True, check=True)
            self.assertEqual(capture.read_text().splitlines(),
                             [str(Path(stage.__file__)), str(main), str(destination), '--main-island'])

    def test_main_island_checksum_failure_preserves_manifest(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source, destination = root / 'source', root / 'data'
            source.mkdir()
            destination.mkdir()
            (source / 'p1uhs0_1247.main-island.bmopf.json').write_bytes(b'changed source')
            manifest = destination / 'distribution-cases.json'
            previous = '[{"id":"existing","file":"existing.pio.json","name":"Existing"}]\n'
            manifest.write_text(previous)
            with patch.object(stage.subprocess, 'run') as convert:
                with self.assertRaisesRegex(ValueError, 'checksum mismatch'):
                    stage.stage(source, destination, root, main_island=True)
                convert.assert_not_called()
            self.assertEqual(manifest.read_text(), previous)
            self.assertEqual(list(destination.iterdir()), [manifest])

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
