import { describe, expect, it } from 'vitest';
import type { Topology } from '@tellegen/engine';
import { placeSyntheticTopology } from '../src/lib/synthetic-layout.js';

/** Build a Topology from an edge list over n buses (ids 0..n-1). */
function topology(n: number, edges: [number, number][]): Topology {
	return {
		buses: Array.from({ length: n }, (_, i) => ({
			id: i,
			uid: `b${i}`,
			demand_mw: 0,
			gen_mw: 0
		})),
		branches: edges.map(([from, to], i) => ({
			id: i,
			uid: `e${i}`,
			from,
			to,
			rate_mw: 0,
			status: 1
		}))
	};
}

/** Chain 0-1-...-(n-1). */
function path(n: number): [number, number][] {
	return Array.from({ length: n - 1 }, (_, i) => [i, i + 1] as [number, number]);
}

// Center at the equator so a longitude degree equals a latitude degree and
// aspect assertions read directly off the coordinate ranges.
const CENTER = { lon: 0, lat: 0 };

function lonsById(placed: ReturnType<typeof placeSyntheticTopology>): Map<number, number> {
	return new Map(placed.buses.map((b) => [b.id, b.lon]));
}

describe('placeSyntheticTopology tree layout', () => {
	it('lays a path graph on a straight line with monotone depth', () => {
		const placed = placeSyntheticTopology(topology(6, path(6)), CENTER, { roots: [0] });
		const lats = placed.buses.map((b) => b.lat);
		for (const lat of lats) expect(lat).toBeCloseTo(lats[0], 9);
		const lon = lonsById(placed);
		for (let i = 1; i < 6; i++) {
			expect(lon.get(i)!).toBeGreaterThan(lon.get(i - 1)!);
		}
	});

	it('puts a hinted root at the west extreme with children strictly east of parents', () => {
		// Balanced binary tree: 0 -> (1, 2), 1 -> (3, 4), 2 -> (5, 6).
		const edges: [number, number][] = [
			[0, 1],
			[0, 2],
			[1, 3],
			[1, 4],
			[2, 5],
			[2, 6]
		];
		const placed = placeSyntheticTopology(topology(7, edges), CENTER, { roots: [0] });
		const lon = lonsById(placed);
		expect(Math.min(...lon.values())).toBeCloseTo(lon.get(0)!, 9);
		for (const [parent, child] of edges) {
			expect(lon.get(child)!).toBeGreaterThan(lon.get(parent)!);
		}
		// No two buses share a position.
		const keys = new Set(placed.buses.map((b) => `${b.lon},${b.lat}`));
		expect(keys.size).toBe(7);
	});

	it('is deterministic', () => {
		const edges: [number, number][] = [...path(8), [3, 8], [3, 9], [8, 10]];
		const a = placeSyntheticTopology(topology(11, edges), CENTER, { roots: [0] });
		const b = placeSyntheticTopology(topology(11, edges), CENTER, { roots: [0] });
		expect(a).toEqual(b);
	});

	it('keeps a long feeder long: aspect ratio survives normalization', () => {
		// A 10-bus trunk with one lateral off the middle: depth range 9, slot
		// range 1. A square-stretched normalize would render this 1:1.
		const edges: [number, number][] = [...path(10), [5, 10]];
		const placed = placeSyntheticTopology(topology(11, edges), CENTER, { roots: [0] });
		const lons = placed.buses.map((b) => b.lon);
		const lats = placed.buses.map((b) => b.lat);
		const lonRange = Math.max(...lons) - Math.min(...lons);
		const latRange = Math.max(...lats) - Math.min(...lats);
		expect(lonRange / latRange).toBeGreaterThan(5);
	});

	it('still tree-lays a near-tree with a closed loop, placing every bus', () => {
		// A radial feeder with one closed tie (4-9) forming a single loop.
		const edges: [number, number][] = [...path(8), [2, 8], [8, 9], [4, 9]];
		const placed = placeSyntheticTopology(topology(10, edges), CENTER, { roots: [0] });
		expect(placed.buses).toHaveLength(10);
		const lon = lonsById(placed);
		expect(Math.min(...lon.values())).toBeCloseTo(lon.get(0)!, 9);
		for (const b of placed.buses) {
			expect(Number.isFinite(b.lon)).toBe(true);
			expect(Number.isFinite(b.lat)).toBe(true);
		}
	});

	it('falls back to the force layout for meshed graphs', () => {
		// K5: 6 independent cycles, well past the near-tree tolerance.
		const edges: [number, number][] = [];
		for (let i = 0; i < 5; i++) {
			for (let j = i + 1; j < 5; j++) edges.push([i, j]);
		}
		const placed = placeSyntheticTopology(topology(5, edges), CENTER);
		expect(placed.buses).toHaveLength(5);
		const keys = new Set(placed.buses.map((b) => `${b.lon},${b.lat}`));
		expect(keys.size).toBe(5);
		for (const b of placed.buses) {
			expect(Math.abs(b.lon)).toBeLessThan(6);
			expect(Math.abs(b.lat)).toBeLessThan(6);
		}
	});
});
