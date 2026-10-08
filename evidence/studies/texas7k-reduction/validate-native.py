#!/usr/bin/env python3
"""Run the original and reduced cases with one native binary and compare all results.

Usage: python3 validate-native.py MC_PF ORIGINAL REDUCED OPTIONS REPORT
Large result dumps stay in a temporary directory and are deleted after comparison.
"""

import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import time


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def run(binary, original, reduced, options, report_path):
    results, reports = [], []
    with tempfile.TemporaryDirectory(prefix="tellegen-reduction-") as directory:
        for label, source in [("original", original), ("reduced", reduced)]:
            target = Path(directory) / (label + ".json")
            started = time.perf_counter()
            with target.open("wb") as output:
                completed = subprocess.run([binary, source, options], stdout=output, stderr=subprocess.PIPE)
            elapsed = time.perf_counter() - started
            if completed.returncode:
                raise RuntimeError(completed.stderr.decode())
            value = json.loads(target.read_bytes())
            assert value["converged"], label
            results.append(value)
            report = {"case": label, "input_sha256": sha(source), "output_sha256": sha(target), "seconds_including_parse_solve_and_result_write": elapsed}
            for key in ["converged", "iterations", "factorization_count", "matrix_dimension", "matrix_nonzeros", "voltage_change", "physical_kcl_residual", "scaled_kcl_residual"]:
                report[key] = value[key]
            report["terminal_count"] = len(value["terminals"])
            report["element_port_count"] = len(value["element_ports"])
            reports.append(report)
        a, b = results
        assert len(a["terminals"]) == len(b["terminals"])
        maximum = 0.0
        for left, right in zip(a["terminals"], b["terminals"]):
            assert (left["bus"], left["terminal"]) == (right["bus"], right["terminal"])
            maximum = max(maximum, abs(complex(left["voltage"]["re"], left["voltage"]["im"]) - complex(right["voltage"]["re"], right["voltage"]["im"])))
        report = {
            "binary": str(Path(binary).name), "binary_sha256": sha(binary),
            "options": json.loads(Path(options).read_text()), "runs": reports,
            "all_result_fields_exactly_equal": a == b,
            "max_complex_terminal_voltage_difference_v": maximum,
            "timing_note": "One run per case, original first; includes parsing and full result serialization. Not a controlled speed benchmark.",
        }
        Path(report_path).write_text(json.dumps(report, indent=2) + "\n")
        print(json.dumps(report, indent=2))
        assert a == b, "Power-flow result changed"


if __name__ == "__main__":
    if len(sys.argv) != 6:
        raise SystemExit(__doc__)
    run(*sys.argv[1:])
