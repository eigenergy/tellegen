import { describe, expect, it } from 'vitest';
import { DEAD_ZONE_MW, sliderKeyStep, sliderKeyValue } from '../src/lib/slider.js';

describe('slider arrow keys', () => {
	it('step about 1% of the span, rounded up to 1, 2 or 5 times a power of ten', () => {
		expect(sliderKeyStep(80)).toBe(1);
		expect(sliderKeyStep(150)).toBe(2);
		expect(sliderKeyStep(300)).toBe(5);
		expect(sliderKeyStep(1000)).toBe(10);
		expect(sliderKeyStep(1600)).toBe(20);
	});

	it('never step less than twice the dead zone', () => {
		for (const span of [1e-9, 1, 5, 10, 24, 49.9]) {
			expect(sliderKeyStep(span)).toBe(2 * DEAD_ZONE_MW);
		}
	});

	it('leave the dead zone with one press on a bus under 50 MW', () => {
		// The native 1% step of these spans stays inside the 0.25 MW dead zone,
		// so the arrows used to do nothing. Committed values off the step grid
		// are the ones rounding error could catch.
		for (const span of [2, 7.5, 12, 24, 49]) {
			for (const committed of [0, 0.1, 0.13, 1.13, -0.37, span / 3]) {
				const min = committed - span / 2;
				const max = committed + span / 2;
				for (const key of ['ArrowRight', 'ArrowUp', 'ArrowLeft', 'ArrowDown']) {
					const next = sliderKeyValue(key, committed, min, max);
					expect(next).not.toBeNull();
					expect(Math.abs(next! - committed)).toBeGreaterThanOrEqual(DEAD_ZONE_MW);
				}
			}
		}
	});

	it('move up for right and up, down for left and down, within the bounds', () => {
		expect(sliderKeyValue('ArrowRight', 0, -10, 10)).toBe(0.5);
		expect(sliderKeyValue('ArrowUp', 0, -10, 10)).toBe(0.5);
		expect(sliderKeyValue('ArrowLeft', 0, -10, 10)).toBe(-0.5);
		expect(sliderKeyValue('ArrowDown', 0, -10, 10)).toBe(-0.5);
		expect(sliderKeyValue('ArrowRight', 9.8, -10, 10)).toBe(10);
		expect(sliderKeyValue('ArrowLeft', -9.8, -10, 10)).toBe(-10);
	});

	it('leave every other key to the browser', () => {
		for (const key of ['Home', 'End', 'PageUp', 'PageDown', 'Tab', 'a']) {
			expect(sliderKeyValue(key, 0, -10, 10)).toBeNull();
		}
	});
});
