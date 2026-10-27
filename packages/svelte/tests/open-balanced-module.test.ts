import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { AppliedGeoCase, IngestedCase } from '@tellegen/engine';

const engine = vi.hoisted(() => ({
	ingestJsonDrop: vi.fn(),
	applyGeo: vi.fn(),
	createStudy: vi.fn(async () => {
		throw new Error('no wasm in this test');
	})
}));
vi.mock('@tellegen/engine', async (importOriginal) => {
	const actual = await importOriginal<typeof import('@tellegen/engine')>();
	return {
		...actual,
		ingestJsonDrop: engine.ingestJsonDrop,
		applyGeo: engine.applyGeo,
		createStudy: engine.createStudy
	};
});

import { AppState } from '../src/lib/state.svelte.js';
import { Controller } from '../src/lib/controller.svelte.js';

/** A three-bus balanced ingest payload without coordinates. */
function payload(overrides: Partial<IngestedCase> = {}): IngestedCase {
	return {
		name: 'case',
		base_mva: 100,
		n_bus: 3,
		n_branch: 2,
		n_gen: 1,
		load_mw: 90,
		gen_mw: 90,
		has_coords: false,
		coords_kind: 'synthetic_pending',
		diagnostics: [],
		warnings: [],
		module_json: '{"module":"stub"}',
		topology: {
			buses: [1, 2, 3].map((id) => ({
				id,
				uid: null,
				demand_mw: id === 2 ? 90 : 0,
				gen_mw: id === 1 ? 90 : 0
			})),
			branches: [
				{ id: 0, uid: null, from: 1, to: 2, rate_mw: 100 },
				{ id: 1, uid: null, from: 2, to: 3, rate_mw: 100 }
			]
		},
		view: null,
		...overrides
	};
}

/** The same case with geographic coordinates. */
const geoView = {
	coordinate_space: 'geographic' as const,
	buses: [1, 2, 3].map((id) => ({
		id,
		lon: -81 + id * 0.1,
		lat: 34,
		demand_mw: id === 2 ? 90 : 0,
		gen_mw: id === 1 ? 90 : 0
	})),
	branches: [
		{
			id: 0,
			from: 1,
			to: 2,
			rate_mw: 100,
			status: 1,
			path: [
				[-80.9, 34],
				[-80.8, 34]
			]
		},
		{
			id: 1,
			from: 2,
			to: 3,
			rate_mw: 100,
			status: 1,
			path: [
				[-80.8, 34],
				[-80.7, 34]
			]
		}
	]
} as unknown as NonNullable<IngestedCase['view']>;

function host() {
	const app = new AppState();
	const ctrl = new Controller(app, { apiBase: null, loadDefaultCases: false });
	const frame = vi.spyOn(app, 'requestFrame').mockResolvedValue();
	const solve = vi.spyOn(ctrl, 'runSolve').mockImplementation(() => {});
	return { app, ctrl, frame, solve };
}

beforeEach(() => {
	engine.ingestJsonDrop.mockReset();
	engine.applyGeo.mockReset();
});

