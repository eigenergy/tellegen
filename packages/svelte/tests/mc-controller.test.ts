import { describe, expect, it, vi } from 'vitest';
import type {
	AppliedMcGeoCase,
	DistGraph,
	IngestedDistCase,
	McPfResult,
	McStudySnapshot
} from '@tellegen/engine';
import { AppState, MulticonductorCase } from '../src/lib/state.svelte.js';
import { Controller } from '../src/lib/controller.svelte.js';
import { buildDiagramView, buildGeographicView } from '../src/lib/multiconductor.js';

const graph: DistGraph = {
	buses: ['source', 'load'].map((id, i) => ({
		id,
		terminals: ['1', 'n'],
		grounded: ['n'],
		xy: [1000 + i * 50, 2000 + i * 20],
		load_kw: i * 10,
		gen_kw: 0,
		has_source: i === 0
	})),
	edges: [
		{
			id: 'line',
			kind: 'line',
			from: 'source',
			to: 'load',
			conductors: [['1', '1']],
			closed: true,
			n_phases: 1
		}
	]
};
const result: McPfResult = {
	converged: true,
	iterations: 3,
	factorization_count: 1,
	matrix_dimension: 2,
	matrix_nonzeros: 4,
	voltage_change: 1e-9,
	physical_kcl_residual: 1e-10,
	scaled_kcl_residual: 1e-11,
	terminals: [
		{
			bus: 'load',
			terminal: '1',
			voltage: { re: 230, im: 5 },
			current_into_network: { re: 5, im: 1 },
			power_into_network: { re: 1155, im: -205 }
		}
	],
	element_ports: [],
	source_reactions: []
};
function payload(input = 'input'): IngestedDistCase {
	return {
		module_json: input,
		mc_pf_enabled: true,
		mc_pf_supported: true,
		mc_pf_unavailable_reason: null,
		name: 'feeder',
		model: 'multiconductor',
		graph,
		coords_kind: 'planar',
		coords_space: 'diagram',
		has_coords: true,
		placed_buses: 2,
		n_bus: 2,
		n_edge: 1,
		n_line: 1,
		n_switch: 0,
		n_transformer: 0,
		n_load: 1,
		n_generator: 0,
		n_ibr: 0,
		n_source: 1,
		n_shunt: 0,
		load_kw: 10,
		gen_kw: 0,
		base_frequency: 60,
		diagnostics: []
	};
}
function host() {
	const app = new AppState();
	const solve = vi.fn(async () => result);
	const apply = vi.fn(async (): Promise<AppliedMcGeoCase> => ({
		...payload('with-geo'),
		report: { matched_buses: 2, matched_branches: 1, unmatched_features: 0, notes: [] }
	}));
	const ctrl = new Controller(app, { mcTransport: { solveMcModule: solve, applyMcGeo: apply } });
	vi.spyOn(app, 'requestFrame').mockResolvedValue();
	const c = new MulticonductorCase({
		id: 'mc',
		label: 'Feeder',
		fileName: 'private.json',
		summary: payload(),
		graph,
		coordsKind: 'planar',
		view: buildDiagramView(graph)
	});
	app.addMulti(c);
	return { app, ctrl, c, solve, apply };
}
describe('multiconductor calculation and coordinates', () => {
	it('records terminal results and rejects concurrent work', async () => {
		const { ctrl, c, solve } = host();
		const pending = ctrl.solveMultiCase(c, { tolerance: 1e-8 });
		await expect(ctrl.solveMultiCase(c)).rejects.toThrow('already running');
		expect(await pending).toEqual(result);
		expect(solve).toHaveBeenCalledWith('input', { tolerance: 1e-8 }, expect.any(AbortSignal));
		expect(c.result).toEqual(result);
		expect(c.revisionGeneration).toBe(1);
		expect(c.solving).toBe(false);
	});
	it('rejects unsupported retained physics before calling the engine', async () => {
		const { app, ctrl, c, solve } = host();
		c.summary = { ...payload(), mc_pf_unavailable_reason: 'Unsupported load model' };
		await expect(ctrl.solveMultiCase(c)).rejects.toThrow('Unsupported load model');
		expect(solve).not.toHaveBeenCalled();
		expect(app.error).toBe('Unsupported load model');
	});
	it('keeps prior results without notices on intentional cancellation', async () => {
		const { app, ctrl, c, solve } = host();
		c.result = result;
		let finish!: (r: McPfResult) => void;
		solve.mockImplementationOnce(
			() =>
				new Promise((resolve) => {
					finish = resolve;
				})
		);
		const abort = new AbortController();
		const pending = ctrl.solveMultiCase(c, {}, abort.signal);
		abort.abort();
		finish({ ...result, iterations: 6 });
		await expect(pending).rejects.toMatchObject({ name: 'AbortError' });
		expect(c.result).toEqual(result);
		expect(c.solving).toBe(false);
		expect(app.error).toBeNull();
	});
	it('does not accept a result computed from a changed input', async () => {
		const { ctrl, c, solve } = host();
		c.result = result;
		let finish!: (r: McPfResult) => void;
		solve.mockImplementationOnce(
			() =>
				new Promise((resolve) => {
					finish = resolve;
				})
		);
		const pending = ctrl.solveMultiCase(c);
		c.moduleJson = 'changed';
		finish({ ...result, iterations: 6 });
		await expect(pending).rejects.toThrow('case changed');
		expect(c.result).toEqual(result);
	});
	it('updates matched coordinates without changing electrical results and refuses a changed selection', async () => {
		const { app, ctrl, c, apply } = host();
		c.result = result;
		await ctrl.applyMultiGeoLayers(c, [
			{ name: 'private.geojson', layer: 'layer', diagnostics: [] }
		]);
		expect(c.moduleJson).toBe('with-geo');
		expect(c.result).toEqual(result);
		expect(c.view!.buses[0].lon).toBe(1000);
		app.activeMultiId = null;
		await expect(
			ctrl.applyMultiGeoLayers(c, [{ name: 'other', layer: 'other', diagnostics: [] }])
		).rejects.toThrow('Select the matching case');
		expect(app.activeMultiId).toBeNull();
		expect(apply).toHaveBeenCalledTimes(1);
	});
	it('retains one snapshot and updates its coordinates atomically without another solve', async () => {
		const { ctrl, c, solve } = host();
		const snapshot: McStudySnapshot = {
			schema: 'tellegen-mc-pf-study',
			version: 1,
			id: 'saved',
			title: 'Feeder',
			formulation: 'mc_ac_pf',
			input_module: 'input',
			solution_module: 'solution',
			options: {},
			result
		};
		const saveSolve = vi.fn(async () => snapshot);
		ctrl.mcTransport.solveMcStudy = saveSolve;
		await ctrl.solveMultiCase(c);
		expect(saveSolve).toHaveBeenCalledWith(
			'input',
			expect.any(String),
			'Feeder',
			{},
			expect.any(AbortSignal)
		);
		expect(solve).not.toHaveBeenCalled();
		expect(c.mcSnapshot).toBe(snapshot);
		c.mcSavedAt = 'saved';
		const applySnapshot = vi.fn(async () => ({
			...snapshot,
			input_module: 'with-geo',
			solution_module: 'solution-with-geo'
		}));
		ctrl.mcTransport.applyMcStudyGeo = applySnapshot;
		await ctrl.applyMultiGeoLayers(c, [{ name: 'coordinates', layer: 'layer', diagnostics: [] }]);
		expect(c.result).toBe(result);
		expect(c.mcSnapshot).toMatchObject({
			input_module: 'with-geo',
			solution_module: 'solution-with-geo',
			result
		});
		expect(c.mcSnapshot!.id).not.toBe('saved');
		expect(c.mcSavedAt).toBeNull();
		expect(saveSolve).toHaveBeenCalledTimes(1);
		const before = c.mcSnapshot;
		applySnapshot.mockRejectedValueOnce(new Error('No coordinates matched'));
		await expect(
			ctrl.applyMultiGeoLayers(c, [{ name: 'other', layer: 'other', diagnostics: [] }])
		).rejects.toThrow('No coordinates');
		expect(c.mcSnapshot).toBe(before);
		expect(c.moduleJson).toBe('with-geo');
	});
	it('preserves raw drawing positions and full routed paths', () => {
		const layer = JSON.stringify({
			features: [
				{
					properties: { from: 'source', to: 'load' },
					geometry: {
						type: 'LineString',
						coordinates: [
							[1000, 2000],
							[1025, 2017],
							[1050, 2020]
						]
					}
				}
			]
		});
		const view = buildDiagramView(graph, layer);
		expect(view.buses.map((b) => [b.lon, b.lat])).toEqual([
			[1000, 2000],
			[1050, 2020]
		]);
		expect(view.edges[0].path).toEqual([
			[1000, 2000],
			[1025, 2017],
			[1050, 2020]
		]);
		expect(buildGeographicView(graph, layer).edges[0].path).toEqual(view.edges[0].path);
		const parallel = { ...graph, edges: [...graph.edges, { ...graph.edges[0], id: 'parallel' }] };
		expect(buildDiagramView(parallel, layer).edges.every((edge) => edge.path.length === 2)).toBe(
			true
		);
	});
});
