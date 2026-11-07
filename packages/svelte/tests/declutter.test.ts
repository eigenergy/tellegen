import { describe, expect, it } from 'vitest';
import type { NetworkBus } from '../src/lib/api.js';
import {
	DECLUTTER_CELL_PX,
	declutterBuses,
	declutterZoom,
	kindRank,
	withBus
} from '../src/lib/declutter.js';

const bus = (id: number, lon: number, lat: number, demand_mw = 0, gen_mw = 0): NetworkBus => ({
	id,
	lon,
	lat,
	demand_mw,
	gen_mw
});

const ids = (buses: NetworkBus[]) => buses.map((b) => b.id);

/** A deterministic generator in [0, 1), so the crowd is the same every run. */
function random(seed: number) {
	let state = seed >>> 0;
	return () => {
		state = (state + 0x6d2b79f5) >>> 0;
		let t = state;
		t = Math.imul(t ^ (t >>> 15), t | 1);
		t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
		return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
	};
}

/** 3,000 buses around Los Angeles, the Bay Area and Sacramento, with a thin
 * scatter over the rest of California. */
function californiaCrowd(): NetworkBus[] {
	const next = random(2026);
	const centres: [number, number, number, number][] = [
		[-118.25, 34.05, 0.45, 0.5],
		[-122.2, 37.6, 0.3, 0.25],
		[-121.4, 38.6, 0.25, 0.1]
	];
	const buses: NetworkBus[] = [];
	for (let id = 1; id <= 3000; id++) {
		const pick = next();
		let lon: number;
		let lat: number;
		let acc = 0;
		const centre = centres.find(([, , , share]) => (acc += share) > pick);
		if (centre) {
			const [cLon, cLat, spread] = centre;
			lon = cLon + (next() + next() + next() - 1.5) * spread;
			lat = cLat + (next() + next() + next() - 1.5) * spread;
		} else {
			lon = -124 + next() * 10;
			lat = 32.5 + next() * 9.5;
		}
		buses.push(bus(id, lon, lat, Math.round(next() * 80), next() < 0.1 ? 200 : 0));
	}
	return buses;
}

describe('declutterZoom', () => {
	it('rounds down to quarter steps', () => {
		expect(declutterZoom(4.7)).toBe(4.5);
		expect(declutterZoom(4.75)).toBe(4.75);
		expect(declutterZoom(4.99)).toBe(4.75);
		expect(declutterZoom(8)).toBe(8);
		expect(declutterZoom(0.1)).toBe(0);
	});
});

