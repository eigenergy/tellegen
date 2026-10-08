#!/usr/bin/env python3
"""Stage the pinned Texas7k distribution demo, preserving other manifest entries.

Requires Python 3, Git, Node, and a built Tellegen browser engine. No network
access: obtain BMOPFDraftData separately, then pass its repository directory.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
EVIDENCE = ROOT / 'evidence/studies/texas7k-reduction'
LOCK = Path(__file__).with_name('texas7k-source.json')
CASE_ID = 'texas7k-p1uhs0_1247'


def stage(repository, destination, wasm):
    lock = json.loads(LOCK.read_text())
    destination.mkdir(parents=True, exist_ok=True)
    manifest_path = destination / 'distribution-cases.json'
    manifest = json.loads(manifest_path.read_text()) if manifest_path.exists() else []
    if not isinstance(manifest, list) or any(not isinstance(x, dict) or 'id' not in x for x in manifest):
        raise ValueError('Existing distribution manifest must be an array of case entries')
    if len({x['id'] for x in manifest}) != len(manifest):
        raise ValueError('Existing distribution manifest contains duplicate IDs')
    with tempfile.TemporaryDirectory(prefix='.texas7k-', dir=destination) as temp:
        temp = Path(temp)
        source, output, evidence = temp / 'source', temp / 'case', temp / 'validation'
        source.mkdir()
        for name, expected in lock['files'].items():
            data = subprocess.check_output(['git', '-C', str(repository), 'show', f"{lock['revision']}:{lock['directory']}/{name}"])
            if hashlib.sha256(data).hexdigest() != expected:
                raise ValueError(f'Source checksum mismatch: {name}')
            (source / name).write_bytes(data)
        subprocess.run(['python3', str(EVIDENCE / 'reduce.py'), str(source / 'p1uhs0_1247.bmopf.json'), str(output)], check=True)
        subprocess.run(['node', str(EVIDENCE / 'validate-wasm.mjs'), str(wasm),
                        str(source / 'p1uhs0_1247.bmopf.json'), str(output / 'p1uhs0_1247.reduced.bmopf.json'),
                        str(output / 'tellegen_options.json'), str(evidence)], check=True)
        shutil.copytree(evidence, output / 'validation')
        shutil.copyfile(LOCK, output / 'source-lock.json')
        shutil.copyfile(source / 'README.md', output / 'SOURCE_README.md')
        provenance = json.loads((output / 'reduction-provenance.json').read_text())
        provenance['source_file'] = f"{lock['repository']}/blob/{lock['revision']}/{lock['directory']}/p1uhs0_1247.bmopf.json"
        (output / 'reduction-provenance.json').write_text(json.dumps(provenance, indent=2) + '\n')
        checksums = {str(p.relative_to(output)): hashlib.sha256(p.read_bytes()).hexdigest()
                     for p in sorted(output.rglob('*')) if p.is_file()}
        (output / 'checksums.json').write_text(json.dumps(checksums, indent=2) + '\n')
        # A content-addressed bundle allows data rollback without mutating old files.
        digest = hashlib.sha256((output / 'checksums.json').read_bytes()).hexdigest()[:16]
        relative = Path('distribution') / f'{CASE_ID}-{digest}'
        final = destination / relative
        final.parent.mkdir(parents=True, exist_ok=True)
        if final.exists():
            raise ValueError(f'Refusing to replace existing bundle: {final}')
        options = json.loads((output / 'tellegen_options.json').read_text())
        entry = {
            'id': CASE_ID, 'name': 'Texas7k distribution — p1uhs0_1247',
            'file': str(relative / 'p1uhs0_1247.reduced.pio.json'),
            'metadata': {
                'description': 'Six-feeder substation: nominal-load snapshot with frozen regulator taps. Voltage drops and equipment overloads are present; this is a power-flow example, not a demonstrated feasible OPF benchmark.',
                'source_url': f"{lock['repository']}/tree/{lock['revision']}/{lock['directory']}",
                'related_case_id': 'case7000', 'pf_options': options,
            },
        }
        next_manifest = [entry if item['id'] == CASE_ID else item for item in manifest]
        if not any(item['id'] == CASE_ID for item in manifest):
            next_manifest.append(entry)
        pending = temp / 'manifest.json'
        pending.write_text(json.dumps(next_manifest, indent=2, ensure_ascii=False) + '\n')
        os.replace(output, final)
        os.replace(pending, manifest_path)
        print(f'Staged {CASE_ID}: {final}')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('repository', type=Path, help='BMOPFDraftData Git checkout')
    parser.add_argument('destination', type=Path, nargs='?', default=ROOT / 'data')
    parser.add_argument('--wasm', type=Path, default=ROOT / 'packages/engine/dist/wasm-pkg')
    args = parser.parse_args()
    stage(args.repository.resolve(), args.destination.resolve(), args.wasm.resolve())
