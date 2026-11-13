import type { BranchConstraintResult, Solution } from '@tellegen/engine';
import { readable } from '../format.js';

export const PRICE_UNIT = 'objective units/MW';
export const PRICE_SCALES = [
	{ lo: 0, hi: 50 },
	{ lo: -50, hi: 50 },
	{ lo: 0, hi: 200 },
	{ lo: -200, hi: 200 }
] as const;

// Suppress multiplier noise at the precision used in the constraint list.
const MARGINAL_TOLERANCE = 1e-5;

/** A price or shadow price rounded for reading (about three significant figures). */
export function economicAmount(value: number | null | undefined): string {
	if (value == null || !Number.isFinite(value)) return 'Unavailable';
	return readable(Math.abs(value) < 5e-7 ? 0 : value, PRICE_UNIT).text;
}

export function pricedBranch(result: BranchConstraintResult | undefined): boolean {
	return !!result?.enforced && result.binding && (result.relief_per_mw ?? 0) > MARGINAL_TOLERANCE;
}

export function thermalStatus(result: BranchConstraintResult | undefined): string {
	if (!result) return 'Constraint data unavailable';
	if (!result.enforced) return 'Not enforced';
	if (!result.binding) return 'Enforced, not binding';
	if (result.relief_per_mw == null) return 'Binding, marginal relief unavailable';
	return pricedBranch(result)
		? 'Binding, with positive marginal relief'
		: 'Binding, with negligible marginal relief';
}

/** Calculations that produce prices and binding limits. */
export const isOpf = (formulation: string) =>
	formulation === 'dcopf' || formulation === 'acopf' || formulation === 'socwr';

export function economicsFor(solution: Solution | null, formulation: string) {
	const result =
		isOpf(formulation) && (!solution?.formulation || solution.formulation === formulation)
			? solution
			: null;
	const prices = (result?.prices ?? []).filter((price) => Number.isFinite(price.value));
	let lo = Infinity;
	let hi = -Infinity;
	for (const { value } of prices) {
		lo = Math.min(lo, value);
		hi = Math.max(hi, value);
	}
	const branchResults = result?.branch_constraints;
	const branches = new Map(branchResults?.map((row) => [row.branch, row]));
	const binding = (branchResults ?? [])
		.filter((row) => row.enforced && row.binding)
		.sort((a, b) => (b.relief_per_mw ?? 0) - (a.relief_per_mw ?? 0) || a.branch - b.branch);
	const constraints = [...(result?.constraints ?? [])].sort(
		(a, b) =>
			Number(b.binding) - Number(a.binding) ||
			Math.abs(b.shadow_price ?? 0) - Math.abs(a.shadow_price ?? 0) ||
			a.id.localeCompare(b.id)
	);
	return {
		prices,
		/** Lowest and highest price; null without prices. */
		range: prices.length > 0 ? { lo, hi } : null,
		uniform: prices.length > 0 && hi - lo <= 1e-5 ? (lo + hi) / 2 : null,
		branches,
		binding,
		constraints,
		thermalAvailable: branchResults !== undefined
	};
}
