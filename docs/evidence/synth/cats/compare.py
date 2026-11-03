# /// script
# requires-python = ">=3.11"
# dependencies = ["numpy>=2.1", "scipy>=1.14"]
# ///
"""Score generated California grids against the California Test System.

  uv run --script docs/evidence/synth/cats/compare.py run \\
      --cats data/CATS --spec docs/evidence/synth/cats/california.json \\
      --tellegen target/release/tellegen --mode synthetic --work /tmp/cats-run
  uv run --script docs/evidence/synth/cats/compare.py score --cats data/CATS DIR...

`run` generates the spec at each seed (or once with --osm DOC for OpenStreetMap
mode), then scores. `score` scores existing `tellegen generate` output
directories. Both write scorecard.json and scorecard.md into --work (or the
first DIR's parent) and print one summary line: the pass verdict and the score
the hill climb minimises.

CATS (Taylor et al., IEEE TEMPR 2(1), 2024; BSD-3-Clause) is read from a
directory staged by scripts/stage-data.sh and never copied into the repository;
`reference` writes its summary metrics, which is all that is committed.
"""
from __future__ import annotations

import argparse
import json
import math
import subprocess
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
import metrics as M  # noqa: E402

HERE = Path(__file__).resolve().parent
CELL_LINES_KM = 25.0
CELL_LOAD_KM = 50.0
PASS_SHARE = 0.80

# (id, group, label, kind, synthetic tolerance, OSM tolerance)
# kinds: rel (|g/r-1| <= tol), ratio (lo <= g/r <= hi), abs (|g-r| <= tol),
#        atleast (g >= tol, for shares and correlations in [0, 1]),
#        atmost (g <= tol)
SCALE = [
    ('peak_load_mw', 'scale', 'Peak load (MW)', 'rel', 0.15, 0.15),
    ('nameplate_mw', 'scale', 'Nameplate excl. condensers and imports (MW)', 'rel', 0.20, 0.20),
    ('reduced_buses', 'scale', 'Buses after series reduction', 'ratio', (0.70, 1.40), (0.75, 1.33)),
    ('sites', 'scale', 'Sites (buses within 300 m)', 'ratio', (0.70, 1.40), (0.75, 1.33)),
    *[(f'bus_share.{c}', 'scale', f'Bus share at {c} kV', 'abs', 0.06, 0.06) for c in ('66', '115', '230', '500')],
    *[(f'circuit_km.{c}', 'scale', f'Circuit-km at {c} kV', 'ratio', (0.67, 1.5), (0.8, 1.25))
      for c in ('66', '115', '230', '500')],
    ('transformers_per_site', 'scale', 'Transformers per site', 'ratio', (0.67, 1.5), (0.67, 1.5)),
    ('generators', 'scale', 'Generators excl. condensers', 'ratio', (0.5, 2.0), (0.5, 2.0)),
    ('fuel_tvd', 'scale', 'Fuel capacity mix, total variation distance', 'atmost', 0.15, 0.15),
]
TOPOLOGY = [
    ('mean_degree', 'topology', 'Mean degree (reduced)', 'rel', 0.12, 0.12),
    ('degree_l1', 'topology', 'Degree distribution, L1 distance', 'atmost', 0.25, 0.20),
    *[(f'bands.{b}.lines_per_site', 'topology', f'Lines per site, {b} band', 'rel', 0.15, 0.15) for b in M.BANDS],
    *[(f'bands.{b}.length_over_mst', 'topology', f'Line length / MST, {b} band', 'rel', 0.25, 0.20) for b in M.BANDS],
    *[(f'bands.{b}.delaunay_1', 'topology', f'Lines on the Delaunay triangulation, {b} band', 'abs', 0.10, 0.10)
      for b in M.BANDS],
    ('clustering', 'topology', 'Mean clustering coefficient', 'abs', 0.05, 0.05),
    ('mean_hops', 'topology', 'Mean hop distance', 'ratio', (0.67, 1.5), (0.67, 1.5)),
]
PARAMETERS = [
    *[(f'params.{c}.{p}', 'parameters', f'{label} inside CATS p10-p90 at {c} kV', 'atleast', 0.70, 0.70)
      for c in ('66', '115', '230', '500')
      for p, label in (('x_ohm_km', 'x per km'), ('x_over_r', 'X/R'), ('rate_mva', 'Rating'))],
    ('params.tx_x', 'parameters', 'Transformer x on own base inside CATS p10-p90', 'atleast', 0.80, 0.80),
]
OPERATION = [
    ('dc_loading_p50', 'operation', 'DC OPF branch loading, median', 'abs', 0.15, 0.15),
    ('dc_loading_p90', 'operation', 'DC OPF branch loading, p90', 'abs', 0.15, 0.15),
    ('dc_binding', 'operation', 'DC OPF binding fraction', 'abs', 0.05, 0.05),
]
GEOGRAPHY = [
    ('chamfer_km', 'geography', 'Site distance to CATS sites, symmetric mean (km)', 'atmost', 20.0, 5.0),
    ('line_density_rho', 'geography', 'Line-km per 25 km cell, Spearman', 'atleast', 0.60, 0.85),
    ('load_density_rho', 'geography', 'Load per 50 km cell, Spearman', 'atleast', 0.70, 0.70),
]
METRICS = SCALE + TOPOLOGY + PARAMETERS + OPERATION + GEOGRAPHY


