import { describe, expect, it } from 'vitest';
import type { DistGraph } from '@tellegen/engine';
import {
	displayTree,
	diagramLayer,
	fromEngineLayout,
	multiWireBranch,
	sourceLayout,
	treePath,
	type PairVoltage,
	type ResultSnapshot
} from '../src/lib/diagram/index.js';

const bus = (id: string, source = false) => ({
	id,
	terminals: ['1', '2', '3', 'n'],
	grounded: [],
	load_kw: 0,
	gen_kw: 0,
	has_source: source
});
const edge = (
	id: string,
	from: string,
	to: string,
	kind: 'line' | 'switch' | 'transformer' = 'line',
	closed = true
) => ({
	kind,
	id,
	from,
	to,
	conductors: [] as [string, string][],
	closed,
	n_phases: 3
});
// MV source S → M → transformer → L1 → L2; an open switch to X; island Y.
const graph: DistGraph = {
	buses: [bus('S', true), bus('M'), bus('L1'), bus('L2'), bus('X'), bus('Y')],
	edges: [
		edge('e1', 'S', 'M'),
		edge('t1', 'M', 'L1', 'transformer'),
		edge('e2', 'l1', 'L2'),
		edge('sw', 'L2', 'X', 'switch', false)
	]
};
const pp = (v: number): PairVoltage => ({
	pair: ['1', '2'],
	zone: 'phase–phase',
	volts: v,
	min: 19000,
	max: 21000
});
const pn = (v: number): PairVoltage => ({
	pair: ['1', 'n'],
	zone: 'phase–neutral',
	volts: v,
	min: 207,
	max: 253
});
const result: ResultSnapshot = {
	kind: 'solved',
	label: 't',
	iterations: 1,
	buses: [[pp(20500)], [pp(20400)], [pn(236)], [pn(231)], [pn(230)], []].map((pairs) => ({
		pairs,
		unbalance: null
	})),
	branches: []
};

describe('displayTree', () => {
	it('roots at sources, matches ids case-insensitively and does not cross open switches', () => {
		const tree = displayTree(graph);
		expect(tree.roots).toEqual([0, 4, 5]);
		expect([...tree.depth]).toEqual([0, 1, 2, 3, 0, 0]);
		expect([...tree.parent]).toEqual([0, 0, 1, 2, 4, 5]);
		expect([...tree.component]).toEqual([0, 0, 0, 0, 1, 2]);
		expect(treePath(tree, 3)).toEqual({ buses: [0, 1, 2, 3], edges: [0, 1, 2] });
	});
});

