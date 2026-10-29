import type { BranchEnd, PairVoltage } from './snapshot.js';
import { pairState, type VoltageState } from './summary.js';

const clampPercent = (x: number) => Math.max(0, Math.min(100, x));

export interface PairBar {
	pair: [string, string];
	volts: number;
	state: VoltageState;
	/** Band and marker in percent of the track; the track is the declared band ±25 %. `null`
	 * band when limits are undeclared, then the marker sits mid-track. */
	band: { left: number; width: number } | null;
	marker: number;
}

/** Each pair on its own band, so position reads as margin to the limits rather than volts. */
export function pairBar(p: PairVoltage): PairBar {
	const state = pairState(p);
	if (p.min === null || p.max === null || !(p.max > p.min))
		return { pair: p.pair, volts: p.volts, state, band: null, marker: 50 };
	const span = p.max - p.min;
	const lo = p.min - 0.25 * span;
	const x = (v: number) => (100 * (v - lo)) / (1.5 * span);
	return {
		pair: p.pair,
		volts: p.volts,
		state,
		band: { left: x(p.min), width: x(p.max) - x(p.min) },
		marker: clampPercent(x(p.volts))
	};
}

export interface ConductorBar {
	terminal: string;
	amps: number;
	/** Percent of the largest conductor current across both ends. */
	fill: number;
	neutral: boolean;
}

export interface ConductorBars {
	scaleAmps: number;
	ends: ({ p: number; q: number; bars: ConductorBar[] } | null)[];
	/** Neutral current as a percentage of the largest phase current at the first live end. */
	neutralShare: number | null;
}

/** Conductor |I| at both ends on one scale; neutral current is called out as an unbalance signal. */
export function conductorBars(
	ends: [BranchEnd | null, BranchEnd | null],
	neutral = 'n'
): ConductorBars | null {
	const live = ends.filter((e): e is BranchEnd => !!e?.currents.length);
	if (!live.length) return null;
	const scaleAmps = Math.max(
		...live.flatMap((e) => e.currents.map((c) => c.amps)).filter(Number.isFinite),
		1e-9
	);
	const first = live[0];
	const n = first.currents.find((c) => c.terminal === neutral);
	const phaseMax = Math.max(
		0,
		...first.currents.filter((c) => c.terminal !== neutral).map((c) => c.amps)
	);
	return {
		scaleAmps,
		ends: ends.map((e) =>
			e?.currents.length
				? {
						p: e.p,
						q: e.q,
						bars: e.currents.map((c) => ({
							terminal: c.terminal,
							amps: c.amps,
							fill: clampPercent((100 * c.amps) / scaleAmps),
							neutral: c.terminal === neutral
						}))
					}
				: null
		),
		neutralShare: n && phaseMax > 0 ? (100 * n.amps) / phaseMax : null
	};
}
