#!/usr/bin/env python3
"""Check the saved country sweep and write a compact, reproducible result table."""

import argparse
import hashlib
import json
from pathlib import Path


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def check(directory):
    report_path = directory / "report.json"
    network_path = directory / "network.pio.json"
    report = json.loads(report_path.read_text())
    module = json.loads(network_path.read_text())
    network = module["value"]["data"]
    areas = report["synchronous_areas"]
    links = report["planned_hvdc_links"]
    stored = module["extensions"]["tellegen.synchronous_areas"]
    assert stored["areas"] == areas
    assert stored["links"] == links
    by_bus = {}
    for area in areas:
        for bus in area["bus_ids"]:
            assert bus not in by_bus, f"{directory.name}: repeated bus {bus}"
            by_bus[bus] = area["id"]
    assert len(areas) > 1
    assert set(by_bus) == {bus["id"] for bus in network["buses"]}
    references = {area["id"]: 0 for area in areas}
    for bus in network["buses"]:
        if bus["kind"] == "REF":
            references[by_bus[bus["id"]]] += 1
    assert all(count == 1 for count in references.values())
    parent = {bus: bus for bus in by_bus}

    def root(bus):
        while parent[bus] != bus:
            parent[bus] = parent[parent[bus]]
            bus = parent[bus]
        return bus

    for branch in network["branches"]:
        if branch.get("in_service", True):
            start, end = branch["from"], branch["to"]
            assert by_bus[start] == by_bus[end], f"{directory.name}: AC line crosses areas"
            parent[root(start)] = root(end)
    assert len({root(bus) for bus in by_bus}) == len(areas)
    assert len(links) == len(areas) - 1 == len(network["hvdc"])
    assert any(link["transfer_mw"] > 0.0 for link in links)
    for planned, link in zip(links, network["hvdc"]):
        assert planned["from_bus"] == link["from"]
        assert planned["to_bus"] == link["to"]
        assert by_bus[link["from"]] != by_bus[link["to"]]
        assert link["pf"] == link["pt"] == planned["transfer_mw"]
        assert 0 <= link["pf"] <= link["pmax"] == planned["rating_mw"]
        assert link["extras"]["tellegen_synth"]["source"] == "synthetic"
    demand = sum(area["demand_mw"] for area in areas)
    assert abs(demand - report["stats"]["total_load_mw"]) < 1e-6
    ac = report["ac_pf"]
    assert report["dc_opf"]["status"] == "optimal"
    assert ac["status"] == "converged"
    assert ac["buses_outside_planning_band"] == 0
    assert 0.95 <= ac["vm_min"] <= ac["vm_max"] <= 1.05
    assert ac["residual"] < 1e-6
    return {
        "case": directory.name,
        "request_sha256": digest(directory / "request.json"),
        "network_sha256": digest(network_path),
        "report_sha256": digest(report_path),
        "buses": len(by_bus),
        "areas": [{"name": area["name"], "buses": len(area["bus_ids"]),
                   "demand_mw": area["demand_mw"]} for area in areas],
        "links": links,
        "voltage_range_pu": [ac["vm_min"], ac["vm_max"]],
        "ac_residual": ac["residual"],
        "dc_solve_ms": report["dc_opf"].get("solve_ms"),
        "ac_solve_ms": ac.get("solve_ms"),
        "planning_warnings": [warning for warning in report["warnings"]
                              if warning["code"] == "planning_incomplete"],
        "checks": "passed",
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("sweep", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    results = [check(path.parent) for path in sorted(args.sweep.glob("*/report.json"))]
    expected = {f"{country}-s{seed}" for country in ["usa", "chn", "idn", "aus"]
                for seed in [1, 7, 2026]}
    assert {row["case"] for row in results} == expected, "The four-country sweep is incomplete"
    sweep = json.loads((args.sweep / "sweep.json").read_text())
    args.output.write_text(json.dumps({"binary_sha256": sweep["tellegen_sha256"],
                                      "seeds": [1, 7, 2026], "runs": results}, indent=2) + "\n")
    print(f"All {len(results)} country/seed runs passed topology, transfer and voltage checks")


if __name__ == "__main__":
    main()
