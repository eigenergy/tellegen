import type { BranchLoading, NetworkBus, SolutionFlow, Solution } from '@tellegen/engine';
import type { RGBA } from '../colors.js';

/** Display tolerances suppress rounding noise; they are not solver feasibility tolerances. */
export const VOLTAGE_TOLERANCE_PU = 1e-5;
export const LOADING_TOLERANCE = 1e-5;
export const DEFAULT_LOADING_WARNING = 0.9;
/** Fixed across solves. Values outside the domain clamp; case limits determine violations. */
export const VOLTAGE_DOMAIN = { lo: 0.9, hi: 1.1 } as const;
export const voltageGradient = 'linear-gradient(90deg, #2166ac, #dedbd2, #b66a35)';

export type VoltageState =
	'deenergized' | 'missing' | 'unbounded' | 'invalid-limits' | 'normal' | 'low' | 'high';
export interface VoltageAssessment {
	state: VoltageState;
	/** Distance beyond the violated limit, per unit. */
	exceedance: number;
}

export function assessVoltage(
	value: number | undefined,
	bus: Pick<NetworkBus, 'vmin_pu' | 'vmax_pu'>,
	deenergized = false
): VoltageAssessment {
	const result = (state: VoltageState, exceedance = 0) => ({ state, exceedance });
	if (deenergized) return result('deenergized');
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
	| 'deenergized'
	| 'inactive'
	| 'missing'
	| 'unknown'
	| 'unrated'
	| 'normal'
	| 'warning'
	| 'at-limit'
	| 'violation';

/** The source rating and solve metadata are separate from the network's editable rating.
 * Legacy responses remain unknown: a topology rating may have changed since that solve.
 * None of these states implies an economically binding constraint. */
export function assessLoading(
	flow: Pick<SolutionFlow, 'loading' | 'loading_info'> | undefined,
	inService: boolean,
	warning = DEFAULT_LOADING_WARNING,
	deenergized = false
): LoadingState {
	if (!Number.isFinite(warning) || warning < 0 || warning >= 1)
		throw new RangeError('Loading warning must be a finite fraction from 0 up to 1');
	if (!inService) return 'inactive';
	if (deenergized) return 'deenergized';
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
	if (state === 'normal') return [158, 153, 144, 190];
	return [155, 150, 140, 130];
}

export function operatingVoltageColor(
	value: number | undefined,
	bus: NetworkBus,
	deenergized = false
): RGBA {
	const { state } = assessVoltage(value, bus, deenergized);
	if (
		state === 'missing' ||
		state === 'invalid-limits' ||
		state === 'deenergized' ||
		state === 'unbounded'
	)
		return [180, 175, 165, 160];
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
export function stationVoltageSummary(
	buses: NetworkBus[],
	values: ReadonlyMap<number, number>,
	deenergized: ReadonlySet<number> = new Set()
) {
	let min: { bus: number; value: number } | null = null;
	let max: { bus: number; value: number } | null = null;
	let worst: { bus: number; value: number; assessment: VoltageAssessment } | null = null;
	let unknown = 0;
	let disconnected = 0;
	for (const bus of buses) {
		const value = values.get(bus.id);
		const assessment = assessVoltage(value, bus, deenergized.has(bus.id));
		if (assessment.state === 'deenergized') {
			disconnected++;
			continue;
		}
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
	return { min, max, worst, unknown, disconnected };
}

/** DC never acquires a voltage magnitude from an angle or a nominal-kV column. */
export function operatingVoltages(
	solution: Solution | null,
	formulation: string
): Map<number, number> {
	if (
		!solution ||
		(solution.formulation !== undefined && solution.formulation !== formulation) ||
		formulation === 'dcopf' ||
		formulation === 'dcpf'
	)
		return new Map();
	const values =
		formulation === 'socwr'
			? solution.w.map((v) => ({ bus: v.bus, value: v.value >= 0 ? Math.sqrt(v.value) : NaN }))
			: (solution.vm ?? []);
	return new Map(values.map((v) => [v.bus, v.value]));
}

export function operatingContext(
	formulation: string,
	solution: Solution | null,
	solving: boolean,
	error: boolean
) {
	const dc = formulation === 'dcopf' || formulation === 'dcpf';
	const resultFormulation = solution?.formulation ?? formulation;
	const dcBasis = resultFormulation === 'dcopf' || resultFormulation === 'dcpf';
	const mismatch = !!solution?.formulation && solution.formulation !== formulation;
	return {
		status: solution?.status ?? 'status unavailable',
		voltage: !dc,
		relaxed: formulation === 'socwr',
		freshness: mismatch
			? `Previous ${solution!.formulation} result · awaiting ${formulation}`
			: solving
				? solution
					? 'Updating · previous result'
					: 'Calculating · no result yet'
				: error
					? solution
						? 'Error · result may be stale'
						: 'Calculation unavailable · no result'
					: solution
						? 'Result available'
						: 'Not solved',
		stale: solving || error || mismatch,
		basis: dcBasis ? '|P| / MW rating' : 'max(|S from|, |S to|) / MVA rating'
	};
}

/** A gauge to place: the branch's screen point it describes (`ax`, `ay`). */
export type GaugeCandidate = {
	edge: number;
	ax: number;
	ay: number;
	loading: number;
	state: LoadingState;
	selected: boolean;
};
/** A placed gauge: its box's top-left corner (`x`, `y`), joined to its branch by a leader. */
export type OperatingGauge = GaugeCandidate & { x: number; y: number };

export const GAUGE_WIDTH = 64;
export const GAUGE_HEIGHT = 44;
/** Box offsets from the branch point, tried in order: above right, above left, below right,
 * below left. A gap keeps the leader visible. */
const GAUGE_OFFSETS: [number, number][] = [
	[10, -GAUGE_HEIGHT - 14],
	[-GAUGE_WIDTH - 10, -GAUGE_HEIGHT - 14],
	[10, 14],
	[-GAUGE_WIDTH - 10, 14]
];

/** Fixed-size, local gauges. Selection wins, followed by violations and warnings. Each gauge
 * takes the first free spot around its branch point; the available canvas rectangle, collisions
 * and the budget limit clutter independently of network size. */
export function placeOperatingGauges(
	candidates: GaugeCandidate[],
	rect: { left: number; top: number; right: number; bottom: number },
	budget: number
): OperatingGauge[] {
	if (budget <= 0) return [];
	const rank = (g: GaugeCandidate) =>
		(g.selected
			? 1e9
			: g.state === 'violation'
				? 1e6
				: g.state === 'warning' || g.state === 'at-limit'
					? 1e4
					: 0) + g.loading;
	const sorted = [...candidates].sort((a, b) => rank(b) - rank(a) || a.edge - b.edge);
	const placed: OperatingGauge[] = [];
	const inside = (x: number, y: number, w: number, h: number) =>
		x >= rect.left && x + w <= rect.right && y >= rect.top && y + h <= rect.bottom;
	for (const g of sorted) {
		if (![g.ax, g.ay, g.loading].every(Number.isFinite)) continue;
		// The branch point itself must be visible, or the leader would point off screen.
		if (!inside(g.ax, g.ay, 0, 0)) continue;
		for (const [dx, dy] of GAUGE_OFFSETS) {
			const x = g.ax + dx;
			const y = g.ay + dy;
			if (!inside(x, y, GAUGE_WIDTH, GAUGE_HEIGHT)) continue;
			if (
				placed.some(
					(p) => Math.abs(p.x - x) < GAUGE_WIDTH + 6 && Math.abs(p.y - y) < GAUGE_HEIGHT + 4
				)
			)
				continue;
			placed.push({ ...g, x, y });
			break;
		}
		if (placed.length >= budget) break;
	}
	return placed;
}

/** Where a gauge's leader meets its box: the nearest point of the box to the branch point. */
export function gaugeLeader(g: OperatingGauge): [number, number] {
	return [
		Math.max(g.x, Math.min(g.ax, g.x + GAUGE_WIDTH)),
		Math.max(g.y, Math.min(g.ay, g.y + GAUGE_HEIGHT))
	];
}

/** A gauge's percentage, with a decimal only near the rating where it decides the state. */
export const gaugeText = (loading: number) =>
	`${(loading * 100).toFixed(loading > 0.99 && loading < 1.01 ? 1 : 0)}%`;

export const loadingStateText: Record<LoadingState, string> = {
	deenergized: 'de-energized',
	inactive: 'out of service',
	missing: 'result missing',
	unknown: 'rating unknown',
	unrated: 'no stated rating',
	normal: 'below warning',
	warning: 'warning',
	'at-limit': 'at rating',
	violation: 'overload'
};

/** Equipment percentage labels are meaningful only against a stated rating. */
export const hasStatedRating = (info: BranchLoading | undefined) =>
	info?.rating_source === 'stated' && Number.isFinite(info.rating) && (info.rating ?? 0) > 0;
