import { bandPosition } from './colour-modes.js';
import type { ResultSnapshot } from './snapshot.js';
import { loadingState } from './summary.js';

export const VALUE_LABEL_BUDGET = 60;
export const MIN_LABEL_ZOOM = -0.2;

export type LabelState = 'violation' | 'warning' | 'normal';

export interface LabelCandidate {
	text: string;
	state: LabelState;
	priority: number;
	/** Screen position in CSS pixels. */
	screen: [number, number];
	selected?: boolean;
}

const SEVERITY: Record<LabelState, number> = { violation: 2, warning: 1, normal: 0 };

/** Greedy placement by (selected, severity, priority) on a coarse collision grid. The selection is
 * always placed; others must fit inside the canvas margins and the budget. */
export function placeLabels<T extends LabelCandidate>(
	candidates: T[],
	{
		width,
		height,
		budget = VALUE_LABEL_BUDGET,
		box = [64, 16] as [number, number],
		margin = [0, 40, 0, 24] as [number, number, number, number]
	}: {
		width: number;
		height: number;
		budget?: number;
		box?: [number, number];
		margin?: [number, number, number, number];
	}
): T[] {
	const sorted = [...candidates].sort(
		(a, b) =>
			Number(!!b.selected) - Number(!!a.selected) ||
			SEVERITY[b.state] - SEVERITY[a.state] ||
			b.priority - a.priority
	);
	const [cw, ch] = box;
	const occupied = new Set<string>();
	const placed: T[] = [];
	for (const c of sorted) {
		if (placed.length >= budget) break;
		const [sx, sy] = c.screen;
		const outside =
			sx < margin[3] ||
			sx + cw > width - margin[1] ||
			sy - ch < margin[0] ||
			sy > height - margin[2];
		if (!c.selected && outside) continue;
		const keys: string[] = [];
		for (let gy = Math.floor((sy - ch) / ch); gy <= Math.floor(sy / ch); gy++)
			for (let gx = Math.floor(sx / cw); gx <= Math.floor((sx + cw) / cw); gx++)
				keys.push(`${gx},${gy}`);
		if (!c.selected && keys.some((k) => occupied.has(k))) continue;
		for (const k of keys) occupied.add(k);
		placed.push(c);
	}
	return placed;
}

const short = (x: number) => Number(x.toPrecision(Math.abs(x) >= 1000 ? 5 : 4)).toLocaleString();

/** The pair nearest a limit, shown in volts. Buses without declared limits rank last. */
export function busLabel(
	result: ResultSnapshot,
	bus: number
): Omit<LabelCandidate, 'screen'> | null {
	const pairs = result.buses[bus].pairs;
	if (!pairs.length) return null;
	// Distance from mid-band; −1 without declared limits, so those pairs and buses rank last.
	const rank = (p: (typeof pairs)[number]) => {
		const band = bandPosition(p);
		return band === null ? -1 : Math.abs(band);
	};
	let pick = pairs[0];
	let t = rank(pick);
	for (const p of pairs) {
		const pt = rank(p);
		if (pt > t) [pick, t] = [p, pt];
	}
	return { text: `${short(pick.volts)} V`, state: t > 1 ? 'violation' : 'normal', priority: t };
}

export function branchLabel(
	result: ResultSnapshot,
	branch: number,
	family: 'loading' | 'power'
): Omit<LabelCandidate, 'screen'> | null {
	const row = result.branches[branch];
	if (family === 'loading') {
		const state = loadingState(row.loading);
		return state === 'missing'
			? null
			: { text: `${(row.loading as number).toFixed(1)} %`, state, priority: row.loading as number };
	}
	const end = row.ends[0] ?? row.ends[1];
	return end
		? { text: `${short(Math.abs(end.p))} kW`, state: 'normal', priority: Math.abs(end.p) }
		: null;
}
