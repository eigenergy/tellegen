import { describe, expect, it } from 'vitest';
import { nearestLongitude, routeBounds, unwrapLongitudePath } from '../src/lib/antimeridian.js';

describe('unwrapLongitudePath', () => {
	it('keeps ordinary routes unchanged', () => {
		expect(
			unwrapLongitudePath([
				[-80, 35],
				[-79, 36],
				[-78, 35]
			])
		).toEqual([
			[-80, 35],
			[-79, 36],
			[-78, 35]
		]);
	});

	it('takes the short eastward hop across +180', () => {
		expect(
			unwrapLongitudePath([
				[179.5, 60],
				[-179.5, 61]
			])
		).toEqual([
			[179.5, 60],
			[180.5, 61]
		]);
	});

	it('takes the short westward hop across -180', () => {
		expect(
			unwrapLongitudePath([
				[-179.5, 60],
				[179.5, 61]
			])
		).toEqual([
			[-179.5, 60],
			[-180.5, 61]
		]);
	});

	it('continues unwrapping across more than one seam without changing latitude', () => {
		const out = unwrapLongitudePath([
			[179, 0],
			[-179, 1],
			[179, 2],
			[-178, 3]
		]);
		expect(out).toEqual([
			[179, 0],
			[181, 1],
			[179, 2],
			[182, 3]
		]);
		for (let i = 1; i < out.length; i++) {
			expect(Math.abs(out[i][0] - out[i - 1][0])).toBeLessThanOrEqual(180);
		}
	});

	it('does not drift on a route that zigzags across the seam', () => {
		const out = unwrapLongitudePath([
			[179.3, 64.95],
			[-179.8, 65.25],
			[179.8, 65.6],
			[-179.5, 65.95],
			[-179.12, 66.32]
		]);
		expect(out.map(([lon]) => lon)).toEqual(
			[179.3, 180.2, 179.8, 180.5, 180.88].map((v) => expect.closeTo(v, 9))
		);
	});

	it('handles a vertex exactly on ±180 from either side', () => {
		expect(
			unwrapLongitudePath([
				[179.5, 64.95],
				[180, 64.8],
				[-178.6, 64.6]
			])
		).toEqual([
			[179.5, 64.95],
			[180, 64.8],
			[expect.closeTo(181.4, 9), 64.6]
		]);
		expect(
			unwrapLongitudePath([
				[177.5, 64.73],
				[-180, 64.6],
				[-178.6, 64.6]
			])
		).toEqual([
			[177.5, 64.73],
			[180, 64.6],
			[expect.closeTo(181.4, 9), 64.6]
		]);
		expect(
			unwrapLongitudePath([
				[-178.6, 64.6],
				[180, 64.6],
				[177.5, 64.73]
			])
		).toEqual([
			[-178.6, 64.6],
			[-180, 64.6],
			[-182.5, 64.73]
		]);
	});

	it('copies a non-finite longitude and keeps unwrapping from the last finite one', () => {
		expect(
			unwrapLongitudePath([
				[Number.POSITIVE_INFINITY, 60],
				[179.5, 60],
				[Number.NaN, 60],
				[-179.5, 61]
			])
		).toEqual([
			[Number.POSITIVE_INFINITY, 60],
			[179.5, 60],
			[Number.NaN, 60],
			[180.5, 61]
		]);
	});

	it('brings a far out-of-range longitude near its neighbour in one step', () => {
		expect(
			unwrapLongitudePath([
				[0, 0],
				[1e12, 0]
			])[1][0]
		).toBeCloseTo(-80, 3);
		expect(
			unwrapLongitudePath([
				[0, 0],
				[1e300, 0]
			])
		).toHaveLength(2);
	});

	it('returns an empty path for an empty route', () => {
		expect(unwrapLongitudePath([])).toEqual([]);
	});
});

describe('nearestLongitude', () => {
	it('moves a longitude by whole turns to within 180° of the reference', () => {
		expect(nearestLongitude(-179.5, 179.5)).toBe(180.5);
		expect(nearestLongitude(179.5, -179.5)).toBe(-180.5);
		expect(nearestLongitude(190, 0)).toBe(-170);
		expect(nearestLongitude(-170, 900)).toBe(910);
	});

	it('leaves a longitude within 180° alone, the antipode included', () => {
		expect(nearestLongitude(-80, -79)).toBe(-80);
		expect(nearestLongitude(180, 0)).toBe(180);
		expect(nearestLongitude(-180, 0)).toBe(-180);
	});
});

describe('routeBounds', () => {
	it('bounds a seam-crossing route around the short arc', () => {
		const bounds = routeBounds([
			[179.5, 60],
			[-179.5, 61]
		]);
		expect(bounds).not.toBeNull();
		expect(bounds![1][0] - bounds![0][0]).toBeCloseTo(1, 9);
		expect(bounds![0][0]).toBeCloseTo(179.5, 9);
		expect(bounds![1][0]).toBeCloseTo(180.5, 9);
	});

	it('extends west past -180 for a route that starts east of the seam', () => {
		expect(
			routeBounds([
				[-179.12, 66.32],
				[179.6, 65.7],
				[177.5, 64.73]
			])
		).toEqual([
			[-182.5, 64.73],
			[-179.12, 66.32]
		]);
	});

	it('matches foldMapBounds on an ordinary route', () => {
		expect(
			routeBounds([
				[-81.2, 34.1],
				[-80.9, 34.4]
			])
		).toEqual([
			[-81.2, 34.1],
			[-80.9, 34.4]
		]);
	});

	it('ignores vertices the map cannot frame', () => {
		expect(
			routeBounds([
				[Number.NaN, 60],
				[179.5, 60],
				[181, 60],
				[0, 89],
				[-179.5, 61]
			])
		).toEqual([
			[179.5, 60],
			[180.5, 61]
		]);
		expect(
			routeBounds([
				[Number.NaN, 0],
				[1e300, 0]
			])
		).toBeNull();
	});

	it('pads a route that collapsed to one point', () => {
		const bounds = routeBounds([
			[180, 64.6],
			[-180, 64.6]
		]);
		expect(bounds).not.toBeNull();
		const [[minLon, minLat], [maxLon, maxLat]] = bounds!;
		expect(maxLon - minLon).toBeCloseTo(0.01, 9);
		expect(maxLat - minLat).toBeCloseTo(0.01, 9);
	});
});
