import { afterEach, expect, it, vi } from 'vitest';
import { BrowserCaseCollection, type ImportedCollectionCase } from '@tellegen/engine';
import { Controller } from '../src/lib/controller.svelte.js';
import { AppState } from '../src/lib/state.svelte.js';

afterEach(() => vi.restoreAllMocks());

async function importedCases(recorded = false) {
	const module_json = JSON.stringify({
		extensions: recorded
			? {
					'tellegen.calculation_settings': {
						version: 1,
						calculation: 'dcopf',
						overrides: { angle_difference: false }
					}
				}
			: {}
	});
	const payload = {
		name: 'case',
		base_mva: 100,
		coords_kind: 'file',
		module_json,
		view: { buses: [], branches: [], coordinate_space: 'geographic' }
	};
	const result = {
		kind: 'balanced',
		payload,
		dependencies: [],
		incomplete: false,
		origin: {}
	} as unknown as ImportedCollectionCase;
	vi.spyOn(BrowserCaseCollection, 'fromZip').mockResolvedValue({
		inspection: {
			name: 'cases.zip',
			entries: [
				{ path: 'a.m', bytes: 1 },
				{ path: 'b.m', bytes: 1 }
			],
			candidates: ['a.m', 'b.m'].map((primary) => ({
				selection: { primary },
				status: 'selectable'
			})),
			original_zip: true,
			limits: { files: 4096, expanded_bytes: 64 << 20 }
		},
		ingest: vi.fn(async () => result),
		cancel: vi.fn(),
		free: vi.fn(async () => {})
	} as unknown as BrowserCaseCollection);
	const app = new AppState();
	const ctrl = new Controller(app, { apiBase: null, loadDefaultCases: false });
	const solve = vi.spyOn(ctrl, 'runSolve').mockImplementation(() => {});
	await ctrl.collectionImport.start(async () => [
		{ path: 'cases.zip', file: new File(['zip'], 'cases.zip') }
	]);
	ctrl.collectionImport.rows.forEach((row) => {
		row.checked = true;
	});
	await ctrl.collectionImport.importSelected();
	ctrl.collectionImport.close();
	return { app, ctrl, solve };
}

it("preserves a collection module's recorded calculation and settings", async () => {
	const { app, ctrl, solve } = await importedCases(true);
	try {
		expect(app.localCases).toHaveLength(2);
		for (const c of app.localCases) {
			expect(c.formulation).toBe('dcopf');
			expect(c.settings.calculation).toBe('dcopf');
			expect(c.settings.baselines.dcopf).toMatchObject({ angle_difference: false });
		}
		expect(solve).not.toHaveBeenCalled();
	} finally {
		ctrl.dispose();
	}
});

it('selecting or revealing an imported collection case does not start a calculation', async () => {
	const { app, ctrl, solve } = await importedCases();
	try {
		const [first, last] = app.localCases;
		expect(first.formulation).toBe('acpf');
		ctrl.activateLocal(first);
		ctrl.activateLocal(last);
		await ctrl.removeLocalCase(last);
		expect(app.activeLocalId).toBe(first.id);
		expect(first.network).not.toBeNull();
		expect(solve).not.toHaveBeenCalled();
	} finally {
		ctrl.dispose();
	}
});
