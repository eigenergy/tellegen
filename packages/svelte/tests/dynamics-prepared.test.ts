import { describe, expect, it, vi } from 'vitest';
import type { NetworkBranch, RmsCaseOptions } from '@tellegen/engine';

// The RMS worker is not available here: a prepared case reports the options
// it was opened with, so the panel's description can be checked against them.
const opened: { options: RmsCaseOptions; module: string }[] = [];
vi.mock('@tellegen/engine', async (orig) => ({
	...(await orig<typeof import('@tellegen/engine')>()),
	openRmsCase: vi.fn(async (module: string, _dyr: string, options: RmsCaseOptions) => {
		opened.push({ options, module });
		const plants = options.renewable_plants === false;
		return {
			options,
			inspection: {
				options: { reactive_limits: options.reactive_limits ?? true },
				plants: plants ? [] : ['p1', 'p2'],
				held_units: plants ? ['5#W1', '6#W1'] : [],
				branches: [[1101, 1401, '1']],
				generators: [],
				sources: [],
				notes: [],
				dyr_records: 1,
				base_frequency: 60
			},
			flatCheck: vi.fn(async () => ({
				seconds: 2,
				max_speed_drift: 1e-9,
				max_voltage_drift: 1e-9,
				steps: 400
			})),
			run: vi.fn(async () => ({ time: new Float64Array([0]) })),
			free: vi.fn()
		};
	}),
	ingestCase: vi.fn(async () => ({ module_json: 'validation-module' }))
}));

const { AppState } = await import('../src/lib/state.svelte.js');
const { DynamicsController } = await import('../src/lib/dynamics/controller.svelte.js');
const { branchCircuits, openBranches, openBranchText } =
	await import('../src/lib/dynamics/layers.js');
const { branchTrip, lineFault } = await import('../src/lib/dynamics/events.js');

const row = (id: number, from: number, to: number, status = 1): NetworkBranch => ({
	id,
	from,
	to,
	rate_mw: 0,
	status,
	path: [
		[0, 0],
		[1, 1]
	]
});

describe('open circuits on the map', () => {
	// WECC 240 has two circuits between 1101 and 1401; the validated trip
	// opens circuit 1 only.
	const branches = [row(1, 1101, 1401), row(2, 1101, 1401), row(3, 1401, 1500)];
	const keys: [number, number, string][] = [
		[1101, 1401, '1 '],
		[1101, 1401, '2 '],
		[1401, 1500, '1']
	];

	it('match each view row to its circuit, in row order', () => {
		expect(branchCircuits(branches, keys)).toEqual(['1', '2', '1']);
		// Rows listed only when in service still match.
		const withOpen = [...branches, row(4, 1401, 1500, 0)];
		expect(branchCircuits(withOpen, keys)).toEqual(['1', '2', '1', null]);
		// Disagreeing counts are not guessed.
		expect(branchCircuits(branches, [[1101, 1401, '1']])).toEqual([null, null, null]);
	});

	it('open only the tripped circuit and say the parallel one still carries flow', () => {
		const circuits = branchCircuits(branches, keys);
		const trip = branchTrip(1101, 1401, '1');
		expect(openBranches(branches, circuits, [trip], 0.5)).toEqual([]);
		const open = openBranches(branches, circuits, [trip], 1.5);
		expect(open).toHaveLength(1);
		expect(open[0]).toMatchObject({ circuit: '1', parallel: 2, live: 1 });
		expect(openBranchText(open[0])).toBe(
			'Branch 1101–1401 circuit 1 open; 1 of 2 parallel circuits still in service'
		);
		// A cleared line fault on circuit 2, too: the corridor is open.
		const fault = {
			...lineFault(1401, 1101, '2', 1401, 60),
			at: 1,
			clearAfter: 0.1
		};
		const both = openBranches(branches, circuits, [trip, fault], 1.2);
		expect(both.map((b) => [b.circuit, b.live])).toEqual([
			['1', 0],
			['2', 0]
		]);
	});

	it('with unknown circuits, opens the pair and says the circuits are drawn together', () => {
		const open = openBranches(branches, [null, null, null], [branchTrip(1101, 1401, '1')], 2);
		expect(open).toHaveLength(2);
		expect(openBranchText(open[0])).toMatch(/drawn together/);
	});
});

describe('the prepared case the panel describes', () => {
	function setup() {
		opened.length = 0;
		const app = new AppState();
		const dyn = new DynamicsController(app, async () => {});
		const local = {
			id: 'local-1',
			label: 'WECC',
			studyInputJson: 'case-module'
		};
		vi.spyOn(app, 'activeLocal', 'get').mockReturnValue(local as never);
		return { dyn, local };
	}
	const settle = () => new Promise((r) => setTimeout(r, 0));

	it('follows the options: held plants and reactive limits are those of the next run', async () => {
		const { dyn, local } = setup();
		await dyn.attach(local as never, 'wecc.dyr', 'dyr', true);
		expect(dyn.active?.inspection?.plants).toHaveLength(2);
		expect(dyn.active?.flat?.max_speed_drift).toBe(1e-9);
		dyn.setOptions({ ...dyn.options, renewable_plants: false });
		await settle();
		await settle();
		expect(dyn.active?.inspection?.held_units).toEqual(['5#W1', '6#W1']);
		expect(dyn.active?.preparing).toBe(false);
		expect(dyn.active?.flat).not.toBeNull();
	});

	it('switches to the validation network with a validated scenario, and back', async () => {
		const { dyn, local } = setup();
		await dyn.attach(local as never, 'wecc.dyr', 'dyr', true);
		vi.stubGlobal(
			'fetch',
			vi.fn(async () => new Response(new Uint8Array([1])))
		);
		dyn.usePreset('trip-1101-1401');
		for (let i = 0; i < 5; i++) await settle();
		expect(dyn.active?.network).toBe('validation');
		expect(dyn.active?.inspection?.options?.reactive_limits).toBe(false);
		expect(opened.at(-1)?.module).toBe('validation-module');
		dyn.clearEvents();
		for (let i = 0; i < 5; i++) await settle();
		expect(dyn.active?.network).toBe('case');
		expect(dyn.active?.inspection?.options?.reactive_limits).toBe(true);
		vi.unstubAllGlobals();
	});
});
