import { describe, expect, it } from 'vitest';
import type { NetworkBus, SolutionFlow } from '@tellegen/engine';
import {
	assessLoading,
	assessVoltage,
	operatingLoadingColor,
	operatingVoltageColor,
	stationVoltageSummary,
	LOADING_TOLERANCE,
	VOLTAGE_TOLERANCE_PU
} from '../src/lib/diagram/transmission-results.js';

const bus = (id: number, bounds = true): NetworkBus => ({
	id,
	lon: 0,
	lat: 0,
	demand_mw: 0,
	gen_mw: 0,
	...(bounds ? { vmin_pu: 0.95, vmax_pu: 1.05 } : {})
});
const flow = (loading: number): SolutionFlow => ({
	branch: 42,
	mw: 30,
	loading,
	loading_info: { basis: 'mva', rating: 100, rating_source: 'stated', limiting_end: 'to' }
});

describe('transmission operating limits', () => {
	it('separates warnings, at-limit values and actual overloads', () => {
		for (const [loading, expected] of [
			[0, 'normal'],
			[0.899, 'normal'],
			[0.9, 'warning'],
			[0.999, 'warning'],
			[1, 'at-limit'],
			[1 + LOADING_TOLERANCE / 2, 'at-limit'],
			[1.001, 'violation']
		] as const)
			expect(assessLoading(flow(loading), true)).toBe(expected);
		expect(assessLoading(flow(0.85), true, 0.8)).toBe('warning');
		expect(() => assessLoading(flow(0.5), true, 1)).toThrow(RangeError);
		expect(operatingLoadingColor('at-limit')).not.toEqual(operatingLoadingColor('violation'));
	});

	it('does not turn unknown, unrated or inactive equipment into normal zero utilization', () => {
		expect(assessLoading(undefined, false)).toBe('inactive');
		expect(assessLoading(undefined, true)).toBe('missing');
		expect(assessLoading(flow(NaN), true)).toBe('missing');
		expect(assessLoading({ loading: 0 }, true)).toBe('unknown');
		expect(assessLoading({ loading: 0, loading_info: { basis: 'mva' } }, true)).toBe('unrated');
		expect(
			assessLoading(
				{
					loading: 0.1,
					loading_info: {
						basis: 'mw',
						rating: 10000,
						rating_source: 'synthesized'
					}
				},
				true
			)
		).toBe('unrated');
		expect(
			assessLoading(
				{
					loading: 0,
					loading_info: {
						basis: 'mva',
						rating: Infinity,
						rating_source: 'stated'
					}
				},
				true
			)
		).toBe('unknown');
	});

	it('uses actual per-bus bounds, including one-sided bounds and numerical tolerance', () => {
		expect(assessVoltage(0.95, bus(10)).state).toBe('normal');
		expect(assessVoltage(0.95 - VOLTAGE_TOLERANCE_PU / 2, bus(10)).state).toBe('normal');
		expect(assessVoltage(0.94, bus(10)).state).toBe('low');
		expect(assessVoltage(1.06, bus(10)).state).toBe('high');
		expect(assessVoltage(1.06, { vmax_pu: 1.05 }).state).toBe('high');
		expect(assessVoltage(0.94, { vmin_pu: 0.95 }).state).toBe('low');
		expect(assessVoltage(1, {}).state).toBe('unbounded');
		expect(assessVoltage(undefined, bus(10)).state).toBe('missing');
		expect(assessVoltage(NaN, bus(10)).state).toBe('missing');
		expect(assessVoltage(1, { vmin_pu: 1.1, vmax_pu: 0.9 }).state).toBe('invalid-limits');
	});

	it('preserves a uniformly violating voltage series and stable colours between solves', () => {
		const low = [bus(10), bus(91)].map((b) => operatingVoltageColor(0.92, b));
		expect(low[0]).toEqual(low[1]);
		expect(low[0]).not.toEqual(operatingVoltageColor(1, bus(10)));
		expect(operatingVoltageColor(1.08, bus(10))).toEqual([190, 32, 36, 255]);
		// Changing another bus does not rescale this bus's colour.
		expect(operatingVoltageColor(1.02, bus(10))).toEqual(operatingVoltageColor(1.02, bus(91)));
	});

	it('retains the worst station violation and extrema with original bus identities', () => {
		const summary = stationVoltageSummary(
			[bus(91), bus(10), bus(42), bus(120, false), bus(130)],
			new Map([
				[91, 1.07],
				[10, 0.91],
				[42, 1.01],
				[120, 1]
			])
		);
		expect(summary.min).toEqual({ bus: 10, value: 0.91 });
		expect(summary.max).toEqual({ bus: 91, value: 1.07 });
		expect(summary.worst?.bus).toBe(10);
		expect(summary.worst?.assessment.state).toBe('low');
		expect(summary.unknown).toBe(2);
	});
});
