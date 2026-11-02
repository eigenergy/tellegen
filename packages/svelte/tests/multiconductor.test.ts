import { describe, expect, it } from 'vitest';
import type { DistGraph } from '@tellegen/engine';
import {
	buildGeographicView,
	edgeWidth,
	isPhaseTerminal,
	phaseColor,
	placeMultiView,
	neutralTerminal,
	transformerMarks,
	type PlacedMultiEdge
} from '../src/lib/multiconductor.js';

describe('neutralTerminal', () => {
	it('uses explicit metadata and does not infer arbitrary non-phase terminals', () => {
		expect(neutralTerminal(['a', 'b', 'c'])).toBeNull();
		expect(neutralTerminal(['a', 'return'], 'return')).toBe('return');
		expect(neutralTerminal(['a', 'return'], null)).toBeNull();
	});

	it('recognizes the conventional neutral aliases for older graph payloads', () => {
		expect(neutralTerminal(['a', 'b', 'c', 'n'])).toBe('n');
		expect(neutralTerminal(['1', '2', '3', '4'])).toBe('4');
	});
});

/** A small three-bus feeder graph: source -> line -> load bus -> transformer ->
 * secondary. Two buses carry geographic coordinates, one does not. */
function graph(withCoords: boolean): DistGraph {
	return {
		buses: [
			{
				id: 'src',
				terminals: ['1', '2', '3', '4'],
				grounded: ['4'],
				xy: withCoords ? [-83.92, 35.96] : undefined,
				load_kw: 0,
				gen_kw: 0,
				has_source: true,
				terminal_attachments: { '1': [{ kind: 'source', id: 'vs' }] }
			},
			{
				id: 'load_bus',
				terminals: ['1', '2', '3', '4'],
				grounded: ['4'],
				xy: withCoords ? [-83.9, 35.95] : undefined,
				load_kw: 150,
				gen_kw: 0,
				has_source: false,
				terminal_attachments: { '1': [{ kind: 'load', id: 'ld1' }] }
			},
			{
				id: 'secondary',
				terminals: ['1', '2', '3'],
				grounded: [],
				load_kw: 20,
				gen_kw: 0,
				has_source: false
			}
		],
		edges: [
			{
				kind: 'line',
				id: 'l1',
				from: 'src',
				to: 'load_bus',
				conductors: [
					['1', '1'],
					['2', '2'],
					['3', '3']
				],
				closed: true,
				n_phases: 3
			},
			{
				kind: 'transformer',
				id: 't1',
				from: 'load_bus',
				to: 'secondary',
				conductors: [['1', '1']],
				closed: true,
				n_phases: 3
			}
		]
	};
}

describe('buildGeographicView', () => {
	it('places buses at their [lon, lat] and draws every edge whose endpoints are placed', () => {
		const view = buildGeographicView(graph(true));
		// secondary has no xy, so it and the transformer edge into it are dropped.
		expect(view.buses).toHaveLength(2);
		const src = view.buses.find((b) => b.id === 'src')!;
		expect([src.lon, src.lat]).toEqual([-83.92, 35.96]);
		expect(view.buses.some((b) => b.id === 'secondary')).toBe(false);
		expect(view.edges).toHaveLength(1);
		expect(view.edges[0].id).toBe('l1');
	});

	it('carries the terminal detail through to the placed bus', () => {
		const view = buildGeographicView(graph(true));
		const loadBus = view.buses.find((b) => b.id === 'load_bus')!;
		expect(loadBus.attachmentKinds).toContain('load');
		expect(loadBus.terminalAttachments['1'][0].kind).toBe('load');
	});

	it('routes an edge by id or endpoints, ignoring case, only when exactly one edge matches', () => {
		const bus = (id: string, xy: [number, number]) => ({
			id,
			terminals: ['1'],
			grounded: [],
			xy,
			load_kw: 0,
			gen_kw: 0,
			has_source: false
		});
		const edge = (id: string, from: string, to: string) => ({
			kind: 'line' as const,
			id,
			from,
			to,
			conductors: [['1', '1']] as [string, string][],
			closed: true,
			n_phases: 1
		});
		const routed: DistGraph = {
			buses: [bus('a', [0, 0]), bus('b', [1, 0]), bus('c', [1, 1])],
			edges: [
				edge('e1', 'a', 'b'),
				edge('e2', 'b', 'c'),
				edge('e3', 'b', 'c'),
				edge('e4', 'a', 'c'),
				edge('e5', 'c', 'c')
			]
		};
		const route = (properties: Record<string, unknown>, coordinates: number[][]) => ({
			properties,
			geometry: { type: 'LineString', coordinates }
		});
		const layer = JSON.stringify({
			features: [
				route({ uid: 'E1' }, [
					[0, 0],
					[0.5, 0.1],
					[1, 0]
				]),
				// e2 and e3 are parallel, so their shared endpoints name neither.
				route({ from: 'C', to: 'B' }, [
					[1, 1],
					[1.1, 0.5],
					[1, 0]
				]),
				// Endpoints stated against the edge's direction reverse the path.
				route({ from: 'C', to: 'a' }, [
					[1, 1],
					[0.4, 0.6],
					[0, 0]
				]),
				// A self loop matches once, not once per direction.
				route({ from: 'c', to: 'C' }, [
					[1, 1],
					[1.2, 1.2],
					[1, 1]
				]),
				// An id that is not a string matches nothing.
				route({ id: 4, from: 'a', to: 'b' }, [
					[0, 0],
					[9, 9]
				])
			]
		});
		const paths = Object.fromEntries(
			buildGeographicView(routed, layer).edges.map((e) => [e.id, e.path])
		);
		expect(paths.e1).toEqual([
			[0, 0],
			[0.5, 0.1],
			[1, 0]
		]);
		expect(paths.e2).toEqual([
			[1, 0],
			[1, 1]
		]);
		expect(paths.e3).toEqual(paths.e2);
		expect(paths.e4).toEqual([
			[0, 0],
			[0.4, 0.6],
			[1, 1]
		]);
		expect(paths.e5).toHaveLength(3);
	});
});

