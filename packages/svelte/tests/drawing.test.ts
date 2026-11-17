import { describe, expect, it, vi } from 'vitest';
import {
	createDrawing,
	restoreDrawing,
	arrangeDrawing,
	drawingExtent,
	drawingBounds,
	drawingNetworkKey,
	drawingPaths,
	parseDrawing,
	saveDrawing,
	savedDrawings
} from '../src/lib/diagram/drawing.js';
import type { Network } from '../src/lib/api.js';
const network = {
	case: 'test',
	buses: [
		{ id: 1, name: 'A', lon: 0, lat: 0, demand_mw: 0, gen_mw: 0, base_kv: 110 },
		{ id: 2, name: 'B', lon: 1, lat: 1, demand_mw: 1, gen_mw: 0, base_kv: 110 }
	],
	branches: [
		{
			id: 8,
			from: 1,
			to: 2,
			status: 1,
			rate_mw: 10,
			path: [
				[0, 0],
				[1, 1]
			]
		}
	]
} as Network;
describe('persistent engineering drawings', () => {
	it('round-trips manual positions, ports, routes, labels and visibility without changing electrical data', () => {
		const before = JSON.stringify(network);
		const d = createDrawing(network, 'Manual');
		d.objects[0].x = 500;
		d.objects[0].rotation = 90;
		d.objects[0].width = 200;
		d.objects[0].ports[0].x = -100;
		d.objects[0].labelOffset = [10, 25];
		d.objects[0].locked = true;
		d.connections[0].bends = [
			[100, 200],
			[150, 200]
		];
		d.layers[0].visible = false;
		const next = parseDrawing(JSON.stringify(d), drawingNetworkKey(network));
		expect(next).toEqual(d);
		expect(drawingPaths(next)[0].points.slice(1, -1)).toEqual([
			[100, 200],
			[150, 200]
		]);
		expect(JSON.stringify(network)).toBe(before);
	});
	it('keeps independent drawings for one network and rejects a different network', () => {
		const values = new Map<string, string>();
		vi.stubGlobal('localStorage', {
			getItem: (key: string) => values.get(key) ?? null,
			setItem: (key: string, value: string) => values.set(key, value)
		});
		const a = createDrawing(network, 'Overview'),
			b = createDrawing(network, 'Station');
		b.objects[0].x = 900;
		saveDrawing(a);
		saveDrawing(b);
		expect(savedDrawings(a.network).map((d) => d.objects[0].x)).toEqual([100, 900]);
		expect(() => parseDrawing(JSON.stringify(a), 'another network')).toThrow('different network');
		vi.unstubAllGlobals();
	});
	it('retains detailed nodes and open switches rather than replacing them with calculated buses', () => {
		const n: Network = {
			...network,
			equipment: {
				transformers: [],
				shunts: [],
				hvdc: [],
				detailed_dc: { nodes: 0, lines: 0, converters: 0 },
				node_breaker: {
					stations: [{ id: 's', name: 'Station' }],
					nodes: [
						{
							id: 'n1',
							name: 'N1',
							voltage_level: 'v',
							station: 's',
							bus: 1,
							nominal_kv: 110,
							container: 'bay'
						},
						{
							id: 'n2',
							name: 'N2',
							voltage_level: 'v',
							station: 's',
							bus: 1,
							nominal_kv: 110,
							container: 'bay'
						}
					],
					busbars: [{ id: 'bb', name: 'Busbar', node: 'n1' }],
					switches: [
						{
							id: 'switch',
							name: 'Disconnect',
							kind: 'disconnector',
							from: 'n1',
							to: 'n2',
							open: true
						}
					],
					connections: [],
					terminals: []
				}
			}
		};
		const d = createDrawing(n, 'Station');
		const generic = structuredClone(n);
		generic.equipment!.node_breaker!.switches[0].kind = 'load_break_switch';
		const genericDrawing = createDrawing(generic, 'Mixed switches');
		expect(genericDrawing.objects.find((o) => o.id === 'switch')?.kind).toBe('switch');
		expect(parseDrawing(JSON.stringify(genericDrawing), genericDrawing.network)).toEqual(
			genericDrawing
		);
		const changed = structuredClone(n);
		changed.equipment!.node_breaker!.busbars[0].node = 'n2';
		expect(drawingNetworkKey(changed)).not.toBe(d.network);
		const internal = structuredClone(n);
		internal.equipment!.node_breaker!.connections.push({ from: 'n1', to: 'n2' });
		expect(drawingNetworkKey(internal)).not.toBe(d.network);
		const stateOnly = structuredClone(n);
		stateOnly.equipment!.node_breaker!.switches[0].open = false;
		expect(drawingNetworkKey(stateOnly)).toBe(d.network);
		expect(d.objects.filter((o) => o.kind === 'node')).toHaveLength(2);
		expect(d.objects.find((o) => o.id === 'switch')?.open).toBe(true);
		expect(d.objects.find((o) => o.id === 'bb')?.bus).toBe(1);
	});
	it('represents a three-winding transformer once with three ports', () => {
		const n: Network = {
			...network,
			buses: [...network.buses, { ...network.buses[0], id: 3, name: 'C', lon: 2, lat: 2 }],
			equipment: {
				transformers: [],
				shunts: [],
				hvdc: [],
				detailed_dc: { nodes: 0, lines: 0, converters: 0 },
				three_winding: [
					{
						id: 7,
						uid: 't',
						name: 'T',
						buses: [1, 2, 3],
						branches: [8],
						star_bus: 9,
						in_service: true
					}
				]
			}
		};
		const d = createDrawing(n, 'Transformer');
		expect(parseDrawing(JSON.stringify(d), d.network)).toEqual(d);
		expect(d.objects.filter((o) => o.kind === 'transformer')).toHaveLength(1);
		expect(d.objects.find((o) => o.kind === 'transformer')?.ports).toHaveLength(3);
		expect(d.connections.some((c) => c.branch === 8)).toBe(false);
	});
});

