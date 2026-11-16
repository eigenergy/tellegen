"""Extract and validate the supplied island of the PowerIO 0.11.7 Texas7k pilot.

Retains all loads and all surviving electrical values. Writes a new BMOPF,
portable module, removal manifest, license notice and paired native validation.
"""

import argparse
import copy
import gzip
import hashlib
import json
import math
import shutil
import subprocess
from pathlib import Path

import jsonschema


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def dump(path, value):
    path.write_text(json.dumps(value, indent=2, allow_nan=False) + '\n')


def connected_components(model):
    adjacency = {bus: set() for bus in model['bus']}
    branches = list(model['line'].values())
    branches += [row for rows in model['transformer'].values() for row in rows.values()]
    for row in branches:
        a, b = row['bus_from'], row['bus_to']
        adjacency[a].add(b)
        adjacency[b].add(a)
    remaining, components = set(adjacency), []
    while remaining:
        first = min(remaining)
        members, pending = set(), [first]
        while pending:
            bus = pending.pop()
            if bus in members:
                continue
            members.add(bus)
            pending.extend(adjacency[bus] - members)
        remaining -= members
        components.append(members)
    return sorted(components, key=lambda members: (-len(members), sorted(members)))


def solve(binary, model, options, destination):
    with destination.open('w') as output:
        subprocess.run([str(binary), str(model), str(options)], stdout=output, check=True)
    return json.loads(destination.read_text())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--schema', required=True, type=Path)
    parser.add_argument('--powerio', required=True, type=Path)
    parser.add_argument('--mc-pf', required=True, type=Path)
    parser.add_argument('--full-runner', required=True, type=Path)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    source_hash = sha(args.source)
    original = json.loads(args.source.read_text())
    # This helper is bounded to the pilot's known two-terminal representation.
    assert set(original) == {'bus', 'extras', 'line', 'linecode', 'load', 'meta',
                             'name', 'terminal_conventions', 'transformer', 'voltage_source'}
    components = connected_components(original)
    sources = {row['bus'] for row in original['voltage_source'].values()}
    supplied = [members for members in components if members & sources]
    assert len(supplied) == 1 and supplied[0] == components[0]
    keep = supplied[0]
    removed_buses = set(original['bus']) - keep
    removed_lines = {name for name, row in original['line'].items()
                     if row['bus_from'] not in keep or row['bus_to'] not in keep}
    assert all(row['bus'] in keep for row in original['load'].values()), 'Would remove loads'
    assert all(row['bus_from'] in keep and row['bus_to'] in keep
               for rows in original['transformer'].values() for row in rows.values())
    assert len(removed_buses) == 5 and len(removed_lines) == 3
    model = copy.deepcopy(original)
    model['bus'] = {name: row for name, row in original['bus'].items() if name in keep}
    model['line'] = {name: row for name, row in original['line'].items() if name not in removed_lines}
    used_before = {row['linecode'] for row in original['line'].values()}
    used_after = {row['linecode'] for row in model['line'].values()}
    removed_codes = used_before - used_after
    model['linecode'] = {name: row for name, row in original['linecode'].items() if name not in removed_codes}
    geo = model['extras']['geojson']
    assert all(f['properties']['kind'] == 'bus' and f['geometry']['type'] == 'Point'
               for f in geo['features'])
    geo['features'] = [feature for feature in geo['features'] if feature['properties']['id'] in keep]
    points = {f['properties']['id']: f['geometry']['coordinates'] for f in geo['features']}
    assert set(points) == keep and len(points) == len(geo['features'])
    for bus, row in model['bus'].items():
        assert points[bus] == [row['longitude'], row['latitude']]
        assert all(math.isfinite(v) and abs(v) <= limit for v, limit in zip(points[bus], (180, 90)))
    removal = {
        'source_file': args.source.name, 'source_sha256': source_hash,
        'method': 'Retain the unique source-connected bus component; remove only unsupplied buses, their lines, newly unused line codes and bus Point features.',
        'removed_buses': sorted(removed_buses), 'removed_lines': sorted(removed_lines),
        'removed_linecodes': sorted(removed_codes), 'removed_load_count': 0,
        'original_component_bus_counts': [len(c) for c in components],
        'remaining_component_count': 1, 'remaining_bus_count': len(keep),
        'removed_records_sidecar': 'island-removal.json',
    }
    model['name'] = 'Texas7k p1uhs0_1247 main energized island nominal snapshot'
    model['meta']['provenance']['main_island'] = removal
    for table in ('load', 'transformer', 'voltage_source', 'terminal_conventions'):
        assert model[table] == original[table]
    for table in ('bus', 'line', 'linecode'):
        assert all(row == original[table][name] for name, row in model[table].items())
    assert len(connected_components(model)) == 1
    projected = copy.deepcopy(model)
    for row in projected['bus'].values():
        del row['longitude'], row['latitude']
    assert sha(args.schema) == model['meta']['provenance']['powerio_bmopf']['schema_sha256']
    jsonschema.Draft202012Validator(json.loads(args.schema.read_text())).validate(projected)
    path = args.output / 'p1uhs0_1247.main-island.bmopf.json'
    dump(path, model)
    dump(args.output / 'island-removal.json', {
        **removal,
        'records': {table: {name: original[table][name] for name in sorted(names)}
                    for table, names in [('bus', removed_buses), ('line', removed_lines), ('linecode', removed_codes)]},
        'geojson_features': [feature for feature in original['extras']['geojson']['features']
                             if feature['properties']['id'] in removed_buses],
    })
    for name in ('License.md', 'tellegen_options.json'):
        shutil.copyfile(args.source.parent / name, args.output / name)
    notice = (args.source.parent / 'SOURCE_NOTICE.md').read_text()
    notice += ('\n## Main energized island derivative — 30 October 2026\n\n'
               'Reconverted with PowerIO 0.11.7, using its native finite-source-impedance\n'
               'equivalent. Removed the two unsupplied line sections: five buses, three\n'
               'lines, three newly unused line codes and five bus coordinate features.\n'
               'All 15,299 loads, all transformers, the source, coordinates and electrical\n'
               'values on the supplied island are unchanged. The original network has not\n'
               'been edited. Removed records and the parent hash are in island-removal.json.\n'
               'This independent derivative retains CC-BY-3.0-US attribution and implies\n'
               'no endorsement by the original authors.\n')
    (args.output / 'SOURCE_NOTICE.md').write_text(notice)
    module_path = args.output / 'p1uhs0_1247.main-island.pio.json'
    with module_path.open('w') as out, (args.output / 'powerio_diagnostics.json').open('w') as diagnostics:
        subprocess.run([str(args.powerio), 'serialize', str(path), '--from', 'bmopf-json',
                        '-o', '-', '--diagnostics-format', 'json'], stdout=out, stderr=diagnostics, check=True)
    assert json.loads((args.output / 'powerio_diagnostics.json').read_text()) == []
    module = json.loads(module_path.read_text())
    assert 'tellegen.energized_network' not in module.get('extensions', {})
    options = args.output / 'tellegen_options.json'
    full_result_path, result_path = args.output / 'full-result.tmp.json', args.output / 'main-result.tmp.json'
    full = solve(args.full_runner, args.source, options, full_result_path)
    result = solve(args.mc_pf, path, options, result_path)
    assert result['converged'] and full['converged']
    assert not result.get('excluded_islands') and not result.get('network_scope')
    ignored = {'network_scope', 'excluded_islands'}
    assert {k: v for k, v in full.items() if k not in ignored} == {k: v for k, v in result.items() if k not in ignored}, 'PF result changed'
    assert not ({row['bus'] for row in result['terminals']} & removed_buses)
    voltage_dump = {'options': json.loads(options.read_text()), 'terminals': [
        {k: v for k, v in row.items() if k in ('bus', 'terminal', 'voltage')} for row in result['terminals']]}
    (args.output / 'tellegen_voltages.json.gz').write_bytes(
        gzip.compress((json.dumps(voltage_dump, indent=2) + '\n').encode(), mtime=0))
    report = {
        'source_sha256': source_hash, 'output_sha256': sha(path),
        'component_count': 1, 'bus_count': len(model['bus']), 'line_count': len(model['line']),
        'linecode_count': len(model['linecode']), 'load_count': len(model['load']),
        'transformer_count': sum(map(len, model['transformer'].values())), 'source_count': len(sources),
        'removal': removal, 'electrical_schema_projection_valid': True,
        'schema_sha256': sha(args.schema), 'coordinates_checked': len(keep),
        'surviving_electrical_records_exactly_equal': True, 'powerio_diagnostics': [],
        'portable_module_requires_island_selection': False,
        'all_native_result_fields_exactly_equal_except_selection_metadata': True,
        'maximum_terminal_complex_voltage_difference_v': 0.0,
        'converged': True, 'iterations': result['iterations'], 'factorization_count': result['factorization_count'],
        'matrix_dimension': result['matrix_dimension'], 'physical_kcl_residual': result['physical_kcl_residual'],
        'solved_terminal_count': len(result['terminals']), 'element_port_count': len(result['element_ports']),
        'executables': {name: {'sha256': sha(binary)} for name, binary in
                        [('powerio', args.powerio), ('mc_pf', args.mc_pf), ('full_island_selection_runner', args.full_runner)]},
    }
    assert sha(args.source) == source_hash
    dump(args.output / 'validation_report.json', report)
    (args.output / 'README.md').write_text(
        '# Texas7k p1uhs0_1247 — main energized island\n\n'
        'PowerIO 0.11.7 derivative containing one supplied component: 22,076 buses,\n'
        '16,819 lines, 7,756 transformer units, one source and all 15,299 loads.\n'
        'Removed two unsupplied sections: five buses, three lines, three newly\n'
        'unused line codes and five coordinate features. Surviving records are\n'
        'unchanged. See island-removal.json for parent hash and removed records.\n\n'
        'Use p1uhs0_1247.main-island.bmopf.json or the corresponding .pio.json.\n'
        'The portable module contains only the supplied island and requires no\n'
        'energized-network selection extension. Use tellegen_options.json for PF.\n'
        'Native PF converges in 23 iterations and exactly matches every result\n'
        'field of the full case solved with explicit island selection, apart\n'
        'from selection metadata. See validation_report.json for executable hashes.\n\n'
        'Source and derivative data: CC-BY-3.0-US. Retain License.md and\n'
        'SOURCE_NOTICE.md, including the research citation and change description.\n'
        'The nominal-load snapshot retains voltage and loading violations.\n')
    for scratch in (full_result_path, result_path):
        scratch.unlink()
    dump(args.output / 'artifact-manifest.json', {
        p.name: {'bytes': p.stat().st_size, 'sha256': sha(p)} for p in sorted(args.output.iterdir()) if p.is_file()})
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
