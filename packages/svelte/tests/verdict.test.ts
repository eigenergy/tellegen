import { describe, expect, it } from 'vitest';
import {
	costPerHour,
	duration,
	iterationCount,
	resultStats,
	savedVerdict,
	solveVerdict,
	statusWords
} from '../src/lib/diagram/verdict.js';

describe('statusWords', () => {
	it('maps raw solver statuses to plain words', () => {
		expect(statusWords('optimal')).toBe('Optimal');
		expect(statusWords('not_converged')).toBe('Did not converge');
		expect(statusWords('Restoration_Failed')).toBe('Stalled');
		expect(statusWords('Solved_To_Acceptable_Level')).toBe('Acceptable');
		expect(statusWords('Maximum_Iterations_Exceeded')).toBe('Iteration limit');
		expect(statusWords(null)).toBe('Not solved');
	});

	it('spaces and sentence-cases an unknown status', () => {
		expect(statusWords('USER_REQUESTED_STOP')).toBe('User requested stop');
	});
});

describe('iterationCount', () => {
	it('reads the interior-point trace and the Newton summary', () => {
		const step = { iter: 0, objective: 0, inf_pr: 1, inf_du: 1 };
		expect(iterationCount([step, step, step])).toBe(3);
		expect(iterationCount({ count: 4, residual: 1e-9 })).toBe(4);
		expect(iterationCount([])).toBeNull();
		expect(iterationCount(undefined)).toBeNull();
	});
});

describe('duration', () => {
	it('keeps milliseconds below a second and seconds above', () => {
		expect(duration(38)).toBe('38 ms');
		expect(duration(1436)).toBe('1.4 s');
		expect(duration(12_400)).toBe('12 s');
	});
});

describe('solveVerdict', () => {
	it('says nothing when a solve succeeded cleanly', () => {
		expect(
			solveVerdict({ formulation: 'acpf', solving: false, failed: false, status: 'feasible' })
		).toBeNull();
		expect(
			solveVerdict({ formulation: 'dcopf', solving: false, failed: false, status: 'optimal' })
		).toBeNull();
		expect(solveVerdict({ formulation: 'socwr', solving: false, failed: false })).toBeNull();
	});

	it('flags a result whose solver status is not clean', () => {
		expect(
			solveVerdict({
				formulation: 'acopf',
				solving: false,
				failed: false,
				status: 'Solved_To_Acceptable_Level'
			})
		).toEqual({ text: 'Acceptable', tone: 'warning' });
	});

	it('reports a running solve', () => {
		expect(solveVerdict({ formulation: 'acpf', solving: true, failed: false })).toEqual({
			text: 'Solving…',
			tone: 'pending'
		});
	});

	it('names what failed in a few words', () => {
		expect(solveVerdict({ formulation: 'acpf', solving: false, failed: true })).toEqual({
			text: 'AC power flow did not converge',
			tone: 'failure'
		});
		expect(
			solveVerdict({
				formulation: 'dcopf',
				solving: false,
				failed: true,
				failure: 'DC OPF solve infeasible'
			})?.text
		).toBe('DC OPF infeasible');
		expect(solveVerdict({ formulation: 'socwr', solving: false, failed: true })?.text).toBe(
			'SOCWR failed'
		);
	});
});

describe('savedVerdict', () => {
	it('reads a saved result as a status', () => {
		expect(savedVerdict({ status: 'optimal' })).toEqual({ text: 'Saved · Optimal', tone: 'ok' });
		expect(savedVerdict({ status: null })).toEqual({ text: 'No result', tone: 'pending' });
	});
});

describe('costPerHour', () => {
	it('drops cents from $100 up and uses a true minus sign', () => {
		expect(costPerHour(70_784.4)).toBe('$70,784/h');
		expect(costPerHour(10)).toBe('$10.00/h');
		expect(costPerHour(-65.5)).toBe('−$65.50/h');
	});
});

describe('resultStats', () => {
	it('gives an OPF its operational cost and the lines at their limit', () => {
		expect(resultStats({ formulation: 'dcopf', objective: 1234.5, binding: 2 })).toEqual([
			{ label: 'Operational cost', value: '$1,235/h', sub: undefined },
			{ label: 'Lines at limit', value: '2', tone: 'warning' }
		]);
		expect(resultStats({ formulation: 'dcopf', objective: 10, binding: 0 })[1].tone).toBe('ok');
	});

	it('marks a relaxation cost as a lower bound', () => {
		expect(resultStats({ formulation: 'socwr', objective: 1300, binding: 0 })[0].value).toBe(
			'≥ $1,300/h'
		);
	});

	it('shows the change from the base case under the cost', () => {
		expect(
			resultStats({
				formulation: 'dcopf',
				objective: 1300,
				binding: null,
				deltaObjective: -65.5
			})[0].sub
		).toBe('−$65.50/h');
		expect(
			resultStats({ formulation: 'dcopf', objective: 1300, binding: null, deltaObjective: 120 })[0]
				.sub
		).toBe('+$120/h');
	});

	it('gives AC power flow its voltage range and heaviest line', () => {
		expect(
			resultStats({
				formulation: 'acpf',
				objective: 0,
				binding: 0,
				voltage: { min: 0.95, max: 1.05 },
				maxLoading: 1.02
			})
		).toEqual([
			{ label: 'Voltage range', value: '0.950–1.050 pu' },
			{ label: 'Max loading', value: '102%', tone: 'failure' }
		]);
	});

	it('gives DC power flow its heaviest line and the lines at their limit', () => {
		expect(
			resultStats({ formulation: 'dcpf', objective: null, binding: 1, maxLoading: 0.5 }).map(
				(s) => [s.label, s.value]
			)
		).toEqual([
			['Max loading', '50%'],
			['Lines at limit', '1']
		]);
	});

	it('shows an ellipsis while solving and a dash with no value', () => {
		expect(
			resultStats({ formulation: 'dcopf', solving: true, objective: null, binding: null })[0].value
		).toBe('…');
		expect(resultStats({ formulation: 'dcopf', objective: null, binding: null })[0].value).toBe(
			'—'
		);
	});
});
