#!/usr/bin/env python3
"""Verify POUNCE versions and the source notices shipped with Python and WASI."""

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
import zipfile
from email.parser import BytesParser
from pathlib import Path

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


def python_notice(packages):
    # Cargo.lock covers the whole workspace. Select normal runtime dependencies
    # of the shipped extension, including target-specific crates across wheels.
    tree = subprocess.check_output([
        'cargo', 'tree', '--locked', '--package', 'tellegen-py', '--features',
        'extension-module,ots', '--target', 'all', '--edges', 'normal',
        '--prefix', 'none', '--format', '{p}',
    ], cwd=ROOT, text=True)
    linked = set(re.findall(r'^(pounce-[a-z0-9_-]+) v(\S+)', tree, re.MULTILINE))
    if not {'pounce-nl', 'pounce-rs'} <= {name for name, _ in linked}:
        raise SystemExit('the Python wheel must link the POUNCE solver')
    rows = ["Tellegen Python wheel", "",
            "The native extension contains the following unmodified POUNCE crates under",
            "the Eclipse Public License 2.0. Each URL supplies its corresponding source.",
            "The license accompanies this notice in POUNCE-EPL-2.0.txt. Tellegen's own",
            "code is available under MIT."]
    for package in packages:
        name, version = package['name'], package['version']
        if (name, version) in linked:
            rows.extend(['', f'{name} {version}',
                         f'https://static.crates.io/crates/{name}/{name}-{version}.crate',
                         f"SHA-256 {package['checksum']}"])
    return '\n'.join(rows) + '\n'


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--asset', type=Path, help='write provenance beside this built WASI asset')
    parser.add_argument('--write-python-notice', action='store_true',
                        help='refresh the wheel source notice after a POUNCE update')
    parser.add_argument('--check-python-wheel', type=Path,
                        help='verify the EPL license and resolved sources in a wheel')
    args = parser.parse_args()
    version, packages = release_packages()
    if args.write_python_notice or args.check_python_wheel:
        notice_path = 'licenses/POUNCE-PYTHON-NOTICE.txt'
        notice = python_notice(packages)
        if args.write_python_notice:
            (ROOT / notice_path).write_text(notice, encoding='utf-8')
        if args.check_python_wheel:
            license_path = 'licenses/POUNCE-EPL-2.0.txt'
            with zipfile.ZipFile(args.check_python_wheel) as wheel:
                metadata_path, = (name for name in wheel.namelist() if name.endswith('.dist-info/METADATA'))
                metadata = BytesParser().parsebytes(wheel.read(metadata_path), headersonly=True)
                if metadata.get('License-Expression') != 'MIT AND EPL-2.0':
                    raise SystemExit('wheel must declare the licenses of Tellegen and its linked solver')
                required = {'LICENSE', str(notice_path), str(license_path)}
                if not required.issubset(metadata.get_all('License-File', [])):
                    raise SystemExit('wheel metadata is missing a license file')
                for path, expected in [(notice_path, notice),
                                       (license_path, (ROOT / license_path).read_text(encoding='utf-8'))]:
                    actual = wheel.read(path).decode('utf-8').splitlines()
                    if actual != expected.splitlines():
                        raise SystemExit(f'{path} in {args.check_python_wheel} differs from the source; '
                                         'refresh the notice with --write-python-notice and rebuild')
    if args.asset:
        revision = os.environ.get('TELLEGEN_SOURCE_REVISION', '').strip()
        if revision and not re.fullmatch(r'[0-9a-f]{40}', revision):
            raise SystemExit('TELLEGEN_SOURCE_REVISION must be a full Git commit SHA')
        # The deploy image build has no .git and names the commit it copied;
        # that is the asset the website distributes.
        release = bool(revision) and not (ROOT / '.git').exists()
        dirty = None
        if (ROOT / '.git').exists():
            revision = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
            dirty = bool(subprocess.check_output(['git', 'status', '--porcelain'], cwd=ROOT, text=True))
        info = {
            'development_only': not release,
            'solver': 'POUNCE',
            'version': version,
            'license': 'EPL-2.0',
            'upstream_source': f'https://github.com/jkitchin/pounce/tree/v{version}',
            'tellegen_source': (f'https://github.com/eigenergy/tellegen/tree/{revision}'
                                if revision else 'https://github.com/eigenergy/tellegen'),
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
