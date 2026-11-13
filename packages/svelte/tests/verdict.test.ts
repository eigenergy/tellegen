import { describe, expect, it } from 'vitest';
import {
	duration,
	iterationCount,
	outcomeVerdict,
	savedVerdict,
	solveVerdict,
	statusWords
} from '../src/lib/diagram/verdict.js';

describe('statusWords', () => {
	it('maps raw solver statuses to plain words', () => {
		expect(statusWords('optimal')).toBe('Optimal');
		expect(statusWords('not_converged')).toBe('Did not converge');
		expect(statusWords('Restoration_Failed')).toBe('Stalled during restoration');
		expect(statusWords('Solved_To_Acceptable_Level')).toBe('Acceptable (looser tolerance)');
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
	it('says an AC power flow converged, with its iterations and time', () => {
		expect(
			solveVerdict({
				formulation: 'acpf',
				solving: false,
				failed: false,
				status: 'feasible',
				iterations: 4,
				ms: 38
			})
		).toEqual({ text: 'AC power flow converged in 4 iterations (38 ms).', tone: 'ok' });
	});

	it('says a DC OPF found the least-cost dispatch', () => {
		expect(
			solveVerdict({
				formulation: 'dcopf',
				solving: false,
				failed: false,
				status: 'optimal',
				iterations: 9,
				ms: 1436
			}).text
		).toBe('DC OPF found the least-cost dispatch in 9 iterations (1.4 s).');
	});

	it('says a relaxation gives a bound', () => {
		expect(
			solveVerdict({ formulation: 'socwr', solving: false, failed: false, iterations: 1 }).text
		).toBe('SOCWR found a lower bound on the least-cost dispatch in 1 iteration.');
	});

	it('flags a result whose solver status is not clean', () => {
		const v = solveVerdict({
			formulation: 'acopf',
			solving: false,
			failed: false,
			status: 'Solved_To_Acceptable_Level',
			ms: 900
		});
		expect(v.tone).toBe('warning');
		expect(v.text).toBe(
			'AC OPF found a locally optimal dispatch (900 ms). Solver status: acceptable (looser tolerance).'
		);
	});

	it('reports a running solve', () => {
		expect(solveVerdict({ formulation: 'acpf', solving: true, failed: false })).toEqual({
			text: 'Solving AC power flow…',
			tone: 'pending'
		});
	});

	it('says what failed and what to try', () => {
		expect(
			solveVerdict({ formulation: 'acpf', solving: false, failed: true, iterations: 20 })
		).toEqual({
			text: 'AC power flow did not converge after 20 iterations. Try DC OPF for a starting point, or relax the tolerance in Settings.',
			tone: 'failure'
		});
		expect(
			solveVerdict({
				formulation: 'dcopf',
				solving: false,
				failed: true,
				failure: 'DC OPF solve infeasible'
			}).text
		).toBe(
			'DC OPF found no feasible dispatch: demand cannot be served within the line and generator limits. Reduce demand or raise line ratings, then solve again.'
		);
	});
});

describe('savedVerdict', () => {
	it('reads a saved result in plain words', () => {
		expect(savedVerdict({ formulation: 'dcopf', status: 'optimal', label: 'Peak' })).toEqual({
			text: 'Saved DC OPF result: optimal.',
			tone: 'ok'
		});
		expect(savedVerdict({ formulation: 'dcopf', status: null, label: 'Peak' }).tone).toBe(
			'pending'
		);
	});
});

describe('outcomeVerdict', () => {
	it('leads with the objective and the lines at their limit', () => {
		expect(
			outcomeVerdict({ formulation: 'dcopf', objective: 1234.5, binding: 2, flagged: true })
		).toEqual({
			text: 'The least-cost dispatch has an objective of 1,234.5. 2 lines are at their limit.',
			tone: 'warning'
		});
		expect(
			outcomeVerdict({ formulation: 'dcopf', objective: 10, binding: 1, flagged: false })?.text
		).toBe(
			'The least-cost dispatch has an objective of 10. 1 line is loaded to its rating or above.'
		);
		expect(
			outcomeVerdict({ formulation: 'dcopf', objective: 10, binding: 0, flagged: true })?.tone
		).toBe('ok');
	});

	it('states the change from the base case', () => {
		expect(
			outcomeVerdict({
				formulation: 'dcopf',
				objective: 1300,
				binding: null,
				flagged: true,
				deltaObjective: -65.5
			})?.text
		).toBe(
			'The least-cost dispatch has an objective of 1,300. That is 65.5 less than the base case.'
		);
	});

	it('has nothing to say for a power flow or a missing objective', () => {
		expect(outcomeVerdict({ formulation: 'acpf', objective: 0, binding: 0, flagged: true })).toBe(
			null
		);
		expect(
			outcomeVerdict({ formulation: 'dcopf', objective: null, binding: 0, flagged: true })
		).toBe(null);
	});
});
