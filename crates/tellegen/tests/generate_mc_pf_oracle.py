#!/usr/bin/env python3
"""Generate pinned OpenDSSDirect references for the small MC-PF corpus.

The inputs are the nine BMOPFTools comparison snapshots.  This is a
reproduction aid rather than a runtime dependency: CI consumes the checked-in
JSON references produced by this script.  OpenDSS is configured as a strict
constant-power, ideal-source snapshot so that its boundary conditions match
the BMOPF representation.  The source circuit's zero-voltage terminals are
represented by node 0 in the original DSS cases and are emitted as exact zero
when a bus terminal is prescribed by the BMOPF source.

Example (with OpenDSSDirect.py 0.9.4 installed)::

    python tests/generate_mc_pf_oracle.py --dss-dir /path/to/BMOPFTools.jl/test/data/pf_comparison
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

import opendssdirect as dss


CASES = [
    "pf_1ph_freeneutral",
    "pf_1ph_impedanceneutral",
    "pf_1ph_line",
    "pf_3ph_line",
    "pf_delta_load",
    "pf_dy_xfmr",
    "pf_dy_xfmr_rneut",
    "pf_dy_xfmr_tap",
    "pf_yd_xfmr",
]


def cplx(values: list[float], offset: int, scale: float = 1.0) -> dict[str, float]:
    return {"re": float(values[offset] * scale), "im": float(values[offset + 1] * scale)}


def terminal_number(name: str) -> int:
    named = {"a": 1, "b": 2, "c": 3, "n": 4, "earth": 0}
    return named[name] if name in named else int(name)


def generate(case: str, dss_dir: Path, output_dir: Path) -> None:
    source = Path(__file__).resolve().parent / "data" / "mc_pf" / "oracle_inputs" / f"{case}.json"
    document = json.loads(source.read_text())
    source_record = next(iter(document["voltage_source"].values()))
    prescribed = {
        (source_record["bus"], terminal): (mag, angle)
        for terminal, mag, angle in zip(
            source_record["terminal_map"], source_record["v_magnitude"], source_record["v_angle"]
        )
    }
    dss.Text.Command("clear")
    dss.Text.Command(f"redirect {dss_dir / (case + '.dss')}")
    dss.Text.Command("set controlmode=off algorithm=normal tolerance=1e-11 maxiterations=1000")
    dss.Text.Command("batchedit load..* vminpu=0 vmaxpu=2 vlowpu=0")
    dss.Text.Command("batchedit vsource..* model=ideal puZideal=[1e-12,0]")
    dss.Text.Command("batchedit transformer..* ppm_antifloat=0")
    # BMOPF source terminals with zero prescribed voltage are ideal boundary
    # nodes.  The comparison DSS files normally model their neutral through a
    # tiny reactor, so alias every source-side neutral conductor to node 0.
    zero_source_terms = {
        terminal
        for terminal, magnitude in zip(source_record["terminal_map"], source_record["v_magnitude"])
        if magnitude == 0.0
    }
    if zero_source_terms:
        source_bus = source_record["bus"]
        zero_nodes = {terminal_number(terminal) for terminal in zero_source_terms}
        for element in dss.Circuit.AllElementNames():
            dss.Circuit.SetActiveElement(element)
            buses = dss.CktElement.BusNames()
            for bus_index, bus in enumerate(buses, 1):
                parts = bus.split(".")
                if parts[0].lower() != source_bus.lower() or not any(
                    int(node) in zero_nodes for node in parts[1:] if node.isdigit()
                ):
                    continue
                nodes = ["0" if node.isdigit() and int(node) in zero_nodes else node for node in parts[1:]]
                if element.lower().startswith("transformer."):
                    dss.Text.Command(
                        f"edit {element} wdg={bus_index} bus={parts[0]}.{'.'.join(nodes)}"
                    )
                else:
                    dss.Text.Command(
                        f"edit {element} bus{bus_index}={parts[0]}.{'.'.join(nodes)}"
                    )
    dss.Solution.Solve()
    if not dss.Solution.Converged():
        raise RuntimeError(f"OpenDSS did not converge for {case}")

    buses: dict[tuple[str, int], dict[str, float]] = {}
    for bus in dss.Circuit.AllBusNames():
        dss.Circuit.SetActiveBus(bus)
        nodes = dss.Bus.Nodes()
        values = dss.Bus.Voltages()
        for offset, node in enumerate(nodes):
            buses[(bus, int(node))] = cplx(values, 2 * offset)

    terminals = []
    for bus, record in document.get("bus", {}).items():
        for terminal in record.get("terminal_names", []):
            node = terminal_number(terminal)
            if (bus, terminal) in prescribed and prescribed[(bus, terminal)][0] == 0.0:
                value = {"re": 0.0, "im": 0.0}
            elif (bus, node) in buses:
                value = buses[(bus, node)]
            elif terminal in record.get("perfectly_grounded_terminals", []):
                value = {"re": 0.0, "im": 0.0}
            else:
                raise RuntimeError(f"OpenDSS did not expose expected node {bus}.{terminal}")
            terminals.append({"bus": bus, "terminal": terminal, "voltage": value})

    elements = []
    for name in dss.Circuit.AllElementNames():
        dss.Circuit.SetActiveElement(name)
        currents = dss.CktElement.Currents()
        powers = dss.CktElement.Powers()
        bus_names = dss.CktElement.BusNames()
        num_conductors = dss.CktElement.NumConductors()
        node_order = dss.CktElement.NodeOrder()
        if len(node_order) != len(bus_names) * num_conductors:
            raise RuntimeError(f"unexpected OpenDSS node order for {name}")
        ordered_nodes = [
            {
                "bus": bus.split(".", 1)[0],
                "nodes": node_order[index * num_conductors : (index + 1) * num_conductors],
            }
            for index, bus in enumerate(bus_names)
        ]
        elements.append(
            {
                "name": name,
                "bus_names": bus_names,
                "node_order": ordered_nodes,
                "num_terminals": dss.CktElement.NumTerminals(),
                "num_conductors": num_conductors,
                "currents": [cplx(currents, i) for i in range(0, len(currents), 2)],
                # OpenDSS CktElement.Powers is kW/kvar; Tellegen ports use VA.
                "powers": [cplx(powers, i, 1000.0) for i in range(0, len(powers), 2)],
            }
        )

    raw = source.read_bytes()
    dss_raw = (dss_dir / f"{case}.dss").read_bytes()
    license_path = dss_dir.parent.parent.parent / "LICENSE.md"
    if not license_path.exists():
        raise RuntimeError(f"missing upstream license file: {license_path}")
    license_raw = license_path.read_bytes()
    result = {
        "schema": "tellegen.mc_pf.open_dss_reference.v1",
        "case": case,
        "provenance": {
            "open_dssdirect": getattr(dss, "__version__", "unknown"),
            "opendss_c_api": dss.Basic.Version(),
            "source_bmopftools": "8ca84ab12c0c91aaa8ad4c9986d6adbeb969ea0b",
            "input_sha256": hashlib.sha256(raw).hexdigest(),
            "dss_sha256": hashlib.sha256(dss_raw).hexdigest(),
            "upstream_license_sha256": hashlib.sha256(license_raw).hexdigest(),
            "configuration": "controlmode=off; algorithm=normal; tolerance=1e-11; maxiterations=1000; load vminpu=0 vmaxpu=2; vsource model=ideal puZideal=[1e-12,0]; transformer ppm_antifloat=0",
            "license": "BMOPFTools.jl LICENSE.md (copyright 2026 Frederik Geth, permissive BSD-3-Clause-style terms); hash recorded above.",
        },
        "converged": True,
        "terminals": terminals,
        "elements": elements,
    }
    output_dir.mkdir(parents=True, exist_ok=True)
    (output_dir / f"{case}.json").write_text(json.dumps(result, indent=2) + "\n")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--dss-dir", type=Path, required=True)
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=Path(__file__).resolve().parent / "data" / "mc_pf" / "oracle_refs",
    )
    args = parser.parse_args()
    for case in CASES:
        generate(case, args.dss_dir, args.output_dir)


if __name__ == "__main__":
    main()
