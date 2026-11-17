"""Reproduce the original Austin P3U matrix discrepancy and its diagnostics.

Arguments: Rust test executable, KLU library, compiled klu_metrics library,
source directory from recover_smartds.py, and an output directory.
The exported matrix can be large; it stays in the chosen output directory.
"""

import json
import sys
import tempfile
from pathlib import Path

import compare
import numpy as np
import opendssdirect as dss


def run():
    executable, library, metrics_library, source, destination = sys.argv[1:6]
    destination = Path(destination)
    destination.mkdir(parents=True, exist_ok=True)
    ctx = dss.NewContext()
    ctx.Basic.AllowForms(False)
    ctx.Text.Command(f"compile [{Path(source).resolve() / 'Master.dss'}]")
    assert ctx.Solution.Converged()
    ctx.Text.Command("set controlmode=off")
    ctx.Solution.Solve()
    assert ctx.Solution.Converged()
    data, rows, pointers = ctx.YMatrix.getYsparse()
    original_voltage = np.array(ctx.YMatrix.getV()[2:]).view(complex)
    matrix = {
        "terminals": [label.rsplit(".", 1) for label in ctx.Circuit.YNodeOrder()],
        "entries": [
            (int(rows[k]), col, [float(data[k].real), float(data[k].imag)])
            for col in range(len(pointers) - 1)
            for k in range(pointers[col], pointers[col + 1])
        ],
        "rhs": np.array(ctx.YMatrix.getI()[2:]).reshape(-1, 2).tolist(),
    }
    (destination / "matrix.json").write_text(json.dumps(matrix, separators=(",", ":")))
    with tempfile.TemporaryDirectory() as directory:
        summary, solved, reference = compare.compare(
            executable,
            library,
            directory,
            "smartds-aus-p3u",
            0,
            matrix,
            voltage_tolerance=None,
            direct=True,
            keep_details=True,
        )
        summary["klu_vs_opendss_voltage_v"] = float(
            np.max(abs(reference - original_voltage))
        )
        error = compare.values(solved["faer"]["solution"]) - reference
        buses = {}
        for index, (bus, _) in enumerate(matrix["terminals"]):
            buses.setdefault(bus, []).append(index)
        differences = []
        for bus, indices in buses.items():
            if len(indices) > 1:
                differences.append(
                    {
                        "bus": bus,
                        "phase_to_ground_difference_v": float(
                            max(abs(error[i]) for i in indices)
                        ),
                        "within_bus_voltage_difference_v": float(
                            max(
                                abs(error[i] - error[j])
                                for i in indices
                                for j in indices
                            )
                        ),
                    }
                )
        differences.sort(
            key=lambda row: row["phase_to_ground_difference_v"], reverse=True
        )
        summary["largest_bus_differences"] = differences[:10]
        summary["max_within_bus_voltage_difference_v"] = max(
            row["within_bus_voltage_difference_v"] for row in differences
        )
        near_zero = abs(reference) < 1e-6
        summary["near_zero_reference_nodes"] = int(np.count_nonzero(near_zero))
        summary["near_zero_reference_difference_v"] = (
            float(np.max(abs(error)[near_zero])) if near_zero.any() else None
        )
        seeded = dict(
            matrix, initial_solution=[[float(v.real), float(v.imag)] for v in reference]
        )
        from_klu = compare.faer(executable, directory, matrix=seeded)
        summary["high_precision_start_agreement_v"] = float(
            np.max(
                abs(
                    compare.values(from_klu["faer"]["high_precision_refined_solution"])
                    - compare.values(solved["faer"]["high_precision_refined_solution"])
                )
            )
        )
        summary["high_precision_corrections_from_klu"] = from_klu["faer"][
            "high_precision_correction_sizes"
        ]
        fixed = {
            i
            for i, (bus, _) in enumerate(matrix["terminals"])
            if bus.lower() == "st_mat"
        }
        assert len(fixed) == 3
        unknown = [i for i in range(len(reference)) if i not in fixed]
        position = {old: new for new, old in enumerate(unknown)}
        rhs = compare.values(matrix["rhs"])[unknown].copy()
        entries = []
        for row, col, pair in matrix["entries"]:
            if row in fixed:
                continue
            if col in fixed:
                rhs[position[row]] -= complex(*pair) * reference[col]
            else:
                entries.append((position[row], position[col], pair))
        reduced = {
            "terminals": [matrix["terminals"][i] for i in unknown],
            "entries": entries,
            "rhs": [[float(v.real), float(v.imag)] for v in rhs],
        }
        reduced_summary = compare.compare(
            executable,
            library,
            directory,
            "opendss-source-eliminated-aus-p3u",
            0,
            reduced,
            voltage_tolerance=None,
            direct=True,
        )
    report = {
        "opendss_version": dss.Basic.Version(),
        "full": summary,
        "source_eliminated": reduced_summary,
    }
    (destination / "results.json").write_text(json.dumps(report, indent=2))
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    run()
