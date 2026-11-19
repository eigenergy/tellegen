import { describe, expect, it } from 'vitest';
import {
	clampZoom,
	minZoomSpan,
	niceTicks,
	paceTarget,
	parseDrafts,
	recordsSignature,
	type DraftRecord
} from '../src/lib/dynamics/live-logic.js';

describe('live view pacing', () => {
	it('follows the clock at a pace, at most 2 s ahead of the session', () => {
		expect(paceTarget(5, 0.1, 2, 5)).toBeCloseTo(5.2);
		expect(paceTarget(5, 10, 1, 5)).toBe(7);
	});

	it('stays at the session time while unpaced, also after a zero-length tick', () => {
		// 0 · Infinity is NaN: the loop must not keep it.
		expect(paceTarget(5, 0, Infinity, 5)).toBe(5);
		expect(paceTarget(5, 0.3, Infinity, 7.5)).toBe(7.5);
	});

	it('resumes from the time reached when a pace is chosen again', () => {
		let expected = 0;
		let t = 0;
		// Unpaced for a while: the session runs ahead of any clock.
		for (const wall of [0, 0.016, 0, 0.02]) {
			expected = paceTarget(expected, wall, Infinity, t);
			t += 0.2;
		}
		expect(expected).toBeCloseTo(0.6);
		// The loop keeps the target at the time reached after each unpaced
		// slice; back at 1× it is finite and follows the clock from there.
		expected = paceTarget(t, 0.05, 1, t);
		expect(Number.isFinite(expected)).toBe(true);
		expect(expected).toBeCloseTo(t + 0.05);
	});

	it('recovers from a target that is not a number', () => {
		expect(paceTarget(NaN, 0.1, 1, 3)).toBe(3);
	});
});

describe('chart ticks and zoom', () => {
	it('gives round ticks inside the range', () => {
		expect(niceTicks(0, 10, 5)).toEqual([0, 2, 4, 6, 8, 10]);
		expect(niceTicks(0.9, 1.1, 4)).toEqual([0.9, 0.95, 1, 1.05, 1.1]);
	});

	it('ends on a range at the floating-point resolution instead of looping', () => {
		const lo = 1000;
		const hi = 1000 + 1e-13;
		const ticks = niceTicks(lo, hi, 6);
		expect(ticks.length).toBeLessThanOrEqual(8);
		expect(niceTicks(1e300, 1e300 * (1 + 1e-15), 10).length).toBeLessThanOrEqual(12);
	});

	it('returns at most count + 2 ticks for any range', () => {
		for (const [lo, hi, n] of [
			[0, 1, 4],
			[-3e-9, 7e-9, 5],
			[12.3456, 12.3457, 3],
			[17.9999999, 18.0000001, 8]
		] as const) {
			const t = niceTicks(lo, hi, n);
			expect(t.length).toBeLessThanOrEqual(n + 2);
			for (let i = 1; i < t.length; i++) expect(t[i]).toBeGreaterThan(t[i - 1]);
		}
	});

	it('handles empty and invalid ranges', () => {
		expect(niceTicks(1, 1, 5)).toEqual([1]);
		expect(niceTicks(2, 1, 5)).toEqual([2]);
		expect(niceTicks(NaN, 1, 5)).toEqual([]);
	});

	it('does not zoom narrower than a millionth of the values', () => {
		expect(minZoomSpan(0, 0.5)).toBe(1e-6);
		expect(minZoomSpan(100, 100.1)).toBeCloseTo(1.001e-4);
		const [a, b] = clampZoom(12, 12 + 1e-12);
		expect(b - a).toBeCloseTo(1.2e-5);
		expect((a + b) / 2).toBeCloseTo(12 + 5e-13);
		expect(clampZoom(1, 2)).toEqual([1, 2]);
	});
});

describe('unit editor parameter drafts', () => {
	const records: DraftRecord[] = [
		{
			line: 3,
			model: 'SEXS',
			parameters: [
				{ name: 'TA/TB', text: '0.1' },
				{ name: 'K', text: '100.0' }
			]
		},
		{ line: 4, model: 'TGOV1', parameters: [{ name: 'R', text: '0.05' }] }
	];

	it('turns changed values into edits and skips unchanged ones', () => {
		const { edits, errors } = parseDrafts({ '3:1': ' 120 ', '3:0': '0.1', '4:0': '5e-2' }, records);
		expect(errors).toEqual([]);
		expect(edits).toEqual([
			{ line: 3, index: 1, value: 120 },
			{ line: 4, index: 0, value: 0.05 }
		]);
	});

	it('refuses blank input instead of applying zero', () => {
		const { edits, errors } = parseDrafts({ '3:1': '  ' }, records);
		expect(edits).toEqual([]);
		expect(errors).toEqual(['SEXS K: enter a number']);
	});

	it('refuses text that is not a decimal number', () => {
		for (const bad of ['abc', '1,5', '0x10', '1e', '--1', 'Infinity', '.']) {
			const { edits, errors } = parseDrafts({ '4:0': bad }, records);
			expect(edits, bad).toEqual([]);
			expect(errors, bad).toEqual([`TGOV1 R: "${bad}" is not a number`]);
		}
	});

	it('drops a draft whose parameter is gone', () => {
		expect(parseDrafts({ '9:0': '1', '3:7': '2' }, records)).toEqual({ edits: [], errors: [] });
	});

	it('changes signature when the records change', () => {
		const sig = recordsSignature(records);
		expect(recordsSignature(structuredClone(records))).toBe(sig);
		const swapped = structuredClone(records);
		swapped[0].model = 'ESAC1A';
		expect(recordsSignature(swapped)).not.toBe(sig);
		const edited = structuredClone(records);
		edited[1].parameters[0].text = '0.04';
		expect(recordsSignature(edited)).not.toBe(sig);
	});
});
