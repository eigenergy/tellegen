#!/usr/bin/env python3
"""Stage the pinned Texas7k distribution demo, preserving other manifest entries.

Requires Python 3, Git, Node, and a built Tellegen browser engine. No network
access: obtain BMOPFDraftData separately, then pass its repository directory.
"""
import argparse
import gzip
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
EVIDENCE = ROOT / 'evidence/studies/texas7k-reduction'
LOCK = Path(__file__).with_name('texas7k-source.json')
MAIN_ISLAND_LOCK = Path(__file__).with_name('texas7k-main-island-source.json')
CASE_ID = 'texas7k-p1uhs0_1247'


def stage(repository, destination, wasm, main_island=False):
    lock_path = MAIN_ISLAND_LOCK if main_island else LOCK
    lock = json.loads(lock_path.read_text())
    case_file = lock.get('case_file', 'p1uhs0_1247.bmopf.json')
    destination.mkdir(parents=True, exist_ok=True)
    manifest_path = destination / 'distribution-cases.json'
    manifest = json.loads(manifest_path.read_text()) if manifest_path.exists() else []
    if not isinstance(manifest, list) or any(not isinstance(x, dict) or 'id' not in x for x in manifest):
        raise ValueError('Existing distribution manifest must be an array of case entries')
    if len({x['id'] for x in manifest}) != len(manifest):
        raise ValueError('Existing distribution manifest contains duplicate IDs')
    with tempfile.TemporaryDirectory(prefix='.texas7k-', dir=destination) as temporary_directory:
        temp = Path(temporary_directory)
        source, output, evidence = temp / 'source', temp / 'case', temp / 'validation'
        source.mkdir()
        for name, expected in lock['files'].items():
            if main_island:
                data = (repository / name).read_bytes()
            else:
                data = subprocess.check_output(['git', '-C', str(repository), 'show', f"{lock['revision']}:{lock['directory']}/{name}"])
            if hashlib.sha256(data).hexdigest() != expected:
                raise ValueError(f'Source checksum mismatch: {name}')
            (source / name).write_bytes(data)
        subprocess.run([sys.executable, str(EVIDENCE / 'reduce.py'), str(source / case_file), str(output),
                        *(['--externalize-linecode-sources'] if main_island else [])], check=True)
        subprocess.run(['node', str(EVIDENCE / 'validate-wasm.mjs'), str(wasm),
                        str(source / case_file), str(output / 'p1uhs0_1247.reduced.bmopf.json'),
                        str(output / 'tellegen_options.json'), str(evidence)], check=True)
        module = output / 'p1uhs0_1247.reduced.pio.json'
        module.with_suffix('.json.gz').write_bytes(gzip.compress(module.read_bytes(), compresslevel=9, mtime=0))
        shutil.copytree(evidence, output / 'validation')
        shutil.copyfile(lock_path, output / 'source-lock.json')
        shutil.copyfile(source / 'README.md', output / 'SOURCE_README.md')
        provenance = json.loads((output / 'reduction-provenance.json').read_text())
        if main_island:
            provenance['source_preparation_lock'] = 'source-lock.json'
            source_url = lock['source_url']
        else:
            provenance['source_file'] = f"{lock['repository']}/blob/{lock['revision']}/{lock['directory']}/p1uhs0_1247.bmopf.json"
            source_url = f"{lock['repository']}/tree/{lock['revision']}/{lock['directory']}"
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
                'description': ('Main energized island of the six-feeder substation; all 15,299 loads retained. ' if main_island else 'Six-feeder substation: ') + 'Nominal-load snapshot with frozen regulator taps. Voltage drops and equipment overloads are present; this is a power-flow example, not a demonstrated feasible OPF benchmark.',
                'source_url': source_url,
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
    parser.add_argument('repository', type=Path, help='BMOPFDraftData Git checkout, or prepared case directory with --main-island')
    parser.add_argument('destination', type=Path, nargs='?', default=ROOT / 'data')
    parser.add_argument('--wasm', type=Path, default=ROOT / 'packages/engine/dist/wasm-pkg')
    parser.add_argument('--main-island', action='store_true', help='Stage the checksum-pinned PowerIO 0.11.7 main-island derivative')
    args = parser.parse_args()
    stage(args.repository.resolve(), args.destination.resolve(), args.wasm.resolve(), args.main_island)
