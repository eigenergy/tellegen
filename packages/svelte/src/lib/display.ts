import type { DemandDeltas } from './api.js';
import { lmpGradient } from './colors.js';
import { priceCopy } from './format.js';
import { CaseState, type DisplayMode, type SolvableCase } from './state.svelte.js';

/** One bus-color variable the panel and map can show, without its per-bus values.
 * The values for the active mode are fetched separately via `displaySeriesFor`, so
 * inactive modes never build an array nobody reads. */
export type DisplayOption = {
	mode: DisplayMode;
	label: string;
	unit: string;
	copy: string;
	gradient: string;
};

/** The display variables available for a case's current solution and formulation:
 * LMP always, the DC phase angle under DC OPF, the relaxed |V| under SOCWR. Metadata
 * only; call `displaySeriesFor` for one mode's per-bus values. Empty when unsolved. */
export function displayMetaFor(c: SolvableCase | null): DisplayOption[] {
	if (!c?.solution) return [];
	const options: DisplayOption[] = [
		{
			mode: 'lmp',
			label: 'LMP',
			unit: '$/MWh',
			copy: priceCopy(c.formulation),
			gradient: lmpGradient
		}
	];
	if (c.formulation === 'dcopf' && c.solution.va.length > 0) {
		options.push({
			mode: 'angle',
			label: 'angle',
			unit: 'rad',
			copy: 'DC bus voltage phase angle from the current OPF solution.',
			gradient: lmpGradient
		});
	}
	if (c.formulation === 'socwr' && c.solution.w.length > 0) {
		options.push({
			mode: 'voltage',
			label: '|V|',
			unit: 'pu',
			copy: 'SOCWR voltage magnitude from the current relaxed solution.',
			gradient: lmpGradient
		});
	}
	return options;
}

/** Per-bus values for one display mode: LMP in $/MWh, the DC phase angle in rad, or
 * the SOCWR voltage magnitude (sqrt of the squared-voltage variable w, clamped at 0).
 * Empty when the case is unsolved. Single source for both the panel legend stats and
 * the map node coloring, so the |V| transform stays in one place. */
export function displaySeriesFor(
	c: SolvableCase | null,
	mode: DisplayMode
): { bus: number; value: number }[] {
	const sol = c?.solution;
	if (!sol) return [];
	if (mode === 'angle') return sol.va;
	if (mode === 'voltage') {
		return sol.w.map((s) => ({ bus: s.bus, value: Math.sqrt(Math.max(0, s.value)) }));
	}
	return sol.lmp.map((s) => ({ bus: s.bus, value: s.usd_per_mwh }));
}

/** The committed demand deltas for a case (MW from base, keyed by bus). A backend
 * case always carries a deltas map; a local case defaults to an empty one. */
export function caseDeltas(c: SolvableCase): DemandDeltas {
	return c instanceof CaseState ? c.deltas : (c.deltas ?? {});
}
