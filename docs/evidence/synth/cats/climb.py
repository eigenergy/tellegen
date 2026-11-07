# /// script
# requires-python = ">=3.11"
# dependencies = ["numpy>=2.1", "scipy>=1.14"]
# ///
"""Coordinate search over the generator's tuned constants against CATS.

  uv run --script docs/evidence/synth/cats/climb.py --cats data/CATS \\
      --tellegen target/release/tellegen --work /tmp/climb [--seeds 2026] [--start T.json]

Each knob of KNOBS is tried at each of its values with every other knob held,
and kept when the mean score over the seeds falls (the score compare.py
reports: each metric's error in units of its tolerance, capped at 3, plus 10
per failed critical check). Passes repeat until one changes nothing or
--passes runs out. Every evaluation is appended to <work>/climb.jsonl; the
best constants are written to <work>/best.json, the file `tellegen generate
--tuning` reads. Only the native CLI's --tuning override changes; the engine
defaults change by hand afterwards, from best.json.
"""
from __future__ import annotations

import argparse
import json
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import compare as C  # noqa: E402
import metrics as M  # noqa: E402

KNOBS = [
    ('mw_per_bus', [5.0, 6.0, 7.0, 8.0, 9.0, 10.0]),
    ('pocket_extra', [[0.05] * 3, [0.12] * 3, [0.2] * 3, [0.3] * 3]),
    ('pocket_ties', [[0.05, 0.03, 0.02], [0.2, 0.15, 0.1], [0.35, 0.3, 0.25]]),
    ('mesh_ratio', [1.15, 1.25, 1.4, 1.55]),
    ('route_noise', [0.15, 0.35, 0.55]),
    ('tap_share', [0.5, 0.85, 1.0]),
    ('tap_straight_share', [0.3, 0.6, 0.85]),
    ('tap_at_site_share', [0.4, 0.7, 0.95]),
    ('fill_gamma', [0.35, 0.5, 0.7, 1.0]),
    ('fill_floor', [0.02, 0.06, 0.12]),
    ('fill_bandwidth_km', [6.0, 12.0, 24.0]),
    ('nearest_load_share', [0.5, 0.75, 0.9]),
    ('remote_renewables', [0.5, 0.8, 1.0]),
    ('pocket_feeders', [1.0, 1.5, 2.0]),
    ('step_down', [[0.5, 0.2, 0.14], [0.5, 0.25, 0.18], [0.5, 0.32, 0.24]]),
    ('level_shares_4', [[0.015, 0.12, 0.24, 0.625], [0.012, 0.1, 0.22, 0.668], [0.018, 0.14, 0.26, 0.582]]),
    ('generator_bus_share', [0.12, 0.16, 0.22]),
]


def evaluate(args, spec, ref, tuning, tag):
    work = Path(args.work) / tag
    work.mkdir(parents=True, exist_ok=True)
    tpath = work / 'tuning.json'
    tpath.write_text(json.dumps(tuning))
    results = {}
    for seed in args.seed_list:
        req = dict(spec['request'], seed=seed)
        rp = work / f'request-{seed}.json'
        rp.write_text(json.dumps(req))
        out = work / f'seed-{seed}'
        if out.exists():
            import shutil
            shutil.rmtree(out)
        try:
            C.generate(args.tellegen, rp, out, tuning=tpath)
            results[seed] = C.score_one(ref, M.load_generated(out), 'synthetic')
        except SystemExit as e:
            results[seed] = {'score': 1e9, 'passed': 0, 'scored': 0, 'error': str(e)[:300]}
        if not args.keep and out.exists():
            import shutil
            shutil.rmtree(out)
    scores = [r['score'] for r in results.values()]
    return sum(scores) / len(scores), {s: (r['passed'], r.get('scored')) for s, r in results.items()}


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--cats', required=True)
    ap.add_argument('--spec', default=str(C.HERE / 'california.json'))
    ap.add_argument('--tellegen', required=True)
    ap.add_argument('--work', required=True)
    ap.add_argument('--seeds', default='2026')
    ap.add_argument('--start', help='tuning JSON to start from (default: the engine defaults)')
    ap.add_argument('--passes', type=int, default=2)
    ap.add_argument('--only', help='comma-separated knob names to search')
    ap.add_argument('--keep', action='store_true', help='keep generated outputs')
    args = ap.parse_args()
    args.seed_list = [int(s) for s in args.seeds.split(',')]
    spec = json.loads(Path(args.spec).read_text())
    ref = C.load_reference(args.cats)
    best = json.loads(Path(args.start).read_text()) if args.start else {}
    log = Path(args.work) / 'climb.jsonl'
    log.parent.mkdir(parents=True, exist_ok=True)
    n = 0

    def run(tuning, note):
        nonlocal n
        n += 1
        t0 = time.time()
        score, passed = evaluate(args, spec, ref, tuning, f'eval-{n:04d}')
        entry = {'eval': n, 'note': note, 'tuning': tuning, 'score': score, 'passed': passed,
                 'seconds': round(time.time() - t0, 1)}
        with log.open('a') as fh:
            fh.write(json.dumps(entry) + '\n')
        print(f'{n:4d} {score:8.3f} {passed} {note}', flush=True)
        return score

    best_score = run(best, 'start')
    knobs = [k for k in KNOBS if not args.only or k[0] in args.only.split(',')]
    for p in range(args.passes):
        changed = False
        for name, values in knobs:
            for v in values:
                if best.get(name) == v:
                    continue
                trial = dict(best, **{name: v})
                score = run(trial, f'pass {p + 1}: {name} = {v}')
                if score < best_score - 1e-6:
                    best, best_score, changed = trial, score, True
                    (Path(args.work) / 'best.json').write_text(json.dumps(best, indent=1) + '\n')
        if not changed:
            break
    (Path(args.work) / 'best.json').write_text(json.dumps(best, indent=1) + '\n')
    print(json.dumps({'best_score': best_score, 'best': best}))


if __name__ == '__main__':
    main()
