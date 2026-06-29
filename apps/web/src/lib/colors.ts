import { extent } from './format.js';
import type { DisplayMode } from './state.svelte.js';

export type RGBA = [number, number, number, number];

export interface SensitivityDomain {
	min: number;
	max: number;
	mean: number;
	absMax: number;
	scale: number;
	flat: boolean;
}

function lerp(a: number, b: number, t: number): number {
	return a + (b - a) * t;
}

function lerpColor(a: RGBA, b: RGBA, t: number): RGBA {
	return [
		Math.round(lerp(a[0], b[0], t)),
		Math.round(lerp(a[1], b[1], t)),
		Math.round(lerp(a[2], b[2], t)),
		Math.round(lerp(a[3], b[3], t))
	];
}

function ramp(stops: RGBA[], t: number): RGBA {
	const x = Math.min(1, Math.max(0, t)) * (stops.length - 1);
	const i = Math.min(stops.length - 2, Math.floor(x));
	return lerpColor(stops[i], stops[i + 1], x - i);
}

function cssGradient(stops: RGBA[]): string {
	const parts = stops.map((s) => `rgb(${s[0]} ${s[1]} ${s[2]})`);
	return `linear-gradient(90deg, ${parts.join(', ')})`;
}

/** Sequential LMP ramp, cheap to expensive: pale straw darkening through
 * amber and rust to near-black maroon. Lightness falls monotonically, so the
 * ordering survives every kind of color vision deficiency. */
const LMP_STOPS: RGBA[] = [
	[246, 227, 180, 235],
	[236, 178, 66, 235],
	[212, 116, 34, 240],
	[160, 56, 21, 245],
	[84, 18, 7, 250]
];

export const lmpColor = (t: number): RGBA => ramp(LMP_STOPS, t);
export const lmpGradient = cssGradient(LMP_STOPS);

/** Trimmed LMP color domain: 5th to 95th percentile, with a $1/MWh
 * minimum span. LMP distributions are heavy tailed (one binding line pins a
 * handful of buses far from the pack) so min-max scaling compresses the pack
 * into a single hue, and a congestion-free case is one price plus solver
 * noise that an exact scale would stretch into fake structure. Values beyond
 * the domain clamp to the ramp ends. */
export function lmpDomain(values: number[]): { lo: number; hi: number } {
	if (values.length === 0) return { lo: 0, hi: 1 };
	const sorted = [...values].sort((a, b) => a - b);
	const q = (p: number) => sorted[Math.min(sorted.length - 1, Math.floor(p * sorted.length))];
	const lo = q(0.05);
	const hi = q(0.95);
	const mid = (lo + hi) / 2;
	const span = Math.max(hi - lo, 1);
	return { lo: mid - span / 2, hi: mid + span / 2 };
}

/** Center a scalar display variable (bus voltage angle, |V|) on a symmetric domain
 * with a per-mode minimum span; LMP keeps its own robust quantile domain. Shared by
 * the control panel readout and the map color scale. */
export function scalarDomain(mode: DisplayMode, values: number[]): { lo: number; hi: number } {
	if (mode === 'lmp') return lmpDomain(values);
	if (values.length === 0) return { lo: 0, hi: 1 };
	const { min: rawLo, max: rawHi } = extent(values);
	const minSpan = mode === 'voltage' ? 0.02 : 0.04;
	const span = Math.max(rawHi - rawLo, minSpan);
	const mid = (rawLo + rawHi) / 2;
	return { lo: mid - span / 2, hi: mid + span / 2 };
}

/** Diverging sensitivity ramp: green (price falls per MW of demand) through
 * a paper-toned neutral to purple (price rises). The green/purple pair stays
 * legible under CVD and shares no hue with the warm LMP ramp, so the two
 * modes cannot be confused. */
const SENS_STOPS: RGBA[] = [
	[27, 120, 55, 240],
	[127, 191, 123, 230],
	[223, 219, 208, 160],
	[194, 165, 207, 230],
	[118, 42, 131, 240]
];

/** t in [-1, 1] */
export const sensColor = (t: number): RGBA => ramp(SENS_STOPS, (t + 1) / 2);
export const sensGradient = cssGradient(SENS_STOPS);
export const sensNeutral: RGBA = SENS_STOPS[2];
export const busNeutral: RGBA = [180, 175, 165, 200];

const FLAT_SENSITIVITY_TINT = 0.55;

export function sensFlatColor(domain: Pick<SensitivityDomain, 'mean'>): RGBA {
	if (domain.mean === 0) return sensNeutral;
	return sensColor(domain.mean > 0 ? FLAT_SENSITIVITY_TINT : -FLAT_SENSITIVITY_TINT);
}

/** Zero anchored sensitivity domain with a flat column guard. */
export function sensitivityDomain(values: number[]): SensitivityDomain {
	if (values.length === 0) {
		return { min: 0, max: 0, mean: 0, absMax: 0, scale: 1, flat: true };
	}
	const sorted = [...values].sort((a, b) => a - b);
	const q = (p: number) => sorted[Math.min(sorted.length - 1, Math.floor(p * sorted.length))];
	const min = sorted[0];
	const max = sorted[sorted.length - 1];
	const mean = values.reduce((acc, v) => acc + v, 0) / values.length;
	// The largest absolute value sits at one sorted extreme; computing it from the
	// endpoints avoids spreading one argument per bus into Math.max (a RangeError on
	// very large cases).
	const absMax = Math.max(Math.abs(min), Math.abs(max));
	const robust = Math.max(Math.abs(q(0.01)), Math.abs(q(0.99)));
	const flat = max - min <= Math.max(1e-7, 0.01 * absMax);
	return {
		min,
		max,
		mean,
		absMax,
		scale: Math.max(robust, 1e-12),
		flat
	};
}

/** Branch color by loading fraction: warm gray, amber past 0.6, red past 0.9. */
export function branchColor(loading: number, inService: boolean): RGBA {
	if (!inService) return [197, 191, 178, 110];
	if (loading < 0.6) return [138, 131, 117, 200];
	if (loading < 0.9)
		return lerpColor([138, 131, 117, 210], [212, 116, 34, 240], (loading - 0.6) / 0.3);
	return [179, 38, 30, 250];
}

export function branchWidth(loading: number): number {
	return 1.6 + 3.4 * Math.min(1, loading);
}

/** Bus radius in px; area tracks max(load, gen) so big plants and big loads
 * both read at a glance. Shared by the map layers and the size legend. */
export function busRadius(maxMw: number): number {
	return 3.2 + 0.45 * Math.sqrt(Math.max(maxMw, 1));
}
