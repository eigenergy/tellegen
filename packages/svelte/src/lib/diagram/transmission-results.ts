import type { BranchLoading, NetworkBus, SolutionFlow } from '@tellegen/engine';
import type { RGBA } from '../colors.js';

/** Display tolerances suppress rounding noise; they are not solver feasibility tolerances. */
export const VOLTAGE_TOLERANCE_PU = 1e-5;
export const LOADING_TOLERANCE = 1e-5;
export const DEFAULT_LOADING_WARNING = 0.9;
/** Fixed across solves. Values outside the domain clamp; case limits determine violations. */
export const VOLTAGE_DOMAIN = { lo: 0.9, hi: 1.1 } as const;
export const voltageGradient = 'linear-gradient(90deg, #2166ac, #dedbd2, #b66a35)';

export type VoltageState = 'missing' | 'unbounded' | 'invalid-limits' | 'normal' | 'low' | 'high';
export interface VoltageAssessment {
	state: VoltageState;
	/** Distance beyond the violated limit, per unit. */
	exceedance: number;
}

export function assessVoltage(
	value: number | undefined,
	bus: Pick<NetworkBus, 'vmin_pu' | 'vmax_pu'>
): VoltageAssessment {
	const result = (state: VoltageState, exceedance = 0) => ({ state, exceedance });
	if (value === undefined || !Number.isFinite(value) || value < 0) return result('missing');
	const { vmin_pu: lo, vmax_pu: hi } = bus;
	if (
		(lo !== undefined && (!Number.isFinite(lo) || lo < 0)) ||
		(hi !== undefined && (!Number.isFinite(hi) || hi <= 0)) ||
		(lo !== undefined && hi !== undefined && lo > hi)
	)
		return result('invalid-limits');
	if (lo !== undefined && value < lo - VOLTAGE_TOLERANCE_PU) return result('low', lo - value);
	if (hi !== undefined && value > hi + VOLTAGE_TOLERANCE_PU) return result('high', value - hi);
	return result(lo === undefined && hi === undefined ? 'unbounded' : 'normal');
}

export type LoadingState =
	'inactive' | 'missing' | 'unknown' | 'unrated' | 'normal' | 'warning' | 'at-limit' | 'violation';

/** The source rating and solve metadata are separate from the network's editable rating.
 * Legacy responses remain unknown: a topology rating may have changed since that solve.
 * None of these states implies an economically binding constraint. */
export function assessLoading(
	flow: Pick<SolutionFlow, 'loading' | 'loading_info'> | undefined,
	inService: boolean,
	warning = DEFAULT_LOADING_WARNING
): LoadingState {
	if (!Number.isFinite(warning) || warning < 0 || warning >= 1)
		throw new RangeError('Loading warning must be a finite fraction from 0 up to 1');
	if (!inService) return 'inactive';
	if (!flow || !Number.isFinite(flow.loading) || flow.loading < 0) return 'missing';
	const info = flow.loading_info;
	if (!info) return 'unknown';
	if (info.rating_source === 'synthesized' || info.rating === undefined) return 'unrated';
	if (!Number.isFinite(info.rating) || info.rating <= 0 || info.rating_source !== 'stated')
		return 'unknown';
	if (flow.loading > 1 + LOADING_TOLERANCE) return 'violation';
	if (Math.abs(flow.loading - 1) <= LOADING_TOLERANCE) return 'at-limit';
	return flow.loading >= warning ? 'warning' : 'normal';
}

export function operatingLoadingColor(state: LoadingState): RGBA {
	if (state === 'violation') return [190, 32, 36, 255];
	if (state === 'warning' || state === 'at-limit') return [210, 137, 27, 245];
	if (state === 'normal') return [138, 134, 125, 200];
	return [155, 150, 140, 130];
}

export function operatingVoltageColor(value: number | undefined, bus: NetworkBus): RGBA {
	const { state } = assessVoltage(value, bus);
	if (state === 'missing' || state === 'invalid-limits') return [180, 175, 165, 160];
	if (state === 'low') return [33, 102, 172, 255];
	if (state === 'high') return [190, 32, 36, 255];
	const t = Math.max(0, Math.min(1, ((value as number) - VOLTAGE_DOMAIN.lo) / 0.2));
	const a = t < 0.5 ? [33, 102, 172] : [222, 219, 210];
	const b = t < 0.5 ? [222, 219, 210] : [182, 106, 53];
	const u = t < 0.5 ? t * 2 : (t - 0.5) * 2;
	return [
		Math.round(a[0] + (b[0] - a[0]) * u),
		Math.round(a[1] + (b[1] - a[1]) * u),
		Math.round(a[2] + (b[2] - a[2]) * u),
		240
	];
}

/** Preserve extrema and their identities rather than hiding a station problem in an average. */
export function stationVoltageSummary(buses: NetworkBus[], values: ReadonlyMap<number, number>) {
	let min: { bus: number; value: number } | null = null;
	let max: { bus: number; value: number } | null = null;
	let worst: { bus: number; value: number; assessment: VoltageAssessment } | null = null;
	let unknown = 0;
	for (const bus of buses) {
		const value = values.get(bus.id);
		const assessment = assessVoltage(value, bus);
		if (assessment.state === 'missing') {
			unknown++;
			continue;
		}
		if (assessment.state === 'unbounded' || assessment.state === 'invalid-limits') unknown++;
		const entry = { bus: bus.id, value: value as number };
		if (!min || entry.value < min.value) min = entry;
		if (!max || entry.value > max.value) max = entry;
		if (
			assessment.exceedance > 0 &&
			(!worst || assessment.exceedance > worst.assessment.exceedance)
		)
			worst = { ...entry, assessment };
	}
	return { min, max, worst, unknown };
}

/** Equipment percentage labels are meaningful only against a stated rating. */
export const hasStatedRating = (info: BranchLoading | undefined) =>
	info?.rating_source === 'stated' && Number.isFinite(info.rating) && (info.rating ?? 0) > 0;