describe('schematic layouts', () => {
	it('converts the engine layout into GPU arrays without reinterpreting it', () => {
		const g = fromEngineLayout({
			version: 1,
			strategy: 'tidy',
			positions: [0, 0, 100, 20],
			segments: [0, 0, 50, 0, 50, 0, 50, 20, 50, 20, 100, 20],
			segment_edge: [0, 0, 0],
			role: [0],
			parent: [0, 0],
			parent_edge: [-1, 0],
			depth: [0, 1],
			roots: [0],
			bounds: [0, 0, 100, 20]
		});
		expect([...g.positions]).toEqual([0, 0, 0, 100, 20, 0]);
		expect([...g.sources]).toEqual([0, 0, 0, 50, 0, 0, 50, 20, 0]);
		expect([...g.targets]).toEqual([50, 0, 0, 50, 20, 0, 100, 20, 0]);
		expect([...g.segmentEdge]).toEqual([0, 0, 0]);
		expect(g.bounds).toEqual([0, 0, 100, 20]);
	});

	it('follows routed paths from the case and keeps unrouted branches straight', () => {
		const tree = displayTree(graph);
		const placed = graph.buses.map((b, i) => ({ id: b.id, lon: i * 10, lat: 0 }));
		const routed = sourceLayout(graph, placed, false, tree, [
			{
				id: 'e1',
				from: 'M',
				to: 'S',
				path: [
					[10, 0],
					[5, 5],
					[0, 0]
				]
			}
		]);
		expect([...routed.segmentEdge].filter((k) => k === 0)).toHaveLength(2);
		expect([...routed.segmentEdge].filter((k) => k === 1)).toHaveLength(1);
	});

	it('writes the drawn schematic as a PowerIO diagram layer, routing lines by id', () => {
		const geometry = fromEngineLayout({
			version: 1,
			strategy: 'tidy',
			positions: [0, 0, 120, 0, 240, 0, 360, 0, 0, 56, 0, 112],
			segments: [0, 0, 120, 0, 120, 0, 180, 0, 180, 0, 240, 0, 240, 0, 360, 0, 360, 0, 0, 56],
			segment_edge: [0, 1, 2, 2, 3],
			role: [0, 0, 0, 2],
			parent: [0, 0, 1, 2, 4, 5],
			parent_edge: [-1, 0, 1, 2, -1, -1],
			depth: [0, 1, 2, 3, 0, 0],
			roots: [0, 4, 5],
			bounds: [0, 0, 360, 112]
		});
		const layer = JSON.parse(diagramLayer(graph, geometry, { strategy: 'tidy', version: 1 }));
		expect(layer.powerio_geo).toEqual({ space: 'diagram', kind: 'synthetic' });
		expect(layer.tellegen_layout).toEqual({ algorithm: 'schematic', strategy: 'tidy', version: 1 });
		const points = layer.features.filter(
			(f: { geometry: { type: string } }) => f.geometry.type === 'Point'
		);
		expect(points).toHaveLength(6);
		expect(points[3]).toMatchObject({
			properties: { bus: 'L2' },
			geometry: { coordinates: [360, 0] }
		});
		const lines = layer.features.filter(
			(f: { geometry: { type: string } }) => f.geometry.type === 'LineString'
		);
		expect(lines.map((f: { properties: { id: string } }) => f.properties.id)).toEqual(['e1', 'e2']);
		expect(lines[1].geometry.coordinates).toEqual([
			[180, 0],
			[240, 0],
			[360, 0]
		]);
	});

	it('draws a bus without a position next to a connected bus, not at the origin', () => {
		// S —l1→ M (both placed); M —t→ L has no position, as an LV stub behind a pole transformer.
		const g: DistGraph = {
			buses: [bus('S', true), bus('M'), bus('L')],
			edges: [edge('l1', 'S', 'M'), edge('t', 'M', 'L', 'transformer')]
		};
		const placed = [
			{ id: 'S', lon: 0, lat: 0 },
			{ id: 'M', lon: 1000, lat: 0 }
		];
		const geo = sourceLayout(g, placed, false, displayTree(g));
		expect(Array.from(geo.inferred ?? [])).toEqual([0, 0, 1]);
		const d = Math.hypot(geo.positions[6] - geo.positions[3], geo.positions[7] - geo.positions[4]);
		// Half a typical branch (the one placed branch is 1000 long) from M.
		expect(d).toBeCloseTo(500, 3);
		// A fully placed case carries no inferred flags.
		expect(
			sourceLayout(g, [...placed, { id: 'L', lon: 1000, lat: 50 }], false, displayTree(g)).inferred
		).toBeUndefined();
	});

	it('projects geographic positions to Web Mercator, y down, relative to the centroid', () => {
		const tree = displayTree(graph);
		const placed = graph.buses.map((b, i) => ({ id: b.id, lon: 150 + i * 1e-5, lat: -30 }));
		const geo = sourceLayout(graph, placed, true, tree);
		// One metre apart near longitude 150 stays one metre apart: no float32 collapse at 1.6e7 m.
		const metre = ((6_378_137 * Math.PI) / 180) * 1e-5;
		expect(geo.positions[3] - geo.positions[0]).toBeCloseTo(metre, 2);
		expect(Math.abs(geo.positions[0])).toBeLessThan(10);
		const north = sourceLayout(
			graph,
			[
				{ id: 'S', lon: 0, lat: 1 },
				{ id: 'M', lon: 0, lat: 0 }
			],
			true,
			tree
		);
		expect(north.positions[1]).toBeLessThan(north.positions[4]);
		const planar = sourceLayout(
			graph,
			graph.buses.map((b, i) => ({ id: b.id, lon: i, lat: i })),
			false,
			tree
		);
		expect(planar.positions[3] - planar.positions[0]).toBe(1);
		// Drawing coordinates are already y down, as DiagramCanvas draws them.
		expect(planar.positions[4] - planar.positions[1]).toBe(1);
		expect(planar.segmentEdge).toHaveLength(graph.edges.length);
	});
});

describe('multi-wire view', () => {
	const wired: DistGraph = {
		buses: [
			{
				...bus('A', true),
				grounded: ['n'],
				terminal_attachments: { '1': [{ kind: 'source', id: 'vs' }] }
			},
			{ ...bus('B'), terminal_attachments: { '3': [{ kind: 'load', id: 'ld3' }] } }
		],
		edges: [
			{
				...edge('L', 'A', 'B'),
				conductors: [
					['1', '2'],
					['2', '1'],
					['3', '3'],
					['n', 'n']
				]
			},
			{
				...edge('T', 'a', 'B', 'transformer'),
				conductors: [
					['1', '1'],
					['2', '2']
				]
			}
		]
	};
	const snapshot: ResultSnapshot = {
		...result,
		buses: [],
		branches: [
			{
				ends: [
					{
						p: 1,
						q: 0,
						currents: [
							{ terminal: '1', amps: 90 },
							{ terminal: 'n', amps: 12 }
						]
					},
					null
				],
				loading: null,
				basis: null,
				primary: true,
				loss: null
			},
			{ ends: [null, null], loading: null, basis: null, primary: true, loss: null }
		]
	};

	it('draws every conductor between terminal rails, crossing where the terminal map crosses', () => {
		const view = multiWireBranch(wired, 0, snapshot)!;
		expect(view.buses.map((b) => b.id)).toEqual(['A', 'B']);
		expect(view.buses[0].terminals.find((t) => t.name === 'n')?.grounded).toBe(true);
		expect(view.buses[1].terminals.find((t) => t.name === '3')?.attachments).toEqual(['load ld3']);
		expect(view.wires).toHaveLength(4);
		const [w1, w2] = view.wires;
		expect(w1.y1).toBeLessThan(w1.y2);
		expect(w2.y1).toBeGreaterThan(w2.y2);
		expect(view.wires.map((w) => w.amps)).toEqual([90, null, null, 12]);
		expect(view.winding).toBeNull();
	});

	it('draws a transformer as a winding box with no conductor continuity across it', () => {
		const view = multiWireBranch(wired, 1, null)!;
		expect(view.winding).not.toBeNull();
		expect(view.wires).toHaveLength(4);
		for (const w of view.wires)
			expect(w.x2 <= view.winding!.x || w.x1 >= view.winding!.x + view.winding!.width).toBe(true);
		expect(multiWireBranch(wired, 9, null)).toBeNull();
	});
});
