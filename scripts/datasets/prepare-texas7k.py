#!/usr/bin/env python3
"""Reconstruct the checksum-pinned main island without building historical CLIs.

Uses the published PowerIO 0.11.7 Python binding and the original pinned
OpenDSS workflow, then checks the historical input hashes and paired current
browser WASM results. No download, staging, or deployment is performed.
"""
import argparse
import hashlib
import importlib.metadata
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[2]
EVIDENCE = ROOT / 'docs/evidence/studies/texas7k-reconversion'
LOCK = Path(__file__).with_name('texas7k-main-island-source.json')


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def replace_once(path, before, after):
    text = path.read_text()
    if text.count(before) != 1:
        raise ValueError(f'Unexpected pinned workflow: {path.name}')
    path.write_text(text.replace(before, after))


def verify_sources(repository, lock):
    repository = repository.resolve()
    revision = subprocess.check_output(
        ['git', '-C', str(repository), 'rev-parse', 'HEAD'], text=True).strip()
    if revision != lock['workflow_revision']:
        raise ValueError('BMOPFDraftData checkout must match the pinned workflow revision')
    # Also reject edits to the selected workflow, source files, and attribution.
    paths = ['scripts/Texas7k', 'test/data/Texas7k',
             *[f'output/Texas7kPilot/{name}' for name in
               ('License.md', 'SOURCE_NOTICE.md', 'tellegen_options.json')]]
    dirty = subprocess.check_output(
        ['git', '-C', str(repository), 'status', '--porcelain', '--untracked-files=no', '--', *paths], text=True)
    if dirty:
        raise ValueError('Pinned workflow and source paths must be clean')
    source = repository / 'test/data/Texas7k/p1uhs0_1247'
    manifest = repository / 'test/data/Texas7k/source_manifest.json'
    rows = json.loads(manifest.read_text())
    for row in rows:
        path = (source / row['path']).resolve()
        if not path.is_relative_to(source) or path.stat().st_size != row['size'] or sha(path) != row['sha256']:
            raise ValueError(f'Source checksum mismatch: {row["path"]}')
    return {'revision': revision, 'source_manifest_sha256': sha(manifest),
            'source_file_count': len(rows), 'source_bytes': sum(row['size'] for row in rows)}


