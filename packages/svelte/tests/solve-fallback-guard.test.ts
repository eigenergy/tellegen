import { describe, expect, it, vi } from 'vitest';
import { AppState, CaseState } from '../src/lib/state.svelte.js';
import { Controller } from '../src/lib/controller.svelte.js';
import type { TellegenApiClient } from '../src/lib/api.js';

function host() {
	const app = new AppState();
	const api = {
		getCaseModuleJson: vi.fn(async () => {
			throw new Error('HTTP 503');
		})
	};
	const ctrl = new Controller(app, { api: api as unknown as TellegenApiClient });
	const c = new CaseState({ id: 'backend', name: 'Backend', n_bus: 3, n_branch: 3, n_gen: 2 });
	app.cases = [c];
	const server = vi.spyOn(ctrl, 'serverSolve').mockImplementation(() => {});
	return { app, ctrl, c, server };
}

describe('missing browser solve input', () => {
	it.each(['acpf', 'socwr', 'acopf'] as const)(
		'never falls back to a DC server solve for %s',
		async (formulation) => {
			const { app, ctrl, c, server } = host();
			c.formulation = formulation;
			ctrl.runSolve(c, null);
			await vi.waitFor(() => expect(c.solving).toBe(false));
			expect(server).not.toHaveBeenCalled();
			expect(c.solution).toBeNull();
			expect(app.error).toContain('requires its PowerIO module');
			expect(app.error).toContain('HTTP 503');
		}
	);
	it('retains the server fallback for DC OPF', async () => {
		const { ctrl, c, server } = host();
		ctrl.runSolve(c, null);
		await vi.waitFor(() => expect(server).toHaveBeenCalledOnce());
	});
});
