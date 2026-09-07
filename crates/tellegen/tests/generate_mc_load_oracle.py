#!/usr/bin/env python3
"""Generate synthetic load references with OpenDSSDirect.py 0.9.4.

No upstream feeder data is copied. Recorded DSS commands reconstruct every
case. The matching profile disables default voltage fallbacks and ZIPV dropout.
Optional --binary compares a native mc_pf runner against the same references.
"""

import argparse
import hashlib
import json
import math
from pathlib import Path
import subprocess

import opendssdirect as dss

V_NOM = 240.0
P_NOM = 2400.0
Q_NOM = 800.0
ANGLE_DEG = 37.0
PROFILE = (
    "Single phase; near-ideal source puZideal=1e-12, angle=37 degrees; "
    "Vnom=240 V, Pnom=2400 W, Qnom=800 var. Voltage models remain active "
    "throughout the positive voltage range: vminpu=0, vlowpu=0, "
    "vmaxpu=1e6. ZIP dropout=0. Controls disabled. Raw BMOPF source "
    "angles are radians; DSS command angles are degrees."
)


def zip_model(name, active, reactive):
    fields = {
        key: [value]
        for key, value in zip(
            ("alpha_z", "alpha_i", "alpha_p", "beta_z", "beta_i", "beta_p"),
            active + reactive,
        )
    }
    values = ",".join(str(value) for value in active + reactive + [0])
    return name, 8, fields, f"zipv=[{values}]", active, reactive


def exp_model(name, active, reactive):
    fields = {"gamma_p": [active], "gamma_q": [reactive]}
    properties = f"cvrwatts={active} cvrvars={reactive}"
    return name, 4, fields, properties, active, reactive


MODELS = [
    ("constant_power", 1, {}, "", [0, 0, 1], [0, 0, 1]),
    ("constant_current", 5, {}, "", [0, 1, 0], [0, 1, 0]),
    ("constant_impedance", 2, {}, "", [1, 0, 0], [1, 0, 0]),
    zip_model("zip", [.2, .3, .5], [.5, .1, .4]),
    zip_model("zip_nonunit", [-.2, .3, 1.2], [.5, -.1, .8]),
    exp_model("exponential", .7, 2.3),
    exp_model("exponential_negative", -.4, 1.4),
    exp_model("exponential_mixed", 0, 2),
    zip_model("zip_cancellation", [.5, -1.5, 1], [.2, -.6, .4]),
]


def complex_json(value):
    return {"re": value.real, "im": value.imag}


def as_complex(value):
    return complex(value["re"], value["im"])


def factor(coefficients, ratio):
    if isinstance(coefficients, list):
        return sum(c * ratio**e for c, e in zip(coefficients, (2, 1, 0)))
    return ratio**coefficients


def raw_input(case, name, fields, pu, resistance):
    bus = "lb" if resistance else "src"
    model = (
        "exponential" if name.startswith("exponential")
        else "zip" if name.startswith("zip") else name
    )
    raw = {
        "name": case,
        "terminal_conventions": {"phase": ["a"], "neutral": ["n"], "earth": []},
        "bus": {
            name: {
                "terminal_names": ["a", "n"],
                "perfectly_grounded_terminals": ["n"],
            }
            for name in (["src", "lb"] if resistance else ["src"])
        },
        "voltage_source": {
            "source": {
                "bus": "src", "terminal_map": ["a", "n"],
                "v_magnitude": [V_NOM * pu, 0],
                "v_angle": [math.radians(ANGLE_DEG), 0],
            }
        },
        "load": {
            "ld": {
                "bus": bus, "terminal_map": ["a", "n"],
                "configuration": "SINGLE_PHASE", "model": model,
                "p_nom": [P_NOM], "q_nom": [Q_NOM], "v_nom": [V_NOM],
                **fields,
            }
        },
    }
    if resistance:
        raw["line"] = {
            "l1": {
                "bus_from": "src", "bus_to": "lb",
                "terminal_map_from": ["a"], "terminal_map_to": ["a"],
                "length": 1, "linecode": "lc",
            }
        }
        raw["linecode"] = {
            "lc": {
                "R_series_1_1": .7, "X_series_1_1": .2,
                "G_from_1_1": 0, "G_to_1_1": 0,
                "B_from_1_1": 0, "B_to_1_1": 0,
            }
        }
    return raw


