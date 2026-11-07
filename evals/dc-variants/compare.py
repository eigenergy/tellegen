#!/usr/bin/env python3
"""Audit matched DC equations and report AC errors without ranking guarantees."""
import argparse
import cmath
import hashlib
import json
import math
import re
from pathlib import Path


def maxerr(items):
    return max(items, default=0.0)


def ac_converged(result):
    return result.get('termination_status') in ['LOCALLY_SOLVED', 'OPTIMAL']


def ac_metrics(result, data):
    if not ac_converged(result):
        return {'status': result.get('termination_status'), 'converged': False,
                'error': result.get('error'), 'note': 'Returned iterates are not AC solutions; accuracy and violation metrics omitted.'}
    if 'solution' not in result:
        return {'error': result.get('error'), 'status': result.get('termination_status')}
    s = result['solution']
    base = data['baseMVA']
    violations = {'voltage': [], 'reactive_generation': [], 'apparent_power': []}
    for i, bus in s['bus'].items():
        lo, hi = data['bus'][i]['vmin'], data['bus'][i]['vmax']
        if bus['vm'] < lo-1e-6 or bus['vm'] > hi+1e-6:
            violations['voltage'].append(i)
    for i, gen in s['gen'].items():
        row = data['gen'][i]
        if gen['qg'] < row['qmin']-1e-6 or gen['qg'] > row['qmax']+1e-6:
            violations['reactive_generation'].append(i)
    loss = 0
    for i, flow in s.get('branch', {}).items():
        loss += (flow['pf']+flow['pt'])*base
        rate = data['branch'][i].get('rate_a', 0)
        if rate and max(math.hypot(flow['pf'],flow['qf']), math.hypot(flow['pt'],flow['qt'])) > rate+1e-6:
            violations['apparent_power'].append(i)
    return {'status':result['termination_status'], 'converged': True, 'loss_mw':loss,
            'violation_counts':{k:len(v) for k,v in violations.items()}, 'violation_examples':{k:v[:20] for k,v in violations.items()}, 'objective':result.get('objective')}


