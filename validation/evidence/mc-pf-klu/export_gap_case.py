"""Export what explain_gap.py needs from the Austin P3U case.

Arguments: source directory from recover_smartds.py, output directory.
Writes the OpenDSS matrix, current vector and KLU voltages, the elements that
touch a 69 kV bus (with their primitive matrices), and the same matrix and
voltages after scaling ppm_antifloat on the transformers with a 69 kV winding.
The files are large and stay in the output directory.
"""

import json
import sys
from pathlib import Path

import numpy as np
import opendssdirect as dss
import scipy.sparse as sp

ANTIFLOAT_FACTORS = [0.1, 10.0, 100.0]


def system(ctx):
    data, rows, pointers = ctx.YMatrix.getYsparse()
    n = len(pointers) - 1
    matrix = sp.csc_matrix(
        (np.asarray(data), np.asarray(rows), np.asarray(pointers)), shape=(n, n)
    )
    voltage = np.array(ctx.YMatrix.getV()[2:]).view(complex).copy()
    current = np.array(ctx.YMatrix.getI()[2:]).view(complex).copy()
    return matrix, voltage, current


def run():
    source, destination = sys.argv[1:3]
    destination = Path(destination)
    destination.mkdir(parents=True, exist_ok=True)
    ctx = dss.NewContext()
    ctx.Basic.AllowForms(False)
    ctx.Text.Command(f"compile [{Path(source).resolve() / 'Master.dss'}]")
    assert ctx.Solution.Converged()
    ctx.Text.Command("set controlmode=off")
    ctx.Solution.Solve()
    assert ctx.Solution.Converged()
    matrix, voltage, current = system(ctx)
    sp.save_npz(destination / "Y.npz", matrix)
    np.savez(destination / "vectors.npz", klu=voltage, rhs=current)
    order = list(ctx.Circuit.YNodeOrder())
    index = {name: i for i, name in enumerate(order)}

    level = set()
    for name in ctx.Circuit.AllBusNames():
        ctx.Circuit.SetActiveBus(name)
        if abs(ctx.Bus.kVBase() * 3**0.5 - 69.0) < 0.5:
            level.add(name.lower())
    elements = []
    for name in ctx.Circuit.AllElementNames():
        ctx.Circuit.SetActiveElement(name)
        kind = name.split(".", 1)[0]
        buses = [bus.split(".")[0] for bus in ctx.CktElement.BusNames()]
        if kind == "Fuse" or not any(bus.lower() in level for bus in buses):
            continue
        conductors = ctx.CktElement.NumConductors()
        nodes = [
            index.get(f"{buses[k // conductors].upper()}.{node}", -1)
            for k, node in enumerate(ctx.CktElement.NodeOrder())
        ]
        primitive = np.asarray(ctx.CktElement.YPrim())
        if not np.iscomplexobj(primitive):
            primitive = primitive.view(complex)
        record = {
            "name": name,
            "buses": buses,
            "at_69kv": [bus.lower() in level for bus in buses],
            "conductors": conductors,
            "nodes": nodes,
            "yprim": [[float(v.real), float(v.imag)] for v in primitive.ravel()],
        }
        if kind == "Transformer":
            ctx.Transformers.Name(name.split(".", 1)[1])
            record["windings"] = []
            for winding in range(ctx.Transformers.NumWindings()):
                ctx.Transformers.Wdg(winding + 1)
                record["windings"].append(
                    {
                        "connection": "delta" if ctx.Transformers.IsDelta() else "wye",
                        "kv": ctx.Transformers.kV(),
                        "kva": ctx.Transformers.kVA(),
                    }
                )
            ctx.Text.Command(f"? {name}.ppm_antifloat")
            record["ppm_antifloat"] = float(ctx.Text.Result())
        if kind == "Line":
            ctx.Lines.Name(name.split(".", 1)[1])
            phases = ctx.Lines.Phases()
            record["length"] = ctx.Lines.Length()
            record["linecode"] = ctx.Lines.LineCode()
            record["capacitance_nf_per_length"] = (
                np.asarray(ctx.Lines.CMatrix()).reshape(phases, phases).tolist()
            )
        elements.append(record)
    (destination / "elements.json").write_text(
        json.dumps(
            {
                "frequency_hz": ctx.Solution.Frequency(),
                "nodes": order,
                "elements": elements,
            }
        )
    )

    bounding = [e for e in elements if "windings" in e]
    for factor in ANTIFLOAT_FACTORS:
        for element in bounding:
            value = element["ppm_antifloat"] * factor
            ctx.Text.Command(f"{element['name']}.ppm_antifloat={value}")
        ctx.Solution.Solve()
        assert ctx.Solution.Converged()
        changed, changed_voltage, changed_current = system(ctx)
        sp.save_npz(destination / f"Y-antifloat-x{factor:g}.npz", changed)
        np.savez(
            destination / f"vectors-antifloat-x{factor:g}.npz",
            klu=changed_voltage,
            rhs=changed_current,
        )
    print("exported", len(order), "nodes,", len(elements), "elements at 69 kV")


if __name__ == "__main__":
    run()
