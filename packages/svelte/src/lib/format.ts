import type { RGBA } from './colors.js';
import type { DisplayMode } from './state.svelte.js';
import { FORMULATIONS, type Formulation } from '@tellegen/engine';

// errorText is defined once in wasm.ts (its first consumer) and surfaced here so UI
// code can import it alongside the other formatting helpers.
export { errorText } from '@tellegen/engine';

/** The short menu label for a formulation tag (e.g. `acopf` -> `AC OPF`). */
export function formulationLabel(id: Formulation): string {
	return FORMULATIONS.find((f) => f.id === id)?.label ?? id;
}

export function formulationHint(id: Formulation): string {
	return FORMULATIONS.find((f) => f.id === id)?.hint ?? id;
}

export function splitName(name: string): [string, string] {
	const m = name.match(/^(.*?)\s*\((.*)\)$/);
	return m ? [m[1], m[2]] : [name, ''];
}

export const INFEASIBLE_DISPATCH =
	'No feasible dispatch: demand cannot be served within line and generator limits';

/** The message for a local case whose browser solve failed, and the
 * technical detail to disclose behind it. An infeasible DC OPF means the
 * network cannot carry the demand, which deserves plain words. The engine
 * spells it `DC OPF solve infeasible`, and an unbounded one as `unbounded`. */
export function browserSolveFailure(
	label: string,
	reason: string | null
): { message: string; detail: string | null } {
	const technical = `${label}: browser solve unavailable${reason ? `: ${reason}` : ''}`;
	if (reason && /\binfeasible\b/i.test(reason)) {
		return { message: INFEASIBLE_DISPATCH, detail: technical };
	}
	return { message: technical, detail: null };
}

export function rgbaCss([r, g, b, a]: RGBA): string {
	return `rgba(${r}, ${g}, ${b}, ${(a / 255).toFixed(3)})`;
}

/** Min and max of a numeric array in one pass. Used instead of
 * `Math.min(...values)` / `Math.max(...values)`, which throw a RangeError once the
 * argument count exceeds the engine limit (~65k on JavaScriptCore) — reachable on a
 * dropped case with tens of thousands of buses. */
export function extent(values: number[]): { min: number; max: number } {
	let min = Infinity;
	let max = -Infinity;
	for (const v of values) {
		if (v < min) min = v;
		if (v > max) max = v;
	}
	return { min, max };
}

export const fmt = new Intl.NumberFormat('en-US', { maximumFractionDigits: 1 });
export const signed = (v: number) => `${v < 0 ? '−' : '+'}${fmt.format(Math.abs(v))}`;
export const signedExp = (v: number) => `${v < 0 ? '−' : '+'}${Math.abs(v).toExponential(2)}`;
export const displayFmt = (mode: DisplayMode, value: number) =>
	mode === 'price' || mode === 'demand' ? fmt.format(value) : value.toFixed(3);

const exactNumber = new Intl.NumberFormat('en', { maximumFractionDigits: 6, useGrouping: false });
const decimals = [0, 1, 2, 3, 4].map(
	(d) => new Intl.NumberFormat('en', { maximumFractionDigits: d, minimumFractionDigits: 0 })
);

/**
 * A quantity rounded for reading: per-unit voltage to four decimals, percentages to one, and
 * other engineering quantities (MW, MVA, MVAr, prices) to about three significant figures,
 * never dropping integer digits. `exact` keeps up to six decimals, for a tooltip or a check.
 */
export function readable(
	value: number | null | undefined,
	unit: string,
	signed = false
): { text: string; exact: string } {
	if (value == null || !Number.isFinite(value))
		return { text: 'Unavailable', exact: 'Unavailable' };
	const magnitude = Math.abs(value);
	const places =
		unit === 'pu' ? 4 : unit === '%' ? 1 : magnitude >= 100 ? 0 : magnitude >= 10 ? 1 : 2;
	const rounded = decimals[places].format(magnitude);
	const zero = Number(rounded.replace(/,/g, '')) === 0;
	const sign = !signed || zero ? (value < 0 && !zero ? '-' : '') : value < 0 ? '-' : '+';
	const join = (n: string) => `${n}${unit === '%' ? '' : ' '}${unit}`.trimEnd();
	const exact = exactNumber.format(value);
	return {
		text: join(`${sign}${rounded}`),
		exact: join(signed && value > 0 ? `+${exact}` : exact === '-0' ? '0' : exact)
	};
}

/** The one glyph for a value that is missing or not finite. */
export const MISSING = '—';

