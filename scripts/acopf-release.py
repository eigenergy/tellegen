#!/usr/bin/env python3
"""Check the published POUNCE pin; optionally describe an opt-in WASI artifact."""

import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
if sys.version_info < (3, 11):
    raise SystemExit("POUNCE release verification requires Python 3.11+; set PYTHON to a supported interpreter for build-acopf-wasi.sh")
import tomllib

ROOT = Path(__file__).resolve().parents[1]
REGISTRY = 'registry+https://github.com/rust-lang/crates.io-index'


def release_packages():
    deps = tomllib.loads((ROOT / 'Cargo.toml').read_text())['workspace']['dependencies']
    version = deps['pounce-rs']
    if not isinstance(version, str) or not re.fullmatch(r'=\d+\.\d+\.\d+', version):
        raise SystemExit('pounce-rs must pin an exact published release')
    if deps['pounce-nl'] != version:
        raise SystemExit('pounce-nl and pounce-rs must pin the same release')
    version = version[1:]
    packages = [p for p in tomllib.loads((ROOT / 'Cargo.lock').read_text())['package']
                if p['name'].startswith('pounce-')]
    if not {'pounce-nl', 'pounce-rs'} <= {p['name'] for p in packages}:
        raise SystemExit('POUNCE dependencies missing from Cargo.lock')
    for package in packages:
        if (package['version'] != version or package.get('source') != REGISTRY
                or not re.fullmatch(r'[0-9a-f]{64}', package.get('checksum', ''))):
            raise SystemExit(f"{package['name']} must resolve to checksummed crates.io {version}")
    return version, packages


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--asset', type=Path, help='write provenance beside this built WASI asset')
    args = parser.parse_args()
    version, packages = release_packages()
    if args.asset:
        revision = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
        dirty = bool(subprocess.check_output(['git', 'status', '--porcelain'], cwd=ROOT, text=True))
        info = {
            'development_only': True,
            'solver': 'POUNCE',
            'version': version,
            'license': 'EPL-2.0',
            'upstream_source': f'https://github.com/jkitchin/pounce/tree/v{version}',
            'tellegen_source': f'https://github.com/eigenergy/tellegen/tree/{revision}',
            'tellegen_worktree_dirty': dirty,
            'artifact_sha256': digest(args.asset),
            'artifact_bytes': args.asset.stat().st_size,
            'cargo_lock_sha256': digest(ROOT / 'Cargo.lock'),
            'crates': [{
                'name': p['name'], 'version': p['version'], 'sha256': p['checksum'],
                'source_archive': f"https://crates.io/api/v1/crates/{p['name']}/{p['version']}/download",
            } for p in packages],
        }
        (args.asset.parent / 'POUNCE-SOURCE.json').write_text(json.dumps(info, indent=2) + '\n')
    print(f'ok: {len(packages)} POUNCE crates use checksummed release {version}')


if __name__ == '__main__':
    main()
