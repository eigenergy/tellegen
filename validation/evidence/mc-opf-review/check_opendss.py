"""Re-solve small MC OPF dispatches in OpenDSS, without PowerIO's DSS adapter.

Run with opendssdirect.py 0.9.4 and pass the mc_opf_case_probe executable.
The source fixture is copied and simplified to constant-power devices and lines.
This checks feasibility of the projected dispatch, not global optimality.
"""
import copy
import json
import math
import subprocess
import sys
import tempfile
from pathlib import Path

import opendssdirect as dss

ROOT = Path(__file__).resolve().parents[3]
BASE = json.loads((ROOT / "crates/tellegen/tests/data/mc_opf/unbalanced.json").read_text())
NODES = {"a": 1, "b": 2, "c": 3, "n": 4}


def matrix(code, prefix):
    return "[" + " | ".join(" ".join(str(code.get(f"{prefix}_{i}_{j}", 0)) for j in range(1, i + 1)) for i in range(1, 5)) + "]"


def run(executable, connection, grounded):
    data = copy.deepcopy(BASE)
    # A separate OpenDSS line has no asymmetric end conductance model.
    code = data["linecode"]["c"]
    for key in list(code):
        if key.startswith(("G_", "B_")):
            del code[key]
    generator = data["generator"]["g"]
    generator["configuration"] = connection
    generator["terminal_map"] = ["a", "b", "c"] + (["n"] if connection == "WYE" else [])
    generator["i_max"] = [2] * len(generator["terminal_map"])
    if grounded:
        data["bus"]["b"]["perfectly_grounded_terminals"] = ["n"]
    with tempfile.TemporaryDirectory() as directory:
        path = Path(directory) / "case.bmopf.json"
        path.write_text(json.dumps(data))
        record = json.loads(subprocess.check_output([executable, str(path)], text=True))
    result = record["result"]
    assert result["status"] == "accepted", result
    ctx = dss.NewContext()
    commands = ["clear", f"new circuit.check bus1=s.1.2.3 basekv={230 * math.sqrt(3) / 1000} pu=1 angle=0 r1=1e-8 x1=1e-8 r0=1e-8 x0=1e-8",
                f"new linecode.c nphases=4 units=none rmatrix={matrix(code, 'R_series')} xmatrix={matrix(code, 'X_series')} cmatrix=[0|0 0|0 0 0|0 0 0 0]",
                f"new line.l phases=4 bus1=s.1.2.3.0 bus2=b.1.2.3.{0 if grounded else 4} linecode=c length=1 units=none"]
    ledgers = {entry["identity"].split(":")[-1]: entry for entry in result["devices"]}
    for kind in ["load", "generator"]:
        for name, device in data[kind].items():
            terminals = device["terminal_map"]
            pairs = list(zip(terminals[:3], [terminals[-1]] * 3)) if device["configuration"] == "WYE" else list(zip(terminals, terminals[1:] + terminals[:1]))
            powers = ledgers[name]["coil_power_va"]
            assert len(pairs) == len(powers)
            for index, ((a, b), (p, q)) in enumerate(zip(pairs, powers)):
                # Generation-positive coil dispatch becomes negative DSS load.
                sign = -1 if kind == "generator" else 1
                nodes = [0 if grounded and terminal == "n" else NODES[terminal] for terminal in [a, b]]
                kv = (230 if device["configuration"] == "WYE" else 230 * math.sqrt(3)) / 1000
                commands.append(f"new load.{kind}_{name}_{index} phases=1 bus1=b.{nodes[0]}.{nodes[1]} conn=delta model=1 kv={kv} kw={sign*p/1000} kvar={sign*q/1000} vminpu=0.1 vmaxpu=2")
    commands += ["set controlmode=off", "set tolerance=1e-12 maxiterations=100", "solve"]
    for command in commands:
        ctx.Text.Command(command)
    assert ctx.Solution.Converged()
    voltage_error = 0.0
    for bus in ["s", "b"]:
        ctx.Circuit.SetActiveBus(bus)
        values = ctx.Bus.Voltages()
        voltage = dict(zip(ctx.Bus.Nodes(), [complex(*values[i:i+2]) for i in range(0, len(values), 2)]))
        for terminal, expected in result["bus"][bus].items():
            node = 0 if terminal == "n" and (bus == "s" or grounded) else NODES[terminal]
            actual = 0j if node == 0 else voltage[node]
            voltage_error = max(voltage_error, abs(actual - complex(expected["vr"], expected["vi"])))
    ctx.Circuit.SetActiveElement("line.l")
    values = ctx.CktElement.Currents()
    actual = [complex(*values[i:i+2]) for i in range(0, len(values), 2)]
    branch = result["branches"][0]
    expected = [complex(*value) for value in branch["current_from_a"] + branch["current_to_a"]]
    assert len(actual) == len(expected) == 8
    current_error = max(abs(a-b) for a,b in zip(actual, expected))
    assert voltage_error < 2e-5, voltage_error
    assert current_error < 2e-5, current_error
    return {"generator_connection": connection, "load_neutral_grounded": grounded,
            "max_voltage_error_v": voltage_error, "max_line_current_error_a": current_error,
            "opf_residuals": result["residuals"], "dispatch_w_var": ledgers["g"]["coil_power_va"],
            "input": data, "opendss_commands": commands}


if __name__ == "__main__":
    report = {"opendss_version": dss.Basic.Version(), "cases": [run(sys.argv[1], connection, grounded) for connection in ["WYE", "DELTA"] for grounded in [False, True]]}
    print(json.dumps(report, indent=2))