def reference_commands(model, properties, pu, resistance):
    bus = "lb" if resistance else "src"
    commands = [
        "clear",
        "set defaultbasefrequency=50",
        f"new circuit.qc phases=1 bus1=src.1.0 basekv=.24 pu={pu} "
        f"angle={ANGLE_DEG} frequency=50",
        "edit vsource.source model=ideal puZideal=[1e-12,0]",
    ]
    if resistance:
        commands.append(
            "new line.l1 phases=1 bus1=src.1 bus2=lb.1 "
            "rmatrix=[.7] xmatrix=[.2] cmatrix=[0] length=1 units=m"
        )
    commands.extend([
        f"new load.ld phases=1 bus1={bus}.1.0 conn=wye kv=.24 "
        f"kw=2.4 kvar=.8 model={model} vminpu=0 vlowpu=0 vmaxpu=1e6 {properties}",
        "set frequency=50 controlmode=off tolerance=1e-12 maxiterations=1000",
        "solve",
    ])
    return commands


def compare_native(binary, input_path, row, resistance):
    run = subprocess.run([binary, str(input_path)], capture_output=True, text=True)
    if run.returncode:
        row["tellegen_error"] = run.stderr.strip()
        return
    result = json.loads(run.stdout)
    bus = "lb" if resistance else "src"
    voltage = next(
        as_complex(t["voltage"]) for t in result["terminals"]
        if t["bus"] == bus and t["terminal"] == "a"
    )
    current = sum(
        as_complex(t["current_into_element"]) for t in result["element_ports"]
        if t["element"] == "ld" and t["terminal"] == "a"
    )
    row.update(
        voltage_error_V=abs(voltage - as_complex(row["voltage"])),
        current_error_A=abs(current - as_complex(row["current"])),
        iterations=result["iterations"],
        factorizations=result["factorization_count"],
        raw_kcl_A=result["physical_kcl_residual"],
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", help="Optional native mc_pf executable")
    parser.add_argument("--out", type=Path, default=Path(__file__).parent / "data/mc_load_oracle")
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    report = {"backend": dss.Basic.Version(), "profile": PROFILE, "cases": []}
    if args.binary:
        report["binary_sha256"] = hashlib.sha256(Path(args.binary).read_bytes()).hexdigest()
    for name, model, fields, properties, active, reactive in MODELS:
        voltages = [0, .8, 1, 1.2] if name == "constant_impedance" else [.8, 1, 1.2]
        for pu in voltages:
            for resistance in [0, .7]:
                case = f"{name}-{pu}-{resistance}"
                commands = reference_commands(model, properties, pu, resistance)
                for command in commands:
                    dss.Text.Command(command)
                assert dss.Solution.Converged(), case
                dss.Circuit.SetActiveElement("Load.ld")
                vv, ii = dss.CktElement.Voltages(), dss.CktElement.Currents()
                voltage = complex(vv[0], vv[1]) - complex(vv[2], vv[3])
                current = complex(ii[0], ii[1])
                ratio = abs(voltage) / V_NOM
                power = complex(P_NOM * factor(active, ratio), Q_NOM * factor(reactive, ratio))
                expected_current = (power / voltage).conjugate() if voltage else 0j
                error = abs(expected_current - current)
                assert error < 1e-8, (case, error)
                input_path = args.out / (case + ".json")
                raw = raw_input(case, name, fields, pu, resistance)
                input_path.write_text(json.dumps(raw, indent=2) + "\n")
                row = {
                    "case": case, "commands": commands,
                    "input_sha256": hashlib.sha256(input_path.read_bytes()).hexdigest(),
                    "voltage": complex_json(voltage), "current": complex_json(current),
                    "power": complex_json(voltage * current.conjugate()),
                    "analytic_current_error_A": error,
                }
                if args.binary:
                    compare_native(args.binary, input_path, row, resistance)
                report["cases"].append(row)
    (args.out / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({
        "cases": len(report["cases"]),
        "max_analytic_current_error_A": max(c["analytic_current_error_A"] for c in report["cases"]),
        "errors": [c for c in report["cases"] if "tellegen_error" in c],
    }, indent=2))


if __name__ == "__main__":
    main()
