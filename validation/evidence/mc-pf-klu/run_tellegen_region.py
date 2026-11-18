"""Compare faer and KLU on the system Tellegen itself prepares for Austin P3U.

Arguments: Rust test executable, KLU library, compiled klu_metrics library,
source directory from recover_smartds.py, and an output directory.
The test executable must be built against a PowerIO that reads the region
(0.12.3 or later). The exported matrix stays in the output directory.
"""

import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

import compare
import numpy as np


def sections_at_69kv(matrix, voltage):
    """Groups of connected nodes whose voltage to ground is that of 69 kV."""
    level = (abs(voltage) > 35e3) & (abs(voltage) < 45e3)
    parent = np.arange(len(voltage))

    def find(i):
        while parent[i] != i:
            parent[i] = parent[parent[i]]
            i = parent[i]
        return i

    for row, col, _pair in matrix["entries"]:
        if row != col and level[row] and level[col]:
            parent[find(row)] = find(col)
    groups = {}
    for i in np.flatnonzero(level):
        groups.setdefault(find(i), []).append(int(i))
    return sorted(groups.values(), key=len)


def within_bus(matrix, nodes, error):
    buses = {}
    for i in nodes:
        buses.setdefault(matrix["terminals"][i][0], []).append(i)
    return float(
        max(
            (abs(error[i] - error[j]) for group in buses.values() for i in group for j in group),
            default=0.0,
        )
    )


def run():
    executable, library, _metrics_library, source, destination = sys.argv[1:6]
    destination = Path(destination)
    destination.mkdir(parents=True, exist_ok=True)
    exported = destination / "tellegen-export.json"
    subprocess.run(
        [executable, compare.TEST, "--ignored", "--exact"],
        env=dict(
            os.environ,
            TELLEGEN_LINEAR_DSS=str(Path(source).resolve() / "Master.dss"),
            TELLEGEN_LINEAR_OUTPUT=str(exported),
        ),
        check=True,
        stdout=subprocess.DEVNULL,
    )
    matrix = json.loads(exported.read_text())["input"]
    with tempfile.TemporaryDirectory() as directory:
        summary, solved, klu = compare.compare(
            executable,
            library,
            directory,
            "tellegen-reduced-aus-p3u",
            0,
            matrix,
            voltage_tolerance=None,
            direct=True,
            keep_details=True,
        )
    faer = compare.values(solved["faer"]["solution"])
    refined = compare.values(solved["faer"]["high_precision_refined_solution"])
    error = faer - klu
    groups = sections_at_69kv(matrix, klu)
    inside = np.zeros(len(klu), dtype=bool)
    report = []
    for nodes in groups:
        inside[nodes] = True
        shift = error[nodes].mean()
        report.append(
            {
                "nodes": len(nodes),
                "buses": len({matrix["terminals"][i][0] for i in nodes}),
                "max_difference_to_ground_v": float(abs(error[nodes]).max()),
                "common_shift_v": float(abs(shift)),
                "max_difference_after_removing_shift_v": float(
                    abs(error[nodes] - shift).max()
                ),
                "max_difference_between_conductors_of_a_bus_v": within_bus(
                    matrix, nodes, error
                ),
                "mean_voltage_to_ground_v": {
                    "klu": float(abs(klu[nodes].mean())),
                    "faer": float(abs(faer[nodes].mean())),
                    "faer_refined_with_fx128_residuals": float(abs(refined[nodes].mean())),
                },
            }
        )
    worst = int(np.argmax(np.where(inside, 0.0, abs(error))))
    summary["sections_at_69kv"] = report
    summary["elsewhere"] = {
        "nodes": int(np.count_nonzero(~inside)),
        "max_difference_v": float(abs(error[~inside]).max()),
        "worst_terminal": matrix["terminals"][worst],
        "nodes_above_1e-3_v": int(np.count_nonzero(abs(error[~inside]) > 1e-3)),
    }
    summary["refined_vs_klu_elsewhere_v"] = float(abs(refined - klu)[~inside].max())
    text = json.dumps(summary, indent=2) + "\n"
    (destination / "results.json").write_text(text)
    print(text, end="")


if __name__ == "__main__":
    run()
