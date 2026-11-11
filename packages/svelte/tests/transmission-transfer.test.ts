import { describe, expect, it } from 'vitest';
import {
	transferDirectionText,
	transferReading,
	transferWidth
} from '../src/lib/diagram/transmission-transfer.js';

const flow = (mw: number, pt?: number) => ({ branch: 7, mw, pt, loading: 0.99 });

describe('transmission transfers', () => {
	it('reads both lossless DC ends and distinguishes zero from missing', () => {
		expect(transferReading(flow(-28), true)).toEqual({
			from: -28,
			to: 28,
			magnitude: 28,
			direction: -1
		});
		expect(transferReading(flow(0), true)).toEqual({ from: 0, to: 0, magnitude: 0, direction: 0 });
		expect(transferReading(undefined, true).magnitude).toBeUndefined();
		expect(transferReading(flow(NaN), true).magnitude).toBeUndefined();
	});

	it('keeps AC signs and represents simultaneous infeed without a through arrow', () => {
		const branch = { from: 2, to: 8 };
		const forward = transferReading(flow(45, -42), false);
		expect(forward.magnitude).toBe(45);
		expect(transferDirectionText(forward, branch)).toBe('Bus 2 → bus 8');
		const reverse = transferReading(flow(-42, 45), false);
		expect(transferDirectionText(reverse, branch)).toBe('Bus 8 → bus 2');
		const both = transferReading(flow(3, 1), false);
		expect(both.direction).toBe(0);
		expect(transferDirectionText(both, branch)).toBe('Active power enters the branch');
		const legacy = transferReading(flow(42), false);
		expect(legacy.magnitude).toBe(42);
		expect(transferDirectionText(legacy, branch)).toBe('Direction unavailable');
	});

	it('caps halo width independently of equipment utilization', () => {
		expect(transferWidth(0, 500)).toBeLessThan(transferWidth(50, 500));
		expect(transferWidth(500, 500)).toBe(transferWidth(50000, 500));
	});
});