it('keeps individual distribution equipment and one transformer across winding-pair edges', () => {
	const windings = ['a', 'b', 'c'].map((bus) => ({
		bus,
		connection: 'wye' as const,
		terminals: ['1', 'n'],
		v_ref: 230,
		s_rating: 10000
	}));
	const graph = {
		buses: ['a', 'b', 'c'].map((id) => ({
			id,
			terminals: ['1', 'n'],
			grounded: ['n'],
			load_kw: 0,
			gen_kw: 0,
			has_source: id === 'a',
			attachments:
				id === 'b'
					? [
							{ id: 'load1', kind: 'load' as const, terminals: ['1', 'n'] },
							{ id: 'load2', kind: 'load' as const, terminals: ['1', 'n'] }
						]
					: []
		})),
		edges: [
			{ from: 'a', to: 'b' },
			{ from: 'a', to: 'c' },
			{ from: 'b', to: 'c' }
		].map((edge) => ({
			...edge,
			id: 'three-winding',
			kind: 'transformer' as const,
			closed: true,
			n_phases: 1,
			conductors: [['1', '1']] as [string, string][],
			windings
		}))
	};
	const three: Network = {
		...network,
		buses: [0, 1, 2].map((id) => ({ ...network.buses[0], id, lon: id * 100 }))
	};
	const drawing = createDrawing(three, 'Feeder', graph);
	const changed = structuredClone(graph);
	changed.buses[1].attachments[0].id = 'another-load';
	expect(drawingNetworkKey(three, changed)).not.toBe(drawing.network);
	const terminalChange = structuredClone(graph);
	terminalChange.buses[0].terminals[0] = '2';
	expect(drawingNetworkKey(three, terminalChange)).not.toBe(drawing.network);
	expect(drawing.objects.filter((o) => o.kind === 'transformer')).toHaveLength(1);
	expect(drawing.objects.filter((o) => o.kind === 'load')).toHaveLength(2);
	expect(drawing.connections.filter((c) => c.to === 'transformer:three-winding')).toHaveLength(3);
	expect(parseDrawing(JSON.stringify(drawing), drawing.network)).toEqual(drawing);
});

