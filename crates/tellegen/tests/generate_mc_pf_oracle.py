#!/usr/bin/env python3
"""Generate pinned OpenDSSDirect references for the small MC-PF corpus.

The inputs are selected BMOPFTools comparison snapshots plus Tellegen's local
centre-tap regression sources. This is a reproduction aid rather than a runtime
dependency: CI consumes the checked-in JSON references produced by this script.
OpenDSS is configured as a strict
constant-power, ideal-source snapshot so that its boundary conditions match
the BMOPF representation.  The source circuit's zero-voltage terminals are
represented by node 0 in the original DSS cases and are emitted as exact zero
when a bus terminal is prescribed by the BMOPF source.

Example (with OpenDSSDirect.py 0.9.4 installed)::

    python tests/generate_mc_pf_oracle.py --dss-dir /path/to/BMOPFTools.jl/test/data/pf_comparison

The local centre-tap cases can be regenerated without a BMOPFTools checkout::

    python tests/generate_mc_pf_oracle.py --case pf_center_tap_rneut
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
from pathlib import Path

import opendssdirect as dss


CASES = [
    "pf_1ph_freeneutral",
    "pf_1ph_impedanceneutral",
    "pf_1ph_line",
    "pf_3ph_line",
    "pf_center_tap_balanced_heavy",
    "pf_center_tap_loaded",
    "pf_center_tap_multi_feeder",
    "pf_center_tap_rneut",
    "pf_delta_load",
    "pf_dy_xfmr",
    "pf_dy_xfmr_rneut",
    "pf_dy_xfmr_tap",
    "pf_yd_xfmr",
]

BACKEND = "OpenDSSDirect.py 0.9.4; DSS-Python 0.15.7; DSS C-API 0.14.5"
TEST_DIR = Path(__file__).resolve().parent
LOCAL_DSS_DIR = TEST_DIR / "data" / "mc_pf" / "oracle_sources"
LOCAL_LICENSE = TEST_DIR / "data" / "mc_pf" / "LICENSE.md"
BMOPFTOOLS_SNAPSHOT = "8ca84ab12c0c91aaa8ad4c9986d6adbeb969ea0b"
BMOPFTOOLS_CENTRE_TAP_SOURCE = "720975f3adf3a78caf8a51157dba48f41bab91cc"
BMOPFTOOLS_LICENSE = (
    "BMOPFTools.jl LICENSE.md (copyright 2026 Frederik Geth, permissive "
    "BSD-3-Clause-style terms); hash recorded above."
)
DERIVED_LICENSE = (
    "Tellegen-authored fixture; the BMOPFTools.jl LICENSE.md (copyright 2026 "
    "Frederik Geth, permissive BSD-3-Clause-style terms) covers the centre-tap "
    "cases it derives from; hash recorded above."
)
LOCAL_SOURCE_PROVENANCE = {
    "pf_center_tap_balanced_heavy": {
        "source_bmopftools": BMOPFTOOLS_CENTRE_TAP_SOURCE,
        "source_fixture": "pf_center_tap_balanced_heavy.dss",
        "license": BMOPFTOOLS_LICENSE,
    },
    "pf_center_tap_rneut": {
        "source_bmopftools": BMOPFTOOLS_CENTRE_TAP_SOURCE,
        "source_fixture": "adapted from BMOPFTools centre-tap and finite-rneut cases",
        "license": BMOPFTOOLS_LICENSE,
    },
    "pf_center_tap_multi_feeder": {
        "source_tellegen": "synthetic reduced feeder for issue #140",
        "derived_bmopftools": BMOPFTOOLS_CENTRE_TAP_SOURCE,
        "license": DERIVED_LICENSE,
    },
}


def stable_float(value: float) -> float:
    """Remove backend/platform noise from the checked-in JSON oracle."""
    if abs(value) < 1e-12:
        return 0.0
    return float(f"{value:.10g}")


def equivalent(
    left,
    right,
    *,
    rel_tol: float = 1e-6,
    abs_tol: float = 2e-5,
    path: str = "$",
) -> bool:
    """Ignore backend differences smaller than the matching oracle tolerance."""
    if isinstance(left, bool) or isinstance(right, bool):
        matches = left is right
        if not matches:
            print(f"oracle difference at {path}: {left!r} != {right!r}")
        return matches
    if isinstance(left, (int, float)) and isinstance(right, (int, float)):
        matches = math.isclose(left, right, rel_tol=rel_tol, abs_tol=abs_tol)
        if not matches:
            print(f"oracle difference at {path}: {left!r} != {right!r}")
        return matches
    if isinstance(left, dict) and isinstance(right, dict):
        if left.keys() != right.keys():
            print(f"oracle difference at {path}: object keys differ")
            return False
        for key in left:
            key_rel_tol, key_abs_tol = rel_tol, abs_tol
            if key == "currents":
                key_rel_tol, key_abs_tol = 2e-6, 2e-4
            elif key == "powers":
                key_rel_tol, key_abs_tol = 2e-6, 2e-2
            if not equivalent(
                left[key],
                right[key],
                rel_tol=key_rel_tol,
                abs_tol=key_abs_tol,
                path=f"{path}.{key}",
            ):
                return False
        return True
    if isinstance(left, list) and isinstance(right, list):
        if len(left) != len(right):
            print(f"oracle difference at {path}: list lengths differ")
            return False
        return all(
            equivalent(
                a,
                b,
                rel_tol=rel_tol,
                abs_tol=abs_tol,
                path=f"{path}[{index}]",
            )
            for index, (a, b) in enumerate(zip(left, right))
        )
    matches = left == right
    if not matches:
        print(f"oracle difference at {path}: {left!r} != {right!r}")
    return matches


def preserve_noncomparative_elements(
    existing: dict, result: dict, compared_transformers: set[str]
) -> None:
    """Keep convention-sensitive element records that the oracle does not compare.

    OpenDSS source, transformer, and reactor terminal metadata and vectors can
    vary with the backend's winding/pivot convention even when the bus voltages
    and externally observable line/load quantities agree.  The Rust oracle omits
    source elements and, apart from centre-tap transformers (whose terminal
    currents and powers it compares numerically), only uses transformer/reactor
    names while requiring the corresponding solver ports to be finite, so
    retaining the checked-in evidence prevents harmless backend differences from
    dirtying a source package on another platform.
    """
    previous_by_name = {
        element["name"]: element for element in existing.get("elements", [])
    }
    for element in result.get("elements", []):
        kind = element.get("name", "").lower()
        if not kind.startswith(("vsource.", "transformer.", "reactor.")):
            continue
        if kind.startswith("transformer.") and kind.split(".", 1)[1] in compared_transformers:
            continue
        previous = previous_by_name.get(element["name"])
        if previous is None:
            continue
        for field in (
            "bus_names",
            "node_order",
            "num_terminals",
            "num_conductors",
            "currents",
            "powers",
        ):
            if field in previous:
                element[field] = previous[field]


def write_reference(path: Path, result: dict, compared_transformers: set[str]) -> None:
    if path.exists():
        existing = json.loads(path.read_text())
        preserve_noncomparative_elements(existing, result, compared_transformers)
        if equivalent(existing, result):
            return
    path.write_text(json.dumps(result, indent=2) + "\n")


def cplx(values: list[float], offset: int, scale: float = 1.0) -> dict[str, float]:
    return {
        "re": stable_float(values[offset] * scale),
        "im": stable_float(values[offset + 1] * scale),
    }


def terminal_number(name: str) -> int:
    named = {"a": 1, "b": 2, "c": 3, "n": 4, "earth": 0}
    return named[name] if name in named else int(name)


def source_paths(case: str, dss_dir: Path | None) -> tuple[Path, Path, dict[str, str]]:
    local_dss = LOCAL_DSS_DIR / f"{case}.dss"
    if local_dss.exists():
        if case not in LOCAL_SOURCE_PROVENANCE:
            raise RuntimeError(f"{case} has a checked-in DSS source but no recorded provenance")
        return local_dss, LOCAL_LICENSE, LOCAL_SOURCE_PROVENANCE[case]
    if dss_dir is None:
        raise RuntimeError(
            f"{case} has no checked-in DSS source; pass --dss-dir for the BMOPFTools corpus"
        )
    return (
        dss_dir / f"{case}.dss",
        dss_dir.parent.parent.parent / "LICENSE.md",
        {"source_bmopftools": BMOPFTOOLS_SNAPSHOT, "license": BMOPFTOOLS_LICENSE},
    )


def generate(case: str, dss_dir: Path | None, output_dir: Path) -> None:
    source = TEST_DIR / "data" / "mc_pf" / "oracle_inputs" / f"{case}.json"
    dss_path, license_path, source_provenance = source_paths(case, dss_dir)
    document = json.loads(source.read_text())
    source_record = next(iter(document["voltage_source"].values()))
    prescribed = {
        (source_record["bus"], terminal): (mag, angle)
        for terminal, mag, angle in zip(
            source_record["terminal_map"], source_record["v_magnitude"], source_record["v_angle"]
        )
    }
    dss.Text.Command("clear")
    dss.Text.Command(f"redirect {dss_path}")
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
    dss_raw = dss_path.read_bytes()
    if not license_path.exists():
        raise RuntimeError(f"missing upstream license file: {license_path}")
    license_raw = license_path.read_bytes()
    result = {
        "schema": "tellegen.mc_pf.open_dss_reference.v1",
        "case": case,
        "provenance": {
            "backend": BACKEND,
            **{key: value for key, value in source_provenance.items() if key != "license"},
            "input_sha256": hashlib.sha256(raw).hexdigest(),
            "dss_sha256": hashlib.sha256(dss_raw).hexdigest(),
            "upstream_license_sha256": hashlib.sha256(license_raw).hexdigest(),
            "configuration": "controlmode=off; algorithm=normal; tolerance=1e-11; maxiterations=1000; load vminpu=0 vmaxpu=2; vsource model=ideal puZideal=[1e-12,0]; transformer ppm_antifloat=0",
            "license": source_provenance["license"],
        },
        "converged": True,
        "terminals": terminals,
        "elements": elements,
    }
    # Mirrors the Rust oracle: centre-tap transformers are compared numerically.
    compared_transformers = {
        name.lower() for name in document.get("transformer", {}).get("center_tap", {})
    }
    output_dir.mkdir(parents=True, exist_ok=True)
    write_reference(output_dir / f"{case}.json", result, compared_transformers)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--dss-dir", type=Path)
    parser.add_argument(
        "--case",
        action="append",
        choices=CASES,
        help="case to regenerate; repeat for multiple cases (default: all)",
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=Path(__file__).resolve().parent / "data" / "mc_pf" / "oracle_refs",
    )
    args = parser.parse_args()
    cases = args.case or CASES
    if args.dss_dir is None:
        missing = [case for case in cases if not (LOCAL_DSS_DIR / f"{case}.dss").exists()]
        if missing:
            parser.error(
                "--dss-dir is required for cases without a checked-in DSS source: "
                + ", ".join(missing)
            )
    for case in cases:
        generate(case, args.dss_dir, args.output_dir)


if __name__ == "__main__":
    main()