def dig(m, path):
    cur = m
    for part in path.split('.'):
        if not isinstance(cur, dict) or part not in cur:
            return None
        cur = cur[part]
    return cur


def band(values, lo_q=10, hi_q=90):
    v = np.asarray(values, dtype=float)
    v = v[np.isfinite(v)]
    if len(v) < 20:
        return None
    return float(np.percentile(v, lo_q)), float(np.percentile(v, hi_q))


def inside(values, window):
    v = np.asarray(values, dtype=float)
    v = v[np.isfinite(v)]
    if window is None or len(v) == 0:
        return None
    return float(np.mean((v >= window[0]) & (v <= window[1])))


def derived(ref, gen):
    """Metrics that need both networks: parameter bands, distances, grids."""
    out = {}
    out['fuel_tvd'] = 0.5 * sum(abs(gen['fuel_mix'][k] - ref['fuel_mix'][k]) for k in M.FUELS)
    out['degree_l1'] = sum(abs(gen['degree_pmf'][k] - ref['degree_pmf'][k]) for k in ref['degree_pmf'])
    params = {}
    for c in ('66', '115', '230', '500'):
        params[c] = {}
        for p in ('x_ohm_km', 'x_over_r', 'rate_mva'):
            window = band(dig(ref, f'line_params.{c}.{p}') or [])
            vals = dig(gen, f'line_params.{c}.{p}') or []
            # CATS has the class but the generated grid does not: a failure.
            params[c][p] = None if window is None else (inside(vals, window) if len(vals) >= 5 else 0.0)
    params['tx_x'] = inside(gen['tx_x_own_base'], band(ref['tx_x_own_base']))
    out['params'] = params
    out['chamfer_km'] = M.chamfer_km(gen['site_xy'], ref['site_xy'])
    for which, cell, key in (('lines', CELL_LINES_KM, 'line_density_rho'), ('load', CELL_LOAD_KM, 'load_density_rho')):
        pts = ref['_route_samples'][0] if which == 'lines' else ref['_load_xy'][0]
        origin = pts.min(axis=0) - cell
        span = pts.max(axis=0) + cell - origin
        shape = tuple(int(math.ceil(s / cell)) for s in span)
        a = M.gridded(ref, origin, shape, cell, which)
        b = M.gridded(gen, origin, shape, cell, which)
        mask = (a > 0) | (b > 0)
        out[key] = M.spearman(a, b, mask)
    return out


def norm_err(kind, tol, g, r):
    """Deviation in units of the tolerance: <= 1 passes."""
    if g is None or (kind in ('rel', 'ratio', 'abs') and r is None):
        return None
    if kind == 'rel':
        return abs(g / r - 1) / tol if r else math.inf
    if kind == 'ratio':
        if not r or g <= 0:
            return math.inf
        q = g / r
        lo, hi = tol
        return math.log(q) / math.log(hi) if q >= 1 else math.log(q) / math.log(lo)
    if kind == 'abs':
        return abs(g - r) / tol
    if kind == 'atleast':
        return max(0.0, 1 - g) / (1 - tol) if tol < 1 else (0.0 if g >= tol else math.inf)
    if kind == 'atmost':
        return g / tol if tol else math.inf
    raise ValueError(kind)


def critical(gen_net, gen):
    notes = gen_net.notes
    ac = notes.get('ac_pf') or {}
    return {
        'main_load_share': gen['main_load_share'] >= 0.95,
        'dc_opf_engine': notes.get('dc_opf') == 'optimal',
        'dc_opf_harness': gen.get('dc_status') == 'optimal',
        'ac_pf': ac.get('status') == 'converged' and (ac.get('buses_outside_emergency_band') or 0) == 0,
    }