describe('placeMultiView', () => {
	it('lays out a coordinate-free graph synthetically around the center', () => {
		const center = { lon: -80, lat: 35 };
		const view = placeMultiView(graph(false), center);
		expect(view.buses).toHaveLength(3);
		expect(view.edges).toHaveLength(2);
		// Every placed bus lands in a bounded box near the center.
		for (const b of view.buses) {
			expect(Math.abs(b.lon - center.lon)).toBeLessThan(6);
			expect(Math.abs(b.lat - center.lat)).toBeLessThan(6);
			expect(Number.isFinite(b.lon)).toBe(true);
			expect(Number.isFinite(b.lat)).toBe(true);
		}
	});

	it('fits provided planar coordinates into a box at the center', () => {
		// A graph with planar (non-geographic) coordinates: the layout keeps every
		// placed bus, projected near the center rather than at the raw coordinates.
		const g = graph(false);
		g.buses[0].xy = [0, 0];
		g.buses[1].xy = [100, 0];
		g.buses[2].xy = [100, 100];
		const center = { lon: 10, lat: 20 };
		const view = placeMultiView(g, center);
		expect(view.buses).toHaveLength(3);
		for (const b of view.buses) {
			expect(Math.abs(b.lon - center.lon)).toBeLessThan(6);
			expect(Math.abs(b.lat - center.lat)).toBeLessThan(6);
		}
	});

	it('roots the synthetic tree layout at the source bus', () => {
		// src -> load_bus -> secondary is radial; the source-hinted root lands at
		// the west extreme and depth grows east along the feeder.
		const view = placeMultiView(graph(false), { lon: 0, lat: 0 });
		const lon = new Map(view.buses.map((b) => [b.id, b.lon]));
		expect(lon.get('src')!).toBeLessThan(lon.get('load_bus')!);
		expect(lon.get('load_bus')!).toBeLessThan(lon.get('secondary')!);
	});

	it('preserves the aspect ratio of planar drawings', () => {
		// A 100 x 10 drawing at the equator: the projected ranges keep ~10:1
		// rather than being stretched to a square.
		const g = graph(false);
		g.buses[0].xy = [0, 0];
		g.buses[1].xy = [50, 10];
		g.buses[2].xy = [100, 0];
		const view = placeMultiView(g, { lon: 0, lat: 0 });
		const lons = view.buses.map((b) => b.lon);
		const lats = view.buses.map((b) => b.lat);
		const lonRange = Math.max(...lons) - Math.min(...lons);
		const latRange = Math.max(...lats) - Math.min(...lats);
		expect(lonRange / latRange).toBeGreaterThan(8);
		expect(lonRange / latRange).toBeLessThan(12);
	});
});

describe('transformerMarks', () => {
	function edge(id: string, path: [[number, number], [number, number]]): PlacedMultiEdge {
		return {
			id,
			kind: 'transformer',
			from: 'a',
			to: 'b',
			conductors: [['1', '1']],
			closed: true,
			n_phases: 1,
			path
		};
	}

	it('marks transformer midpoints angled along the edge', () => {
		const marks = transformerMarks([
			edge('east', [
				[0, 0],
				[10, 0]
			]),
			edge('north', [
				[0, 0],
				[0, 10]
			])
		]);
		expect(marks).toHaveLength(2);
		expect(marks[0].position).toEqual([5, 0]);
		expect(marks[0].angle).toBeCloseTo(0, 6);
		expect(marks[1].position).toEqual([0, 5]);
		expect(marks[1].angle).toBeCloseTo(90, 6);
	});

	it('skips lines and switches', () => {
		const line = {
			...edge('l', [
				[0, 0],
				[1, 1]
			]),
			kind: 'line' as const
		};
		expect(transformerMarks([line])).toHaveLength(0);
	});
});

describe('palette', () => {
	it('maps node numbers and named phases to the three phase colors, others to neutral', () => {
		expect(phaseColor('1')).toEqual(phaseColor('a'));
		expect(phaseColor('2')).toEqual(phaseColor('b'));
		expect(phaseColor('3')).toEqual(phaseColor('c'));
		// Distinct phases.
		expect(phaseColor('1')).not.toEqual(phaseColor('2'));
		// Neutral / ground returns are not phase colors.
		expect(isPhaseTerminal('4')).toBe(false);
		expect(isPhaseTerminal('n')).toBe(false);
		expect(isPhaseTerminal('1')).toBe(true);
		expect(phaseColor('4')).toEqual(phaseColor('n'));
	});

	it('grows edge width with conductor count and draws transformers wider', () => {
		expect(edgeWidth('line', 3)).toBeGreaterThan(edgeWidth('line', 1));
		expect(edgeWidth('transformer', 3)).toBeGreaterThan(edgeWidth('line', 3));
	});
});
