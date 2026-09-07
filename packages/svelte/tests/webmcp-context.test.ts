import type { StudyWorkspace } from '../../../apps/web/src/lib/studies/workspace.svelte.js';
import { describe, expect, it, vi } from 'vitest';
import type { Controller, Network, SolvableCase, StudyDisplaySnapshot } from '../src/lib/index.js';
import { createTellegenWebMcpAdapter } from '../../../apps/web/src/lib/webmcp/tellegen-adapter.js';
import type { PlanningActivityStore } from '../../../apps/web/src/lib/webmcp/planning-activity.svelte.js';

vi.mock('@tellegen/svelte', () => ({
	FORMULATIONS: [{ id: 'dcopf' }],
	createStudy: vi.fn(() => {
		throw new Error('Unexpected solve');
	})
}));

function host() {
	const network: Network = {
		id: 'case-a',
		name: 'Case A',
		base_mva: 100,
		synthetic_coords: false,
		buses: [1, 2].map((id) => ({
			id,
			lon: -100 + id,
			lat: 35,
			demand_mw: id * 10,
			gen_mw: 100
		})),
		branches: [
			{
				id: 1,
				from: 1,
				to: 2,
				rate_mw: 100,
				status: 1,
				path: [
					[-99, 35],
					[-98, 35]
				]
			}
		]
	};
	const live = {
		id: 'case-a',
		name: 'Case A',
		network,
		formulation: 'dcopf',
		revisionGeneration: 0,
		deltas: { 1: 9 },
		ratings: {},
		solving: false,
		solution: {
			objective: 600,
			prices: [
				{ bus: 1, value: 50 },
				{ bus: 2, value: 1 }
			],
			va: [],
			w: [],
			flows: [],
			dispatch: []
		}
	} as unknown as SolvableCase;
	const second = { ...live, id: 'case-b', name: 'Case B' };
	const saved: StudyDisplaySnapshot = {
		id: 'state-1',
		studyId: 'study-1',
		caseId: live.id,
		revision: 3,
		label: 'Lower demand',
		formulation: 'dcopf',
		network: {
			...network,
			buses: network.buses.map((b) => ({ ...b, demand_mw: b.demand_mw - 2 }))
		},
		solution: {
			formulation: 'dcopf',
			status: 'optimal',
			objective: 250,
			lmp: [
				{ bus: 1, value: 2 },
				{ bus: 2, value: 7 }
			]
		},
		baseDemandMw: { '1': 10, '2': 20 }
	};
	const app = {
		studyView: saved as StudyDisplaySnapshot | null,
		cases: [live, second],
		localCases: [],
		multiCases: [],
		activeCaseId: live.id,
		activeLocalId: null,
		activeMultiId: null,
		activeMulti: null,
		selectedBus: null as number | null,
		selectedBranch: null as number | null,
		byId(id: string) {
			return this.cases.find((c) => c.id === id);
		},
		requestFrame: vi.fn(async () => {})
	};
	const ctrl = {
		app,
		get activeSolvable() {
			return app.byId(app.activeCaseId) ?? null;
		},
		caseName: () => 'Case A',
		caseStudies: new Map(),
		studyUnavailable: new WeakMap(),
		isBackendCase: (c: SolvableCase) => app.cases.includes(c),
		ensureStudyInputJson: vi.fn(async () => null),
		activateCase: vi.fn(async (id: string) => {
			app.studyView = null;
			app.activeCaseId = id;
		}),
		clearSelection: vi.fn()
	};
	return { ctrl: ctrl as unknown as Controller, app, live, saved };
}
const signal = () => new AbortController().signal;

