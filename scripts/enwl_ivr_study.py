#!/usr/bin/env python3
"""Opt-in ENWL study; original cases stay external and are never rewritten.

Run native cases with --binary, then run mc_opf_external_oracle.jl on cases.json,
then invoke this script with --compare to check independently mapped SI results.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path
import subprocess

# Absolute SI tolerances chosen for this larger, smooth-control study.
TOLERANCES = {"objective": 1e-4, "voltage_v": 1e-3, "line_current_a": 1e-2,
              "ibr_power_va": 0.1, "original_active_bound_w": 1e-4}


def write(path, value):
    path.write_text(json.dumps(value, indent=2, allow_nan=False) + "\n")


def plan(root):
    return [next((root / f"{n}bus_{kind}").glob(f"*_t{slot}_*.bmopf.json"))
            for n in [30, 99, 538] for kind in ["LG", "LN"]
            for slot in ["01", "12", "25"]]


def native(cases, binary, output):
    rows = []
    write(output / "cases.json", [str(p.resolve()) for p in cases])
    for path in cases:
        stem = path.name.removesuffix(".bmopf.json")
        try:
            run = subprocess.run([str(binary.resolve()), str(path.resolve())],
                                 capture_output=True, text=True, timeout=120, check=True)
            (output / f"{stem}-native.log").write_text(run.stderr)
            record = json.loads(run.stdout.strip().splitlines()[-1])
            write(output / f"{stem}-native.json", record)
            row = {k: record["result"].get(k) for k in
                   ["status", "objective", "iterations", "residuals", "counts", "error"]}
            row.update(case=stem, elapsed_s=record["elapsed_s"], sha256=record["sha256"])
        except (subprocess.SubprocessError, ValueError, IndexError) as error:
            row = dict(case=stem, status="harness_error", error=str(error))
        rows.append(row)
        write(output / "native-summary.json", rows)
        print(json.dumps(row), flush=True)
    return all(r["status"] == "accepted" for r in rows)


def complex_error(actual, expected):
    return math.hypot(actual[0] - expected[0], actual[1] - expected[1])


def compare(output):
    oracle_rows = {r["case"]: r for r in json.loads((output / "oracle-summary.json").read_text())}
    rows = []
    for path in map(Path, json.loads((output / "cases.json").read_text())):
        stem = path.name.removesuffix(".bmopf.json")
        data = json.loads(path.read_text())
        record = json.loads((output / f"{stem}-native.json").read_text())
        oracle = json.loads((output / f"{stem}-oracle.json").read_text())
        native_result = record["result"]
        ref_meta = oracle_rows[stem]
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        assert digest == record["sha256"] == ref_meta["sha256"], stem
        assert native_result["status"] == "accepted", (stem, native_result)
        assert ref_meta["status"] in ["LOCALLY_SOLVED", "OPTIMAL"], (stem, ref_meta)
        assert oracle["feasible"], stem
        metrics = {k: 0.0 for k in TOLERANCES}
        metrics["objective"] = abs(native_result["objective"] - oracle["objective"])
        for bus, terminals in oracle["bus"].items():
            for terminal, values in terminals.items():
                if "vr" not in values:
                    continue
                found = native_result["bus"][bus][terminal]
                metrics["voltage_v"] = max(metrics["voltage_v"], complex_error(
                    [found["vr"], found["vi"]], [values["vr"], values["vi"]]))
        for branch in native_result["branches"]:
            name = branch["identity"].removeprefix("line:")
            for end, suffix in [("from", "fr"), ("to", "to")]:
                axis = data["line"][name][f"terminal_map_{end}"]
                currents = branch[f"current_{end}_a"]
                assert len(axis) == len(currents)
                for terminal, current in zip(axis, currents):
                    expected = oracle["line"][name][terminal]
                    metrics["line_current_a"] = max(metrics["line_current_a"], complex_error(
                        current, [expected[f"cr_{suffix}"], expected[f"ci_{suffix}"]]))
        # These snapshots use single-phase IBRs. Check the original input bounds
        # as well as the prepared-model residuals: a parser can drop a bound and
        # still produce a numerically feasible answer to the wrong problem.
        for device in native_result["devices"]:
            if device["kind"] != "Ibr":
                continue
            inv = data["ibr"][device["identity"]]
            assert inv["topology"] == "SINGLE_PHASE"
            assert len(device["coil_power_va"]) == 1
            power = device["coil_power_va"][0]
            expected = oracle["ibr"][device["identity"]][inv["terminal_map"][0]]
            metrics["ibr_power_va"] = max(metrics["ibr_power_va"], complex_error(
                power, [expected["pg"], expected["qg"]]))
            for key, sign in [("p_max", 1), ("p_min", -1)]:
                if key in inv:
                    bound = inv[key][0] if isinstance(inv[key], list) else inv[key]
                    metrics["original_active_bound_w"] = max(
                        metrics["original_active_bound_w"], sign * (power[0] - bound))
        assert all(math.isfinite(v) for v in metrics.values()), stem
        residual = max(native_result["residuals"].values())
        passed = all(metrics[k] <= TOLERANCES[k] for k in metrics) and residual <= 1e-6
        row = dict(case=stem, sha256=digest, passed=passed, maximum_errors=metrics,
                   objective=native_result["objective"], oracle_objective=oracle["objective"],
                   maximum_normalized_residual=residual, iterations=native_result["iterations"],
                   native_elapsed_s=record["elapsed_s"], oracle_elapsed_s=ref_meta["elapsed_s"],
                   counts=native_result["counts"])
        rows.append(row)
        print(json.dumps(row), flush=True)
    write(output / "comparison.json", dict(tolerances=TOLERANCES, cases=rows))
    return all(r["passed"] for r in rows)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--case-root", type=Path)
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--compare", action="store_true")
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    if args.compare:
        return compare(args.output)
    if args.binary is None or args.case_root is None:
        parser.error("native runs require --binary and --case-root")
    return native(plan(args.case_root), args.binary, args.output)


if __name__ == "__main__":
    raise SystemExit(0 if main() else 1)
