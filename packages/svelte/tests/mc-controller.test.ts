import { describe, expect, it, vi } from 'vitest';
import {
	summarizeMcPfResult,
	type AppliedMcGeoCase,
	type BrowserMcPfSession,
	type DistGraph,
	type IngestedDistCase,
	type McLoadBranchState,
	type McPfDetail,
	type McPfResult,
	type McPfSummary,
	type McStudySnapshot
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
	voltage_valid: true,
	min_voltage_pu: 0.98,
	max_voltage_pu: 1,
	voltage_violations: [],
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
const summary = summarizeMcPfResult(result);
/** The summary a live session reports after its `solve`-th solve. */
const live = (solve: number, patch: Partial<McPfSummary> = {}): McPfSummary => ({
	...summarizeMcPfResult(result, solve),
	...patch
});
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
	it('retains a fixed-point session and automatically warm-solves load edits', async () => {
		vi.useFakeTimers();
		try {
			const { ctrl, c, solve } = host();
			const loads: McLoadBranchState[] = [
				{
					load: 'customer',
					bus: 'load',
					branch: 0,
					p_w: 10_000,
					q_var: 2_000,
					base_p_w: 10_000,
					base_q_var: 2_000
				}
			];
			const edited = live(2, { iterations: 1 });
			const editedFull = { ...result, iterations: 1 };
			const snapshot = (value: McPfResult): McStudySnapshot => ({
				schema: 'tellegen-mc-pf-study',
				version: 1,
				id: 'live',
				title: 'Feeder',
				formulation: 'mc_ac_pf',
				input_module: 'edited-input',
				solution_module: 'solution',
				options: {},
				result: value
			});
			const replaceLoadPowers = vi.fn(async () => edited);
			const page = (solve_count: number): McPfDetail => ({
				solve_count,
				terminals: result.terminals,
				element_ports: [],
				element_port_total: 0
			});
			const detail = vi.fn(async () => page(2));
			const session = {
				initialSummary: live(1),
				lastTiming: { engine_ms: 3, round_trip_ms: 4, parse_ms: 0.1, payload_chars: 600 },
				result: vi.fn(async () => result),
				loadBranches: vi.fn(async () => loads),
				replaceLoadPowers,
				detail,
				inputModule: vi.fn(async () => 'edited-input'),
				snapshot: vi.fn(async () =>
					snapshot(replaceLoadPowers.mock.calls.length ? editedFull : result)
				),
				free: vi.fn()
			} as unknown as BrowserMcPfSession;
			ctrl.mcTransport.createMcPfSession = vi.fn(async () => session);

			await ctrl.solveMultiCase(c);
			expect(solve).not.toHaveBeenCalled();
			expect(c.mcSession).toBe(session);
			expect(c.mcLoadBranches).toEqual(loads);
			// The session's own summary is the result; nothing else is fetched.
			expect(c.result).toEqual(live(1));
			expect(session.result).not.toHaveBeenCalled();

			ctrl.queueMultiLoadPower(c, 'customer', 0, 11_000, 2_100);
			await vi.advanceTimersByTimeAsync(200);
			expect(replaceLoadPowers).toHaveBeenCalledWith([
				{ load: 'customer', branch: 0, p_w: 11_000, q_var: 2_100 }
			]);
			expect(c.result).toEqual(edited);
			// One round trip per edit: the queued state already holds the
			// confirmed branch powers, so they are not fetched again.
			expect(session.loadBranches).toHaveBeenCalledTimes(1);
			expect(c.mcLoadBranches[0]).toMatchObject({ p_w: 11_000, q_var: 2_100 });
			expect(session.result).not.toHaveBeenCalled();
			expect(c.mcTiming).toMatchObject({ engine_ms: 3, payload_chars: 600 });
			expect(c.moduleJson).toBe('input');
			expect(c.mcSnapshot).toBeNull();
			expect(session.snapshot).not.toHaveBeenCalled();
			// Detail pages come from the session and must describe this solve.
			expect(await ctrl.multiResultDetail(c, { bus: 'load' })).toEqual(page(2));
			expect(detail).toHaveBeenCalledWith({ bus: 'load' });
			detail.mockResolvedValueOnce(page(1));
			expect(await ctrl.multiResultDetail(c, { bus: 'load' })).toBeNull();
			expect(await ctrl.snapshotMultiCase(c)).toEqual(snapshot(editedFull));
			expect(c.mcSnapshot).toEqual(snapshot(editedFull));
			expect(c.solving).toBe(false);
		} finally {
			vi.useRealTimers();
		}
	});

	it('releases the edit flags when a fresh solve replaces the session mid-flush', async () => {
		vi.useFakeTimers();
		try {
			const { ctrl, c } = host();
			const loads: McLoadBranchState[] = [
				{
					load: 'customer',
					bus: 'load',
					branch: 0,
					p_w: 10_000,
					q_var: 2_000,
					base_p_w: 10_000,
					base_q_var: 2_000
				}
			];
			let finishReplace!: (value: McPfSummary) => void;
			const first = {
				initialSummary: live(1),
				result: vi.fn(async () => result),
				loadBranches: vi.fn(async () => loads),
				replaceLoadPowers: vi.fn(
					() => new Promise<McPfSummary>((resolve) => (finishReplace = resolve))
				),
				inputModule: vi.fn(async () => 'input'),
				snapshot: vi.fn(),
				free: vi.fn()
			} as unknown as BrowserMcPfSession;
			const second = {
				initialSummary: live(1),
				result: vi.fn(async () => result),
				loadBranches: vi.fn(async () => loads),
				replaceLoadPowers: vi.fn(async () => live(2, { iterations: 2 })),
				inputModule: vi.fn(async () => 'input'),
				snapshot: vi.fn(),
				free: vi.fn()
			} as unknown as BrowserMcPfSession;
			let finishCreate!: (value: BrowserMcPfSession) => void;
			const create = vi
				.fn<() => Promise<BrowserMcPfSession>>()
				.mockResolvedValueOnce(first)
				.mockImplementationOnce(
					() => new Promise<BrowserMcPfSession>((resolve) => (finishCreate = resolve))
				);
			ctrl.mcTransport.createMcPfSession = create;

			await ctrl.solveMultiCase(c);
			expect(c.mcSession).toBe(first);

			// An explicit re-solve is in flight while the user edits a load
			// against the retained session.
			const resolve = ctrl.solveMultiCase(c);
			ctrl.queueMultiLoadPower(c, 'customer', 0, 11_000, 2_100);
			await vi.advanceTimersByTimeAsync(200);
			expect(first.replaceLoadPowers).toHaveBeenCalledTimes(1);
			expect(c.mcEditRunning).toBe(true);

			// The solve finishes first and installs a new session; the stale
			// flush then completes against the replaced one.
			finishCreate(second);
			await resolve;
			expect(c.mcSession).toBe(second);
			finishReplace(live(2));
			await vi.advanceTimersByTimeAsync(0);
			expect(c.mcEditRunning).toBe(false);
			expect(c.solving).toBe(false);

			// Later edits still reach the live session.
			ctrl.queueMultiLoadPower(c, 'customer', 0, 12_000, 2_100);
			await vi.advanceTimersByTimeAsync(200);
			expect(second.replaceLoadPowers).toHaveBeenCalledWith([
				{ load: 'customer', branch: 0, p_w: 12_000, q_var: 2_100 }
			]);
			expect(c.mcEditRunning).toBe(false);
			expect(c.solving).toBe(false);
		} finally {
			vi.useRealTimers();
		}
	});

	it('drops a session that stops answering and offers a full re-solve', async () => {
		vi.useFakeTimers();
		try {
			const { ctrl, c, app } = host();
			const loads: McLoadBranchState[] = [
				{
					load: 'customer',
					bus: 'load',
					branch: 0,
					p_w: 10_000,
					q_var: 2_000,
					base_p_w: 10_000,
					base_q_var: 2_000
				}
			];
			const dead = new Error('engine worker failed');
			let alive = true;
			const session = {
				initialSummary: live(1),
				result: vi.fn(async () => result),
				loadBranches: vi.fn(async () => {
					if (!alive) throw dead;
					return loads;
				}),
				replaceLoadPowers: vi.fn(async () => {
					alive = false;
					throw dead;
				}),
				inputModule: vi.fn(async () => 'input'),
				snapshot: vi.fn(),
				free: vi.fn()
			} as unknown as BrowserMcPfSession;
			ctrl.mcTransport.createMcPfSession = vi.fn(async () => session);

			await ctrl.solveMultiCase(c);
			ctrl.queueMultiLoadPower(c, 'customer', 0, 11_000, 2_100);
			await vi.advanceTimersByTimeAsync(200);
			expect(app.error).toBe('engine worker failed');
			expect(c.mcSession).toBeNull();
			expect(session.free).toHaveBeenCalled();
			expect(c.mcEditRunning).toBe(false);
			expect(c.solving).toBe(false);
			expect(app.errorRetry).not.toBeNull();
		} finally {
			vi.useRealTimers();
		}
	});

	it('records terminal results and rejects concurrent work', async () => {
		const { ctrl, c, solve } = host();
		const pending = ctrl.solveMultiCase(c, { tolerance: 1e-8 });
		await expect(ctrl.solveMultiCase(c)).rejects.toThrow('already running');
		expect(await pending).toEqual(summary);
		expect(solve).toHaveBeenCalledWith('input', { tolerance: 1e-8 }, expect.any(AbortSignal));
		// Without a session the full result is kept for on-demand detail.
		expect(c.result).toEqual(summary);
		expect(c.mcFullResult).toBe(result);
		expect(await ctrl.multiResultDetail(c, { bus: 'load' })).toMatchObject({
			terminals: result.terminals,
			element_port_total: 0
		});
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
		c.result = summary;
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
		expect(c.result).toEqual(summary);
		expect(c.solving).toBe(false);
		expect(app.error).toBeNull();
	});
	it('does not accept a result computed from a changed input', async () => {
		const { ctrl, c, solve } = host();
		c.result = summary;
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
		expect(c.result).toEqual(summary);
	});
	it('updates matched coordinates without changing electrical results and refuses a changed selection', async () => {
		const { app, ctrl, c, apply } = host();
		c.result = summary;
		await ctrl.applyMultiGeoLayers(c, [
			{ name: 'private.geojson', layer: 'layer', diagnostics: [] }
		]);
		expect(c.moduleJson).toBe('with-geo');
		expect(c.result).toEqual(summary);
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
		const solved = c.result;
		expect(solved).toEqual(summary);
		await ctrl.applyMultiGeoLayers(c, [{ name: 'coordinates', layer: 'layer', diagnostics: [] }]);
		expect(c.result).toBe(solved);
		expect(c.mcFullResult).toBe(result);
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
	it('keeps result detail from the saved snapshot when coordinates replace a live session', async () => {
		const { ctrl, c } = host();
		const editedFull = { ...result, iterations: 1 };
		const saved: McStudySnapshot = {
			schema: 'tellegen-mc-pf-study',
			version: 1,
			id: 'live',
			title: 'Feeder',
			formulation: 'mc_ac_pf',
			input_module: 'edited-input',
			solution_module: 'solution',
			options: {},
			result: editedFull
		};
		const session = {
			initialSummary: live(1),
			loadBranches: vi.fn(async () => []),
			detail: vi.fn(),
			snapshot: vi.fn(async () => saved),
			free: vi.fn()
		} as unknown as BrowserMcPfSession;
		ctrl.mcTransport.createMcPfSession = vi.fn(async () => session);
		ctrl.mcTransport.applyMcStudyGeo = vi.fn(async (snapshot: McStudySnapshot) => ({
			...snapshot,
			input_module: 'with-geo'
		}));
		await ctrl.solveMultiCase(c);
		await ctrl.applyMultiGeoLayers(c, [{ name: 'coordinates', layer: 'layer', diagnostics: [] }]);
		expect(session.free).toHaveBeenCalled();
		expect(c.mcSession).toBeNull();
		expect(c.mcFullResult).toBe(editedFull);
		expect(await ctrl.multiResultDetail(c, { bus: 'load' })).toMatchObject({
			terminals: editedFull.terminals
		});
		expect(session.detail).not.toHaveBeenCalled();
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
