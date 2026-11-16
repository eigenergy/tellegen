"""Compare bus connectivity in an OpenDSS snapshot and its BMOPF conversion.

Uses enabled, closed equipment and every transformer winding. Ground does not
join separate buses. Disabled boundary lines are recorded, never energized.
Requires the Texas7k workflow's OpenDSSDirect.py environment.
"""

import argparse
import hashlib
import json
from collections import defaultdict
from pathlib import Path

import opendssdirect as dss


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def components(buses, edges, sources, loads, voltage=None):
    parent = {bus: bus for bus in buses}

    def root(bus):
        while parent[bus] != bus:
            parent[bus] = parent[parent[bus]]
            bus = parent[bus]
        return bus

    for edge in edges:
        if edge['connected']:
            for bus in edge['buses'][1:]:
                parent[root(bus)] = root(edge['buses'][0])
    groups = defaultdict(list)
    for bus in buses:
        groups[root(bus)].append(bus)
    result = []
    for members in sorted(groups.values(), key=lambda bs: (-len(bs), sorted(bs))):
        member_set = set(members)
        supplied = sorted(set(sources) & member_set)
        attached_loads = [load for load in loads if load['bus'] in member_set]
        row = {
            'bus_count': len(members), 'source_buses': supplied,
            'load_count': len(attached_loads),
            'load_p_w': sum(load['p_w'] for load in attached_loads),
            'load_q_var': sum(load['q_var'] for load in attached_loads),
        }
        if not supplied:
            row['buses'] = sorted(members)
            row['equipment'] = [edge for edge in edges if set(edge['buses']) & member_set]
            if voltage is not None:
                row['max_voltage_v'] = max(voltage[bus] for bus in members)
        result.append(row)
    return result


def audit_dss(snapshot):
    dss.Text.Command(f'Redirect "{Path(snapshot).resolve()}"')
    assert dss.Solution.Converged()
    buses = dss.Circuit.AllBusNames()
    edges = []
    for name in dss.Circuit.AllElementNames():
        dss.Circuit.SetActiveElement(name)
        kind = name.split('.', 1)[0].lower()
        if kind not in ('line', 'transformer', 'reactor'):
            continue
        terminals = dss.CktElement.NumTerminals()
        is_open = any(dss.CktElement.IsOpen(t, c)
                      for t in range(1, terminals + 1)
                      for c in range(1, dss.CktElement.NumConductors() + 1))
        enabled = dss.CktElement.Enabled()
        edges.append({
            'name': name, 'kind': kind,
            'buses': [b.split('.')[0].lower() for b in dss.CktElement.BusNames()],
            'enabled': enabled, 'open': is_open, 'connected': enabled and not is_open,
        })
    loads = []
    for name in dss.Loads.AllNames():
        dss.Loads.Name(name)
        if dss.CktElement.Enabled():
            loads.append({'name': name, 'bus': dss.CktElement.BusNames()[0].split('.')[0].lower(),
                          'p_w': 1000 * dss.Loads.kW(), 'q_var': 1000 * dss.Loads.kvar()})
    sources = []
    for name in dss.Vsources.AllNames():
        dss.Vsources.Name(name)
        if dss.CktElement.Enabled():
            sources.append(dss.CktElement.BusNames()[0].split('.')[0].lower())
    voltage = {}
    for bus in buses:
        dss.Circuit.SetActiveBus(bus)
        values = dss.Bus.Voltages()
        voltage[bus] = max((abs(complex(*values[i:i + 2])) for i in range(0, len(values), 2)), default=0)
    rows = components(buses, edges, sources, loads, voltage)
    return {
        'snapshot_sha256': digest(snapshot), 'converged': True,
        'bus_count': len(buses), 'node_count': dss.Circuit.NumNodes(),
        'disabled_line_count': sum(not e['enabled'] for e in edges if e['kind'] == 'line'),
        'component_count': len(rows), 'components': rows,
    }


def audit_bmopf(path):
    model = json.loads(Path(path).read_text())
    edges = []
    for kind in ('line', 'switch', 'transformer'):
        tables = model.get(kind, {})
        if kind != 'transformer':
            tables = {kind: tables}
        for subtype, table in tables.items():
            for name, row in table.items():
                # Texas7k adapter emits two-terminal transformer subtypes.
                buses = [row[key] for key in ('bus_from', 'bus_to')]
                edges.append({'kind': kind, 'subtype': subtype, 'name': name,
                              'buses': buses, 'connected': not row.get('open', False)})
    loads = [{'name': name, 'bus': row['bus'], 'p_w': sum(row['p_nom']), 'q_var': sum(row['q_nom'])}
             for name, row in model['load'].items()]
    sources = [row['bus'] for row in model['voltage_source'].values()]
    rows = components(model['bus'], edges, sources, loads)
    return {'sha256': digest(path), 'bus_count': len(model['bus']),
            'load_count': len(loads), 'component_count': len(rows), 'components': rows}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--snapshot', required=True, type=Path)
    parser.add_argument('--normalized-snapshot', type=Path)
    parser.add_argument('--bmopf', required=True, type=Path)
    parser.add_argument('--previous', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    source = audit_dss(args.snapshot)
    current = audit_bmopf(args.bmopf)
    previous = audit_bmopf(args.previous)
    normalized = audit_dss(args.normalized_snapshot) if args.normalized_snapshot else None
    def dead(report):
        return sorted(row['buses'] for row in report['components'] if not row['source_buses'])

    assert dead(source) == dead(current) == dead(previous), 'Unsupplied islands changed during conversion'
    if normalized:
        assert dead(normalized) == dead(source), 'Adapter normalization changed unsupplied islands'
    reports = [source, current, previous] + ([normalized] if normalized else [])
    assert all(row['load_count'] == 0 for report in reports
               for row in report['components'] if not row['source_buses'])
    assert all(row['max_voltage_v'] == 0 for report in (source, normalized) if report
               for row in report['components'] if not row['source_buses'])
    old_model = json.loads(args.previous.read_text())
    new_model = json.loads(args.bmopf.read_text())
    equipment = {}
    for key in ('bus', 'line', 'load', 'voltage_source'):
        old_ids, new_ids = set(old_model[key]), set(new_model[key])
        equipment[key] = {'previous': len(old_ids), 'current': len(new_ids),
                          'added': sorted(new_ids - old_ids), 'removed': sorted(old_ids - new_ids)}
    changed_tables = {key: old_model[key] != new_model[key]
                      for key in ('bus', 'line', 'linecode', 'load', 'transformer', 'voltage_source')}
    report = {
        'connectivity': 'Bus-level through enabled, closed lines and all transformer windings; ground is not an inter-bus edge.',
        'opendss_original_equipment_frozen_taps': source,
        'opendss_normalized_equipment': normalized,
        'previous_bmopf': previous, 'powerio_0_11_7_bmopf': current,
        'same_unsupplied_bus_sets_at_all_stages': True, 'unsupplied_load_count': 0,
        'equipment_identity_comparison': equipment, 'electrical_tables_changed': changed_tables,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, allow_nan=False) + '\n')
    print(json.dumps({k: v for k, v in report.items() if k not in
                      ('opendss_original_equipment_frozen_taps', 'opendss_normalized_equipment', 'previous_bmopf', 'powerio_0_11_7_bmopf')}, indent=2))


if __name__ == '__main__':
    main()