it('rejects malformed imported layers, identifiers, ports and connections', () => {
	const original = createDrawing(network, 'Import');
	const invalid = [
		(d: any) => d.layers.push(null),
		(d: any) => d.layers.push({ ...d.layers[0] }),
		(d: any) => (d.layers[0].visible = 'yes'),
		(d: any) => (d.objects[0].locked = null),
		(d: any) => (d.objects[0].layer = 'missing'),
		(d: any) => d.objects.push({ ...d.objects[0] }),
		(d: any) => d.objects[0].ports.push({ ...d.objects[0].ports[0] }),
		(d: any) => d.connections.push({ ...d.connections[0] }),
		(d: any) => (d.connections[0].fromPort = 'missing'),
		(d: any) => (d.connections[0].connected = 1),
		(d: any) => (d.connections[0].bends = [[null, 1]])
	];
	for (const mutate of invalid) {
		const edited = structuredClone(original);
		mutate(edited);
		expect(() => parseDrawing(JSON.stringify(edited), original.network)).toThrow();
	}
});

it('restores electrical state from the current case and refuses altered source connections', () => {
	const original = createDrawing(network, 'Saved');
	const saved = structuredClone(original);
	saved.objects[0].x = 777;
	saved.objects[0].bus = 999;
	saved.connections[0].connected = false;
	const restored = restoreDrawing(saved, original);
	expect(restored.objects[0].x).toBe(777);
	expect(restored.objects[0].bus).toBe(1);
	expect(restored.connections[0].connected).toBe(true);
	saved.connections[0].from = saved.connections[0].to;
	expect(() => restoreDrawing(saved, original)).toThrow('connectivity');
});

it('keeps locked equipment fixed during graph layout and handles large extents without argument limits', () => {
	const drawing = createDrawing(network, 'Layout');
	drawing.objects[0].locked = true;
	drawing.objects[0].x = 555;
	arrangeDrawing(drawing);
	expect(drawing.objects[0].x).toBe(555);
	expect(drawing.objects[1].x).toBe(280);
	expect(drawingExtent(Array.from({ length: 200000 }, (_, i) => [i, -i]))).toEqual([
		0, -199999, 199999, -0
	]);
});

it('includes rotated symbols, distant labels and routed bends in full drawing bounds', () => {
	const d = createDrawing(network, 'Bounds');
	d.objects[0].width = 3000;
	d.objects[0].rotation = 45;
	d.objects[0].labelOffset = [4000, 0];
	d.connections[0].bends = [[-5000, -2000]];
	const [x, y, width, height] = drawingBounds(d);
	expect(x).toBeLessThan(-5000);
	expect(y).toBeLessThan(-2000);
	expect(x + width).toBeGreaterThan(2800);
	expect(y + height).toBeGreaterThan(2800);
});

it('retains source route bends independently for each distribution conductor', () => {
	const graph = {
		buses: ['a', 'b'].map((id) => ({
			id,
			terminals: ['1', 'n'],
			grounded: ['n'],
			load_kw: 0,
			gen_kw: 0,
			has_source: id === 'a',
			attachments: []
		})),
		edges: [
			{
				id: 'line',
				from: 'a',
				to: 'b',
				kind: 'line' as const,
				closed: true,
				n_phases: 1,
				conductors: [
					['1', '1'],
					['n', 'n']
				] as [string, string][]
			}
		]
	};
	const feeder: Network = {
		...network,
		coordinate_space: 'diagram',
		buses: network.buses.map((b, id) => ({ ...b, id })),
		branches: [
			{
				...network.branches[0],
				id: 0,
				from: 0,
				to: 1,
				path: [
					[0, 0],
					[0.25, 0.75],
					[1, 1]
				]
			}
		]
	};
	const drawing = createDrawing(feeder, 'Feeder', graph);
	expect(drawing.connections).toHaveLength(2);
	expect(drawing.connections[0].bends).toEqual([[300, 700]]);
	expect(drawing.connections[1].bends).toEqual([[300, 700]]);
	drawing.connections[0].bends[0][0] = 400;
	expect(drawing.connections[1].bends[0][0]).toBe(300);
	expect(parseDrawing(JSON.stringify(drawing), drawing.network)).toEqual(drawing);
});
