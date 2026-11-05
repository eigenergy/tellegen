import type { Solution } from '@tellegen/engine';
import { describe, expect, it } from 'vitest';
import { economicsFor, pricedBranch, thermalStatus } from '../src/lib/diagram/transmission-economics.js';

const solution: Solution = {
	formulation: 'dcopf',
	objective: 20,
	prices: [
		{ bus: 1, value: -10 },
		{ bus: 2, value: -10.000001 }
	],
	va: [],
	w: [],
	flows: [],
	dispatch: [],
	branch_constraints: [
		{ branch: 1, enforced: true, binding: false, relief_per_mw: 0.00000001 },
		{ branch: 2, enforced: false, binding: false },
		{ branch: 3, enforced: true, binding: true, relief_per_mw: 10 },
		{ branch: 4, enforced: true, binding: true, relief_per_mw: 30 },
		{ branch: 5, enforced: true, binding: true }
	],
	constraints: [
		{ id: 'lower', binding: true, value: -1, lower: -1, shadow_price: -3 },
		{ id: 'upper', binding: true, value: 1, upper: 1, shadow_price: 2 }
	]
};

describe('OPF economics', () => {
	it('reports a uniform negative price and ranks binding constraints by marginal impact', () => {
		const result = economicsFor(solution, 'dcopf');
		expect(result.uniform).toBeCloseTo(-10);
		expect(result.binding.map((r) => r.branch)).toEqual([4, 3, 5]);
		expect(result.constraints.map((r) => r.id)).toEqual(['lower', 'upper']);
		expect(pricedBranch(result.branches.get(1))).toBe(false);
		expect(pricedBranch(result.branches.get(4))).toBe(true);
		expect(thermalStatus(result.branches.get(2))).toBe('Not enforced');
		expect(thermalStatus(result.branches.get(5))).toContain('unavailable');
	});

	it('keeps legacy and other formulation constraint economics unavailable', () => {
		const legacy = economicsFor({ ...solution, branch_constraints: undefined }, 'dcopf');
		expect(legacy.thermalAvailable).toBe(false);
		expect(legacy.prices).toHaveLength(2);
		expect(economicsFor(solution, 'acopf').prices).toHaveLength(0);
		expect(
			economicsFor({ ...solution, formulation: 'socwr', branch_constraints: undefined }, 'socwr')
				.prices
		).toHaveLength(2);
	});
});
