import { beforeEach, describe, expect, it, vi } from 'vitest';
import { AppState, CaseState, LocalCase } from '../src/lib/state.svelte.js';
import { Controller } from '../src/lib/controller.svelte.js';
import type { TellegenApiClient } from '../src/lib/api.js';
import { applyDisplayGeo, applyGeo, createStudy, ingestJsonDrop, parseGeo } from '@tellegen/engine';

vi.mock('@tellegen/engine', async (importOriginal) => ({
	...(await importOriginal<typeof import('@tellegen/engine')>()),
	ingestCase: vi.fn(async () => payload('base')),
	ingestJsonDrop: vi.fn(async () => ({ kind: 'balanced', payload: payload('base') })),
	parseGeo: vi.fn(async () => ({ layer: '{"id":"coordinates"}', diagnostics: [] })),
	parseDisplay: vi.fn(async () => ({ layer: 'drawing', diagnostics: [] })),
	applyGeo: vi.fn(async (input: string, layer: string) => ({
		...payload(`${input}:${layer}`),
		view: geographic,
		report: { matched_buses: 2, matched_branches: 1, unmatched_features: 0, notes: [] }
	})),
	applyDisplayGeo: vi.fn(async () => ({
		...payload('drawing'),
		view: drawing,
		report: { matched_buses: 2, matched_branches: 1, unmatched_features: 0, notes: [] }
	})),
	extractGeo: vi.fn(async () => 'final-geography'),
	createStudy: vi.fn(() => {
		throw new Error('Unexpected solve');
	})
}));

const geographic = {
	buses: [1, 2].map((id) => ({ id, lon: -80 + id, lat: 35, demand_mw: id * 10, gen_mw: 100 })),
	branches: [
		{
			id: 1,
			from: 1,
			to: 2,
			rate_mw: 100,
			status: 1,
			path: [
				[-79, 35],
				[-78, 35]
			]
		}
	]
};
const drawing = { ...geographic, coordinate_space: 'diagram' as const };
function payload(module_json: string) {
	return {
		name: 'Example',
		n_bus: 2,
		n_branch: 1,
		n_gen: 1,
		base_mva: 100,
		coords_kind: 'file',
		module_json,
		topology: { buses: geographic.buses, branches: geographic.branches },
		view: geographic,
		warnings: []
	};
}
function host() {
	const app = new AppState();
	const ctrl = new Controller(app, { api: {} as TellegenApiClient });
	vi.spyOn(app, 'requestFrame').mockResolvedValue(undefined);
	vi.spyOn(ctrl, 'maybeStartLocalSolve').mockImplementation(() => {});
	return { app, ctrl };
}
const files = () => [
	new File(['m'], 'example.m'),
	new File(['csv'], 'positions.csv'),
	new File(['pwd'], 'drawing.pwd')
];

beforeEach(() => vi.clearAllMocks());