def audit(t, r, case):
    data, report, golden = r['data'], {}, {}
    base = data['baseMVA']
    rows = re.search(r"mpc\.branch\s*=\s*\[(.*?)\];", case, re.S).group(1)
    endpoints = [tuple(map(int, line.split('%')[0].strip().rstrip(';').split()[:2])) for line in rows.splitlines() if line.split('%')[0].strip()]
    directions = {}
    for i, branch in data['branch'].items():
        source = endpoints[int(i)-1]
        target = (branch['f_bus'], branch['t_bus'])
        assert target in [source, source[::-1]], (i,source,target)
        directions[i] = 1 if target == source else -1
        assert directions[i] == 1 or branch['tap'] == 1, 'Nonunity reversed transformer needs a source-coordinate equation audit'
    report['reference_reversed_branch_count'] = sum(v == -1 for v in directions.values())
    for name, solves in t.items():
        report[name], golden[name] = {}, {}
        for problem in ['dcpf', 'dcopf']:
            actual, ref = solves[problem], r[name][problem]
            if 'error' in actual or 'solution' not in ref:
                report[name][problem] = {'actual_error':actual.get('error'),'reference_error':ref.get('error'), 'status':ref.get('termination_status')}
                continue
            rs = ref['solution']
            va = {str(x['bus']):x['value'] for x in actual['va']}
            flows = {str(x['branch']):directions[str(x['branch'])]*x['pf']/base for x in actual['flows']}
            pg = {str(x['gen']):x['pg']/base for x in actual.get('dispatch',[]) or []}
            equation_errors, balance = [], {i:0.0 for i in va}
            for i, flow in flows.items():
                row = data['branch'][i]
                x, resistance, tap = row['br_x'], row['br_r'], row['tap']
                k = 1/x if name in ['reactance_only','tap_adjusted_reactance'] else x/(resistance**2+x*x)
                if name in ['tap_adjusted_reactance','tap_adjusted_series_susceptance']: k /= tap
                shift = 0 if name in ['reactance_only','series_susceptance_no_phase_shift'] else row['shift']
                f, to = str(row['f_bus']),str(row['t_bus'])
                equation_errors.append(abs(flow-k*(va[f]-va[to]-shift)))
                balance[f] += flow; balance[to] -= flow
            for i, row in data['gen'].items():
                if row['gen_status']:
                    balance[str(row['gen_bus'])] -= pg.get(i, rs['gen'][i]['pg'])
            for row in data['load'].values():
                if row['status']: balance[str(row['load_bus'])] += row['pd']
            for row in data['shunt'].values():
                if row['status']: balance[str(row['shunt_bus'])] += row['gs']
            errors = {'actual_status':actual['status'], 'reference_status':ref['termination_status'], 'equation_pu':maxerr(equation_errors),'balance_pu':maxerr(map(abs,balance.values())),
                'flow_pu':maxerr(abs(v-rs['branch'][i]['pf']) for i,v in flows.items()),
                'angle_rad':maxerr(abs(v-rs['bus'][i]['va']) for i,v in va.items())}
            if problem == 'dcopf':
                errors['dispatch_pu'] = maxerr(abs(v-rs['gen'][i]['pg']) for i,v in pg.items())
                errors['objective_relative'] = abs(actual['objective']-ref['objective'])/max(1,abs(ref['objective']))
                errors['lmp_per_mw'] = maxerr(abs(v['value']+rs['bus'][str(v['bus'])]['lam_kcl_r']/base) for v in actual['lmp'])
            # Unique dispatch/dual comparison is gated only for the synthetic,
            # strictly convex, nondegenerate fixture. Large-case metrics report.
            report[name][problem] = errors
            golden[name][problem] = {'va':{i:v['va'] for i,v in rs['bus'].items()}, 'pf':{i:v['pf']*base for i,v in rs['branch'].items()}, 'objective':ref['objective'], 'pg':{i:v['pg']*base for i,v in rs['gen'].items()}, 'lmp':{i:-v['lam_kcl_r']/base for i,v in rs['bus'].items() if v.get('lam_kcl_r') is not None}}
        ac = r['ac_pf'].get('solution',{}).get('branch',{})
        if ac_converged(r['ac_pf']) and ac and 'flows' in solves['dcpf']:
            errors = []
            for f in solves['dcpf']['flows']:
                i = str(f['branch'])
                dc = f['pf']*directions[i]
                row, b = ac[i], data['branch'][i]
                sym = (row['pf']-row['pt'])*base/2
                error = abs(dc-sym)
                rate = b.get('rate_a',0)*base
                errors.append({'branch':i, 'dc_from_mw':dc, 'ac_from_mw':row['pf']*base, 'ac_to_mw':row['pt']*base, 'symmetric_ac_mw':sym, 'from_error_mw':dc-row['pf']*base, 'to_error_mw':-dc-row['pt']*base, 'symmetric_error_mw':error, 'rating_fraction':error/rate if rate else None, 'dc_overload':bool(rate and abs(dc)>rate), 'ac_overload':bool(rate and max(math.hypot(row['pf'],row['qf']),math.hypot(row['pt'],row['qt']))*base>rate)})
            report[name]['ac_fixed_dispatch'] = {'max_symmetric_error_mw':maxerr(e['symmetric_error_mw'] for e in errors), 'worst_branches':sorted(errors,key=lambda e:e['symmetric_error_mw'],reverse=True)[:10]}
        if not ac_converged(r['ac_pf']):
            report[name]['ac_fixed_dispatch'] = {'status': r['ac_pf'].get('termination_status'), 'converged': False}
        if name in r['ac_replay']: report[name]['ac_dispatch_replay'] = ac_metrics(r['ac_replay'][name],data)
    # Full complex pi-model finite difference at unit V and delta = shift.
    slopes = []
    for b in data['branch'].values():
        if b['br_status'] != 1: continue
        y = 1/complex(b['br_r'],b['br_x']); tap = b['tap']*cmath.exp(1j*b['shift'])
        def p(delta):
            vf = cmath.exp(1j*delta)
            current = (y+complex(b['g_fr'],b['b_fr']))/abs(tap)**2*vf-y/tap.conjugate()
            return (vf*current.conjugate()).real
        eps = 1e-6
        derivative = (p(b['shift']+eps)-p(b['shift']-eps))/(2*eps)
        slopes.append(abs(derivative+y.imag/b['tap']))
    report['ac_flat_slope_max_error'] = maxerr(slopes)
    report['ac_pf'] = ac_metrics(r['ac_pf'],data)
    if 'ac_opf_local' in r: report['ac_opf_local'] = ac_metrics(r['ac_opf_local'],data)
    return report,golden


if __name__ == '__main__':
    p=argparse.ArgumentParser();p.add_argument('case',type=Path);p.add_argument('tellegen',type=Path);p.add_argument('reference',type=Path);p.add_argument('output',type=Path);p.add_argument('--golden',type=Path);p.add_argument('--check',action='store_true');a=p.parse_args()
    t,r=json.loads(a.tellegen.read_text()),json.loads(a.reference.read_text())
    report,golden=audit(t,r,a.case.read_text())
    report['case_sha256']=hashlib.sha256(a.case.read_bytes()).hexdigest();report['versions']=r['versions']
    a.output.write_text(json.dumps(report,indent=2)+'\n')
    if a.golden: a.golden.write_text(json.dumps(golden,indent=2)+'\n')
    if a.check:
        for name in t:
            for problem in ['dcpf','dcopf']:
                e=report[name][problem]
                assert e.get('reference_status') in ['LOCALLY_SOLVED','OPTIMAL'] and e.get('actual_status') in ['optimal','feasible'], (name,e)
                assert 'actual_error' not in e and e['equation_pu']<1e-8 and e['balance_pu']<1e-8,(name,e)
                assert e['flow_pu']<(1e-8 if problem=='dcpf' else 1e-5),(name,e)
                assert e['angle_rad']<1e-6,(name,e)
                if problem=='dcopf': assert e['objective_relative']<1e-6 and e['dispatch_pu']<1e-5 and e['lmp_per_mw']<1e-4,(name,e)
        assert report['ac_flat_slope_max_error']<1e-7
