import type { BranchRatingDeltas, DemandDeltas } from './api.js';
import { demandGradient, priceGradient } from './colors.js';
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

/** Variables present in the current result. Power flow has no LMP values. */
export function displayMetaFor(c: SolvableCase | null): DisplayOption[] {
	if (!c?.solution) return [];
	const options: DisplayOption[] = [];
	if (c.formulation !== 'acpf' && c.solution.prices.length > 0) {
		options.push({
			mode: 'price',
			label: 'LMP',
			unit: 'objective units/MW',
			copy:
				c.formulation === 'socwr'
					? 'Locational marginal price, SOCWR relaxation'
					: 'Locational marginal price',
			gradient: priceGradient
		});
	}
	if (c.solution.vm?.length || c.solution.w.length > 0) {
		options.push({
			mode: 'voltage',
			label: '|V|',
			unit: 'pu',
			copy: c.formulation === 'socwr' ? 'Voltage magnitude, SOCWR relaxation' : 'Voltage magnitude',
			gradient: priceGradient
		});
	}
	if (c.solution.va.length > 0) {
		options.push({
			mode: 'angle',
			label: 'Angle',
			unit: 'rad',
			copy: 'Voltage angle',
			gradient: priceGradient
		});
	}
	// Last, so a mode the result cannot show still falls back to a result variable.
	if (c.network?.buses.length) {
		options.push({
			mode: 'demand',
			label: 'Demand',
			unit: 'MW',
			copy: 'Demand, square-root scale',
			gradient: demandGradient
		});
	}
	return options;
}

/** Bus LMPs, voltage angles in radians, voltage magnitudes in per unit, or
 * demand in MW with the case's committed changes. SOCWR supplies squared
 * voltages, which are converted to magnitudes here. */
export function displaySeriesFor(
	c: SolvableCase | null,
	mode: DisplayMode
): { bus: number; value: number }[] {
	const sol = c?.solution;
	if (!c || !sol) return [];
	if (mode === 'demand') {
		const deltas = caseDeltas(c);
		return (c.network?.buses ?? []).map((b) => ({
			bus: b.id,
			value: b.demand_mw + (deltas[b.id] ?? 0)
		}));
	}
	if (mode === 'angle') return sol.va;
	if (mode === 'voltage') {
		if (sol.vm?.length) return sol.vm;
		return sol.w.map((s) => ({
			bus: s.bus,
			value: Math.sqrt(Math.max(0, s.value))
		}));
	}
	return sol.prices.map((s) => ({ bus: s.bus, value: s.value }));
}

/** The committed demand deltas for a case (MW from base, keyed by bus). A backend
 * case always carries a deltas map; a local case defaults to an empty one. */
export function caseDeltas(c: SolvableCase): DemandDeltas {
	return c instanceof CaseState ? c.deltas : (c.deltas ?? {});
}

/** The committed branch rating deltas for a case (MW from base, keyed by branch).
 * The rating counterpart of `caseDeltas`. */
export function caseRatings(c: SolvableCase): BranchRatingDeltas {
	return c instanceof CaseState ? c.ratings : (c.ratings ?? {});
}
