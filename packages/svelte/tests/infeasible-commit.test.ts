import { expect, it, vi } from 'vitest';
import type { BrowserStudy, EngineError } from '@tellegen/engine';
import { Controller } from '../src/lib/controller.svelte.js';
import { AppState, LocalCase } from '../src/lib/state.svelte.js';

function failingCommit(error: Error) {
	const app = new AppState();
	const ctrl = new Controller(app, { apiBase: null, loadDefaultCases: false });
	const c = new LocalCase({
		id: 'case',
		label: 'Case',
		fileName: 'case.pio.json',
		studyInputJson: 'input',
		coordsKind: 'file'
	});
	c.formulation = 'socwr';
	const study = { commit: vi.fn().mockRejectedValue(error) } as unknown as BrowserStudy;
	vi.spyOn(ctrl, 'getStudy').mockResolvedValue(study);
	const dispose = vi.spyOn(ctrl, 'disposeStudy').mockImplementation(() => {});
	return { app, ctrl, c, dispose };
}

function coded(message: string, code: EngineError['code']): EngineError {
	const error: EngineError = new Error(message);
	error.code = code;
	return error;
}

it('keeps the Study when the engine codes the failed commit as infeasible', async () => {
	const { app, ctrl, c, dispose } = failingCommit(coded('solver stopped', 'infeasible'));
	ctrl.runSolve(c, null);
	await vi.waitFor(() => expect(c.solving).toBe(false));
	expect(dispose).not.toHaveBeenCalled();
	expect(app.error).toContain('has no solution at this demand');
	ctrl.dispose();
});

it('drops the Study on another code even when the text mentions infeasible', async () => {
	const { app, ctrl, c, dispose } = failingCommit(
		coded('stopped before deciding feasible or infeasible', 'not_converged')
	);
	ctrl.runSolve(c, null);
	await vi.waitFor(() => expect(c.solving).toBe(false));
	expect(dispose).toHaveBeenCalledWith(c);
	expect(app.error).toContain('browser study commit failed');
	ctrl.dispose();
});

it('reads the text of a failed commit that carries no code', async () => {
	const { app, ctrl, c, dispose } = failingCommit(new Error('SOCWR solve infeasible'));
	ctrl.runSolve(c, null);
	await vi.waitFor(() => expect(c.solving).toBe(false));
	expect(dispose).not.toHaveBeenCalled();
	expect(app.error).toContain('has no solution at this demand');
	ctrl.dispose();
});
