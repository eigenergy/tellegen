import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { BrowserStudy, IngestedCase } from '@tellegen/engine';

const engine = vi.hoisted(() => ({
	createStudy: vi.fn()
}));
vi.mock('@tellegen/engine', async (importOriginal) => {
	const actual = await importOriginal<typeof import('@tellegen/engine')>();
	return { ...actual, createStudy: engine.createStudy };
});

import { AppState } from '../src/lib/state.svelte.js';
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

function fakeStudy(formulation = 'dcopf') {
	return {
		formulation,
		currentSolution: vi.fn(async () => ({})),
		free: vi.fn()
	} as unknown as BrowserStudy & {
		free: ReturnType<typeof vi.fn>;
		currentSolution: ReturnType<typeof vi.fn>;
	};
}

function controller() {
	const app = new AppState();
	vi.spyOn(app, 'requestFrame').mockResolvedValue();
	return new Controller(app, { apiBase: null, loadDefaultCases: false });
}

describe('Controller.openBalancedModule with a Study built ahead', () => {
	beforeEach(() => {
		engine.createStudy.mockReset();
	});

	it('uses the given Study for the case instead of building one', async () => {
		const ctrl = controller();
		const study = fakeStudy();
		let finish!: (study: BrowserStudy) => void;
		const local = await ctrl.openBalancedModule({
			name: 'generated',
			payload,
			formulation: 'dcopf',
			study: new Promise((resolve) => (finish = resolve)),
			activate: false
		});
		// A solve that starts while the Study still builds waits for it.
		const first = ctrl.getStudy(local, payload.module_json);
		finish(study);
		expect(await first).toBe(study);
		expect(await ctrl.getStudy(local, payload.module_json)).toBe(study);
		expect(engine.createStudy).not.toHaveBeenCalled();
		expect(study.currentSolution).toHaveBeenCalledTimes(1);
		expect(ctrl.caseStudies.get(local)?.study).toBe(study);

		ctrl.disposeStudy(local);
		expect(study.free).toHaveBeenCalledTimes(1);
	});

	it('builds its own Study when the given one fails', async () => {
		const ctrl = controller();
		const own = fakeStudy();
		engine.createStudy.mockResolvedValueOnce(own);
		const local = await ctrl.openBalancedModule({
			name: 'generated',
			payload,
			formulation: 'dcopf',
			study: Promise.reject(new Error('worker lost')),
			activate: false
		});
		expect(await ctrl.getStudy(local, payload.module_json)).toBe(own);
		expect(engine.createStudy).toHaveBeenCalledTimes(1);
	});

	it('frees a Study built for another formulation and builds its own', async () => {
		const ctrl = controller();
		const other = fakeStudy('acpf');
		const own = fakeStudy();
		engine.createStudy.mockResolvedValueOnce(own);
		const local = await ctrl.openBalancedModule({
			name: 'generated',
			payload,
			formulation: 'dcopf',
			study: Promise.resolve(other),
			activate: false
		});
		expect(await ctrl.getStudy(local, payload.module_json)).toBe(own);
		expect(other.free).toHaveBeenCalledTimes(1);
	});

	it('frees the given Study when the case is removed before it is built', async () => {
		const ctrl = controller();
		const study = fakeStudy();
		let finish!: (study: BrowserStudy) => void;
		const local = await ctrl.openBalancedModule({
			name: 'generated',
			payload,
			formulation: 'dcopf',
			study: new Promise((resolve) => (finish = resolve)),
			activate: false
		});
		const building = ctrl.getStudy(local, payload.module_json);
		ctrl.disposeStudy(local);
		finish(study);
		expect(await building).toBeNull();
		expect(study.free).toHaveBeenCalledTimes(1);
		expect(ctrl.caseStudies.get(local)).toBeUndefined();
	});

	it('frees the given Study when the module cannot be opened', async () => {
		const ctrl = controller();
		const study = fakeStudy();
		await expect(
			ctrl.openBalancedModule({ name: 'none', study: Promise.resolve(study) })
		).rejects.toThrow(/needs moduleJson or payload/);
		await Promise.resolve();
		expect(study.free).toHaveBeenCalledTimes(1);
	});
});
