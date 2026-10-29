import { describe, expect, it, vi } from 'vitest';
import type { DistGraph, McPfResult } from '@tellegen/engine';
import type { MultiCaseSummary } from '../src/lib/state.svelte.js';

const built = vi.hoisted(() => ({ inputs: 0, snapshots: 0 }));
vi.mock('../src/lib/diagram/graph-inputs.js', async (load) => {
	const actual = await load<typeof import('../src/lib/diagram/graph-inputs.js')>();
	return {
		...actual,
		graphInputs: (graph: DistGraph) => (built.inputs++, actual.graphInputs(graph))
	};
});
vi.mock('../src/lib/diagram/snapshot.js', async (load) => {
	const actual = await load<typeof import('../src/lib/diagram/snapshot.js')>();
	return {
		...actual,
		snapshotFromMcPf: (...args: Parameters<typeof actual.snapshotFromMcPf>) => (
			built.snapshots++,
			actual.snapshotFromMcPf(...args)
		)
	};
});
const { MulticonductorCase } = await import('../src/lib/state.svelte.js');
const { search } = await import('../src/lib/diagram/search.js');

const graph = (min: number): DistGraph => ({
	buses: [
		{
			id: 'B',
			terminals: ['a', 'n'],
			grounded: [],
			neutral_terminal: 'n',
			load_kw: 0,
			gen_kw: 0,
			has_source: false,
			voltage_limits: { vpn_min: [min], vpn_max: [253] }
		}
	],
	edges: []
});
const result = (volts: number) =>
	({
		iterations: 1,
		terminals: [
			{ bus: 'B', terminal: 'a', voltage: { re: volts, im: 0 } },
			{ bus: 'B', terminal: 'n', voltage: { re: 0, im: 0 } }
		],
		element_ports: []
	}) as unknown as McPfResult;

// The overview and the diagram read these from the case, so in the browser each is computed once
// per change of what it derives from. These tests run on Svelte's server runtime, where a derived
// recomputes on every read, so they check what each derives from rather than how often.
describe('the overview a multiconductor case shares between its views', () => {
	it('builds nothing until a view reads it', () => {
		const c = new MulticonductorCase({
			id: 'mc',
			label: 'Feeder',
			fileName: 'feeder.json',
			summary: { name: 'feeder' } as MultiCaseSummary,
			graph: graph(207),
			coordsKind: 'planar'
		});
		c.overviewResult = result(230);
		expect(built).toEqual({ inputs: 0, snapshots: 0 });
		expect(c.overviewSnapshot?.buses[0].pairs[0].volts).toBe(230);
		expect(built.snapshots).toBeGreaterThan(0);
	});

	it('follows the latest operating point and graph, with no stale copy', () => {
		const c = new MulticonductorCase({
			id: 'mc',
			label: 'Feeder',
			fileName: 'feeder.json',
			summary: { name: 'feeder' } as MultiCaseSummary,
			graph: graph(207),
			coordsKind: 'planar'
		});
		expect(c.overviewSnapshot).toBeNull();
		expect(c.overviewInputs?.buses[0].limits).toEqual({ phaseNeutral: { min: [207], max: [253] } });
		expect(search(c.overviewSearch!, 'B').matches.map((m) => m.id)).toEqual(['B']);

		c.overviewResult = result(230);
		expect(c.overviewSnapshot?.buses[0].pairs[0]).toMatchObject({ volts: 230, min: 207 });
		c.overviewResult = result(220);
		expect(c.overviewSnapshot?.buses[0].pairs[0]).toMatchObject({ volts: 220, min: 207 });

		// A replaced graph (coordinates attached) carries its own limits into the snapshot.
		c.graph = graph(210);
		expect(c.overviewInputs?.buses[0].limits).toEqual({ phaseNeutral: { min: [210], max: [253] } });
		expect(c.overviewSnapshot?.buses[0].pairs[0]).toMatchObject({ volts: 220, min: 210 });

		c.overviewResult = null;
		expect(c.overviewSnapshot).toBeNull();
		c.graph = null;
		expect([c.overviewInputs, c.overviewSearch]).toEqual([null, null]);
	});
});
