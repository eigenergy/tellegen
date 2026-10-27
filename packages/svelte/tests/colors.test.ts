import { describe, expect, it } from 'vitest';
import {
	busMarkerRadius,
	busRadius,
	busRadiusFor,
	demandColor,
	demandDomain,
	demandGradient,
	previewScaleFor,
	scalarDomain,
	sensitivityDomain,
	UNIFORM_BUS_GROUND_RADIUS,
	UNIFORM_BUS_MIN_RADIUS,
	UNIFORM_BUS_RADIUS
} from '../src/lib/colors.js';

const column = [0.0017, -0.0004, 0.0009, -0.0016, 0.0002, 0.0011, -0.0008];

describe('sensitivityDomain', () => {
	it('is degree-1 homogeneous in its inputs (the cancellation behind the preview bug)', () => {
		const base = sensitivityDomain(column);
		for (const k of [0.1, 2.5, 40]) {
			const scaled = sensitivityDomain(column.map((v) => v * k));
			expect(scaled.scale).toBeCloseTo(base.scale * k, 12);
		}
	});

	it('flags constant columns flat and structured columns not', () => {
		expect(sensitivityDomain([0.5, 0.5, 0.5]).flat).toBe(true);
		expect(sensitivityDomain(column).flat).toBe(false);
	});
});

describe('previewScaleFor', () => {
	it('is linear in the slider deflection', () => {
		const domain = sensitivityDomain(column);
		const atOne = previewScaleFor(domain, 1);
		const atFive = previewScaleFor(domain, 5);
		expect(atOne).not.toBeNull();
		expect(atFive).toBeCloseTo((atOne as number) * 5, 12);
	});

	it('saturates the robust-max bus exactly at full deflection', () => {
		const domain = sensitivityDomain(column);
		const maxAbsStep = 5;
		const scale = previewScaleFor(domain, maxAbsStep) as number;
		// The robust-max column value at the full step divides to |t| = 1.
		expect(Math.abs((domain.scale * maxAbsStep) / scale)).toBeCloseTo(1, 12);
		// A half deflection lands at half intensity, not full.
		expect(Math.abs((domain.scale * (maxAbsStep / 2)) / scale)).toBeCloseTo(0.5, 12);
	});

	it('returns null for flat columns', () => {
		expect(previewScaleFor(sensitivityDomain([1, 1, 1]), 5)).toBeNull();
	});

	it('floors the deflection so a degenerate range cannot divide by zero', () => {
		const domain = sensitivityDomain(column);
		const scale = previewScaleFor(domain, 0);
		expect(scale).not.toBeNull();
		expect(scale as number).toBeGreaterThan(0);
	});
});

describe('demand encoding', () => {
	const luminance = ([r, g, b]: number[]) => 0.2126 * r + 0.7152 * g + 0.0722 * b;

	it('anchors the demand domain at zero and clamps the top two percent', () => {
		const values = Array.from({ length: 100 }, (_, i) => i + 1);
		values[99] = 50_000;
		expect(demandDomain(values)).toEqual({ lo: 0, hi: 99 });
		expect(scalarDomain('demand', values)).toEqual(demandDomain(values));
		expect(demandDomain([0, 0, 0])).toEqual({ lo: 0, hi: 1 });
		expect(demandDomain([])).toEqual({ lo: 0, hi: 1 });
	});

	it('brightens monotonically from no demand to the top of the domain', () => {
		const steps = Array.from({ length: 11 }, (_, i) => luminance(demandColor(i / 10)));
		for (let i = 1; i < steps.length; i++) expect(steps[i]).toBeGreaterThan(steps[i - 1]);
	});

	it('spreads small demands on a square-root scale and clamps out-of-domain values', () => {
		// A quarter of the domain lands on the middle stop of the ramp.
		expect(demandColor(0.25)).toEqual([33, 145, 140, 240]);
		expect(demandColor(-1)).toEqual(demandColor(0));
		expect(demandColor(4)).toEqual(demandColor(1));
	});

	it('places the legend stops where the square-root scale puts them', () => {
		expect(demandGradient).toMatch(/^linear-gradient\(90deg, rgb\(/);
		expect(demandGradient).toContain('6.25%');
		expect(demandGradient).toContain('25.00%');
		expect(demandGradient).toContain('100.00%');
	});

	it('sizes buses by power or draws them all alike', () => {
		expect(busRadiusFor('power', 900)).toBe(busRadius(900));
		expect(busRadiusFor('uniform', 900)).toBe(UNIFORM_BUS_RADIUS);
		expect(busRadiusFor('uniform', 0)).toBe(UNIFORM_BUS_RADIUS);
	});

	it('gives uniform map markers one ground radius between pixel bounds', () => {
		const uniform = busMarkerRadius('uniform');
		expect(uniform.radiusUnits).toBe('meters');
		expect(uniform.radius(10)).toBe(UNIFORM_BUS_GROUND_RADIUS);
		expect(uniform.radius(10_000)).toBe(UNIFORM_BUS_GROUND_RADIUS);
		expect(uniform.radiusMinPixels).toBe(UNIFORM_BUS_MIN_RADIUS);
		expect(uniform.radiusMaxPixels).toBe(UNIFORM_BUS_RADIUS);
		const power = busMarkerRadius('power');
		expect(power.radiusUnits).toBe('pixels');
		expect(power.radius(900)).toBe(busRadius(900));
	});
});