def score_one(ref, gen_net, mode):
    gen = M.measure(gen_net)
    gen.update(derived(ref, gen))
    rows = []
    for mid, group, label, kind, tol_s, tol_o in METRICS:
        tol = tol_o if mode == 'osm' else tol_s
        g = dig(gen, mid)
        r = dig(ref, mid) if kind in ('rel', 'ratio', 'abs') else None
        e = norm_err(kind, tol, g, r)
        if e is None and r is not None:
            # CATS has this quantity and the generated grid cannot produce it.
            e = math.inf
        rows.append({'id': mid, 'group': group, 'label': label, 'kind': kind, 'tolerance': tol,
                     'value': g, 'reference': r, 'error': e,
                     'pass': None if e is None else bool(e <= 1.0 + 1e-12)})
    crit = critical(gen_net, gen)
    scored = [row for row in rows if row['pass'] is not None]
    passed = sum(row['pass'] for row in scored)
    share = passed / max(len(scored), 1)
    score = sum(min(row['error'], 3.0) for row in scored) + 10.0 * sum(not v for v in crit.values())
    return {
        'critical': crit,
        'metrics': rows,
        'passed': passed,
        'scored': len(scored),
        'pass_share': share,
        'pass': all(crit.values()) and share >= PASS_SHARE,
        'score': score,
        'context': gen_net.notes.get('context'),
        'ac_pf': gen_net.notes.get('ac_pf'),
        'summary': {k: gen.get(k) for k in ('raw_buses', 'raw_branches', 'reduced_buses', 'sites',
                                            'peak_load_mw', 'nameplate_mw', 'mean_degree', 'max_degree',
                                            'clustering', 'transformers')},
    }


def public(m):
    """A metrics dict without the private arrays (for JSON)."""
    out = {}
    for k, v in m.items():
        if k.startswith('_') or k in ('site_xy', 'line_params', 'tx_x_own_base'):
            continue
        out[k] = v
    out['line_params'] = {c: {p: {'n': len(vals), 'p10': float(np.percentile(vals, 10)),
                                  'p50': float(np.percentile(vals, 50)), 'p90': float(np.percentile(vals, 90))}
                              for p, vals in d.items() if vals}
                          for c, d in m.get('line_params', {}).items()}
    if m.get('tx_x_own_base'):
        v = m['tx_x_own_base']
        out['tx_x_own_base'] = {'n': len(v), 'p10': float(np.percentile(v, 10)),
                                'p50': float(np.percentile(v, 50)), 'p90': float(np.percentile(v, 90))}
    return out


def fmt(v):
    if v is None:
        return '–'
    if isinstance(v, bool):
        return 'yes' if v else 'no'
    if isinstance(v, (int, np.integer)):
        return f'{int(v):,}'
    if isinstance(v, float):
        if v == 0:
            return '0'
        if abs(v) >= 1000:
            return f'{v:,.0f}'
        if abs(v) >= 10:
            return f'{v:.1f}'
        return f'{v:.3f}'
    return str(v)


def tol_text(kind, tol):
    return {'rel': lambda t: f'±{t:.0%}', 'ratio': lambda t: f'×{t[0]}–{t[1]}', 'abs': lambda t: f'±{t}',
            'atleast': lambda t: f'≥ {t}', 'atmost': lambda t: f'≤ {t}'}[kind](tol)


def write_scorecard(work, mode, label, results, ref_summary):
    work.mkdir(parents=True, exist_ok=True)
    doc = {'schema': 'tellegen.cats-scorecard/1', 'mode': mode, 'label': label,
           'pass_rule': f'every critical check, and at least {PASS_SHARE:.0%} of the scored metrics, in every run',
           'runs': results, 'reference': ref_summary}
    (work / 'scorecard.json').write_text(json.dumps(doc, indent=1, default=float) + '\n')
    names = list(results)
    lines = [f'# CATS scorecard: {label} ({mode})', '',
             f'Pass rule: {doc["pass_rule"]}.', '',
             '| Run | Verdict | Passed | Score | Critical failures |', '| --- | --- | --- | --- | --- |']
    for n in names:
        r = results[n]
        bad = [k for k, v in r['critical'].items() if not v]
        lines.append(f'| {n} | {"pass" if r["pass"] else "fail"} | {r["passed"]}/{r["scored"]} '
                     f'({r["pass_share"]:.0%}) | {r["score"]:.2f} | {", ".join(bad) or "none"} |')
    lines += ['', '| Metric | CATS | ' + ' | '.join(names) + ' | Tolerance |',
              '| --- | --- | ' + ' | '.join('---' for _ in names) + ' | --- |']
    for i, (_mid, _group, label_, kind, tol_s, tol_o) in enumerate(METRICS):
        tol = tol_o if mode == 'osm' else tol_s
        ref_v = results[names[0]]['metrics'][i]['reference']
        cells = []
        for n in names:
            row = results[n]['metrics'][i]
            mark = '' if row['pass'] is None else (' ✓' if row['pass'] else ' ✗')
            cells.append(fmt(row['value']) + mark)
        lines.append(f'| {label_} | {fmt(ref_v)} | ' + ' | '.join(cells) + f' | {tol_text(kind, tol)} |')
    (work / 'scorecard.md').write_text('\n'.join(lines) + '\n')


