"""Run the original bounded adapter with explicit de-energized-island validation.

Original scripts and source files are read only. A private workflow copy
compares solved nodes and separately verifies the 15 excluded OpenDSS nodes
are exactly zero. No zero voltages are inserted into the Tellegen result.
"""

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path


def replace_once(path, before, after):
    text = path.read_text()
    assert text.count(before) == 1, (path, before)
    path.write_text(text.replace(before, after))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--bmopf-root', required=True, type=Path)
    parser.add_argument('--workdir', required=True, type=Path)
    parser.add_argument('--powerio', required=True, type=Path)
    parser.add_argument('--schema', required=True, type=Path)
    parser.add_argument('--runner', required=True, type=Path)
    args = parser.parse_args()
    args.workdir.mkdir(parents=True, exist_ok=False)
    original = args.bmopf_root / 'scripts/Texas7k'
    copied = args.workdir / 'workflow'
    shutil.copytree(original, copied, ignore=shutil.ignore_patterns('__pycache__'))
    hashes = {path.name: hashlib.sha256(path.read_bytes()).hexdigest()
              for path in original.glob('*.py')}
    # 0.11.7 retains the actual OpenDSS source impedance. The old adapter
    # manually added it and replaced the DSS source impedance by 1e-12 ohm;
    # keeping that workaround would add a redundant, ill-conditioned branch.
    normalize = copied / 'normalize.py'
    text = normalize.read_text()
    start = text.index("  if cls.lower()=='circuit':")
    end = text.index("  if cls.lower()=='line':", start)
    normalize.write_text(text[:start] + "  if cls.lower()=='circuit':\n   out.append(line);continue\n" + text[end:])
    patch = copied / 'patch_bmopf.py'
    replace_once(patch, "'id':'__source_internal'", "'id':x['voltage_source']['source']['bus']")
    replace_once(patch, "x['name']='Texas7k p1uhs0_1247 nominal snapshot'",
                 "x['extras']['conversion']['source_impedance']='Native PowerIO 0.11.7 Thevenin equivalent; original R1/X1/R0/X0 retained, no manual source branch'\n"
                 "x['name']='Texas7k p1uhs0_1247 nominal snapshot'")
    replace_once(copied / 'summarize_pf.py', " if name=='__source_impedance':continue",
                 " if name=='_pio_vsource_source_impedance':continue")
    compare = copied / 'compare_tellegen.py'
    replace_once(compare, 'errors=[];missing=[]',
                 "excluded_buses={b for island in result['excluded_islands'] for b in island['buses']}\n"
                 "errors=[];missing=[];excluded=[]")
    replace_once(compare, ' if key not in volts:missing.append(key);continue',
                 " if key not in volts:\n"
                 "  if key.rsplit('.',1)[0] in excluded_buses:\n"
                 "   assert complex(*value)==0, ('nonzero excluded reference node',key)\n"
                 "   excluded.append(key);continue\n"
                 "  missing.append(key);continue")
    replace_once(compare, 'report.update(compared_nodes=len(errors),missing_nodes=missing,worst_10=errors[:10])',
                 "report.update(compared_nodes=len(errors),missing_nodes=missing,worst_10=errors[:10],"
                 "excluded_reference_node_count=len(excluded),excluded_reference_nodes=sorted(excluded),"
                 "excluded_reference_voltages_exactly_zero=True)")
    antifloat = copied / 'check_antifloat.py'
    replace_once(antifloat, "worst=(0,'');cnt=0", "excluded_buses={b for island in t['excluded_islands'] for b in island['buses']}\nworst=(0,'');cnt=0;excluded=[]")
    replace_once(antifloat,
                 "  key=f'{b}.{n}';dev=abs(complex(vs[2*i],vs[2*i+1])-v[key])/scale;cnt+=1",
                 "  key=f'{b}.{n}'\n"
                 "  if key not in v and b in excluded_buses:\n"
                 "   assert complex(vs[2*i],vs[2*i+1])==0\n"
                 "   excluded.append(key);continue\n"
                 "  dev=abs(complex(vs[2*i],vs[2*i+1])-v[key])/scale;cnt+=1")
    replace_once(antifloat, "(O/'antifloat_comparison.json').write_text",
                 "report.update(excluded_reference_node_count=len(excluded),excluded_reference_voltages_exactly_zero=True)\n"
                 "(O/'antifloat_comparison.json').write_text")
    driver = copied / 'convert_substation.py'
    replace_once(driver, "    assert comparison['converged'] and not comparison['missing_nodes']",
                 "    assert comparison['converged'] and not comparison['missing_nodes']\n"
                 "    assert comparison['excluded_reference_node_count'] == 15\n"
                 "    assert comparison['compared_nodes'] == 38203\n"
                 "    assert antifloat['excluded_reference_node_count'] == 15")
    command = [sys.executable, str(driver), '--source',
               str(args.bmopf_root / 'test/data/Texas7k/p1uhs0_1247'),
               '--manifest', str(args.bmopf_root / 'test/data/Texas7k/source_manifest.json'),
               '--workdir', str(args.workdir), '--powerio', str(args.powerio),
               '--schema', str(args.schema), '--tellegen', str(args.runner)]
    metadata = {'original_workflow_sha256': hashes, 'command': command,
                'workflow_revision': subprocess.check_output(
                    ['git', '-C', str(args.bmopf_root), 'rev-parse', 'HEAD'], text=True).strip(),
                'source_change': 'Remove obsolete manual source-impedance branch; let PowerIO 0.11.7 retain the original finite source impedance and assign its internal bus the source location.',
                'comparison_change': 'Explicitly exclude only the 15 zero-voltage source nodes on the two recorded unsupplied islands; retain all original equipment in BMOPF.'}
    (args.workdir / 'rerun-provenance.json').write_text(json.dumps(metadata, indent=2) + '\n')
    subprocess.run(command, check=True, env={**os.environ, 'PYTHONDONTWRITEBYTECODE': '1',
                                            'TEXAS7K_MODULE_OUTPUT': str(args.workdir / 'output/p1uhs0_1247.energized.pio.json'),
                                            'MPLCONFIGDIR': str(args.workdir / 'matplotlib')})


if __name__ == '__main__':
    main()