describe('openBalancedModule', () => {
	it('opens an ingested payload as the active local case, laid out as a diagram', async () => {
		const { app, ctrl, frame, solve } = host();
		const local = await ctrl.openBalancedModule({ name: 'synth', payload: payload() });
		expect(app.localCases).toEqual([local]);
		expect(app.activeLocalId).toBe(local.id);
		expect(local.label).toBe('synth');
		expect(local.fileName).toBe('synth.pio.json');
		expect(local.studyInputJson).toBe('{"module":"stub"}');
		expect(local.view).toBeNull();
		expect(local.diagram?.view.buses).toHaveLength(3);
		expect(local.displayMode).toBe('diagram');
		expect(local.network?.buses).toHaveLength(3);
		expect(frame).toHaveBeenCalledWith(local.id);
		expect(solve).toHaveBeenCalledWith(local, null);
		expect(engine.ingestJsonDrop).not.toHaveBeenCalled();
		expect(app.parsingFile).toBe(false);
	});

	it('ingests module text through the engine when no payload is given', async () => {
		const { app, ctrl } = host();
		engine.ingestJsonDrop.mockResolvedValue({
			kind: 'module',
			payload: payload({ name: 'ieee3' })
		});
		const local = await ctrl.openBalancedModule({ name: 'from-text', moduleJson: '{"pio":1}' });
		expect(engine.ingestJsonDrop).toHaveBeenCalledTimes(1);
		const [bytes] = engine.ingestJsonDrop.mock.calls[0] as [Uint8Array];
		expect(new TextDecoder().decode(bytes)).toBe('{"pio":1}');
		expect(app.activeLocalId).toBe(local.id);
		expect(local.label).toBe('from-text');
	});

	it('accepts a transmission payload and rejects anything else', async () => {
		const { ctrl, app } = host();
		engine.ingestJsonDrop.mockResolvedValueOnce({ kind: 'transmission', payload: payload() });
		await expect(ctrl.openBalancedModule({ name: 'tx', moduleJson: '{}' })).resolves.toBeDefined();
		engine.ingestJsonDrop.mockResolvedValueOnce({
			kind: 'distribution',
			payload: { model: 'multiconductor' }
		});
		await expect(ctrl.openBalancedModule({ name: 'dist', moduleJson: '{}' })).rejects.toThrow(
			/not a balanced network module \(distribution\)/
		);
		await expect(ctrl.openBalancedModule({ name: 'empty' })).rejects.toThrow(
			/needs moduleJson or payload/
		);
		expect(app.localCases).toHaveLength(1);
		expect(app.parsingFile).toBe(false);
	});

	it('keeps geographic coordinates the module carries', async () => {
		const { ctrl } = host();
		const local = await ctrl.openBalancedModule({
			name: 'geo',
			payload: payload({ has_coords: true, coords_kind: 'file', view: geoView })
		});
		expect(local.view).toBe(geoView);
		expect(local.diagram).toBeNull();
		expect(local.displayMode).toBe('geographic');
		expect(local.coordsKind).toBe('file');
	});

	it('applies a geographic layer before the case activates', async () => {
		const { ctrl, solve } = host();
		const applied: AppliedGeoCase = {
			...payload({ has_coords: true, coords_kind: 'file', module_json: 'with-geo', view: geoView }),
			report: { matched_buses: 3, matched_branches: 2, unmatched_features: 0, notes: [] }
		};
		engine.applyGeo.mockResolvedValue(applied);
		const layer = JSON.stringify({ type: 'FeatureCollection', features: [] });
		const local = await ctrl.openBalancedModule({
			name: 'placed',
			payload: payload(),
			geoLayer: layer
		});
		expect(engine.applyGeo).toHaveBeenCalledWith('{"module":"stub"}', layer);
		expect(local.studyInputJson).toBe('with-geo');
		expect(local.view).toBe(geoView);
		expect(local.coordsKind).toBe('geofile');
		expect(local.geoSource).toBe('placed.geo.json');
		expect(local.diagram).toBeNull();
		expect(solve).toHaveBeenCalledWith(local, null);
	});

	it('returns a prepared case without adding it when activate is false', async () => {
		const { app, ctrl, frame, solve } = host();
		const local = await ctrl.openBalancedModule({
			name: 'later',
			payload: payload(),
			activate: false
		});
		expect(local.diagram).not.toBeNull();
		expect(app.localCases).toEqual([]);
		expect(app.activeLocalId).toBeNull();
		expect(frame).not.toHaveBeenCalled();
		expect(solve).not.toHaveBeenCalled();
		ctrl.addAndActivateLocal(local);
		expect(app.activeLocalId).toBe(local.id);
	});

	it('activates without flying the map when frame is false', async () => {
		const { app, ctrl, frame, solve } = host();
		const local = await ctrl.openBalancedModule({
			name: 'still',
			payload: payload(),
			frame: false
		});
		expect(app.activeLocalId).toBe(local.id);
		expect(frame).not.toHaveBeenCalled();
		expect(solve).toHaveBeenCalledWith(local, null);
	});

	it('honours a formulation override', async () => {
		const { app, ctrl } = host();
		const local = await ctrl.openBalancedModule({
			name: 'ac',
			payload: payload({ formulation: 'dcopf' }),
			formulation: 'acpf'
		});
		expect(local.formulation).toBe('acpf');
		expect(local.declaredFormulation).toBe('dcopf');
		expect(app.displayMode).toBe('voltage');
	});
});

describe('replaceLocalCase', () => {
	it('swaps the active case in place, keeping formulation, display mode and selection', async () => {
		const { app, ctrl, solve } = host();
		const a = await ctrl.openBalancedModule({ name: 'a', payload: payload() });
		const c = await ctrl.openBalancedModule({ name: 'c', payload: payload() });
		ctrl.activateLocal(a);
		a.formulation = 'socwr';
		app.selectedBus = 2;
		const select = vi.spyOn(ctrl, 'selectLocalBus').mockResolvedValue();
		const dispose = vi.spyOn(ctrl, 'disposeStudy');
		solve.mockClear();

		const b = await ctrl.replaceLocalCase(a, { name: 'b', payload: payload() });
		expect(app.localCases.map((x) => x.id)).toEqual([b.id, c.id]);
		expect(app.activeLocalId).toBe(b.id);
		expect(b.formulation).toBe('socwr');
		expect(b.displayMode).toBe('diagram');
		expect(dispose).toHaveBeenCalledWith(a);
		expect(solve).toHaveBeenCalledWith(b, null);
		expect(select).toHaveBeenCalledWith(b.id, 2);
	});

	it('drops a selection the new case cannot show', async () => {
		const { app, ctrl } = host();
		const a = await ctrl.openBalancedModule({ name: 'a', payload: payload() });
		app.selectedBus = 7;
		const select = vi.spyOn(ctrl, 'selectLocalBus').mockResolvedValue();
		await ctrl.replaceLocalCase(a, { name: 'b', payload: payload() });
		expect(select).not.toHaveBeenCalled();
		expect(app.selectedBus).toBeNull();
	});

	it('replaces an inactive case without changing the active one', async () => {
		const { app, ctrl, frame, solve } = host();
		const a = await ctrl.openBalancedModule({ name: 'a', payload: payload() });
		const c = await ctrl.openBalancedModule({ name: 'c', payload: payload() });
		frame.mockClear();
		solve.mockClear();
		const b = await ctrl.replaceLocalCase(a, { name: 'b', payload: payload() });
		expect(app.localCases.map((x) => x.id)).toEqual([b.id, c.id]);
		expect(app.activeLocalId).toBe(c.id);
		expect(frame).not.toHaveBeenCalled();
		expect(solve).toHaveBeenCalledWith(b, null);
	});
});
