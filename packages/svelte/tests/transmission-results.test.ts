import { describe, expect, it } from 'vitest';
import type { NetworkBus, SolutionFlow, Solution } from '@tellegen/engine';
import {
	assessLoading,
	assessVoltage,
	operatingLoadingColor,
	operatingVoltageColor,
	stationVoltageSummary,
	operatingVoltages,
	operatingContext,
	placeOperatingGauges,
	gaugeLeader,
	gaugeText,
	GAUGE_HEIGHT,
	GAUGE_WIDTH,
	type GaugeCandidate,
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
	it('separates voltage availability and freshness from an economic display series', () => {
		const solution: Solution = {
			objective: 1,
			prices: [{ bus: 91, value: 20 }],
			vm: [{ bus: 91, value: 1.02 }],
			w: [
				{ bus: 91, value: 1.04 },
				{ bus: 42, value: -1 }
			],
			va: [],
			flows: [],
			dispatch: []
		};
		expect(operatingVoltages(solution, 'dcopf').size).toBe(0);
		expect(operatingVoltages(solution, 'acopf').get(91)).toBe(1.02);
		expect(operatingVoltages(solution, 'socwr').get(91)).toBeCloseTo(Math.sqrt(1.04));
		expect(operatingVoltages(solution, 'socwr').get(42)).toBeNaN();
		expect(operatingContext('socwr', solution, false, false)).toMatchObject({
			relaxed: true,
			voltage: true,
			freshness: 'Result available'
		});
		expect(operatingContext('acpf', solution, true, false)).toMatchObject({
			stale: true,
			freshness: 'Updating · previous result'
		});
		expect(operatingContext('acpf', solution, false, true)).toMatchObject({
			stale: true,
			freshness: 'Error · result may be stale'
		});
		expect(operatingContext('dcopf', null, false, false)).toMatchObject({
			voltage: false,
			freshness: 'Not solved'
		});
		expect(operatingContext('acpf', null, false, true).freshness).toContain('no result');
		const previous = { ...solution, formulation: 'dcopf', status: 'optimal' };
		expect(operatingContext('acpf', previous, false, false)).toMatchObject({
			stale: true,
			status: 'optimal'
		});
		expect(operatingContext('acpf', previous, false, false).freshness).toContain('Previous dcopf');
		expect(operatingVoltages(previous, 'acpf').size).toBe(0);
	});

	it('excludes diagnosed de-energized buses from voltage extrema and violations', () => {
		expect(assessVoltage(0, bus(10), true).state).toBe('deenergized');
		expect(assessLoading(flow(0), true, 0.9, true)).toBe('deenergized');
		const s = stationVoltageSummary(
			[bus(10), bus(91)],
			new Map([
				[10, 0],
				[91, 1.02]
			]),
			new Set([10])
		);
		expect(s).toMatchObject({
			min: { bus: 91, value: 1.02 },
			max: { bus: 91, value: 1.02 },
			worst: null,
			disconnected: 1,
			unknown: 0
		});
	});

	it('prioritizes selection and overload gauges within the uncovered viewport and collision budget', () => {
		const candidate = (
			edge: number,
			ax: number,
			state: GaugeCandidate['state'],
			loading: number,
			selected = false
		): GaugeCandidate => ({ edge, ax, ay: 120, state, loading, selected });
		const candidates = [
			candidate(1, 30, 'normal', 0.7),
			candidate(2, 30, 'violation', 1.3),
			candidate(3, 230, 'warning', 0.95),
			candidate(4, 230, 'normal', 0.2, true),
			candidate(5, 999, 'violation', 2)
		];
		const rect = { left: 20, top: 40, right: 400, bottom: 260 };
		expect(placeOperatingGauges(candidates, rect, 2).map((g) => g.edge)).toEqual([4, 2]);
		expect(placeOperatingGauges(candidates, rect, 0)).toEqual([]);
	});

	it('moves a gauge around its branch before dropping it, and joins it with a leader', () => {
		const at = (edge: number, ax: number, ay: number): GaugeCandidate => ({
			edge,
			ax,
			ay,
			state: 'violation',
			loading: 1.2,
			selected: false
		});
		const rect = { left: 0, top: 0, right: 400, bottom: 400 };
		// Two branches at one point: the second takes the next free spot instead of vanishing.
		const placed = placeOperatingGauges([at(1, 200, 200), at(2, 200, 200)], rect, 10);
		expect(placed.map((g) => g.edge)).toEqual([1, 2]);
		expect(placed[0].x).not.toBe(placed[1].x);
		// Near the top edge, the box goes below its branch.
		const [low] = placeOperatingGauges([at(3, 200, 20)], rect, 10);
		expect(low.y).toBeGreaterThan(20);
		// The leader ends on the box edge nearest the branch point.
		const [x, y] = gaugeLeader(placed[0]);
		expect(x).toBeGreaterThanOrEqual(placed[0].x);
		expect(x).toBeLessThanOrEqual(placed[0].x + GAUGE_WIDTH);
		expect(y).toBe(placed[0].y + GAUGE_HEIGHT);
		// A branch point off screen gets no gauge.
		expect(placeOperatingGauges([at(4, -5, 200)], rect, 10)).toEqual([]);
		expect(gaugeText(1.003)).toBe('100.3%');
		expect(gaugeText(0.874)).toBe('87%');
	});
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
