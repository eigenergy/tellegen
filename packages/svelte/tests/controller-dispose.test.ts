import { describe, expect, it, vi } from 'vitest';
import type { BrowserMcPfSession, BrowserStudy, IngestedCase } from '@tellegen/engine';

const engine = vi.hoisted(() => ({
	createStudy: vi.fn()
}));
vi.mock('@tellegen/engine', async (importOriginal) => {
	const actual = await importOriginal<typeof import('@tellegen/engine')>();
	return { ...actual, createStudy: engine.createStudy };
});

import { AppState, MulticonductorCase } from '../src/lib/state.svelte.js';
import { Controller } from '../src/lib/controller.svelte.js';

const payload: IngestedCase = {
	name: 'case',
	base_mva: 100,
	n_bus: 2,
	n_branch: 1,
	n_gen: 1,
	load_mw: 50,
	gen_mw: 50,
	has_coords: false,
	coords_kind: 'synthetic_pending',
	diagnostics: [],
	warnings: [],
	module_json: '{"module":"stub"}',
	topology: {
		buses: [
			{ id: 1, uid: null, demand_mw: 0, gen_mw: 50 },
			{ id: 2, uid: null, demand_mw: 50, gen_mw: 0 }
		],
		branches: [{ id: 0, uid: null, from: 1, to: 2, rate_mw: 100 }]
	},
	view: null
};

function fakeStudy() {
	return {
		currentSolution: vi.fn(async () => ({})),
		free: vi.fn()
	} as unknown as BrowserStudy & { free: ReturnType<typeof vi.fn> };
}

describe('Controller.dispose', () => {
	it('frees every Study and session, including ones still being built', async () => {
		const app = new AppState();
		const ctrl = new Controller(app, { apiBase: null, loadDefaultCases: false });
		vi.spyOn(app, 'requestFrame').mockResolvedValue();

		// A local case with a built Study.
		const built = await ctrl.openBalancedModule({ name: 'built', payload, activate: false });
		app.addLocal(built);
		const study = fakeStudy();
		engine.createStudy.mockResolvedValueOnce(study);
		expect(await ctrl.getStudy(built, 'input')).toBe(study);

		// A second case whose Study is still being built when the provider goes.
		const pending = await ctrl.openBalancedModule({ name: 'pending', payload, activate: false });
		app.addLocal(pending);
		const late = fakeStudy();
		let finish!: (study: BrowserStudy) => void;
		engine.createStudy.mockReturnValueOnce(new Promise((resolve) => (finish = resolve)));
		const building = ctrl.getStudy(pending, 'input');

		// A multiconductor case holding a retained session.
		const session = { free: vi.fn() } as unknown as BrowserMcPfSession;
		const mc = new MulticonductorCase({
			id: 'mc',
			label: 'Feeder',
			fileName: 'feeder.json',
			summary: { name: 'feeder' } as never,
			graph: { buses: [], edges: [] },
			coordsKind: 'planar',
			view: null
		});
		mc.mcSession = session;
		app.addMulti(mc);

		ctrl.dispose();
		expect(study.free).toHaveBeenCalledTimes(1);
		expect(session.free).toHaveBeenCalledTimes(1);
		expect(mc.mcSession).toBeNull();

		finish(late);
		expect(await building).toBeNull();
		expect(late.free).toHaveBeenCalledTimes(1);

		// Nothing is built after disposal, and a second dispose is a no-op.
		const calls = engine.createStudy.mock.calls.length;
		expect(await ctrl.getStudy(built, 'input')).toBeNull();
		expect(engine.createStudy).toHaveBeenCalledTimes(calls);
		await expect(ctrl.solveMultiCase(mc)).rejects.toMatchObject({ name: 'AbortError' });
		ctrl.dispose();
		expect(study.free).toHaveBeenCalledTimes(1);
	});
});
