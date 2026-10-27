import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { IngestedCase } from '@tellegen/engine';

// The browser Study is wasm; here it fails the way it would in a browser that
// cannot run it, which exercises every fallback path a host without a backend
// could reach.
vi.mock('@tellegen/engine', async (importOriginal) => {
	const actual = await importOriginal<typeof import('@tellegen/engine')>();
	return {
		...actual,
		createStudy: vi.fn(async () => {
			throw new Error('no wasm in this test');
		})
	};
});

import { AppState } from '../src/lib/state.svelte.js';
import { Controller } from '../src/lib/controller.svelte.js';

const payload: IngestedCase = {
	name: 'synth',
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

const fetchSpy = vi.fn(() => {
	throw new Error('fetch must not be called without a backend');
});

beforeEach(() => {
	vi.stubGlobal('fetch', fetchSpy);
	vi.stubGlobal('EventSource', undefined);
});
afterEach(() => {
	vi.unstubAllGlobals();
	fetchSpy.mockClear();
});

describe('a controller without a backend', () => {
	it('never fetches through load, open, select and solve', async () => {
		const app = new AppState();
		const ctrl = new Controller(app, { apiBase: null, loadDefaultCases: false });
		const frame = vi.spyOn(app, 'requestFrame').mockResolvedValue();
		expect(ctrl.api).toBeNull();
		expect(ctrl.loadDefaultCases).toBe(false);

		await ctrl.load();
		expect(ctrl.casesLoaded).toBe(true);
		expect(frame).toHaveBeenCalledWith('all');
		expect(app.cases).toEqual([]);

		ctrl.restoreDefaultCases();
		expect(ctrl.loading).toBeNull();

		const local = await ctrl.openBalancedModule({ name: 'synth', payload });
		expect(app.activeLocalId).toBe(local.id);
		// The solve started by activation runs to its terminal local-case state.
		await vi.waitFor(() => expect(local.solving).toBe(false));
		expect(local.solveBackend).toBe('clarabel-wasm');
		expect(app.error).toMatch(/browser solve unavailable/);

		await ctrl.selectLocalBus(local.id, 2);
		expect(app.selectedBus).toBe(2);
		expect(app.error).toMatch(/sensitivity unavailable in the browser/);

		ctrl.runSolve(local, { bus: 2 });
		await vi.waitFor(() => expect(local.solving).toBe(false));

		expect(fetchSpy).not.toHaveBeenCalled();
	});

	it('also skips the case list fetch when only default cases are off', async () => {
		const app = new AppState();
		const ctrl = new Controller(app, { apiBase: '/api', loadDefaultCases: false });
		vi.spyOn(app, 'requestFrame').mockResolvedValue();
		expect(ctrl.api).not.toBeNull();
		await ctrl.load();
		expect(ctrl.casesLoaded).toBe(true);
		ctrl.restoreDefaultCases();
		expect(fetchSpy).not.toHaveBeenCalled();
	});

	it('still fetches the case list by default', async () => {
		const app = new AppState();
		const ctrl = new Controller(app, { apiBase: '/api' });
		expect(ctrl.loadDefaultCases).toBe(true);
		await ctrl.load();
		expect(fetchSpy).toHaveBeenCalled();
		expect(ctrl.casesLoaded).toBe(false);
	});
});
