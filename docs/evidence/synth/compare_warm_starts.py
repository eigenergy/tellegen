#!/usr/bin/env python3
"""Compare two retained country sweeps, including independent AC re-solves.

Run run.py sweep with --keep for each binary, then pass the two sweep
directories and binaries here. Times come from the generator's AC timer;
CLI re-solves compare bus voltages and branch powers at the emitted dispatch.
"""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import statistics
import subprocess


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def solve(binary, module, start):
    result = subprocess.run(
        [str(binary.resolve()), json.dumps({'formulation': 'acpf', 'start': start})],
        input=module.read_text(), text=True, capture_output=True, check=False,
        timeout=120,
    )
    return json.loads(result.stdout) if result.returncode == 0 else {'error': result.stderr.strip()}


def maximum_difference(a, b, field, key, values):
    left = {row[key]: row for row in a[field]}
    right = {row[key]: row for row in b[field]}
    if left.keys() != right.keys():
        raise ValueError(f'{field} identity mismatch')
    return max((abs(left[k][v] - right[k][v]) for k in left for v in values), default=0)


def summarize(rows, side):
    records = [row[side] for row in rows]
    failed = [r for r in records if r['ac_pf'] != 'converged']
    return {
        'runs': len(records),
        'dc_pass': sum(r['dc_opf'] == 'optimal' for r in records),
        'ac_converged': sum(r['ac_pf'] == 'converged' for r in records),
        'ac_in_band': sum(r['valid'] for r in records),
        'ac_total_ms': sum(r['ac_solve_ms'] for r in records),
        'ac_median_ms': statistics.median(r['ac_solve_ms'] for r in records),
        'failed_ac_total_ms': sum(r['ac_solve_ms'] for r in failed),
        'failed_ac_mean_ms': statistics.mean(r['ac_solve_ms'] for r in failed) if failed else None,
        'starts': dict(Counter(i['used'] if i else 'unreported'
                              for r in records for i in r['ac_initializations'])),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('before', type=Path)
    parser.add_argument('after', type=Path)
    parser.add_argument('--before-binary', required=True, type=Path)
    parser.add_argument('--after-binary', required=True, type=Path)
    parser.add_argument('--before-start', default='flat', choices=['flat', 'case'])
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    packets = [json.loads((p / 'sweep.json').read_text()) for p in [args.before, args.after]]
    if packets[0]['specs'] != packets[1]['specs'] or packets[0]['seeds'] != packets[1]['seeds']:
        raise ValueError('sweeps must use identical specifications and seeds')
    lookups = [{(r['region'], r['seed']): r for r in p['rows']} for p in packets]
    if lookups[0].keys() != lookups[1].keys():
        raise ValueError('sweep run mismatch')
    rows = []
    for (region, seed), before in lookups[0].items():
        after = lookups[1][region, seed]
        if before['request_sha256'] != after['request_sha256']:
            raise ValueError('request mismatch')
        files = [d / f'{region}-s{seed}' / 'network.pio.json' for d in [args.before, args.after]]
        # Every physical input and dispatch must agree; only the stored bus
        # voltage state and module metadata may differ.
        networks = [json.loads(p.read_text())['value']['data'] for p in files]
        for network in networks:
            for bus in network['buses']:
                bus.pop('vm', None)
                bus.pop('va', None)
        if networks[0] != networks[1]:
            raise ValueError(f'physical network changed: {region} {seed}')
        solutions = [solve(args.before_binary, files[0], args.before_start),
                     solve(args.after_binary, files[1], 'case')]
        record = {'region': region, 'seed': seed, 'before': before, 'after': after}
        for side, response, row in zip(['before', 'after'], solutions, [before, after]):
            if ('error' not in response) != (row['ac_pf'] == 'converged'):
                raise ValueError(f'{region} {seed}: {side} re-solve disagrees with validation')
        if all('error' not in s for s in solutions):
            a, b = solutions
            record['max_vm_difference_pu'] = maximum_difference(a, b, 'vm', 'bus', ['value'])
            record['max_va_difference_rad'] = maximum_difference(a, b, 'va', 'bus', ['value'])
            record['max_flow_difference_mw_mvar'] = maximum_difference(a, b, 'flows', 'branch', ['pf', 'qf', 'pt', 'qt'])
        elif 'error' in solutions[0] and 'error' not in solutions[1]:
            record['newly_converged_residual'] = solutions[1]['iterations']['residual']
        elif 'error' not in solutions[0]:
            raise ValueError(f'convergence regression: {region} {seed}')
        rows.append(record)
        print(region, seed, before['ac_pf'], after['ac_pf'], flush=True)
    result = {
        'schema': 'tellegen.synth-warm-start-comparison/1',
        'before_binary_sha256': digest(args.before_binary),
        'after_binary_sha256': digest(args.after_binary),
        'specs': packets[0]['specs'],
        'before': summarize(rows, 'before'), 'after': summarize(rows, 'after'),
        'rows': rows,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({s: result[s] for s in ['before', 'after']}, indent=2))


if __name__ == '__main__':
    main()
