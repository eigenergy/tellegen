import type { SolveIteration } from './api.js';
import type { RGBA } from './colors.js';
import type { DisplayMode, SolveBackend } from './state.svelte.js';
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

export function priceCopy(id: Formulation): string {
	return id === 'socwr'
		? 'SOCWR active power balance prices. Select a bus for ∂LMP/∂d and demand perturbation.'
		: 'DC OPF prices. Select a bus for ∂LMP/∂d and demand perturbation.';
}

export function splitName(name: string): [string, string] {
	const m = name.match(/^(.*?)\s*\((.*)\)$/);
	return m ? [m[1], m[2]] : [name, ''];
}

/** Structural shape `solveMetaLabel` needs, satisfied by `CaseState` and `LocalCase`. */
type SolveMeta = { iterations?: SolveIteration[]; solveBackend: SolveBackend | null };

export function solveMetaLabel(c: SolveMeta): string {
	if ((c.iterations ?? []).length > 1) return `${c.iterations?.length} iterations`;
	if (c.solveBackend === 'clarabel-wasm-server-sensitivity') return 'server dLMP/dd';
	return c.solveBackend === 'rust-server' ? 'server solve' : 'browser solve';
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
	mode === 'lmp' ? fmt.format(value) : value.toFixed(3);