describe('WebMCP displayed-state context', () => {
	it('ranks AC power flow voltages and reports unavailable LMP analysis', async () => {
		const { ctrl, app, live, saved } = host();
		app.studyView = null;
		live.formulation = 'acpf';
		live.solution = {
			...live.solution!,
			objective: null,
			prices: [],
			vm: [
				{ bus: 1, value: 1.01 },
				{ bus: 2, value: 0.97 }
			]
		};
		const adapter = createTellegenWebMcpAdapter(ctrl);
		const query = {
			caseId: live.id,
			elementKind: 'bus' as const,
			sortBy: 'voltage_pu' as const,
			direction: 'asc' as const,
			limit: 1
		};
		expect(await adapter.queryNetwork(query, signal())).toMatchObject({
			units: { voltage: 'pu', lmp: null },
			elements: [{ legacy_id: 2, voltage_pu: 0.97, price: null }]
		});
		await expect(
			Promise.resolve().then(() => adapter.queryNetwork({ ...query, sortBy: 'price' }, signal()))
		).rejects.toMatchObject({ code: 'METRIC_UNAVAILABLE' });
		await expect(
			adapter.analyzeSensitivity(
				{ caseId: live.id, target: { kind: 'bus', elementId: '1' }, limit: 1 },
				signal()
			)
		).rejects.toMatchObject({ code: 'SENSITIVITY_UNAVAILABLE' });
		saved.formulation = 'acpf';
		saved.solution = {
			formulation: 'acpf',
			status: 'converged',
			objective: null,
			vm: [
				{ bus: 1, value: 0.92 },
				{ bus: 2, value: 1.02 }
			]
		};
		app.studyView = saved;
		expect(await adapter.queryNetwork(query, signal())).toMatchObject({
			state_id: saved.id,
			elements: [{ legacy_id: 1, voltage_pu: 0.92, price: null }]
		});
		expect(ctrl.ensureStudyInputJson).not.toHaveBeenCalled();
	});

	it('offers bounded planning from a cached server result without requiring a browser solve', () => {
		const { ctrl, app, live } = host();
		app.studyView = null;
		const activity = { subscribe: vi.fn() } as unknown as PlanningActivityStore;
		const workspace = {} as StudyWorkspace;
		const adapter = createTellegenWebMcpAdapter(ctrl, activity, workspace);
		expect(adapter.planning!.planningAvailable()).toBe(true);
		expect(ctrl.caseStudies.has(live)).toBe(false);
		expect(ctrl.ensureStudyInputJson).not.toHaveBeenCalled();
		expect(createTellegenWebMcpAdapter(ctrl, activity).planning!.planningAvailable()).toBe(false);
		ctrl.studyUnavailable.set(live, 'Browser solver is unavailable');
		expect(adapter.planning!.planningAvailable()).toBe(false);
	});

	it('queries a saved SOCWR voltage from its squared-voltage result', async () => {
		const { ctrl, live, saved } = host();
		saved.formulation = 'socwr';
		saved.solution = {
			formulation: 'socwr',
			status: 'optimal',
			w: [
				{ bus: 1, value: 0.9604 },
				{ bus: 2, value: 1.0201 }
			]
		};
		const result = await createTellegenWebMcpAdapter(ctrl).queryNetwork(
			{ caseId: live.id, elementKind: 'bus', sortBy: 'voltage_pu', direction: 'asc', limit: 1 },
			signal()
		);
		expect(result).toMatchObject({ elements: [{ legacy_id: 1, voltage_pu: 0.98 }] });
	});

	it('returns the current document revision after recording a saved-state observation', async () => {
		const { ctrl, saved } = host();
		const document = { id: saved.studyId, revision: 3 };
		const workspace = {
			document,
			captureCaseEvidence: () => ({}),
			recordCaseEvidence: async () => {
				document.revision++;
			}
		} as unknown as StudyWorkspace;
		const result = await createTellegenWebMcpAdapter(ctrl, undefined, workspace).inspectCase(
			signal()
		);
		expect(result).toMatchObject({
			study_revision: 4,
			revision: 'study-1:state-1',
			state_id: 'state-1'
		});
	});

	it('inspects and ranks the saved state, without solving or reading live edits', async () => {
		const { ctrl, live, saved } = host();
		const adapter = createTellegenWebMcpAdapter(ctrl);
		const inspected = await adapter.inspectCase(signal());
		expect(inspected).toMatchObject({
			case_id: live.id,
			state_id: saved.id,
			study_id: saved.studyId,
			revision: 'study-1:state-1',
			editable: false,
			solution: { objective: 250 }
		});
		const result = await adapter.queryNetwork(
			{
				caseId: live.id,
				elementKind: 'bus',
				sortBy: 'price',
				direction: 'desc',
				limit: 1
			},
			signal()
		);
		expect(result).toMatchObject({
			state_id: saved.id,
			units: { lmp: 'objective units/MW' },
			elements: [
				{
					element_id: 'bus:2',
					price: 7,
					demand_mw: 18,
					base_demand_mw: 20,
					editable: false
				}
			]
		});
		expect(ctrl.ensureStudyInputJson).not.toHaveBeenCalled();
		expect(live.deltas).toEqual({ 1: 9 });
	});

	it('focuses a saved bus without selecting or changing the live case', async () => {
		const { ctrl, app } = host();
		const adapter = createTellegenWebMcpAdapter(ctrl);
		await adapter.focusNetwork(
			{ caseId: 'case-a', target: { kind: 'bus', elementId: 'bus:2' } },
			signal()
		);
		expect(app.selectedBus).toBe(2);
		expect(app.studyView?.id).toBe('state-1');
		expect(app.requestFrame).toHaveBeenCalledWith({
			caseId: 'case-a',
			busId: 2
		});
		expect(ctrl.clearSelection).not.toHaveBeenCalled();
	});

	it('represents unsolved saved cases without borrowing the live solution', async () => {
		const { ctrl, app, saved } = host();
		app.studyView = { ...saved, solution: null };
		const adapter = createTellegenWebMcpAdapter(ctrl);
		expect(await adapter.inspectCase(signal())).toMatchObject({
			state_id: saved.id,
			solution: null
		});
		const result = await adapter.queryNetwork(
			{ caseId: 'case-a', elementKind: 'bus', direction: 'desc', limit: 1 },
			signal()
		);
		expect(result.elements).toMatchObject([{ price: null, demand_mw: 18 }]);
		await expect(
			Promise.resolve().then(() =>
				adapter.queryNetwork(
					{ caseId: 'case-a', elementKind: 'bus', sortBy: 'price', direction: 'desc', limit: 1 },
					signal()
				)
			)
		).rejects.toMatchObject({ code: 'METRIC_UNAVAILABLE' });
	});

	it('rejects live reset and edits while a saved state is displayed', async () => {
		const { ctrl, live } = host();
		const adapter = createTellegenWebMcpAdapter(ctrl);
		await expect(
			adapter.resetCase({ caseId: live.id, expectedRevision: 'study-1:state-1' }, signal())
		).rejects.toMatchObject({ code: 'SAVED_STATE_READ_ONLY' });
		await expect(
			adapter.updateCase(
				{
					caseId: live.id,
					expectedRevision: 'study-1:state-1',
					mode: 'increment',
					demand: [{ busId: '1', deltaMw: 5 }],
					ratings: []
				},
				signal()
			)
		).rejects.toMatchObject({ code: 'SAVED_STATE_READ_ONLY' });
		expect(live.deltas).toEqual({ 1: 9 });
	});

	it('selects the requested case through the controller and exits saved-state inspection', async () => {
		const { ctrl, app } = host();
		const adapter = createTellegenWebMcpAdapter(ctrl);
		const listed = await adapter.cases!.listCases({ offset: 0, limit: 1 }, signal());
		expect(listed).toMatchObject({
			cases: [{ case_id: 'case-a' }],
			next_offset: 1,
			total: 2
		});
		const next = await adapter.cases!.listCases({ offset: 1, limit: 1 }, signal());
		expect(next).toMatchObject({
			cases: [{ case_id: 'case-b' }],
			next_offset: null
		});
		expect(
			await adapter.cases!.selectCase(
				{ caseId: 'case-b', expectedRevision: String(listed.revision) },
				signal()
			)
		).toMatchObject({ case_id: 'case-b', selected: true, state_id: null });
		expect(ctrl.activateCase).toHaveBeenCalledWith('case-b');
		expect(app.studyView).toBeNull();
	});

	it('keeps unavailable configured cases in the catalogue and rejects selection', async () => {
		const { ctrl, app } = host();
		Object.assign(app.cases[1], { network: null, unavailableReason: 'Case data is missing' });
		const adapter = createTellegenWebMcpAdapter(ctrl);
		const listed = await adapter.cases!.listCases({ offset: 1, limit: 1 }, signal());
		expect(listed).toMatchObject({
			total: 2,
			cases: [{ case_id: 'case-b', availability: 'unavailable', reason: 'Case data is missing' }]
		});
		await expect(adapter.cases!.selectCase({ caseId: 'case-b' }, signal())).rejects.toMatchObject({
			code: 'CASE_UNAVAILABLE',
			message: 'Case data is missing'
		});
		expect(ctrl.activateCase).not.toHaveBeenCalled();
		expect(app.studyView?.id).toBe('state-1');
	});

	it('rejects cancelled, stale, and unknown case selections before changing the view', async () => {
		const { ctrl, app } = host();
		const adapter = createTellegenWebMcpAdapter(ctrl);
		await expect(
			adapter.cases!.selectCase({ caseId: 'case-b', expectedRevision: 'old' }, signal())
		).rejects.toMatchObject({ code: 'STALE_REVISION' });
		await expect(adapter.cases!.selectCase({ caseId: 'missing' }, signal())).rejects.toMatchObject({
			code: 'CASE_NOT_FOUND'
		});
		const abort = new AbortController();
		abort.abort();
		await expect(
			adapter.cases!.selectCase({ caseId: 'case-b' }, abort.signal)
		).rejects.toMatchObject({ name: 'AbortError' });
		expect(ctrl.activateCase).not.toHaveBeenCalled();
		expect(app.studyView?.id).toBe('state-1');
	});

	it('reports distribution cases as display-only and never invents prices', async () => {
		const { ctrl, app } = host();
		const multi = {
			id: 'dist-1',
			label: 'Feeder',
			placed: true,
			graph: {
				buses: [{ id: 'source', terminals: ['a'], load_kw: 1200, gen_kw: 3000 }],
				edges: []
			},
			view: { buses: [], edges: [] }
		};
		Object.assign(app, {
			studyView: null,
			activeCaseId: null,
			activeMultiId: multi.id,
			activeMulti: multi,
			multiCases: [multi]
		});
		const adapter = createTellegenWebMcpAdapter(ctrl);
		expect(await adapter.cases!.listCases({ offset: 0, limit: 10 }, signal())).toMatchObject({
			cases: expect.arrayContaining([
				{
					case_id: 'dist-1',
					name: 'Feeder',
					kind: 'distribution',
					availability: 'ready',
					calculation: 'display_only',
					selected: true
				}
			])
		});
		expect(await adapter.inspectCase(signal())).toMatchObject({
			case_id: multi.id,
			editable: false,
			calculation: 'display_only'
		});
		expect(
			await adapter.queryNetwork(
				{ caseId: multi.id, elementKind: 'bus', direction: 'desc', limit: 1 },
				signal()
			)
		).toMatchObject({ elements: [{ element_id: 'source', demand_mw: 1.2 }] });
		await expect(
			Promise.resolve().then(() =>
				adapter.queryNetwork(
					{
						caseId: multi.id,
						elementKind: 'bus',
						sortBy: 'price',
						direction: 'desc',
						limit: 1
					},
					signal()
				)
			)
		).rejects.toMatchObject({ code: 'METRIC_UNAVAILABLE' });
	});

	it('checks queued selection cancellation immediately before dispatch', async () => {
		const { ctrl, app } = host();
		let release!: () => void;
		vi.mocked(ctrl.activateCase).mockImplementationOnce(async (id) => {
			app.studyView = null;
			app.activeCaseId = id;
			await new Promise<void>((resolve) => {
				release = resolve;
			});
		});
		const adapter = createTellegenWebMcpAdapter(ctrl);
		const first = adapter.cases!.selectCase({ caseId: 'case-b' }, signal());
		await Promise.resolve();
		const abort = new AbortController();
		const second = adapter.cases!.selectCase({ caseId: 'case-a' }, abort.signal);
		abort.abort();
		release();
		await first;
		await expect(second).rejects.toMatchObject({ name: 'AbortError' });
		expect(ctrl.activateCase).toHaveBeenCalledTimes(1);
		expect(app.activeCaseId).toBe('case-b');
	});
});