def summary_line(results):
    verdicts = [r['pass'] for r in results.values()]
    scores = [r['score'] for r in results.values()]
    passed = [f'{r["passed"]}/{r["scored"]}' for r in results.values()]
    return json.dumps({'pass': all(verdicts), 'score_mean': float(np.mean(scores)),
                       'score_max': float(np.max(scores)), 'passed': passed})


def load_reference(cats_dir):
    net = M.load_cats(cats_dir)
    return M.measure(net)


def cmd_reference(args):
    ref = load_reference(args.cats)
    out = {'schema': 'tellegen.cats-reference/1',
           'source': 'California Test System (CATS), Taylor et al., IEEE TEMPR 2(1):107-118, 2024, '
                     'doi:10.1109/TEMPR.2023.3338568; data BSD-3-Clause, '
                     'https://github.com/WISPO-POP/CATS-CaliforniaTestSystem (MATPOWER case of 2024-02-28)',
           'metrics': public(ref)}
    Path(args.out).write_text(json.dumps(out, indent=1, default=float) + '\n')
    print(f'wrote {args.out}')


def generate(tellegen, request, out, osm=None, tuning=None):
    cmd = [tellegen, 'generate', '--request', str(request), '--out', str(out)]
    if osm:
        cmd += ['--osm', str(osm)]
    if tuning:
        cmd += ['--tuning', str(tuning)]
    res = subprocess.run(cmd, capture_output=True, text=True, check=False)
    if res.returncode != 0:
        raise SystemExit(f'generate failed for {out}: {res.stderr.strip()[:2000]}')
    return json.loads(res.stdout)


def cmd_run(args):
    spec = json.loads(Path(args.spec).read_text())
    work = Path(args.work)
    work.mkdir(parents=True, exist_ok=True)
    seeds = [int(s) for s in args.seeds.split(',')] if args.seeds else spec['seeds']
    if args.mode == 'osm':
        seeds = seeds[:1] if args.seeds else [2026]
    ref = load_reference(args.cats)
    results = {}
    for seed in seeds:
        req = dict(spec['request'], seed=seed)
        if args.mode == 'osm':
            req['source'] = 'osm'
        if args.request_patch:
            req.update(json.loads(args.request_patch))
        rp = work / f'request-{seed}.json'
        rp.write_text(json.dumps(req))
        out = work / f'seed-{seed}'
        if out.exists():
            import shutil
            shutil.rmtree(out)
        generate(args.tellegen, rp, out, osm=args.osm, tuning=args.tuning)
        results[f'seed {seed}'] = score_one(ref, M.load_generated(out), args.mode)
    write_scorecard(work, args.mode, spec['id'], results, public(ref))
    print(summary_line(results))


def cmd_score(args):
    ref = load_reference(args.cats)
    results = {Path(d).name: score_one(ref, M.load_generated(d), args.mode) for d in args.dirs}
    work = Path(args.work) if args.work else Path(args.dirs[0]).parent
    write_scorecard(work, args.mode, args.label, results, public(ref))
    print(summary_line(results))


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest='cmd', required=True)
    r = sub.add_parser('reference')
    r.add_argument('--cats', required=True)
    r.add_argument('--out', default=str(HERE / 'cats_reference.json'))
    r.set_defaults(func=cmd_reference)
    g = sub.add_parser('run')
    g.add_argument('--cats', required=True)
    g.add_argument('--spec', default=str(HERE / 'california.json'))
    g.add_argument('--tellegen', required=True)
    g.add_argument('--mode', choices=('synthetic', 'osm'), default='synthetic')
    g.add_argument('--osm', help='Overpass JSON document for --mode osm')
    g.add_argument('--tuning', help='tuning overrides passed to tellegen generate --tuning')
    g.add_argument('--seeds', help='comma-separated seeds (default: the spec seeds)')
    g.add_argument('--request-patch', help='JSON object merged into the request')
    g.add_argument('--work', required=True)
    g.set_defaults(func=cmd_run)
    s = sub.add_parser('score')
    s.add_argument('--cats', required=True)
    s.add_argument('--mode', choices=('synthetic', 'osm'), default='synthetic')
    s.add_argument('--label', default='california')
    s.add_argument('--work')
    s.add_argument('dirs', nargs='+')
    s.set_defaults(func=cmd_score)
    args = ap.parse_args()
    args.func(args)


if __name__ == '__main__':
    main()
