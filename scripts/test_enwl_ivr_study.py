"""Cheap harness contracts: no solver, external dataset or Julia is launched."""
import contextlib
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import enwl_ivr_study as study


class StudyTests(unittest.TestCase):
    def test_qualification_refusal_preserves_acceptance_and_records_budget(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binary = root / "probe"
            binary.write_bytes(b"test binary")
            case = root / "feeder.bmopf.json"
            case.write_text("{}")
            result = {"status": "accepted", "qualification": {
                "dual_status": "rank_not_checked", "sensitivity": {"status": "unavailable"}}}
            record = {"result": result, "sha256": "input", "elapsed_s": 0.2}
            with patch.object(study.subprocess, "run", return_value=subprocess.CompletedProcess(
                    [], 0, json.dumps(record), "")) as run, contextlib.redirect_stdout(io.StringIO()):
                self.assertTrue(study.native([case], binary, root, qualify=True, timeout=3))
            self.assertEqual(run.call_args.args[0][-1], "--qualify")
            self.assertEqual(run.call_args.kwargs["timeout"], 3)
            row = json.loads((root / "native-summary.json").read_text())[0]
            self.assertEqual(row["qualification"], result["qualification"])
            self.assertEqual(len(row["binary_sha256"]), 64)
            self.assertTrue(row["qualify"])
            # A failed rerun overwrites the prior accepted result.
            with patch.object(study.subprocess, "run", side_effect=subprocess.TimeoutExpired([], 3)), contextlib.redirect_stdout(io.StringIO()):
                self.assertFalse(study.native([case], binary, root, timeout=3))
            self.assertEqual(json.loads((root / "feeder-native.json").read_text())["result"]["status"], "timeout")

    def test_explicit_case_selection_is_relative_and_limited(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = root / "selection.json"
            manifest.write_text(json.dumps(["one.bmopf.json", "two.bmopf.json"]))
            args = ["study", "--cases", str(manifest), "--binary", "probe", "--output", str(root), "--limit", "1"]
            with patch.object(sys, "argv", args), patch.object(study, "native", return_value=True) as native:
                self.assertTrue(study.main())
            self.assertEqual(native.call_args.args[0], [root / "one.bmopf.json"])
            self.assertFalse(native.call_args.kwargs["qualify"])
            for tail in [["--limit", "0"], ["--timeout", "nan"], ["--timeout", "0"]]:
                with patch.object(sys, "argv", args + tail), contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                    study.main()

    def test_duplicate_output_names_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = root / "selection.json"
            manifest.write_text(json.dumps(["a/same.bmopf.json", "b/same.bmopf.json"]))
            with patch.object(sys, "argv", ["study", "--cases", str(manifest), "--binary", "probe", "--output", str(root)]), contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                study.main()


if __name__ == "__main__":
    unittest.main()
