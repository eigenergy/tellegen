import { describe, expect, it } from 'vitest';
import { previewScaleFor, sensitivityDomain } from '../src/lib/colors.js';

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
