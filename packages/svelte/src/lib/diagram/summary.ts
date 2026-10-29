import { LOADING_ATTENTION } from '../colors.js';
import type {
	BusSnapshot,
	PairVoltage,
	PairZone,
	ResultSnapshot,
	UnbalanceValue
} from './snapshot.js';

/** Loading thresholds in percent of rating: a warning from the shared attention line, a violation
 * above the rating itself, since the multiconductor power flow does not enforce ratings. */
export const LOADING_WARNING = 100 * LOADING_ATTENTION;
export const LOADING_LIMIT = 100;

export type VoltageState = 'normal' | 'violation' | 'unbounded' | 'missing';
export type LoadingState = 'normal' | 'warning' | 'violation' | 'missing';

export function pairState(p: PairVoltage): VoltageState {
	if (!Number.isFinite(p.volts)) return 'missing';
	if (p.min === null || p.max === null) return 'unbounded';
	return p.volts < p.min || p.volts > p.max ? 'violation' : 'normal';
}

export function busState(bus: BusSnapshot): VoltageState {
	if (!bus.pairs.length) return 'missing';
	const states = bus.pairs.map(pairState);
	return states.includes('violation')
		? 'violation'
		: states.includes('normal')
			? 'normal'
			: states[0];
}

export function loadingState(loading: number | null): LoadingState {
	if (loading === null || !Number.isFinite(loading)) return 'missing';
	return loading > LOADING_LIMIT ? 'violation' : loading >= LOADING_WARNING ? 'warning' : 'normal';
}

/** Distance outside [min, max] as a percentage of the violated limit; negative inside the band. */
export function exceedance(p: PairVoltage): number {
	if (p.min === null || p.max === null) return Number.NEGATIVE_INFINITY;
	if (p.volts < p.min) return (100 * (p.min - p.volts)) / p.min;
	if (p.volts > p.max) return (100 * (p.volts - p.max)) / p.max;
	return (-100 * Math.min(p.volts - p.min, p.max - p.volts)) / (p.max - p.min);
}

/** Magnitude-only fallback: max deviation from the mean / mean (IEEE PVUR or NEMA LVUR), over the
 * three pairs of the bus's first reference. */
export function magnitudeUnbalance(all: PairVoltage[]): UnbalanceValue | null {
	const zone = all[0]?.zone;
	const pairs = all.filter((p) => p.zone === zone);
	if (pairs.length !== 3) return null;
	const mean = (pairs[0].volts + pairs[1].volts + pairs[2].volts) / 3;
	if (!(mean > 0)) return null;
	const percent = (100 * Math.max(...pairs.map((p) => Math.abs(p.volts - mean)))) / mean;
	return { percent, definition: zone === 'phase–phase' ? 'NEMA LVUR' : 'IEEE PVUR' };
}

/** Phasor unbalance when the snapshot has it, otherwise the labelled magnitude fallback. */
export const busUnbalance = (bus: BusSnapshot) => bus.unbalance ?? magnitudeUnbalance(bus.pairs);

export interface Issue {
	kind: 'voltage' | 'loading';
	severity: 'violation' | 'warning';
	target: { kind: 'bus' | 'branch'; index: number };
	value: number;
	limit: number;
	score: number;
	pair?: [string, string];
	low?: boolean;
}

/** Violations first, then the largest relative exceedance; the index keeps ties deterministic. */
export function rankIssues(result: ResultSnapshot): Issue[] {
	const issues: Issue[] = [];
	result.buses.forEach((bus, index) => {
		let worst: PairVoltage | null = null;
		for (const p of bus.pairs) {
			if (pairState(p) === 'violation' && (!worst || exceedance(p) > exceedance(worst))) worst = p;
		}
		if (!worst || worst.min === null || worst.max === null) return;
		const low = worst.volts < worst.min;
		issues.push({
			kind: 'voltage',
			severity: 'violation',
			target: { kind: 'bus', index },
			pair: worst.pair,
			value: worst.volts,
			limit: low ? worst.min : worst.max,
			low,
			score: exceedance(worst)
		});
	});
	result.branches.forEach((branch, index) => {
		if (!branch.primary) return;
		const state = loadingState(branch.loading);
		if (state !== 'warning' && state !== 'violation') return;
		const value = branch.loading as number;
		issues.push({
			kind: 'loading',
			severity: state,
			target: { kind: 'branch', index },
			value,
			limit: LOADING_LIMIT,
			score: value - LOADING_LIMIT
		});
	});
	const rank = (i: Issue) => (i.severity === 'violation' ? 0 : 1);
	return issues.sort(
		(a, b) => rank(a) - rank(b) || b.score - a.score || a.target.index - b.target.index
	);
}

export interface VoltageExtreme {
	bus: number;
	pair: [string, string];
	volts: number;
	state: VoltageState;
}

export interface ZoneSummary {
	zone: PairZone;
	min: VoltageExtreme;
	max: VoltageExtreme;
	buses: number;
}

export interface SnapshotSummary {
	loss: { p: number; q: number; branches: number };
	loading: { branch: number; value: number } | null;
	unbalance: (UnbalanceValue & { bus: number }) | null;
	zones: ZoneSummary[];
	counts: { voltage: number; overload: number; warning: number };
}

export function snapshotSummary(
	result: ResultSnapshot,
	issues: Issue[] = rankIssues(result)
): SnapshotSummary {
	const zones = new Map<
		PairZone,
		{ min: VoltageExtreme; max: VoltageExtreme; buses: Set<number> }
	>();
	let unbalance: SnapshotSummary['unbalance'] = null;
	result.buses.forEach((bus, index) => {
		for (const p of bus.pairs) {
			const extreme = { bus: index, pair: p.pair, volts: p.volts, state: pairState(p) };
			const zone = zones.get(p.zone);
			if (!zone) zones.set(p.zone, { min: extreme, max: extreme, buses: new Set([index]) });
			else {
				zone.buses.add(index);
				if (p.volts < zone.min.volts) zone.min = extreme;
				if (p.volts > zone.max.volts) zone.max = extreme;
			}
		}
		const u = busUnbalance(bus);
		if (u && (!unbalance || u.percent > unbalance.percent)) unbalance = { ...u, bus: index };
	});
	const loss = { p: 0, q: 0, branches: 0 };
	let loading: SnapshotSummary['loading'] = null;
	result.branches.forEach((branch, index) => {
		// Once per piece of equipment: a multi-winding transformer has several edges.
		if (branch.loss) {
			loss.p += branch.loss.p;
			loss.q += branch.loss.q;
			loss.branches++;
		}
		if (
			branch.loading !== null &&
			Number.isFinite(branch.loading) &&
			(!loading || branch.loading > loading.value)
		)
			loading = { branch: index, value: branch.loading };
	});
	const count = (kind: Issue['kind'], severity: Issue['severity']) =>
		issues.filter((i) => i.kind === kind && i.severity === severity).length;
	return {
		loss,
		loading,
		unbalance,
		zones: [...zones]
			.map(([zone, z]) => ({ zone, min: z.min, max: z.max, buses: z.buses.size }))
			.sort((a, b) => b.max.volts - a.max.volts),
		counts: {
			voltage: count('voltage', 'violation'),
			overload: count('loading', 'violation'),
			warning: count('loading', 'warning')
		}
	};
}
