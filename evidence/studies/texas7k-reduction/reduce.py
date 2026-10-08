#!/usr/bin/env python3
"""Reduce the Texas7k pilot's storage without changing electrical values.

Usage: python3 reduce.py SOURCE.bmopf.json OUTPUT_DIRECTORY
This is deliberately scoped to this pilot, not a general BMOPF optimizer.
"""

import copy
import gzip
import hashlib
import json
from pathlib import Path
import shutil
import sys


def encode(value):
    return (json.dumps(value, ensure_ascii=False, separators=(",", ":"), allow_nan=False) + "\n").encode()


def sha(data):
    return hashlib.sha256(data).hexdigest()


def measure(label, data):
    return {"stage": label, "bytes": len(data), "gzip_bytes": len(gzip.compress(data, compresslevel=9, mtime=0)), "sha256": sha(data)}


def reduce(source, output):
    output.mkdir(parents=True, exist_ok=True)
    original_bytes = source.read_bytes()
    original = json.loads(original_bytes)
    reduced = copy.deepcopy(original)
    stages = [measure("original_pretty", original_bytes), measure("minified_only", encode(reduced))]

    # Every feature must be a redundant bus Point. Never discard line routes.
    geo = reduced["extras"]["geojson"]
    assert geo["type"] == "FeatureCollection"
    assert geo["powerio_geo"]["space"] == "geographic"
    assert set(geo) == {"type", "features", "powerio_geo"}
    seen, derived = set(), {}
    for feature in geo["features"]:
        props, geometry = feature["properties"], feature["geometry"]
        assert feature["type"] == "Feature" and props["kind"] == "bus"
        assert geometry["type"] == "Point"
        assert set(props) <= {"id", "kind", "derived_location_from_bus"}
        bus_id = props["id"]
        assert bus_id not in seen
        seen.add(bus_id)
        bus = reduced["bus"][bus_id]
        assert geometry["coordinates"] == [bus["longitude"], bus["latitude"]]
        if "derived_location_from_bus" in props:
            derived[bus_id] = props["derived_location_from_bus"]
    assert seen == set(reduced["bus"])
    del reduced["extras"]["geojson"]
    stages.append(measure("single_copy_of_coordinates", encode(reduced)))

    codes = reduced["linecode"]
    used = {line["linecode"] for line in reduced["line"].values()}
    assert used <= set(codes)
    unused = sorted(set(codes) - used)
    reduced["linecode"] = {key: value for key, value in codes.items() if key in used}
    stages.append(measure("unused_linecodes_removed", encode(reduced)))

    # Compare the complete record, including ratings. Keep existing code names;
    # no bus, branch, transformer or load identifiers change.
    signatures, aliases, unique = {}, {}, {}
    for name, code in reduced["linecode"].items():
        signature = json.dumps(code, sort_keys=True, separators=(",", ":"), allow_nan=False)
        representative = signatures.setdefault(signature, name)
        aliases[name] = representative
        unique.setdefault(representative, code)
    for line in reduced["line"].values():
        line["linecode"] = aliases[line["linecode"]]
    reduced["linecode"] = unique
    stages.append(measure("identical_linecodes_shared", encode(reduced)))

    # BMOPF 0.2.0 and both readers default an absent model to constant power.
    # Keep v_nom: it participates in load voltage interpretation and display.
    default_models_removed = 0
    for load in reduced["load"].values():
        assert not any(k in load for k in ["alpha_z", "alpha_i", "alpha_p", "beta_z", "beta_i", "beta_p", "gamma_p", "gamma_q"])
        if load.get("model") == "CONSTANT_POWER":
            del load["model"]
            default_models_removed += 1
    stages.append(measure("default_constant_power_labels_omitted", encode(reduced)))

    # Update only provenance that referred to removed duplicated geometry.
    reduced["meta"]["provenance"]["bus_coordinates"]["source"] = "Direct bus longitude/latitude, verified against the original pilot GeoJSON"
    reduced["meta"]["provenance"]["bus_coordinates"]["internal_bus_locations"] = "Derived locations retained unchanged; original bus associations are in reduction-provenance.json"
    reduced["meta"]["provenance"]["tellegen_reduction"] = {
        "source_file": source.name,
        "source_sha256": sha(original_bytes),
        "method": "Compact JSON; remove duplicate GeoJSON Points and unused linecodes; share identical linecode records; omit default CONSTANT_POWER load labels",
        "electrical_values_rounded": False,
        "provenance_sidecar": "reduction-provenance.json",
    }

    # Stronger than checking one operating point: every line resolves to the
    # exact same complete code, and every other electrical table is unchanged.
    for table in ["bus", "transformer", "voltage_source", "terminal_conventions"]:
        assert original[table] == reduced[table], table
    for name, load in reduced["load"].items():
        restored = {"model": "CONSTANT_POWER", **load}
        assert original["load"][name] == restored
    assert original["meta"]["frequency"] == reduced["meta"]["frequency"]
    assert original["name"] == reduced["name"]
    for name, line in reduced["line"].items():
        before = original["line"][name]
        assert {k: v for k, v in line.items() if k != "linecode"} == {k: v for k, v in before.items() if k != "linecode"}
        assert original["linecode"][before["linecode"]] == reduced["linecode"][line["linecode"]]

    data = encode(reduced)
    filename = "p1uhs0_1247.reduced.bmopf.json"
    (output / filename).write_bytes(data)
    (output / (filename + ".gz")).write_bytes(gzip.compress(data, compresslevel=9, mtime=0))
    stages.append(measure("final_with_reduction_provenance", data))
    provenance = {
        "source_file": source.name, "source_sha256": sha(original_bytes),
        "original_meta": original["meta"], "original_geo_meta": geo["powerio_geo"],
        "geometry_provenance_note": "Direct bus coordinates retain global Source provenance. Their per-point Source tags, assigned by the duplicate GeoJSON reader, are omitted from the generated display layer; coordinates and geographic space are unchanged.",
        "derived_bus_locations": derived, "linecode_aliases": aliases,
        "unused_linecodes": unused,
    }
    (output / "reduction-provenance.json").write_text(json.dumps(provenance, indent=2) + "\n")
    for name in ["License.md", "SOURCE_NOTICE.md", "tellegen_options.json"]:
        shutil.copyfile(source.parent / name, output / name)
    report = {
        "stages": stages,
        "counts": {"buses": len(reduced["bus"]), "lines": len(reduced["line"]), "loads": len(reduced["load"]), "transformers": sum(map(len, reduced["transformer"].values())), "linecodes_original": len(codes), "linecodes_unused": len(unused), "linecodes_used_unique": len(unique), "duplicate_point_features_removed": len(seen), "default_load_models_omitted": default_models_removed, "derived_location_associations_in_sidecar": len(derived)},
        "electrical_tables_and_expanded_linecodes_equal_after_restoring_default_load_model": True,
        "all_bus_coordinates_equal": True,
        "numeric_rounding": False,
        "source_unchanged": sha(source.read_bytes()) == sha(original_bytes),
    }
    (output / "size-report.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    if len(sys.argv) != 3:
        raise SystemExit(__doc__)
    reduce(Path(sys.argv[1]), Path(sys.argv[2]))
