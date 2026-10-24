import { describe, expect, it } from 'vitest';
import type { DistGraph, SchematicLayout } from '@tellegen/engine';
import {
	attachmentGlyph,
	branchGlyph,
	fromEngineLayout,
	glyphPath,
	INLINE_HALF,
	schematicEquipment,
	sourceLayout,
	fanRoute,
	routeMiddle,
	busStacks,
	spreadOffsets,
	displaceBuses,
	displayTree,
	conductorSet,
	conductorTicks,
	neutralEarths,
	energisation,
	shortNames,
	distinctiveCores,
	treeWalk,
	describeBus,
	flowArrows,
	elementPowers,
	formatPower,
	galvanicZones,
	zoneState,
	zoneOutline,
	zoneCorridors,
	zoneVoltage,
	feeders,
	pathToSource,
	voltageClass,
	levelLabel,
	zoneLevel,
	zoneSummary,
	landmarkCandidates,
	placeLandmarks,
	landmarkGlyph,
	basemapCamera,
	mercatorToLngLat,
	cameraFromMap
} from '../src/lib/diagram/index.js';
import type { GalvanicZones } from '../src/lib/diagram/index.js';

const bus = (id: string, attachments: [string, string, string][] = [], source = false) => ({
	id,
	terminals: ['1', '2', '3', 'n'],
	grounded: [],
	load_kw: 0,
	gen_kw: 0,
	has_source: source,
	terminal_attachments: Object.fromEntries(
		[...new Set(attachments.map(([t]) => t))].map((t) => [
			t,
			attachments.filter(([at]) => at === t).map(([, kind, id]) => ({ kind: kind as 'load', id }))
		])
	)
});
const edge = (
	kind: 'line' | 'switch' | 'transformer',
	id: string,
	from: string,
	to: string,
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

// S —t1→ M —l1→ L, and an open switch M–X. M carries a generator and two loads; the loads appear
// on two terminals each, as multi-phase loads do in PowerIO graphs.
const graph: DistGraph = {
	buses: [
		bus('S', [['1', 'source', 'grid']], true),
		bus('M', [
			['1', 'load', 'ld1'],
			['2', 'load', 'ld1'],
			['2', 'load', 'ld2'],
			['3', 'generator', 'g1']
		]),
		bus('L'),
		bus('X')
	],
	edges: [
		edge('transformer', 't1', 'S', 'M'),
		edge('line', 'l1', 'M', 'L'),
		edge('switch', 'sw', 'M', 'X', false)
	]
};
// An engine layout as Rust returns it: t1 straight, l1 elbowed, sw straight and open.
const layout: SchematicLayout = {
	version: 1,
	strategy: 'tidy',
	positions: [0, 0, 120, 0, 240, 0, 240, 100],
	segments: [0, 0, 120, 0, 120, 0, 240, 0, 120, 0, 240, 100],
	segment_edge: [0, 1, 2],
	role: [0, 0, 2],
	parent: [0, 0, 1, 3],
	parent_edge: [-1, 0, 1, -1],
	depth: [0, 1, 2, 0],
	roots: [0, 3],
	bounds: [0, 0, 240, 100],
	attachment_gap: 22,
	attachment_slots: [1, 2, 0, 0]
};

describe('schematic equipment symbols', () => {
	const geometry = fromEngineLayout(layout);
	const eq = schematicEquipment(graph, geometry);

	it('draws transformers and switches on their branch and lines bare', () => {
		const inline = eq.items.filter((d) => d.edge >= 0);
		expect(inline.map((d) => [d.kind, d.ids[0]])).toEqual([
			['transformer', 't1'],
			['switch', 'sw']
		]);
		expect(branchGlyph('line', true)).toBeNull();
		// The tree transformer sits a fixed distance back from its child bus, the open switch mid-span.
		const unit = 22 * 0.4;
		expect(eq.unit).toBeCloseTo(unit);
		expect(inline[0].x).toBeCloseTo(120 - 2.5 * INLINE_HALF * unit);
		expect(inline[0].y).toBe(0);
		expect([inline[1].x, inline[1].y]).toEqual([180, 50]);
	});

	it('interrupts each symbol branch line and keeps edge ownership for picking', () => {
		expect(Array.from(eq.cut.segmentEdge)).toEqual([0, 0, 1, 2, 2]);
		const half = INLINE_HALF * eq.unit;
		const centre = 120 - 2.5 * half;
		expect(eq.cut.targets[0]).toBeCloseTo(centre - half);
		expect(eq.cut.sources[3]).toBeCloseTo(centre + half);
		// Bus positions are untouched.
		expect(eq.cut.positions).toBe(geometry.positions);
	});

	it('hangs one symbol per attachment kind in the reserved slots, with all ids', () => {
		const attached = eq.items.filter((d) => d.bus >= 0);
		expect(attached.map((d) => [d.kind, graph.buses[d.bus].id, d.ids])).toEqual([
			['source', 'S', ['grid']],
			['generator', 'M', ['g1']],
			['load', 'M', ['ld1', 'ld2']]
		]);
		expect(attached.map((d) => d.y)).toEqual([22, 22, 44]);
		expect([...eq.kinds].sort()).toEqual(['generator', 'load', 'source', 'switch', 'transformer']);
	});

	it('shows an open switch with its blade lifted off the far contact', () => {
		const closed = branchGlyph('switch', true)!;
		const open = branchGlyph('switch', false)!;
		expect(closed.at(-1)!.at(-1)).toEqual([1.05, 0]);
		expect(open.at(-1)!.at(-1)![1]).toBeLessThan(-0.5);
	});

	it('draws a shunt on the neutral alone as a neutral earth, any other shunt as a shunt', () => {
		const b = {
			...bus('N', [
				['n', 'shunt', 'men'],
				['1', 'shunt', 'cap'],
				['n', 'shunt', 'cap']
			]),
			neutral_terminal: 'n'
		};
		// `men` connects only the neutral; `cap` also a phase, so it stays a general shunt.
		expect([...neutralEarths(b)]).toEqual(['men']);
		expect(neutralEarths({ ...b, neutral_terminal: null }).size).toBe(0);
		const g: DistGraph = { buses: [{ ...b, has_source: true }], edges: [] };
		const one = schematicEquipment(
			g,
			fromEngineLayout({
				...layout,
				positions: [0, 0],
				segments: [],
				segment_edge: [],
				role: [],
				parent: [0],
				parent_edge: [-1],
				depth: [0],
				roots: [0],
				attachment_slots: [1]
			})
		);
		const kinds = one.items.map((d) => [d.kind, d.ids, d.y]);
		// Both share the one shunt slot the engine reserved, the earth beside the shunt.
		expect(kinds).toEqual([
			['shunt', ['cap'], 22],
			['earth', ['men'], 22]
		]);
	});

	it('draws perfectly grounded terminals as one solid earth in a slot of its own', () => {
		const b = {
			...bus('G', [['1', 'load', 'ld']], true),
			grounded: ['n'],
			neutral_terminal: 'n'
		};
		const g: DistGraph = { buses: [b], edges: [] };
		const eq = schematicEquipment(
			g,
			fromEngineLayout({
				...layout,
				positions: [0, 0],
				segments: [],
				segment_edge: [],
				role: [],
				parent: [0],
				parent_edge: [-1],
				depth: [0],
				roots: [0],
				attachment_slots: [2]
			})
		);
		expect(eq.items.map((d) => [d.kind, d.ids, d.y])).toEqual([
			['load', ['ld'], 22],
			['ground', ['n'], 44]
		]);
	});

	it('gives every attachment kind a distinct glyph connected at its origin', () => {
		const kinds = ['load', 'generator', 'source', 'ibr', 'shunt', 'earth', 'ground'] as const;
		const paths = kinds.map((k) => glyphPath(attachmentGlyph(k), 10));
		expect(new Set(paths).size).toBe(kinds.length);
		for (const k of kinds) expect(attachmentGlyph(k)[0][0]).toEqual([0, 0]);
	});

	it('still draws a transformer whose two buses share one location', () => {
		const placed = [
			{ id: 'S', lon: 0, lat: 0 },
			{ id: 'M', lon: 0, lat: 0 },
			{ id: 'L', lon: 400, lat: 0 },
			{ id: 'X', lon: 400, lat: 300 }
		];
		const symbols = schematicEquipment(
			graph,
			sourceLayout(graph, placed, false, displayTree(graph))
		);
		const t = symbols.items.find((d) => d.kind === 'transformer')!;
		expect([t.x, t.y]).toEqual([symbols.anchors[0], symbols.anchors[1]]);
		expect(Array.from(symbols.owner)).toContain(0);
	});

	it('spreads the marks of branches drawn on the same segment instead of stacking them', () => {
		// An MV and an LV line between buses at the same two positions (shared poles).
		const g: DistGraph = {
			buses: [bus('A', [], true), bus('B'), bus('A2'), bus('B2'), bus('C')],
			edges: [
				edge('line', 'mv', 'A', 'B'),
				edge('line', 'lv', 'A2', 'B2'),
				edge('line', 'x', 'B', 'C')
			]
		};
		const placed = [
			{ id: 'A', lon: 0, lat: 0 },
			{ id: 'B', lon: 300, lat: 0 },
			{ id: 'A2', lon: 0, lat: 0 },
			{ id: 'B2', lon: 300, lat: 0 },
			{ id: 'C', lon: 300, lat: 200 }
		];
		const eq = schematicEquipment(g, sourceLayout(g, placed, false, displayTree(g)));
		expect(Array.from(eq.sharedRoute)).toEqual([2, 2, 1]);
		// The two lines' marks sit a third and two thirds of the way along, not both at the middle.
		expect(Math.abs(eq.anchors[0] - eq.anchors[2])).toBeCloseTo(100, 3);
	});

	it('fans the members of a shared route apart, meeting at the shared ends', () => {
		const g: DistGraph = {
			buses: [bus('A', [], true), bus('B'), bus('A2'), bus('B2')],
			edges: [edge('line', 'mv', 'A', 'B'), edge('line', 'lv', 'A2', 'B2')]
		};
		const placed = [
			{ id: 'A', lon: 0, lat: 0 },
			{ id: 'B', lon: 300, lat: 0 },
			{ id: 'A2', lon: 0, lat: 0 },
			{ id: 'B2', lon: 300, lat: 0 }
		];
		const geometry = sourceLayout(g, placed, false, displayTree(g));
		const eq = schematicEquipment(g, geometry);
		expect(eq.routeGroups).toEqual([[0, 1]]);
		expect(Array.from(eq.routeOf)).toEqual([0, 0]);
		// At 1 px per unit: 7 px apart, centred on the route, tapering in over 14 px.
		const fans = fanRoute(eq.routeGroups[0], eq.host, geometry, 1);
		const [a, b] = fans;
		expect(a.path[0]).toEqual(b.path[0]);
		expect(a.path[3]).toEqual(b.path[3]);
		expect(Math.abs(a.path[1][1] - b.path[1][1])).toBeCloseTo(7, 6);
		expect(a.path[1][1] + b.path[1][1]).toBeCloseTo(2 * a.path[0][1], 6);
		expect(Math.abs(a.path[1][0] - a.path[0][0])).toBeCloseTo(14, 6);
		// At 2 px per unit the same spread on screen is half as far in drawing units.
		const zoomed = fanRoute(eq.routeGroups[0], eq.host, geometry, 2);
		expect(Math.abs(zoomed[0].path[1][1] - zoomed[1].path[1][1])).toBeCloseTo(3.5, 6);
		expect(routeMiddle(eq.routeGroups[0], eq.host, geometry)).toEqual([
			(geometry.positions[0] + geometry.positions[3]) / 2,
			geometry.positions[1]
		]);
	});

	it('scales symbols to the drawing for source positions', () => {
		const placed = [
			{ id: 'S', lon: 0, lat: 0 },
			{ id: 'M', lon: 400, lat: 0 },
			{ id: 'L', lon: 800, lat: 0 },
			{ id: 'X', lon: 400, lat: 300 }
		];
		const src = sourceLayout(graph, placed, false, displayTree(graph));
		const symbols = schematicEquipment(graph, src);
		expect(symbols.unit).toBeCloseTo(400 * 0.25 * 0.4);
		expect(symbols.items.find((d) => d.kind === 'transformer')!.x).toBeCloseTo(-200);
	});
});

describe('energisation', () => {
	it('dims what no source reaches through closed branches', () => {
		const g: DistGraph = {
			buses: [...graph.buses, bus('Y')],
			edges: [...graph.edges, edge('line', 'xy', 'X', 'Y')]
		};
		const e = energisation(g);
		expect(Array.from(e.buses)).toEqual([1, 1, 1, 0, 0]);
		// The open switch is live on its source side; the line beyond it is not.
		expect(Array.from(e.edges)).toEqual([1, 1, 1, 0]);
		expect(e.deadBuses).toBe(2);
	});
});

describe('conductor marks', () => {
	it('count phases and recognise the neutral from the bus', () => {
		const g: DistGraph = {
			buses: [
				{ ...bus('A', [], true), neutral_terminal: 'n' },
				{ ...bus('B'), neutral_terminal: 'n' }
			],
			edges: [
				{
					...edge('line', 'ab', 'A', 'B'),
					conductors: [
						['2', '2'],
						['n', 'n']
					]
				}
			]
		};
		expect(conductorSet(g, 0)).toEqual({ phases: 1, neutral: true });
		// One slash per conductor, and the neutral's slash carries a circle.
		expect(conductorTicks(1, true)).toHaveLength(3);
		expect(conductorTicks(3, false)).toHaveLength(3);
	});

	it('anchor every branch, and own every symbol stroke', () => {
		const eq = schematicEquipment(graph, fromEngineLayout(layout));
		for (let k = 0; k < graph.edges.length; k++)
			expect(Number.isFinite(eq.anchors[k * 2])).toBe(true);
		expect(eq.owner.length).toBe(eq.sources.length / 3);
		// The bare line l1 has no neutral terminal declared here, so no marks; others own symbols.
		expect(new Set(eq.owner)).toEqual(new Set([0, 2, -1, -2]));
	});
});

describe('short names', () => {
	it('drop the parts every id shares at its start or end', () => {
		expect(
			shortNames(['75_MVLV060_Transformer', '75_MVLV160_Transformer', '75_MVLV207_Transformer'], 16)
		).toEqual(['MVLV060', 'MVLV160', 'MVLV207']);
		expect(distinctiveCores(['energex:ne_29311', 'energex:ne_40112'])).toEqual(['29311', '40112']);
		// Nothing shared, a single id, or stripping that would empty or merge ids: kept whole.
		expect(distinctiveCores(['src', 'lb'])).toEqual(['src', 'lb']);
		expect(distinctiveCores(['75_MVLV060_Transformer'])).toEqual(['75_MVLV060_Transformer']);
		expect(distinctiveCores(['a_x', 'a'])).toEqual(['a_x', 'a']);
		expect(distinctiveCores(['t_a', 't-a'])).toEqual(['t_a', 't-a']);
	});

	it('keep short ids and shorten long ones uniquely', () => {
		const ids = [
			'lb',
			'lv_extension_02ad4cfa5ebe_0062383670bceb3c28d5',
			'lv_extension_02ad4cfa5ebe_9962383670bceb3c28d5',
			'lv_extension_02ad4cfa5ebe_0062383670bceb3c11d5'
		];
		const names = shortNames(ids);
		expect(names[0]).toBe('lb');
		expect(new Set(names).size).toBe(ids.length);
		expect(names[3]).toBe('lv_extension_0…c11d5');
		expect(names[1].length).toBeLessThan(ids[1].length);
		for (const n of names.slice(1)) expect(n).toContain('…');
	});
});

describe('keyboard walk', () => {
	const tree = displayTree(graph);
	const walk = treeWalk(tree, layout.positions, 2);
	const [S, M, L, X] = [0, 1, 2, 3];

	it('moves to the feeding bus, onward, and between drawn neighbours', () => {
		expect(walk.step(M, 'parent')).toBe(S);
		expect(walk.step(S, 'parent')).toBe(S);
		expect(walk.step(S, 'child')).toBe(M);
		expect(walk.step(M, 'child')).toBe(L);
		expect(walk.step(L, 'child')).toBe(L);
		// X is beyond the open switch, so it roots its own component below S.
		expect(walk.step(S, 'next')).toBe(X);
		expect(walk.step(X, 'previous')).toBe(S);
		expect(walk.step(L, 'root')).toBe(S);
		expect(walk.step(-1, 'child')).toBe(S);
	});

	it('describes where a bus sits and what hangs off it', () => {
		expect(describeBus(graph, tree, walk, M, 'Worst voltage a–n 230.0 V')).toBe(
			'M: 1 step from S, fed by transformer t1. 1 branch onward. 1 generator, 2 loads. Worst voltage a–n 230.0 V'
		);
		expect(describeBus(graph, tree, walk, X)).toBe(
			'X: not reached from a source. 0 branches onward'
		);
	});
});

describe('solved values on the drawing', () => {
	const eq = schematicEquipment(graph, fromEngineLayout(layout));

	it('points flow arrows the way active power goes, upstream of each symbol', () => {
		// t1 carries power from S to M; l1 carries it back from L to M; the open switch none.
		const a = flowArrows(
			eq,
			[
				[{ p: 1000 }, null],
				[{ p: -500 }, null],
				[{ p: 10 }, null]
			],
			[true, true, false]
		);
		expect(Array.from(a.edge)).toEqual([0, 0, 1, 1]);
		const tipDx = (s: number) => a.targets[s * 3] - a.sources[s * 3];
		expect(tipDx(0)).toBeGreaterThan(0);
		expect(tipDx(2)).toBeLessThan(0);
		// The chevron sits before the transformer symbol, not on it.
		expect(a.targets[0]).toBeLessThan(eq.anchors[0] - INLINE_HALF * eq.unit);
		// A missing from end falls back to the to end with the sign flipped.
		const back = flowArrows(
			eq,
			[
				[null, { p: 1000 }],
				[null, null],
				[null, null]
			],
			[true, true, true]
		);
		expect(back.targets[0] - back.sources[0]).toBeLessThan(0);
	});

	it('sums element powers and reads sources as consumption', () => {
		const powers = elementPowers({
			element_ports: [
				{ element: 'ld1', power_into_element: { re: 100, im: 10 } },
				{ element: 'ld1', power_into_element: { re: 50, im: 5 } }
			],
			source_reactions: [{ source: 'grid', power_into_network: { re: 160, im: 20 } }]
		});
		expect(powers.get('ld1')).toEqual({ p: 150, q: 15 });
		expect(powers.get('grid')).toEqual({ p: -160, q: -20 });
		expect(formatPower(15_000, 5_000)).toBe('15.0 kW 5.0 kvar');
		expect(formatPower(2.5e6, -1e5)).toBe('2.5 MW -0.1 Mvar');
		expect(formatPower(160, 20)).toBe('160 W 20.0 var');
	});
});

describe('galvanic zones', () => {
	// src —t0→ mv —l→ mv2; mv —t2→ a1 — a2 — a3; mv2 —t3→ b1 — b2.
	const g: DistGraph = {
		buses: ['src', 'hv', 'mv', 'mv2', 'a1', 'a2', 'a3', 'b1', 'b2'].map((id) =>
			bus(id, [], id === 'src')
		),
		edges: [
			edge('line', 's', 'src', 'hv'),
			edge('transformer', 't0', 'hv', 'mv'),
			edge('line', 'm', 'mv', 'mv2'),
			edge('transformer', 't2', 'mv', 'a1'),
			edge('line', 'a12', 'a1', 'a2'),
			edge('line', 'a23', 'a2', 'a3'),
			edge('transformer', 't3', 'mv2', 'b1'),
			edge('line', 'b12', 'b1', 'b2')
		]
	};
	const tree = displayTree(g);
	const found = galvanicZones(g, tree);
	const ids = (z: { buses: number[] }) => z.buses.map((i) => g.buses[i].id);

	it('splits the network at transformers and names each zone’s feeding transformer', () => {
		expect(found.zones.map(ids)).toEqual([
			['src', 'hv'],
			['mv', 'mv2'],
			['a1', 'a2', 'a3'],
			['b1', 'b2']
		]);
		expect(found.zones.map((z) => z.hasSource)).toEqual([true, false, false, false]);
		expect(found.zones.map((z) => g.edges[z.transformers[0]].id)).toEqual(['t0', 't0', 't2', 't3']);
		// The MV zone has three transformer windings in it, fed through t0.
		expect(found.zones[1].transformers.map((k) => g.edges[k].id)).toEqual(['t0', 't2', 't3']);
		expect(found.zones[2].edges.map((k) => g.edges[k].id)).toEqual(['a12', 'a23']);
	});

	it('reports the worst state inside, including the transformers', () => {
		const pair = (volts: number) => ({
			pair: ['a', 'n'] as [string, string],
			zone: 'phase–neutral' as const,
			volts,
			min: 207,
			max: 253
		});
		const snap = (volts: number, tLoading: number) => ({
			kind: 'solved' as const,
			label: '',
			iterations: 1,
			buses: g.buses.map((_, i) => ({
				pairs: [pair(i === 5 ? volts : 230)],
				unbalance: null
			})),
			branches: g.edges.map((_, k) => ({
				ends: [null, null] as [null, null],
				loading: k === 3 ? tLoading : 10,
				basis: null,
				primary: true,
				loss: null
			}))
		});
		const a = found.zones[2];
		expect(zoneState(a, null)).toBe('missing');
		expect(zoneState(a, snap(230, 50))).toBe('normal');
		expect(zoneState(a, snap(230, 95))).toBe('warning');
		expect(zoneState(a, snap(200, 50))).toBe('violation');
	});

	it('tells MV from LV by declared limits, then by solved voltages, then by the source', () => {
		const limited: DistGraph = {
			...g,
			buses: g.buses.map((b) =>
				b.id === 'mv' || b.id === 'mv2'
					? { ...b, voltage_limits: { vpp_min: [10_000], vpp_max: [12_000] } }
					: b.id.startsWith('a')
						? { ...b, voltage_limits: { vpn_min: [207], vpn_max: [253] } }
						: b
			)
		};
		expect(zoneVoltage(found.zones[1], limited, null)).toBe(11_000);
		expect(zoneVoltage(found.zones[2], limited, null)).toBeCloseTo(230 * Math.sqrt(3));
		expect(zoneVoltage(found.zones[3], limited, null)).toBeNull();
		expect(zoneLevel(found.zones[1], 11_000)).toBe('mv');
		expect(zoneLevel(found.zones[2], 398)).toBe('lv');
		expect(zoneLevel(found.zones[0], null)).toBe('mv');
		expect(zoneLevel(found.zones[3], null)).toBe('lv');
	});

	it('summarises a zone by its lowest phase–neutral voltage and highest loading', () => {
		const snap = {
			kind: 'solved' as const,
			label: '',
			iterations: 1,
			buses: g.buses.map((_, i) => ({
				pairs: [
					{
						pair: ['a', 'b'] as [string, string],
						zone: 'phase–phase' as const,
						volts: 400,
						min: null,
						max: null
					},
					{
						pair: ['a', 'n'] as [string, string],
						zone: 'phase–neutral' as const,
						volts: 230 - i,
						min: null,
						max: null
					}
				],
				unbalance: null
			})),
			branches: g.edges.map((_, k) => ({
				ends: [null, null] as [null, null],
				loading: k * 10,
				basis: null,
				primary: true,
				loss: null
			}))
		};
		// Zone a: buses 4–6 (a3 lowest at 224 V); branches a12, a23 and transformer t2.
		expect(zoneSummary(found.zones[2], snap)).toEqual({
			minVolts: 224,
			maxLoading: 50
		});
		expect(zoneSummary(found.zones[2], null)).toEqual({
			minVolts: null,
			maxLoading: null
		});
	});

	it('outlines a zone as a staircase over its drawing columns', () => {
		// Two columns at x = 100 (rows 0–20) and x = 200 (row 40, with one symbol slot below).
		const positions = [100, 0, 0, 100, 20, 0, 200, 40, 0];
		const outline = zoneOutline(
			{ buses: [0, 1, 2], edges: [], transformers: [], hasSource: false },
			positions,
			[0, 0, 10],
			{
				dx: 100,
				pad: 5
			}
		);
		expect(outline).toEqual([
			[85, -5],
			[150, -5],
			[150, 35],
			[240, 35],
			[240, 55],
			[150, 55],
			[150, 25],
			[85, 25]
		]);
	});

	it('draws a zone on geographic positions as a corridor along its own branches', () => {
		// Two LV networks interleave: their hulls would overlap, their corridors do not share a
		// segment or a bus. Edge 2 is the transformer between them and belongs to neither.
		const found: GalvanicZones = {
			zones: [
				{ buses: [0, 1, 2], edges: [0, 1], transformers: [2], hasSource: false },
				{ buses: [3, 4], edges: [3], transformers: [2], hasSource: false },
				{ buses: [5], edges: [], transformers: [], hasSource: true }
			],
			busZone: Int32Array.from([0, 0, 0, 1, 1, 2])
		};
		const geometry = {
			positions: [0, 0, 0, 10, 0, 0, 10, 10, 0, 2, 2, 0, 8, 8, 0, 50, 50, 0],
			// Edge 1 is routed in two segments.
			segmentEdge: Int32Array.from([0, 1, 1, 2, 3])
		};
		const corridors = zoneCorridors(found, geometry, (z) => !z.hasSource);
		expect(corridors.map((c) => [c.z, c.segments, c.buses])).toEqual([
			[0, [0, 1, 2], [0, 1, 2]],
			[1, [4], [3, 4]]
		]);
		expect(corridors[1].bounds).toEqual([2, 2, 8, 8]);
	});
});

describe('feeders', () => {
	// S (source) feeds two MV feeders: S–M1 (with transformer T1 to L0) and S–M2. L0 feeds two LV
	// circuits: L0–L1–L3 and L0–L2. M1–M2 closes a loop, drawn on the tree it does not belong to.
	const g: DistGraph = {
		buses: ['S', 'M1', 'M2', 'L0', 'L1', 'L2', 'L3'].map((id) => bus(id, [], id === 'S')),
		edges: [
			edge('line', 'sm1', 'S', 'M1'),
			edge('line', 'sm2', 'S', 'M2'),
			edge('transformer', 't1', 'M1', 'L0'),
			edge('line', 'l01', 'L0', 'L1'),
			edge('line', 'l02', 'L0', 'L2'),
			edge('line', 'l13', 'L1', 'L3'),
			edge('line', 'm12', 'M1', 'M2')
		]
	};
	const tree = displayTree(g);
	const f = feeders(g, tree, galvanicZones(g, tree));

	it('splits each network at the branches leaving its supply bus, on MV and LV alike', () => {
		expect(f.feeders.map((x) => [g.edges[x.head].id, g.buses[x.supply].id, x.number])).toEqual([
			['sm1', 'S', 1],
			['sm2', 'S', 2],
			['l01', 'L0', 1],
			['l02', 'L0', 2]
		]);
		// Supply buses belong to their network, not to one feeder.
		expect([...f.busFeeder]).toEqual([-1, 0, 1, -1, 2, 3, 2]);
		expect(f.feeders[2].buses.map((i) => g.buses[i].id)).toEqual(['L1', 'L3']);
	});

	it('puts a transformer on the feeder that reaches it, and a loop on its far end', () => {
		expect([...f.edgeFeeder]).toEqual([0, 1, 0, 2, 3, 2, 1]);
	});

	it('walks the drawn path back to the source across transformers', () => {
		expect(pathToSource(tree, 6).map((k) => g.edges[k].id)).toEqual(['l13', 'l01', 't1', 'sm1']);
		expect(pathToSource(tree, 0)).toEqual([]);
	});
});

describe('voltage levels', () => {
	const zone = (hasSource: boolean) => ({ buses: [], edges: [], transformers: [], hasSource });

	it('classes a level in wide bands, and guesses MV for the source network without one', () => {
		expect(voltageClass(zone(false), 132_000)).toBe('hv');
		expect(voltageClass(zone(false), 33_000)).toBe('sub');
		expect(voltageClass(zone(false), 7_200)).toBe('mv');
		expect(voltageClass(zone(false), 400)).toBe('lv');
		expect(voltageClass(zone(true), null)).toBe('mv');
		expect(voltageClass(zone(false), null)).toBe('lv');
	});

	it('names levels the way engineers write them', () => {
		expect([11_000, 12_470, 19_052, 400, 132_000].map(levelLabel)).toEqual([
			'11 kV',
			'12.47 kV',
			'19.05 kV',
			'400 V',
			'132 kV'
		]);
	});

	it('prefers a declared winding or source rating over limits and solved voltages', () => {
		const g: DistGraph = {
			buses: [
				{ ...bus('A', [], true), nominal_voltage: 11_000 },
				{ ...bus('B'), voltage_limits: { vpp_min: [10_000], vpp_max: [10_400] } }
			],
			edges: [edge('line', 'ab', 'A', 'B')]
		};
		const zones = galvanicZones(g);
		expect(zoneVoltage(zones.zones[0], g, null)).toBe(11_000);
	});
});

describe('landmarks', () => {
	// S (source) —t1→ M —l1→ L; M has a generator; L has a large and two small IBRs.
	const g: DistGraph = {
		buses: [
			bus('S', [['1', 'source', 'grid']], true),
			bus('M', [['1', 'generator', 'g1']]),
			bus('L', [
				['1', 'ibr', 'farm'],
				['2', 'ibr', 'pv1']
			]),
			bus('R', [['1', 'ibr', 'pv2']])
		],
		edges: [
			edge('transformer', 't1', 'S', 'M'),
			edge('line', 'l1', 'M', 'L'),
			edge('line', 'l2', 'M', 'R')
		]
	};
	const geometry = sourceLayout(
		g,
		[
			{ id: 'S', lon: 0, lat: 0 },
			{ id: 'M', lon: 100, lat: 0 },
			{ id: 'L', lon: 200, lat: 0 },
			{ id: 'R', lon: 200, lat: 100 }
		],
		false,
		displayTree(g)
	);
	const zones = galvanicZones(g, displayTree(g));
	const powers = new Map([
		['g1', { p: -50_000, q: 0 }],
		['farm', { p: -40_000, q: 0 }],
		['pv1', { p: -300, q: 0 }],
		['pv2', { p: -200, q: 0 }]
	]);

	it('ranks sources, problem transformers, machines and large IBRs, then the rest', () => {
		const states = zones.zones.map((z) =>
			z.hasSource ? ('normal' as const) : ('violation' as const)
		);
		const c = landmarkCandidates(g, geometry, null, zones, states, powers);
		expect(c.map((l) => [l.kind, l.tier, l.ids.join('+')])).toEqual([
			['source', 0, 'grid'],
			['transformer', 1, 't1'],
			['generator', 2, 'g1'],
			// L's IBRs group to 40.3 kW, over 1 % of all generation; R's 0.2 kW is not.
			['ibr', 2, 'farm+pv1'],
			['ibr', 4, 'pv2']
		]);
		const calm = landmarkCandidates(
			g,
			geometry,
			null,
			zones,
			states.map(() => 'normal' as const),
			powers
		);
		expect(calm.find((l) => l.kind === 'transformer')!.tier).toBe(3);
	});

	it('places by priority on a coarse grid, sources regardless, within a budget', () => {
		const c = landmarkCandidates(
			g,
			geometry,
			null,
			zones,
			zones.zones.map(() => 'normal' as const),
			powers
		);
		// Squeezed so every marker shares one cell: the source takes it and nothing else fits.
		const tight = placeLandmarks(c, (x, y) => [100 + x * 0.01, 100 + y * 0.01], {
			width: 400,
			height: 400
		});
		expect(tight.map((l) => l.kind)).toEqual(['source']);
		const roomy = placeLandmarks(c, (x, y) => [200 + x, 200 + y], {
			width: 800,
			height: 800,
			budget: 2
		});
		// The budget counts every marker, the source included.
		expect(roomy.map((l) => l.kind)).toEqual(['source', 'generator']);
		expect(placeLandmarks(c, () => [-10, -10], { width: 100, height: 100 })).toEqual([]);
	});

	it('draws a vertical transformer at the same size as any other', () => {
		// S above M: the transformer runs exactly north-south, so its direction has x = 0.
		const g2: DistGraph = {
			buses: [bus('S', [['1', 'source', 'grid']], true), bus('M')],
			edges: [edge('transformer', 't', 'S', 'M')]
		};
		const geo = sourceLayout(
			g2,
			[
				{ id: 'S', lon: 0, lat: 0 },
				{ id: 'M', lon: 0, lat: 400 }
			],
			false,
			displayTree(g2)
		);
		const z = galvanicZones(g2, displayTree(g2));
		const t = landmarkCandidates(
			g2,
			geo,
			schematicEquipment(g2, geo),
			z,
			z.zones.map(() => 'normal' as const),
			null
		).find((l) => l.kind === 'transformer')!;
		expect(Math.hypot(t.cos, t.sin)).toBeCloseTo(1, 9);
		expect(Math.abs(t.sin)).toBeCloseTo(1, 9);
	});

	it('draws markers centred on their point, without the symbol lead', () => {
		const xs = landmarkGlyph('source')
			.flat()
			.map(([x]) => x);
		expect(Math.min(...xs) + Math.max(...xs)).toBeCloseTo(0, 5);
		expect(landmarkGlyph('transformer')).toHaveLength(2);
	});
});

describe('basemap camera', () => {
	const WORLD = 2 * Math.PI * 6_378_137;

	it('converts Web Mercator metres back to longitude and latitude', () => {
		expect(mercatorToLngLat(0, 0)).toEqual([0, 0]);
		const [lng, lat] = mercatorToLngLat(
			WORLD / 4,
			-6_378_137 * Math.log(Math.tan(Math.PI / 4 + Math.PI / 12))
		);
		expect(lng).toBeCloseTo(90, 9);
		expect(lat).toBeCloseTo(30, 9);
	});

	it('round-trips between the map camera and the schematic camera', () => {
		const origin: [number, number] = [261_000, -6_250_000];
		const map = { center: [2.35, 48.85] as [number, number], zoom: 13.4 };
		const back = basemapCamera(cameraFromMap(map, origin), origin);
		expect(back.center[0]).toBeCloseTo(2.35, 9);
		expect(back.center[1]).toBeCloseTo(48.85, 9);
		expect(back.zoom).toBeCloseTo(13.4, 9);
	});

	it('matches the schematic camera: same centre, and one metre over the same pixels', () => {
		// 512 px across the whole world is MapLibre zoom 0.
		const view = basemapCamera(
			{ target: [100, -200, 0], zoom: Math.log2(512 / WORLD) },
			[-100, 200]
		);
		expect(view.center[0]).toBeCloseTo(0, 9);
		expect(view.center[1]).toBeCloseTo(0, 9);
		expect(view.zoom).toBeCloseTo(0, 9);
	});
});

describe('stacked buses', () => {
	// An MV bus and an LV bus at one pole, joined by a transformer; a line runs on from the LV bus.
	const g: DistGraph = {
		buses: [bus('S', [], true), bus('MV'), bus('LV')],
		edges: [
			edge('line', 'f', 'S', 'MV'),
			edge('transformer', 't', 'MV', 'LV'),
			edge('line', 'l', 'LV', 'S')
		]
	};
	const geo = sourceLayout(
		g,
		[
			{ id: 'S', lon: 0, lat: 0 },
			{ id: 'MV', lon: 300, lat: 0 },
			{ id: 'LV', lon: 300, lat: 0 }
		],
		false,
		displayTree(g)
	);

	it('finds buses at exactly the same point', () => {
		const st = busStacks(geo);
		expect(st.groups).toEqual([[1, 2]]);
		expect(Array.from(st.stackOf)).toEqual([-1, 0, 0]);
	});

	it('spreads a stack by a fixed screen distance and takes branch ends along', () => {
		const offsets = spreadOffsets(2, 2);
		// 12 px at 2 px per unit is 6 units either side.
		expect(offsets).toEqual([
			[-6, 0],
			[6, 0]
		]);
		expect(spreadOffsets(3, 1).map(([x, y]) => Math.round(Math.hypot(x, y)))).toEqual([12, 12, 12]);
		const moved = displaceBuses(
			g,
			geo,
			new Map([
				[1, offsets[0]],
				[2, offsets[1]]
			])
		);
		const at = (i: number) => [moved.positions[i * 3], moved.positions[i * 3 + 1]];
		expect(at(1)[0] - geo.positions[3]).toBeCloseTo(-6);
		expect(at(2)[0] - geo.positions[6]).toBeCloseTo(6);
		// The transformer now runs from the spread MV bus to the spread LV bus.
		const t = Array.from(moved.segmentEdge).indexOf(1);
		expect([moved.sources[t * 3], moved.targets[t * 3]]).toEqual([at(1)[0], at(2)[0]]);
		// The feeder line ends at MV, the LV line starts at LV.
		const f = Array.from(moved.segmentEdge).indexOf(0);
		const l = Array.from(moved.segmentEdge).indexOf(2);
		expect(moved.targets[f * 3]).toBe(at(1)[0]);
		expect(moved.sources[l * 3]).toBe(at(2)[0]);
		expect(displaceBuses(g, geo, new Map())).toBe(geo);
	});
});
