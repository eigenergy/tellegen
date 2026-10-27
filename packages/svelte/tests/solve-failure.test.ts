import { describe, expect, it } from 'vitest';
import { INFEASIBLE_DISPATCH, browserSolveFailure } from '../src/lib/format.js';
import { AppState } from '../src/lib/state.svelte.js';

describe('browser solve failure text', () => {
	it('says an infeasible DC OPF in plain words and keeps the solver text as detail', () => {
		const reason = 'browser study unavailable: DC OPF solve infeasible: PrimalInfeasible';
		expect(browserSolveFailure('synthetic grid', reason)).toEqual({
			message: INFEASIBLE_DISPATCH,
			detail: `synthetic grid: browser solve unavailable: ${reason}`
		});
		expect(INFEASIBLE_DISPATCH).toBe(
			'No feasible dispatch: demand cannot be served within line and generator limits'
		);
	});

	it('also catches a demand edit that makes the dispatch infeasible', () => {
		const reason = 'browser study commit failed: DC OPF solve infeasible: PrimalInfeasible';
		expect(browserSolveFailure('case', reason).message).toBe(INFEASIBLE_DISPATCH);
	});

	it('leaves other failures as they were, an unbounded solve included', () => {
		for (const reason of [
			'browser study unavailable: DC OPF solve unbounded: DualInfeasible',
			'browser study unavailable: DC OPF solve did not converge: MaxIterations',
			'browser study wasm is not supported by this browser'
		]) {
			expect(browserSolveFailure('case', reason)).toEqual({
				message: `case: browser solve unavailable: ${reason}`,
				detail: null
			});
		}
		expect(browserSolveFailure('case', null)).toEqual({
			message: 'case: browser solve unavailable',
			detail: null
		});
	});

	it('drops the detail with the next error, like the retry', () => {
		const app = new AppState();
		app.error = INFEASIBLE_DISPATCH;
		app.errorDetail = 'case: browser solve unavailable: DC OPF solve infeasible';
		app.error = 'Another error';
		expect(app.errorDetail).toBeNull();
	});
});
