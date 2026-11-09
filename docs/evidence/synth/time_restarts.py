#!/usr/bin/env python3
"""Alternate unchanged and experimental solvers on each remaining failed request.

Use after both full 105-run sweeps. Reports the generator's AC timer, excluding
generation and I/O. Warm-up samples are discarded; order reverses each round.
"""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import statistics
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--warm', required=True, type=Path)
    parser.add_argument('--experiment', required=True, type=Path)
    parser.add_argument('--sweep', required=True, type=Path, action='append')
    parser.add_argument('--repeats', type=int, default=10)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    binaries = {'warm': args.warm.resolve(), 'experiment': args.experiment.resolve()}
    rows = []
    for sweep in args.sweep:
        packet = json.loads((sweep / 'sweep.json').read_text())
        for row in packet['rows']:
            if row['ac_pf'] == 'converged':
                continue
            request = (sweep / f"{row['region']}-s{row['seed']}" / 'request.json').resolve()
            samples = {name: [] for name in binaries}
            with tempfile.TemporaryDirectory() as temp:
                for trial in range(args.repeats + 1):
                    order = ['warm', 'experiment'] if trial % 2 == 0 else ['experiment', 'warm']
                    for name in order:
                        out = Path(temp) / name
                        subprocess.run([str(binaries[name]), 'generate', '--request', str(request), '--out', str(out)],
                                       check=True, capture_output=True, timeout=120)
                        report = json.loads((out / 'report.json').read_text())
                        if report['ac_pf']['status'] != row['ac_pf']:
                            raise ValueError(f'{request}: changed verdict under {name}')
                        if trial:
                            samples[name].append(report['ac_pf']['solve_ms'])
            medians = {name: statistics.median(values) for name, values in samples.items()}
            result = {'outline': packet['outline'], 'region': row['region'], 'seed': row['seed'],
                      'samples_ms': samples, 'median_ms': medians,
                      'median_reduction_percent': 100 * (1 - medians['experiment'] / medians['warm'])}
            rows.append(result)
            print(json.dumps(result), flush=True)
    totals = {name: sum(row['median_ms'][name] for row in rows) for name in binaries}
    result = {'schema': 'tellegen.restart-timing/1', 'platform': platform.platform(),
              'binary_sha256': {name: hashlib.sha256(p.read_bytes()).hexdigest() for name, p in binaries.items()},
              'repeats': args.repeats, 'discarded_warmups_per_case_and_binary': 1,
              'sum_of_case_medians_ms': totals,
              'median_reduction_percent': 100 * (1 - totals['experiment'] / totals['warm']),
              'rows': rows}
    args.output.write_text(json.dumps(result, indent=2) + '\n')


if __name__ == '__main__':
    main()