describe('declutterBuses', () => {
	it('returns sparse input unchanged, as the same array', () => {
		const buses = [bus(1, -122, 37), bus(2, -118, 34), bus(3, -120, 36)];
		expect(declutterBuses(buses, { zoom: 5 })).toBe(buses);
		expect(declutterBuses([], { zoom: 5 })).toEqual([]);
	});

	it('keeps a generator, then a substation, over a larger bus of another kind', () => {
		// Four buses within a few metres of each other share any cell.
		const buses = [
			bus(1, -120, 36, 500),
			bus(2, -120.00001, 36, 10),
			bus(3, -120, 36.00001, 0, 5),
			bus(4, -120.00001, 36.00001, 900)
		];
		const kinds = new Map([
			[1, 'load'],
			[2, 'substation'],
			[3, 'generator'],
			[4, 'load']
		]);
		expect(ids(declutterBuses(buses, { zoom: 6, kinds }))).toEqual([3]);
		kinds.delete(3);
		expect(ids(declutterBuses(buses, { zoom: 6, kinds }))).toEqual([2]);
	});

	it('ranks converter stations with substations', () => {
		expect(kindRank('generator')).toBeGreaterThan(kindRank('substation'));
		expect(kindRank('converter')).toBe(kindRank('substation'));
		expect(kindRank('substation')).toBeGreaterThan(kindRank('load'));
		expect(kindRank(undefined)).toBe(kindRank('load'));
	});

	it('keeps the larger bus when kinds are unknown, then the lower id', () => {
		const buses = [bus(5, -120, 36, 20, 0), bus(3, -120, 36, 0, -60), bus(4, -120, 36, 30, 30)];
		// |gen| + |demand|: 20, 60, 60. Ids 3 and 4 tie; 3 is lower.
		expect(ids(declutterBuses(buses, { zoom: 6 }))).toEqual([3]);
	});

	it('is deterministic: the same buses in any order keep the same set, in input order', () => {
		const crowd = californiaCrowd();
		const kept = ids(declutterBuses(crowd, { zoom: 5 }));
		const reversed = [...crowd].reverse();
		const keptReversed = ids(declutterBuses(reversed, { zoom: 5 }));
		expect(keptReversed).toEqual([...kept].reverse());
		expect(ids(declutterBuses(crowd, { zoom: 5 }))).toEqual(kept);
		// In input order.
		expect(kept).toEqual([...kept].sort((a, b) => a - b));
	});

	it('keeps one bus per cell of the crowd, the strongest one', () => {
		const crowd = californiaCrowd();
		const zoom = 5;
		const kept = declutterBuses(crowd, { zoom });
		expect(kept.length).toBeLessThan(crowd.length / 5);
		const world = 512 * 2 ** zoom;
		const columns = Math.round(world / DECLUTTER_CELL_PX);
		const cellOf = (b: NetworkBus) => {
			const x = ((b.lon + 180) / 360) * world;
			const phi = (b.lat * Math.PI) / 180;
			const y = (0.5 - Math.log(Math.tan(Math.PI / 4 + phi / 2)) / (2 * Math.PI)) * world;
			const cell = world / columns;
			return `${Math.floor(x / cell)},${Math.floor(y / cell)}`;
		};
		const keptCells = new Map(kept.map((b) => [cellOf(b), b]));
		// One per occupied cell.
		expect(keptCells.size).toBe(kept.length);
		expect(new Set(crowd.map(cellOf)).size).toBe(kept.length);
		// No bus in a cell outweighs the one it kept.
		const weight = (b: NetworkBus) => Math.abs(b.gen_mw) + Math.abs(b.demand_mw);
		for (const b of crowd) {
			const winner = keptCells.get(cellOf(b))!;
			expect(weight(b)).toBeLessThanOrEqual(weight(winner));
		}
	});

	it('keeps more buses as the map zooms in, and all of them close up', () => {
		const crowd = californiaCrowd();
		const counts = [4.5, 5.5, 6.5, 8, 10].map((zoom) => declutterBuses(crowd, { zoom }).length);
		for (let i = 1; i < counts.length; i++) expect(counts[i]).toBeGreaterThan(counts[i - 1]);
		expect(counts[counts.length - 1]).toBeLessThanOrEqual(crowd.length);
		const distinct = [bus(1, -120, 36), bus(2, -120.001, 36.001)];
		expect(declutterBuses(distinct, { zoom: 20 })).toBe(distinct);
	});

	it('lays the cells out in screen pixels at a MapLibre zoom', () => {
		// At zoom 6 the world is 512·64 px across; on the equator a pixel is
		// 360 / 32768 degrees in both directions.
		const zoom = 6;
		const px = 360 / (512 * 2 ** zoom);
		const centre = (DECLUTTER_CELL_PX / 2) * px;
		// 3 px either side of one cell's centre: one cell.
		const near = [bus(1, centre - 3 * px, -centre, 1), bus(2, centre + 3 * px, -centre, 2)];
		expect(ids(declutterBuses(near, { zoom }))).toEqual([2]);
		// The centres of two neighbouring cells: two cells.
		const apart = [bus(1, centre, -centre), bus(2, centre + DECLUTTER_CELL_PX * px, -centre)];
		expect(declutterBuses(apart, { zoom })).toBe(apart);
	});

	it('takes a cell size', () => {
		const crowd = californiaCrowd();
		const fine = declutterBuses(crowd, { zoom: 6, cellPx: 6 }).length;
		const coarse = declutterBuses(crowd, { zoom: 6, cellPx: 24 }).length;
		expect(fine).toBeGreaterThan(coarse);
	});

	it('wraps longitude, so 190 and -170 share a cell, as do 180 and -180', () => {
		expect(ids(declutterBuses([bus(1, 190, 60, 5), bus(2, -170, 60, 1)], { zoom: 6 }))).toEqual([
			1
		]);
		expect(ids(declutterBuses([bus(1, 180, 60, 1), bus(2, -180, 60, 5)], { zoom: 6 }))).toEqual([
			2
		]);
		// The seam is a cell edge: buses either side of it are neighbours, not one cell.
		const seam = [bus(1, 179.99, 60), bus(2, -179.99, 60)];
		expect(declutterBuses(seam, { zoom: 6 })).toBe(seam);
	});

	it('clamps latitude to the Mercator limit and keeps buses without coordinates', () => {
		const polar = [bus(1, 10, 90, 1), bus(2, 10, 89, 2), bus(3, 10, -90, 3)];
		expect(ids(declutterBuses(polar, { zoom: 3 }))).toEqual([2, 3]);
		const lost = [bus(1, Number.NaN, 36), bus(2, -120, 36, 5), bus(3, -120, 36, 1)];
		expect(ids(declutterBuses(lost, { zoom: 6 }))).toEqual([1, 2]);
	});
});

describe('the selected bus', () => {
	it('always shows, on top, without hiding the bus its spot kept', () => {
		const buses = [bus(1, -120, 36, 100), bus(2, -120, 36, 1), bus(3, -120, 36, 50)];
		const thinned = declutterBuses(buses, { zoom: 6 });
		expect(ids(thinned)).toEqual([1]);
		expect(ids(withBus(thinned, buses, 2))).toEqual([1, 2]);
		expect(withBus(thinned, buses, 1)).toBe(thinned);
		const crowd = californiaCrowd();
		const crowdThinned = declutterBuses(crowd, { zoom: 4.5 });
		for (const id of [7, 1234, 2999]) {
			const shown = withBus(crowdThinned, crowd, id);
			expect(ids(shown)).toContain(id);
			expect(shown.length - crowdThinned.length).toBeLessThanOrEqual(1);
		}
	});
});

describe('withBus', () => {
	const all = [bus(1, 0, 0), bus(2, 1, 1), bus(3, 2, 2)];
	const shown = [all[0], all[2]];

	it('adds a missing bus at the end', () => {
		expect(ids(withBus(shown, all, 2))).toEqual([1, 3, 2]);
	});

	it('returns the same array when the bus is drawn, unknown or none is given', () => {
		expect(withBus(shown, all, 1)).toBe(shown);
		expect(withBus(shown, all, 9)).toBe(shown);
		expect(withBus(shown, all, null)).toBe(shown);
	});
});