def prepare(repository, destination, schema, wasm):
    import powerio

    lock = json.loads(LOCK.read_text())
    proof = verify_sources(repository, lock)
    required = {'powerio': lock['powerio_version']}
    for line in (repository / 'scripts/Texas7k/requirements.txt').read_text().splitlines():
        name, version = line.split('==')
        required[name] = version
    for name, version in required.items():
        if importlib.metadata.version(name) != version:
            raise ValueError(f'Preparation requires {name}=={version}')
    if destination.exists():
        raise ValueError(f'Refusing to replace prepared data: {destination}')
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='.texas7k-prepare-', dir=destination.parent) as directory:
        work = Path(directory)
        workflow, output, prepared = work / 'workflow', work / 'output', work / 'prepared'
        workflow.mkdir()
        tracked = subprocess.check_output(
            ['git', '-C', str(repository), 'ls-files', '-z', '--', 'scripts/Texas7k'], text=True)
        for name in filter(None, tracked.split('\0')):
            source_path = repository / name
            target = workflow / source_path.relative_to(repository / 'scripts/Texas7k')
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source_path, target)
        output.mkdir()
        # These are the same two adapter changes recorded in rerun.py. PowerIO
        # 0.11.7 now supplies the finite-source equivalent itself.
        normalize = workflow / 'normalize.py'
        text = normalize.read_text()
        start = text.index("  if cls.lower()=='circuit':")
        end = text.index("  if cls.lower()=='line':", start)
        normalize.write_text(text[:start] + "  if cls.lower()=='circuit':\n   out.append(line);continue\n" + text[end:])
        patch = workflow / 'patch_bmopf.py'
        replace_once(patch, "'id':'__source_internal'", "'id':x['voltage_source']['source']['bus']")
        replace_once(patch, "x['name']='Texas7k p1uhs0_1247 nominal snapshot'",
                     "x['extras']['conversion']['source_impedance']='Native PowerIO 0.11.7 Thevenin equivalent; original R1/X1/R0/X0 retained, no manual source branch'\n"
                     "x['name']='Texas7k p1uhs0_1247 nominal snapshot'")
        env = {**os.environ, 'TEXAS7K_WORKDIR': str(work),
               'TEXAS7K_SOURCE': str(repository / 'test/data/Texas7k/p1uhs0_1247'),
               'PYTHONDONTWRITEBYTECODE': '1'}
        for name in ('prepare_snapshot.py', 'audit.py', 'normalize.py'):
            subprocess.run([sys.executable, str(workflow / name)], env=env, check=True,
                           stdout=subprocess.DEVNULL)
        module = powerio.parse(output / 'normalized_snapshot.dss', format='dss')
        (output / 'normalized.bmopf.json').write_text(powerio.emit(module, 'bmopf-json@0.2.0').text)
        subprocess.run([sys.executable, str(patch)], env=env, check=True, stdout=subprocess.DEVNULL)
        full = output / 'p1uhs0_1247.bmopf.json'
        (output / 'pilot.bmopf.json').rename(full)
        if sha(full) != lock['full_conversion_sha256']:
            raise ValueError('Reconstructed full case differs from the recorded PowerIO 0.11.7 conversion')
        for name in ('License.md', 'SOURCE_NOTICE.md', 'tellegen_options.json'):
            shutil.copyfile(repository / 'output/Texas7kPilot' / name, output / name)

        # Adapt the existing extraction helper's executable interface without
        # pretending these are historical native binaries. Its paired checks
        # run against the actual browser WASM; record that explicitly below.
        serializer = work / 'serialize'
        serializer.write_text(f'#!{sys.executable}\n' +
            "import sys,json,powerio\n"
            "assert sys.argv[1]=='serialize'\n"
            "module=powerio.parse(sys.argv[2],format='bmopf-json')\n"
            "sys.stdout.write(powerio.serialize(module).text)\n"
            "sys.stderr.write(json.dumps(powerio.diagnostic_records(module.diagnostics)))\n")
        runner = work / 'solve.mjs'
        runner.write_text('#!/usr/bin/env node\n' +
            "import fs from 'node:fs';\n" +
            f"import * as engine from {json.dumps((wasm / 'tellegen.js').as_uri())};\n" +
            f"engine.initSync({{module:fs.readFileSync({json.dumps(str(wasm / 'tellegen_bg.wasm'))})}});\n" +
            "const text=fs.readFileSync(process.argv[2],'utf8'),options=fs.readFileSync(process.argv[3],'utf8');\n"
            "const payload=JSON.parse(engine.ingest_dist_case(text,'bmopf-json'));\n"
            "const input=process.argv[2].endsWith('.main-island.bmopf.json')?payload.module_json:JSON.parse(engine.prepare_mc_energized_network(payload.module_json)).module_json;\n"
            "process.stdout.write(engine.solve_mc_module(input,options));\n")
        serializer.chmod(0o755)
        runner.chmod(0o755)
        subprocess.run([sys.executable, str(EVIDENCE / 'extract-main-island.py'),
                        '--source', str(full), '--output', str(prepared), '--schema', str(schema),
                        '--powerio', str(serializer), '--mc-pf', str(runner), '--full-runner', str(runner)],
                       check=True, stdout=subprocess.DEVNULL)
        for name, expected in lock['files'].items():
            if sha(prepared / name) != expected:
                raise ValueError(f'Prepared input checksum mismatch: {name}')
        report_path = prepared / 'validation_report.json'
        report = json.loads(report_path.read_text())
        report['execution_note'] = ('This fresh paired check uses browser WASM through Node, not native '
                                    'Tellegen executables. The six prepared input files exactly match '
                                    'the historical native-qualified source lock.')
        report['all_wasm_result_fields_exactly_equal_except_selection_metadata'] = report.pop(
            'all_native_result_fields_exactly_equal_except_selection_metadata')
        report_path.write_text(json.dumps(report, indent=2) + '\n')
        proof.update(full_conversion_sha256=sha(full), prepared_input_sha256=lock['files'],
                     python_dependencies=required, schema_sha256=sha(schema),
                     wasm_sha256=sha(wasm / 'tellegen_bg.wasm'),
                     wasm_loader_sha256=sha(wasm / 'tellegen.js'),
                     source_lock_sha256=sha(LOCK),
                     validation='Exact historical input hashes and paired current browser WASM results; no new native run.')
        (prepared / 'reconstruction-provenance.json').write_text(json.dumps(proof, indent=2) + '\n')
        (prepared / 'artifact-manifest.json').write_text(json.dumps({
            p.name: {'bytes': p.stat().st_size, 'sha256': sha(p)}
            for p in sorted(prepared.iterdir()) if p.is_file() and p.name != 'artifact-manifest.json'
        }, indent=2) + '\n')
        os.replace(prepared, destination)
    print(f'Prepared and verified all six locked inputs: {destination}')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('repository', type=Path, help='Clean BMOPFDraftData checkout at the locked revision')
    parser.add_argument('destination', type=Path, help='New directory for the prepared source')
    parser.add_argument('--schema', required=True, type=Path, help='BMOPF schema from PowerIO 0.11.7')
    parser.add_argument('--wasm', required=True, type=Path, help='Current stable Tellegen wasm-pkg directory')
    args = parser.parse_args()
    prepare(args.repository.resolve(), args.destination.resolve(), args.schema.resolve(), args.wasm.resolve())
