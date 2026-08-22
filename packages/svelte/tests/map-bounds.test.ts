import { describe, expect, it } from 'vitest';
import { foldMapBounds } from '../src/lib/map-bounds.js';

describe('foldMapBounds', () => {
	it('bounds valid points while ignoring non-finite and out-of-range coordinates', () => {
		expect(
			foldMapBounds([
				[Number.NaN, 0],
				[0, Number.POSITIVE_INFINITY],
				[-181, 0],
				[0, 85.051],
				[-81.2, 34.1],
				[-80.9, 34.4]
			])
		).toEqual([
			[-81.2, 34.1],
			[-80.9, 34.4]
		]);
	});

	it('returns null when no point can be rendered', () => {
		expect(
			foldMapBounds([
				[Number.NaN, 0],
				[0, Number.NEGATIVE_INFINITY],
				[181, 0]
			])
		).toBeNull();
	});
	it('pads a single surviving point into a box fitBounds can frame', () => {
		const bounds = foldMapBounds([
			[Number.NaN, 0],
			[-81.2, 34.1]
		]);
		expect(bounds).not.toBeNull();
		const [[minLon, minLat], [maxLon, maxLat]] = bounds!;
		expect(maxLon - minLon).toBeCloseTo(0.01, 9);
		expect(maxLat - minLat).toBeCloseTo(0.01, 9);
		expect((minLon + maxLon) / 2).toBeCloseTo(-81.2, 9);
		expect((minLat + maxLat) / 2).toBeCloseTo(34.1, 9);
	});

	it('pads only the axis that collapsed', () => {
		const bounds = foldMapBounds([
			[-81.2, 34.1],
			[-81.2, 34.4]
		]);
		expect(bounds).not.toBeNull();
		const [[minLon, minLat], [maxLon, maxLat]] = bounds!;
		expect(maxLon - minLon).toBeCloseTo(0.01, 9);
		expect(minLat).toBe(34.1);
		expect(maxLat).toBe(34.4);
	});
});
