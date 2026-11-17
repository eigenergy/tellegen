"""Compare the same matrix/RHS with retained faer and the OpenDSS KLU library.

Arguments: compiled release test executable, libklusolvex shared library.
Requires numpy and OpenDSSDirect.py. Writes only the JSON report to stdout.
"""

import ctypes as c
import hashlib
import json
import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path

import numpy as np
import opendssdirect as dss

TEST = "mc_pf::network::linear_comparison::export_comparison"


def faer(executable, directory, sections=None, matrix=None):
    output = Path(directory) / "output.json"
    env = dict(os.environ, TELLEGEN_LINEAR_OUTPUT=str(output))
    if matrix is not None:
        source = Path(directory) / "input.json"
        source.write_text(json.dumps(matrix))
        env["TELLEGEN_LINEAR_INPUT"] = str(source)
    else:
        env["TELLEGEN_LINEAR_SECTIONS"] = str(sections)
    subprocess.run(
        [executable, TEST, "--ignored", "--exact"],
        env=env,
        check=True,
        stdout=subprocess.DEVNULL,
    )
    return json.loads(output.read_text())


def klu(library, matrix):
    lib = c.CDLL(library)
    lib.NewSparseSet.argtypes = [c.c_uint]
    lib.NewSparseSet.restype = c.c_void_p
    for name in ["DeleteSparseSet", "FactorSparseMatrix"]:
        getattr(lib, name).argtypes = [c.c_void_p]
    lib.AddPrimitiveMatrix.argtypes = [
        c.c_void_p,
        c.c_uint,
        c.POINTER(c.c_uint),
        c.POINTER(c.c_double),
    ]
    lib.SolveSparseSet.argtypes = [
        c.c_void_p,
        c.POINTER(c.c_double),
        c.POINTER(c.c_double),
    ]
    n = len(matrix["rhs"])
    handle = lib.NewSparseSet(n)
    assert handle
    try:
        for row, col, value in matrix["entries"]:
            if row == col:
                assert (
                    lib.AddPrimitiveMatrix(
                        handle, 1, (c.c_uint * 1)(row + 1), (c.c_double * 2)(*value)
                    )
                    == 1
                )
            else:
                # Column-major primitive; AddMatrixElement would mirror off-diagonals.
                assert (
                    lib.AddPrimitiveMatrix(
                        handle,
                        2,
                        (c.c_uint * 2)(row + 1, col + 1),
                        (c.c_double * 8)(0, 0, 0, 0, *value, 0, 0),
                    )
                    == 1
                )
        start = time.perf_counter()
        assert lib.FactorSparseMatrix(handle) == 1
        factor_seconds = time.perf_counter() - start
        # Verify the matrix KLU will factor, including nonsymmetric scaled cases.
        lib.GetNNZ.argtypes = [c.c_void_p, c.POINTER(c.c_uint)]
        count = c.c_uint()
        assert lib.GetNNZ(handle, c.byref(count)) == 1
        pointers, rows, data = (
            (c.c_uint * (n + 1))(),
            (c.c_uint * count.value)(),
            (c.c_double * (2 * count.value))(),
        )
        lib.GetCompressedMatrix.argtypes = [
            c.c_void_p,
            c.c_uint,
            c.c_uint,
            c.POINTER(c.c_uint),
            c.POINTER(c.c_uint),
            c.POINTER(c.c_double),
        ]
        assert (
            lib.GetCompressedMatrix(handle, n + 1, count.value, pointers, rows, data)
            == 1
        )
        actual = {
            (rows[k], col): complex(data[2 * k], data[2 * k + 1])
            for col in range(n)
            for k in range(pointers[col], pointers[col + 1])
            if data[2 * k] or data[2 * k + 1]
        }
        expected = {
            (r, col): complex(*value)
            for r, col, value in matrix["entries"]
            if any(value)
        }
        assert actual == expected, "KLU input matrix differs"
        rhs = (c.c_double * (2 * n))(
            *(part for value in matrix["rhs"] for part in value)
        )
        solution = (c.c_double * (2 * n))()
        assert lib.SolveSparseSet(handle, solution, rhs) == 1
        start = time.perf_counter()
        for _ in range(20):
            assert lib.SolveSparseSet(handle, solution, rhs) == 1
        warm_seconds = (time.perf_counter() - start) / 20
        info = {}
        for name, kind in [
            ("GetCondEst", c.c_double),
            ("GetRCond", c.c_double),
            ("GetSparseNNZ", c.c_uint),
        ]:
            fn = getattr(lib, name)
            fn.argtypes = [c.c_void_p, c.POINTER(kind)]
            value = kind()
            assert fn(handle, c.byref(value)) == 1
            info[name] = value.value
        return np.array(solution).view(complex).copy(), dict(
            factor_seconds=factor_seconds, warm_solve_seconds=warm_seconds, **info
        )
    finally:
        lib.DeleteSparseSet(handle)