/** Units {@link quantity} formats. `V` switches to kV at 1 kV; `°` and `%` attach to the number. */
export type QuantityUnit = 'V' | 'kV' | 'A' | 'kW' | 'kvar' | 'kVA' | '°' | '%' | 'pu' | 'ms' | '';

export interface QuantityOptions {
	/** Prefix positive values with `+`. */
	signed?: boolean;
	/** Up to six decimals in the base unit, no grouping and no kV switch: for a tooltip or a check. */
	exact?: boolean;
	/** Significant figures for the scaled units (V, kV, A, kW, kvar, kVA, ms); default 4. */
	figures?: number;
	/** Fixed decimals, overriding the unit's rule. */
	decimals?: number;
}

const fixedFormats = new Map<number, Intl.NumberFormat>();
function fixedFormat(places: number): Intl.NumberFormat {
	let f = fixedFormats.get(places);
	if (!f) {
		f = new Intl.NumberFormat('en-US', {
			minimumFractionDigits: places,
			maximumFractionDigits: places
		});
		fixedFormats.set(places, f);
	}
	return f;
}

/** Decimals for a unit: pu to four, angles to two, percent to one, everything else to `figures`
 * significant figures without ever dropping integer digits. */
function placesFor(magnitude: number, unit: QuantityUnit, figures: number): number {
	if (unit === 'pu') return 4;
	if (unit === '°') return 2;
	if (unit === '%') return 1;
	const integerDigits = magnitude >= 1 ? Math.floor(Math.log10(magnitude)) + 1 : 1;
	return Math.min(figures - 1, Math.max(0, figures - integerDigits));
}

/**
 * The one formatter for a multiconductor or balanced quantity on screen: thousands separators,
 * `figures` significant figures (four by default) for V, A, kW, kvar, kVA and ms, two decimals for
 * degrees, one for percent and four for pu. Volts switch to kV at |v| ≥ 1000. `%` and `°` attach to
 * the number, other units follow a space. A missing or non-finite value is {@link MISSING}.
 */
export function quantity(
	value: number | null | undefined,
	unit: QuantityUnit,
	options: QuantityOptions = {}
): string {
	if (value == null || !Number.isFinite(value)) return MISSING;
	const { signed = false, exact = false, figures = 4, decimals } = options;
	let shown = value;
	let shownUnit: string = unit;
	if (!exact && unit === 'V' && Math.abs(value) >= 1000) {
		shown = value / 1000;
		shownUnit = 'kV';
	}
	const attach = shownUnit === '%' || shownUnit === '°' || shownUnit === '';
	const join = (n: string) => (attach ? `${n}${shownUnit}` : `${n} ${shownUnit}`);
	if (exact) {
		const text = exactNumber.format(shown);
		return join(text === '-0' ? '0' : signed && shown > 0 ? `+${text}` : text);
	}
	const magnitude = Math.abs(shown);
	const places = decimals ?? placesFor(magnitude, shownUnit as QuantityUnit, figures);
	const rounded = fixedFormat(places).format(magnitude);
	const zero = Number(rounded.replace(/,/g, '')) === 0;
	const sign = zero ? '' : shown < 0 ? '-' : signed ? '+' : '';
	return join(`${sign}${rounded}`);
}

/** A voltage with its per-unit value beside it when the base is known: `239.6 V (0.9983 pu)`. */
export function voltsWithPu(volts: number | null | undefined, base: number | null | undefined) {
	const v = quantity(volts, 'V');
	if (volts == null || !Number.isFinite(volts) || !base || !Number.isFinite(base) || base <= 0)
		return v;
	return `${v} (${quantity(volts / base, 'pu')})`;
}

/** Degrees from radians, through {@link quantity}. */
export const degrees = (radians: number | null | undefined, options?: QuantityOptions) =>
	quantity(radians == null ? radians : (radians * 180) / Math.PI, '°', options);

/** A range through {@link quantity}, the unit written once when both ends share it:
 * `203.4–239.6 V`, `0.9500–1.0500 pu`, `950.0 V – 1.050 kV`. */
export function quantityRange(
	low: number | null | undefined,
	high: number | null | undefined,
	unit: QuantityUnit
): string {
	const a = quantity(low, unit);
	const b = quantity(high, unit);
	if (a === MISSING || b === MISSING) return a === b ? MISSING : `${a} – ${b}`;
	const suffix = (s: string) => s.match(/\s?[^\d\s.,+-]*$/)?.[0] ?? '';
	const sa = suffix(a);
	return sa && sa === suffix(b) ? `${a.slice(0, -sa.length)}–${b}` : `${a} – ${b}`;
}
