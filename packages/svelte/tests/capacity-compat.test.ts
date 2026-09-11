import { describe, expect, it } from 'vitest';
import type { CapacityPlanSpecJson, StudyBundle } from '@tellegen/engine';
import {
	capacityGoal,
	capacityOutcome
} from '../../../apps/web/src/lib/studies/capacity-compat.js';

const spec: CapacityPlanSpecJson = {
	objective: { kind: 'weighted_lmp', weights: [{ bus: 1, weight: 1 }] },
	candidates: ['branches:7724'],
	max_increase_per_branch_mw: 10,
	budget_mw: 10,
	increment_mw: 5,
	max_changed_lines: 1,
	exact_solve_budget: 3
};

describe('capacity tool adaptation', () => {
	it('uses the verified source line ID without assuming the generated label is a carried UID', () => {
		const goal = capacityGoal(spec, { 'branches:7724': 7725 });
		expect(goal.decisions?.variables).toEqual([
			expect.objectContaining({ id: 'branches:7724', element: 7725 })
		]);
		expect(() => capacityGoal(spec, {})).toThrow('no matching source line');
	});

	it.each([0, 1])('counts %i setup solves and the actual trial solves', (setupSolves) => {
		const bundle = {
			document: {
				recommended_state: 'result',
				states: { base: { view: 'base-view' }, result: { view: 'result-view' } },
				experiments: {
					save: { solve_count: setupSolves },
					plan: {
						start_state: 'base',
						goal: 'goal',
						evidence: [],
						solve_count: 1,
						trials: [
							{
								evidence: ['trial'],
								predicted_value: 9,
								exact_value: 9,
								changes: [5],
								accepted: true
							}
						]
					}
				}
			},
			artifacts: {
				'base-view': { text: JSON.stringify({ objective: 50, lmp: [{ bus: 1, value: 10 }] }) },
				'result-view': { text: JSON.stringify({ objective: 48, lmp: [{ bus: 1, value: 9 }] }) },
				trial: { text: JSON.stringify({ iteration: 0 }) }
			}
		} as unknown as StudyBundle;
		const result = capacityOutcome(bundle, 'plan', spec);
		expect(result.exact_solves).toBe(setupSolves + 1);
		expect(result.baseline.exact_solve).toBe(setupSolves);
		expect(result.exact_verified_result.exact_solve).toBe(setupSolves + 1);
		expect(result.proposal).toEqual([{ branch: 'branches:7724', delta_mw: 5 }]);
	});
});