describe('coordinate attachments', () => {
	it('starts a dropped declared AC PF instance with its declared calculation', async () => {
		const { app, ctrl } = host();
		vi.mocked(ingestJsonDrop).mockResolvedValueOnce({
			kind: 'module',
			format: null,
			payload: { ...payload('declared-ac-pf'), formulation: 'acpf' }
		} as never);
		await ctrl.ingestFiles([new File(['{}'], 'declared-ac-pf.pio.json')]);
		expect(app.error).toBeNull();
		expect(app.activeLocal?.formulation).toBe('acpf');
		expect(app.activeLocal?.declaredFormulation).toBe('acpf');
		expect(app.activeLocal?.studyInputJson).toBe('declared-ac-pf');
		expect(app.displayMode).toBe('voltage');
		expect(ctrl.maybeStartLocalSolve).toHaveBeenCalledWith(app.activeLocal!.id);
		ctrl.changeFormulation(app.activeLocal!, 'dcopf');
		expect(app.activeLocal?.formulation).toBe('acpf');
		expect(app.error).toContain('declared calculation');
	});
	it('selects AC power flow equipment without requesting LMP derivatives or previews', async () => {
		const { app, ctrl } = host();
		const source = new CaseState({
			id: 'case-pf',
			name: 'Power flow',
			n_bus: 2,
			n_branch: 1,
			n_gen: 1
		});
		source.formulation = 'acpf';
		source.network = {
			id: source.id,
			name: source.name,
			base_mva: 100,
			synthetic_coords: false,
			...geographic
		};
		app.cases = [source];
		app.activeCaseId = source.id;
		const sensitivity = vi.spyOn(ctrl, 'browserSensitivity');
		await ctrl.selectBus(source.id, 1);
		expect(app.selectedBus).toBe(1);
		expect(app.sensitivityLoading).toBe(false);
		await ctrl.selectBranch(source.id, 1);
		expect(app.selectedBranch).toBe(1);
		expect(sensitivity).not.toHaveBeenCalled();
		ctrl.runPreview(source, 1, 5);
		ctrl.runRatingPreview(source, 1, 5);
		expect(app.previewPrices).toBeNull();
		expect(createStudy).not.toHaveBeenCalled();
	});

	it('retains co-dropped geographic coordinates and a drawing without preparing an extra solve', async () => {
		const { app, ctrl } = host();
		await ctrl.ingestFiles(files());
		expect(app.error).toBeNull();
		expect(app.localCases).toHaveLength(1);
		expect(app.activeLocal?.view).toEqual(geographic);
		expect(app.activeLocal?.diagram?.view).toEqual(drawing);
		expect(app.activeLocal?.displayMode).toBe('geographic');
		expect(app.activeLocal?.studyInputJson).toBe('base:{"id":"coordinates"}');
		expect(applyDisplayGeo).toHaveBeenCalledWith(
			'base:{"id":"coordinates"}',
			expect.any(Uint8Array)
		);
		expect(createStudy).not.toHaveBeenCalled();
	});

	it('solves a diagram-only case without requiring geographic coordinates', () => {
		const { app, ctrl } = host();
		vi.mocked(ctrl.maybeStartLocalSolve).mockRestore();
		const run = vi.spyOn(ctrl, 'runSolve').mockImplementation(() => {});
		const c = new LocalCase({
			id: 'local-diagram',
			label: 'Drawing',
			fileName: 'case.m',
			studyInputJson: 'base',
			summary: payload('base') as never
		});
		c.diagram = { view: drawing, layer: 'drawing', name: 'Diagram', warnings: [] };
		c.displayMode = 'diagram';
		c.coordsKind = 'synthetic_pending';
		ctrl.addAndActivateLocal(c);
		expect(app.placingLocalId).toBeNull();
		ctrl.activateLocal(c);
		expect(app.placingLocalId).toBeNull();
		expect(c.network?.coordinate_space).toBe('diagram');
		expect(run).toHaveBeenCalledWith(c, null);
	});

	it('retains a matching drawing when the accompanying geographic file does not match', async () => {
		const { app, ctrl } = host();
		vi.mocked(applyGeo).mockRejectedValueOnce(new Error('No bus IDs matched'));
		await ctrl.ingestFiles(files());
		expect(app.activeLocal?.diagram?.view).toEqual(drawing);
		expect(app.error).toContain('No bus IDs matched');
	});

	it('copies the selected backend case with current edits and result, then saves without solving', async () => {
		const { app, ctrl } = host();
		const source = new CaseState({
			id: 'case-a',
			name: 'Example',
			n_bus: 2,
			n_branch: 1,
			n_gen: 1
		});
		source.studyInputJson = 'base';
		source.network = {
			id: source.id,
			name: source.name,
			base_mva: 100,
			synthetic_coords: false,
			...geographic
		};
		source.deltas = { 2: 5 };
		source.ratings = { 1: 10 };
		source.solution = {
			objective: 99,
			prices: [],
			flows: [],
			dispatch: [],
			va: [],
			w: []
		} as never;
		app.cases = [source];
		app.activeCaseId = source.id;
		const save = vi.spyOn(ctrl, 'captureSavedCase');
		save.mockResolvedValueOnce({ input: 'current-instance', solution: 'current-result' });
		await ctrl.ingestFiles(files().slice(1));
		expect(app.error).toBeNull();
		const copy = app.activeLocal!;
		expect(copy).toBeInstanceOf(LocalCase);
		expect(copy.id).not.toBe(source.id);
		expect(copy.deltas).toEqual({ 2: 5 });
		expect(copy.ratings).toEqual({ 1: 10 });
		expect(copy.solution).toBe(source.solution);
		expect(copy.diagram?.view).toEqual(drawing);
		expect(copy.deltas).not.toBe(source.deltas);
		expect(source.studyInputJson).toBe('base');
		expect(source.network.buses).toEqual(geographic.buses);
		expect(app.cases[0]).toBe(source);
		expect(await ctrl.captureSavedCase(copy)).toEqual({
			input: 'current-instance',
			solution: 'current-result'
		});
		expect(createStudy).not.toHaveBeenCalled();
	});

	it('does not attach to an unrelated local case when no case is selected', async () => {
		const { app, ctrl } = host();
		const unrelated = new LocalCase({
			id: 'local-other',
			label: 'Other',
			fileName: 'other.m',
			studyInputJson: 'other'
		});
		app.localCases = [unrelated];
		await ctrl.ingestFiles(files().slice(1));
		expect(app.error).toBe('Select the matching case before attaching coordinates');
		expect(unrelated.studyInputJson).toBe('other');
		expect(applyGeo).not.toHaveBeenCalled();
		expect(applyDisplayGeo).not.toHaveBeenCalled();
	});

	it('leaves a local case unchanged if selection changes during matching', async () => {
		const { app, ctrl } = host();
		await ctrl.ingestFiles(files().slice(0, 1));
		const source = app.activeLocal!;
		const input = source.studyInputJson;
		const view = source.view;
		vi.mocked(applyGeo).mockImplementationOnce(async () => {
			app.activeLocalId = null;
			return {
				...payload('changed'),
				report: { matched_buses: 2, matched_branches: 0, unmatched_features: 0, notes: [] }
			} as never;
		});
		await ctrl.ingestFiles(files().slice(1, 2));
		expect(app.error).toContain('selected case changed');
		expect(source.studyInputJson).toBe(input);
		expect(source.view).toBe(view);
		expect(app.localCases).toHaveLength(1);
	});

	it('invalidates a copied cached result after its demand changes', async () => {
		const { app, ctrl } = host();
		const source = new CaseState({
			id: 'case-a',
			name: 'Example',
			n_bus: 2,
			n_branch: 1,
			n_gen: 1
		});
		source.studyInputJson = 'base';
		source.solution = {
			objective: 99,
			prices: [],
			flows: [],
			dispatch: [],
			va: [],
			w: []
		} as never;
		app.cases = [source];
		app.activeCaseId = source.id;
		vi.spyOn(ctrl, 'captureSavedCase').mockResolvedValueOnce({
			input: 'current-instance',
			solution: 'current-result'
		});
		await ctrl.ingestFiles(files().slice(1, 2));
		const copy = app.activeLocal!;
		copy.deltas = { 2: 9 };
		await expect(ctrl.captureSavedCase(copy)).rejects.toThrow('no matching saved result');
		expect(source.deltas).toEqual({});
	});

	it('does not retarget an attachment if selection changes while the file is parsed', async () => {
		const { app, ctrl } = host();
		const original = new LocalCase({
			id: 'local-a',
			label: 'A',
			fileName: 'a.m',
			studyInputJson: 'a'
		});
		const other = new LocalCase({
			id: 'local-b',
			label: 'B',
			fileName: 'b.m',
			studyInputJson: 'b'
		});
		app.localCases = [original, other];
		app.activeLocalId = original.id;
		vi.mocked(parseGeo).mockImplementationOnce(async () => {
			app.activeLocalId = other.id;
			return { layer: '{}', diagnostics: [], n_points: 1, n_routes: 0 };
		});
		await ctrl.ingestFiles(files().slice(1, 2));
		expect(app.error).toContain('selected case changed');
		expect(applyGeo).not.toHaveBeenCalled();
		expect(original.studyInputJson).toBe('a');
		expect(other.studyInputJson).toBe('b');
	});

	it('refuses attachment to an inspected saved state', async () => {
		const { app, ctrl } = host();
		app.studyView = { id: 'saved' } as never;
		await ctrl.ingestFiles(files().slice(1));
		expect(app.error).toBe('Return to the live case before attaching coordinates');
		expect(applyGeo).not.toHaveBeenCalled();
	});

	it('discards a backend copy if the user changes cases while loading it', async () => {
		const { app, ctrl } = host();
		const source = new CaseState({
			id: 'case-a',
			name: 'Example',
			n_bus: 2,
			n_branch: 1,
			n_gen: 1
		});
		source.studyInputJson = 'base';
		app.cases = [source];
		app.activeCaseId = source.id;
		vi.mocked(ingestJsonDrop).mockImplementationOnce(async () => {
			app.activeCaseId = null;
			return { kind: 'balanced', payload: payload('base') } as never;
		});
		await ctrl.ingestFiles(files().slice(1));
		expect(app.error).toContain('selected case changed');
		expect(app.localCases).toHaveLength(0);
		expect(applyDisplayGeo).not.toHaveBeenCalled();
	});
});
