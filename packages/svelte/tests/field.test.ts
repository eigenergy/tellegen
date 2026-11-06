import { describe, expect, it } from 'vitest';
import {
	voltageBandPosition,
	fieldImage,
	median,
	priceFieldColour,
	priceSpread,
	voltageFieldColour
} from '../src/lib/diagram/field.js';
import type { RGBA } from '../src/lib/colors.js';

const grey = (v: number): RGBA => [v, v, v, 200];
const pixel = (img: NonNullable<ReturnType<typeof fieldImage>>, x: number, y: number) => {
	const [x0, y0, x1, y1] = img.bounds;
	const px = Math.floor(((x - x0) / (x1 - x0)) * img.width);
	const py = Math.floor(((y - y0) / (y1 - y0)) * img.height);
	return Array.from(img.data.subarray((py * img.width + px) * 4, (py * img.width + px) * 4 + 4));
};

describe('bus value field', () => {
	it('interpolates between buses and fades out beyond reach', () => {
		const positions = Float32Array.from([0, 0, 0, 100, 0, 0]);
		const img = fieldImage(positions, [0, 200], grey, { reach: 60, maxSize: 200 })!;
		expect(img.bounds).toEqual([-60, -60, 160, 60]);
		// Next to each bus, its own value at full strength.
		expect(pixel(img, 1, 0)[0]).toBeLessThan(10);
		expect(pixel(img, 99, 0)[0]).toBeGreaterThan(190);
		expect(pixel(img, 1, 0)[3]).toBe(200);
		// Halfway, the mean.
		expect(pixel(img, 50, 0)[0]).toBeGreaterThan(90);
		expect(pixel(img, 50, 0)[0]).toBeLessThan(110);
		// Far from both, transparent.
		expect(pixel(img, 50, 58)[3]).toBe(0);
		expect(pixel(img, -58, 58)[3]).toBe(0);
	});

	it('skips missing values and returns null when nothing is finite', () => {
		const positions = Float32Array.from([0, 0, 0, 10, 0, 0]);
		expect(fieldImage(positions, [NaN, NaN], grey, { reach: 5 })).toBeNull();
		const img = fieldImage(positions, [NaN, 100], grey, { reach: 5, maxSize: 40 })!;
		// The missing bus neither shades nor extends the image.
		expect(img.bounds).toEqual([5, -5, 15, 5]);
		expect(pixel(img, 10, 0)[0]).toBe(100);
	});

	it('shades voltage by its place in the bus band: clear mid-band, strong at a limit', () => {
		expect(voltageBandPosition(1.0, 0.95, 1.05)).toBeCloseTo(0);
		expect(voltageBandPosition(1.05, 0.95, 1.05)).toBeCloseTo(1);
		expect(voltageBandPosition(0.9, 0.95, 1.05)).toBeCloseTo(-2);
		expect(voltageBandPosition(1, undefined, 1.05)).toBeUndefined();
		expect(voltageBandPosition(1, 1.05, 0.95)).toBeUndefined();
		expect(voltageFieldColour(0)).toBeNull();
		expect(voltageFieldColour(0.4)).toBeNull();
		const low = voltageFieldColour(-0.9)!;
		const high = voltageFieldColour(0.9)!;
		expect(low[2]).toBeGreaterThan(low[0]);
		expect(high[0]).toBeGreaterThan(high[2]);
		expect(voltageFieldColour(-1)![3]).toBeGreaterThan(voltageFieldColour(-0.7)![3]);
		expect(voltageFieldColour(1.5)![3]).toBe(voltageFieldColour(1)![3]);
	});
});

describe('price separation shading', () => {
	it('centres on the median and ignores small spreads', () => {
		expect(median([3, 1, 2])).toBe(2);
		expect(median([4, 1, 3, 2])).toBe(2.5);
		expect(median([NaN])).toBeUndefined();
		// A spread of 0.02 on a price of 30 is noise: the full-strength spread is 5% of 30.
		expect(priceSpread([29.99, 30, 30.01], 30)).toBeCloseTo(1.5);
		expect(priceFieldColour(0.01, 1.5)).toBeNull();
		// Real separation shades by direction.
		const spread = priceSpread([10, 30, 60], 30);
		expect(spread).toBe(30);
		const cheap = priceFieldColour(-20, spread)!;
		const dear = priceFieldColour(30, spread)!;
		expect(cheap[2]).toBeGreaterThan(cheap[0]);
		expect(dear[0]).toBeGreaterThan(dear[2]);
		expect(dear[3]).toBeGreaterThan(cheap[3]);
	});
});
