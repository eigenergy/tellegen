import { describe, expect, it } from 'vitest';
import { routeBounds, unwrapLongitudePath } from '../src/lib/antimeridian.js';

describe('unwrapLongitudePath', () => {
	it('keeps ordinary routes unchanged', () => {
		expect(unwrapLongitudePath([[-80, 35], [-79, 36], [-78, 35]])).toEqual([
			[-80, 35],
			[-79, 36],
			[-78, 35]
		]);
	});

	it('takes the short eastward hop across +180', () => {
		expect(unwrapLongitudePath([[179.5, 60], [-179.5, 61]])).toEqual([
			[179.5, 60],
			[180.5, 61]
		]);
	});

	it('takes the short westward hop across -180', () => {
		expect(unwrapLongitudePath([[-179.5, 60], [179.5, 61]])).toEqual([
			[-179.5, 60],
			[-180.5, 61]
		]);
	});

	it('continues unwrapping across more than one seam without changing latitude', () => {
		const out = unwrapLongitudePath([[179, 0], [-179, 1], [179, 2], [-178, 3]]);
		expect(out).toEqual([[179, 0], [181, 1], [179, 2], [182, 3]]);
		for (let i = 1; i < out.length; i++) {
			expect(Math.abs(out[i][0] - out[i - 1][0])).toBeLessThanOrEqual(180);
		}
	});

	it('bounds a seam-crossing route around the short arc', () => {
		const bounds = routeBounds(unwrapLongitudePath([[179.5, 60], [-179.5, 61]]));
		expect(bounds).not.toBeNull();
		expect(bounds![1][0] - bounds![0][0]).toBeCloseTo(1, 9);
		expect(bounds![0][0]).toBeCloseTo(179.5, 9);
		expect(bounds![1][0]).toBeCloseTo(180.5, 9);
	});
});
