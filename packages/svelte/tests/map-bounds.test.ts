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
});
