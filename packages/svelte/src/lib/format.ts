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