def direct_klu(metrics_library, matrix, ordering):
    library = c.CDLL(metrics_library)
    pointer = c.POINTER(c.c_double)
    library.compare_klu.argtypes = [
        c.c_int,
        c.POINTER(c.c_int),
        c.POINTER(c.c_int),
        pointer,
        pointer,
        c.c_int,
        pointer,
        pointer,
    ]
    n = len(matrix["rhs"])
    entries = sorted(matrix["entries"], key=lambda item: (item[1], item[0]))
    columns = np.zeros(n + 1, dtype=np.int32)
    for _r, col, _v in entries:
        columns[col + 1] += 1
    np.cumsum(columns, out=columns)
    rows = np.array([r for r, col, v in entries], dtype=np.int32)
    data = np.array([v for r, col, v in entries], dtype=float).reshape(-1)
    rhs = np.array(matrix["rhs"], dtype=float).reshape(-1)
    output = np.zeros_like(rhs)
    metrics = np.zeros(7)
    status = library.compare_klu(
        n,
        columns.ctypes.data_as(c.POINTER(c.c_int)),
        rows.ctypes.data_as(c.POINTER(c.c_int)),
        data.ctypes.data_as(pointer),
        rhs.ctypes.data_as(pointer),
        ordering,
        output.ctypes.data_as(pointer),
        metrics.ctypes.data_as(pointer),
    )
    assert status == 0, status
    return output.view(complex), dict(
        zip(
            [
                "factor_seconds",
                "warm_solve_seconds",
                "retained_heap_bytes",
                "peak_heap_bytes",
                "factor_nonzeros",
                "condition_estimate",
                "reciprocal_pivot_ratio",
            ],
            metrics.tolist(),
        )
    )


def values(pairs):
    return np.array(pairs, dtype=float).view(complex).reshape(-1)


def residual(matrix, voltage):
    rhs = values(matrix["rhs"])
    mismatch = rhs.copy()
    scale = np.abs(rhs)
    for row, col, pair in matrix["entries"]:
        value = complex(*pair)
        mismatch[row] -= value * voltage[col]
        scale[row] += abs(value) * abs(voltage[col])
    return float(np.max(np.abs(mismatch) / np.maximum(scale, np.finfo(float).tiny)))


def raw_dss(sections):
    ctx = dss.NewContext()
    commands = [
        "clear",
        "new circuit.test phases=1 bus1=b0.1 basekv=0.23 r1=0.001 x1=0.001 r0=0.001 x0=0.001",
        "new linecode.c nphases=2 rmatrix=[0.1|0.02 0.12] xmatrix=[0.04|0.01 0.05] cmatrix=[0|0 0] units=none",
    ]
    for i in range(1, sections + 1):
        parent = (i - 1) // 2
        commands += [
            f"new line.l{i} phases=2 bus1=b{parent}.1.{0 if parent == 0 else 4} bus2=b{i}.1.4 linecode=c length=1 units=none",
            f"new load.d{i} phases=1 bus1=b{i}.1.4 conn=delta model=1 kv=.23 kw=.001 kvar=.0002",
        ]
    commands += [
        "set controlmode=off",
        "set tolerance=1e-12 maxiterations=100",
        "solve",
    ]
    for command in commands:
        ctx.Text.Command(command)
    assert ctx.Solution.Converged()
    data, rows, pointers = ctx.YMatrix.getYsparse()
    entries = [
        (int(rows[k]), col, [float(data[k].real), float(data[k].imag)])
        for col in range(len(pointers) - 1)
        for k in range(pointers[col], pointers[col + 1])
    ]
    rhs = np.array(ctx.YMatrix.getI()[2:]).reshape(-1, 2).tolist()
    return {
        "terminals": [label.rsplit(".", 1) for label in ctx.Circuit.YNodeOrder()],
        "entries": entries,
        "rhs": rhs,
    }


