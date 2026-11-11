"""Comparator regression tests; run with python3 -m unittest discover here."""
import cmath
import math
import unittest

import compare

NAMES = ['series_susceptance', 'series_susceptance_no_phase_shift',
         'tap_adjusted_reactance', 'tap_adjusted_series_susceptance', 'reactance_only']


class ComparatorTests(unittest.TestCase):
    def test_reversed_nonunity_branch_only_excludes_tap_ignoring_models(self):
        branch = dict(f_bus=2, t_bus=1, br_r=.03*1.2**2, br_x=.1*1.2**2,
                      tap=1/1.2, shift=-.1, br_status=1, g_fr=0, b_fr=0)
        data = dict(baseMVA=100, branch={'1': branch}, gen={}, load={}, shunt={})
        reference = dict(data=data, ac_pf={'termination_status': 'LOCALLY_INFEASIBLE'}, ac_replay={})
        tellegen = {}
        for name in NAMES:
            flow = compare.dc_coefficient(name, branch)*(-.05-compare.dc_shift(name, branch))
            actual = dict(status='optimal', va=[{'bus': 1, 'value': .05}, {'bus': 2, 'value': 0}],
                          flows=[{'branch': 1, 'pf': -100*flow}], dispatch=[], lmp=[], objective=12.5)
            result = dict(termination_status='LOCALLY_SOLVED', objective=12.5,
                          solution={'bus': {'1': {'va': .05}, '2': {'va': 0}},
                                    'branch': {'1': {'pf': flow}}, 'gen': {}})
            tellegen[name] = {p: actual.copy() for p in ['dcpf', 'dcopf']}
            reference[name] = {p: result for p in ['dcpf', 'dcopf']}
        report, golden = compare.audit(tellegen, reference, 'mpc.branch = [\n1 2;\n];')
        self.assertEqual(report['reference_reversed_nonunity_branches'], ['1'])
        for name in NAMES:
            for problem in ['dcpf', 'dcopf']:
                row = report[name][problem]
                if name in compare.TAP_ADJUSTED:
                    self.assertLess(row['equation_pu'], 1e-12)
                    self.assertLess(row['flow_pu'], 1e-12)
                    self.assertEqual(golden[name][problem]['reference_directions'], {'1': -1})
                    if problem == 'dcopf':
                        self.assertEqual(row['actual_objective'], 12.5)
                        self.assertEqual(row['reference_objective'], 12.5)
                        self.assertEqual(row['objective_absolute_gap'], 0)
                else:
                    self.assertEqual(row['comparison_status'], 'incompatible_preprocessing')
                    self.assertNotIn('flow_pu', row)
        # A missing flow-sign correction must produce a visible mismatch.
        tellegen['tap_adjusted_reactance']['dcpf']['flows'] = [{'branch': 1, 'pf': 10}]
        report, _ = compare.audit(tellegen, reference, 'mpc.branch = [\n1 2;\n];')
        self.assertGreater(report['tap_adjusted_reactance']['dcpf']['flow_pu'], .1)

    def test_ac_operating_point_derivatives_against_complex_pi_model(self):
        for delta in [-.2, 0, .15]:
            for vf, vt in [(1, 1), (.93, 1.07)]:
                r, x, tau, shift, base = .03, .1, 1.2, .12, 100
                y, tap = 1/complex(r, x), tau*cmath.exp(1j*shift)
                def powers(angle):
                    vfrom, vto = vf*cmath.exp(1j*angle), complex(vt)
                    pf = (vfrom*((y/tau**2)*vfrom-y/tap.conjugate()*vto).conjugate()).real
                    pt = (vto*(y*vto-y/tap*vfrom).conjugate()).real
                    return pf, pt
                angle, eps = delta+shift, 1e-6
                pf, pt = powers(angle)
                solution = {'bus': {'1': {'vm': vf, 'va': angle}, '2': {'vm': vt, 'va': 0}},
                            'branch': {'1': {'pf': pf, 'pt': pt}}}
                row = dict(f_bus=1, t_bus=2, br_r=r, br_x=x, tap=tau, shift=shift, br_status=1)
                result = compare.ac_operating_point_diagnostics(
                    {'termination_status': 'LOCALLY_SOLVED', 'solution': solution},
                    {'baseMVA': base, 'branch': {'1': row}}, NAMES)['transformers'][0]
                plus, minus = powers(angle+eps), powers(angle-eps)
                self.assertAlmostEqual(result['from_angle_derivative_mw_per_rad'], base*(plus[0]-minus[0])/(2*eps), delta=1e-6)
                self.assertAlmostEqual(result['symmetric_angle_derivative_mw_per_rad'], base*((plus[0]-plus[1])-(minus[0]-minus[1]))/(4*eps), delta=1e-6)
                self.assertAlmostEqual(result['rho'], vf*vt)
                self.assertAlmostEqual(result['symmetric_ac_mw'], base*(pf-pt)/2)

    def test_failed_ac_iterates_have_no_accuracy_metrics(self):
        result = compare.ac_operating_point_diagnostics({'termination_status': 'LOCALLY_INFEASIBLE', 'solution': {}}, {}, NAMES)
        self.assertFalse(result['converged'])
        self.assertNotIn('transformers', result)


if __name__ == '__main__':
    unittest.main()
