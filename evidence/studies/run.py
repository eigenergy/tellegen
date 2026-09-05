#!/usr/bin/env python3
"""Run a declared Study through the installed native CLI and retain exact evidence."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import time


def digest(path):
    return 'sha256:' + hashlib.sha256(Path(path).read_bytes()).hexdigest()


def call(binary, action, destination, request=None):
    result = subprocess.run(
        [str(binary), 'study', action, str(destination)],
        input=None if request is None else json.dumps(request),
        text=True, capture_output=True, check=False,
    )
    if result.returncode:
        raise RuntimeError(result.stderr.strip() or result.stdout.strip())
    return json.loads(result.stdout) if result.stdout.strip() else None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('spec', type=Path)
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--powerio', type=Path, required=True)
    parser.add_argument('--tellegen', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--scenario-evidence', type=Path)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    spec = json.loads(args.spec.read_text())
    if spec.get('scenario') and not args.scenario_evidence:
        raise ValueError('This scenario requires its explicit cost comparison evidence')
    if spec.get('source_sha256') and digest(args.source) != spec['source_sha256']:
        raise ValueError('Source digest differs from the declared Study fixture')
    instance = args.output / 'input.pio.json'
    if args.source.name.endswith('.pio.json'):
        instance.write_bytes(args.source.read_bytes())
        (args.output / 'conversion.log').write_text('Supplied PowerIO IR is validated by native Study creation.\n')
    else:
        conversion = subprocess.run(
            [str(args.powerio), 'serialize', str(args.source), '-o', str(instance)],
            capture_output=True, text=True, check=True,
        )
        (args.output / 'conversion.log').write_text(conversion.stderr)
    request = dict(spec['create'], input=instance.read_text())
    study_path = args.output / 'study.json'
    packet = {
        'schema': 'tellegen.study-evidence/1',
        'spec_sha256': digest(args.spec), 'source_sha256': digest(args.source),
        'source_name': args.source.name, 'powerio_sha256': digest(args.powerio),
        'tellegen_sha256': digest(args.tellegen), 'input_sha256': digest(instance),
        'apply_requested': False, 'tolerances': spec['tolerances'],
    }
    start = time.monotonic()
    try:
        summary = call(args.tellegen, 'create', study_path, request)
        initial = summary['applied_state']
        goal = summary['active_goal'][0]
        if args.scenario_evidence:
            evidence = json.loads(args.scenario_evidence.read_text())
            if evidence.get('scenario_sha256') != digest(args.source):
                raise ValueError('Scenario evidence does not identify the supplied source')
            recorded = call(args.tellegen, 'run', study_path, {
                'expected_revision': summary['revision'],
                'operation': {'kind': 'record_evidence', 'state': initial,
                              'goal': goal, 'sensitivity': False,
                              'rationale': 'Record the explicit inner-cost model scenario and its source comparison',
                              'evidence': evidence},
            })
            summary = recorded['summary']
            packet['scenario_evidence_sha256'] = digest(args.scenario_evidence)
        operation = {
            'kind': 'propose', 'state': initial, 'goal': goal,
            'options': spec['search'], 'rationale': spec['rationale'],
        }
        proposed = call(args.tellegen, 'run', study_path, {
            'expected_revision': summary['revision'], 'operation': operation,
        })
        summary = proposed['summary']
        packet['initial_state'] = initial
        packet['recommended_state'] = summary['recommended_state']
        packet['applied_state'] = summary['applied_state']
        if summary['applied_state'] != initial:
            raise AssertionError('Planning applied a candidate without a user action')
        if summary['recommended_state']:
            compared = call(args.tellegen, 'run', study_path, {
                'expected_revision': summary['revision'],
                'operation': {'kind': 'compare', 'left': initial,
                              'right': summary['recommended_state'],
                              'goal': goal},
            })
            comparison = compared['comparison']
            packet['comparison'] = {key: comparison[key] for key in (
                'goal', 'left_value', 'right_value', 'improvement')}
            tolerance = max(spec['tolerances'].get('objective_absolute', 0),
                            spec['tolerances'].get('objective_relative', 0) * abs(comparison['left_value']))
            packet['improvement_tolerance'] = tolerance
            packet['outcome'] = ('no_verified_improvement' if summary['recommended_state'] == initial
                                 else 'verified_candidate' if comparison['improvement'] > tolerance
                                 else 'improvement_below_tolerance')
        else:
            packet['outcome'] = 'no_verified_candidate'
        bundle = call(args.tellegen, 'export', study_path)
        packet['experiment'] = bundle['document']['experiments'][proposed['experiment']]
        packet['study_sha256'] = digest(study_path)
    except (RuntimeError, AssertionError) as error:
        packet['outcome'] = 'failure'
        packet['failure'] = str(error)
    packet['elapsed_seconds'] = time.monotonic() - start
    (args.output / 'result.json').write_text(json.dumps(packet, indent=2) + '\n')
    print(json.dumps({key: packet[key] for key in ('outcome', 'elapsed_seconds')}))
    if packet['outcome'] == 'failure':
        raise SystemExit(1)


if __name__ == '__main__':
    main()