def compare(
    executable,
    library,
    directory,
    name,
    sections,
    matrix=None,
    voltage_tolerance=2e-5,
    direct=False,
    keep_details=False,
):
    result = faer(executable, directory, sections, matrix)
    matrix = result["input"]
    reference, timing = (
        direct_klu(sys.argv[3], matrix, 0) if direct else klu(library, matrix)
    )
    solution = values(result["faer"]["solution"])
    refined = values(result["faer"]["refined_solution"])
    classes = {}
    for label in ["phase", "neutral"]:
        mask = np.array(
            [
                (terminal == "4") == (label == "neutral")
                for _, terminal in matrix["terminals"]
            ]
        )
        if mask.any():
            classes[label] = float(np.max(np.abs(solution - reference)[mask]))
    # Apply row and column equilibration to both solvers, retaining original units.
    n = len(reference)
    rowmax = np.zeros(n)
    for row, _col, pair in matrix["entries"]:
        rowmax[row] = max(rowmax[row], abs(complex(*pair)))
    row_scale = 1 / np.maximum(rowmax, np.finfo(float).tiny)
    colmax = np.zeros(n)
    for row, col, pair in matrix["entries"]:
        colmax[col] = max(colmax[col], abs(complex(*pair)) * row_scale[row])
    col_scale = 1 / np.maximum(colmax, np.finfo(float).tiny)
    scaled = {
        "terminals": matrix["terminals"],
        "entries": [
            (r, col, [part * row_scale[r] * col_scale[col] for part in pair])
            for r, col, pair in matrix["entries"]
        ],
        "rhs": [
            [part * row_scale[r] for part in pair]
            for r, pair in enumerate(matrix["rhs"])
        ],
    }
    scaled_result = faer(executable, directory, matrix=scaled)
    scaled_faer = values(scaled_result["faer"]["solution"]) * col_scale
    scaled_klu, _ = (
        direct_klu(sys.argv[3], scaled, 0) if direct else klu(library, scaled)
    )
    scaled_klu *= col_scale
    reversed_input = {
        "terminals": list(reversed(matrix["terminals"])),
        "entries": [
            (n - 1 - r, n - 1 - col, pair) for r, col, pair in matrix["entries"]
        ],
        "rhs": list(reversed(matrix["rhs"])),
    }
    reversed_result = faer(executable, directory, matrix=reversed_input)
    reversed_solution = values(reversed_result["faer"]["solution"])[::-1]
    ordering_measurements = {}
    for ordering, ordering_name in [(0, "amd"), (1, "colamd")]:
        ordered_solution, ordered_metrics = direct_klu(sys.argv[3], matrix, ordering)
        ordered_metrics["difference_from_reference_v"] = float(
            np.max(np.abs(ordered_solution - reference))
        )
        ordered_metrics["backward_residual"] = residual(matrix, ordered_solution)
        ordering_measurements[ordering_name] = ordered_metrics
    error = float(np.max(np.abs(solution - reference)))
    if voltage_tolerance is not None:
        assert error < voltage_tolerance, (name, error)
    worst = int(np.argmax(np.abs(solution - reference)))
    summary = {
        "name": name,
        "dimension": n,
        "nonzeros": len(matrix["entries"]),
        "matrix_rhs_sha256": hashlib.sha256(
            json.dumps(matrix, separators=(",", ":")).encode()
        ).hexdigest(),
        "faer": {k: v for k, v in result["faer"].items() if not k.endswith("solution")},
        "klu": timing,
        "klu_orderings": ordering_measurements,
        "max_voltage_difference_v": error,
        "worst_terminal": matrix["terminals"][worst],
        "worst_reference_voltage_v": [
            float(reference[worst].real),
            float(reference[worst].imag),
        ],
        "difference_by_terminal_v": classes,
        "high_precision_refined_difference_v": float(
            np.max(
                np.abs(
                    values(result["faer"]["high_precision_refined_solution"])
                    - reference
                )
            )
        ),
        "faer_refined_difference_v": float(np.max(np.abs(refined - reference))),
        "scaled_faer_difference_v": float(np.max(np.abs(scaled_faer - reference))),
        "reverse_input_order_difference_v": float(
            np.max(np.abs(reversed_solution - reference))
        ),
        "scaled_klu_difference_v": float(np.max(np.abs(scaled_klu - reference))),
        "row_scaled_backward_residual": {
            "faer": residual(matrix, solution),
            "klu": residual(matrix, reference),
            "faer_refined": residual(matrix, refined),
            "faer_scaled": residual(matrix, scaled_faer),
        },
    }
    return (summary, result, reference) if keep_details else summary


if __name__ == "__main__":
    executable, library = sys.argv[1:3]
    with tempfile.TemporaryDirectory() as directory:
        cases = [
            compare(executable, library, directory, f"tellegen-reduced-{n}", n)
            for n in [100, 1000, 10000]
        ]
        cases += [
            compare(executable, library, directory, f"opendss-raw-{n}", n, raw_dss(n))
            for n in [100, 1000]
        ]
    print(
        json.dumps(
            {
                "opendss_version": dss.Basic.Version(),
                "klu_library_sha256": hashlib.sha256(
                    Path(library).read_bytes()
                ).hexdigest(),
                "cases": cases,
            },
            indent=2,
        )
    )
